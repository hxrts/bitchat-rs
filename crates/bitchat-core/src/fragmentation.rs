//! Message fragmentation and reassembly for large BitChat messages
//!
//! This module handles splitting large messages into smaller fragments for transport
//! and reassembling them on the receiving end, following the BitChat protocol specification.

use alloc::vec::Vec;
use core::cmp;
use crc32fast::Hasher;
#[cfg(not(feature = "std"))]
use hashbrown::HashMap;
#[cfg(feature = "std")]
use std::collections::HashMap;
use uuid::Uuid;

use crate::packet::{BitchatPacket, MessageType};
use crate::types::{PeerId, Timestamp};
use crate::{BitchatError, Result};

// ----------------------------------------------------------------------------
// Constants
// ----------------------------------------------------------------------------

/// Maximum payload size for a single fragment (bytes)
pub const MAX_FRAGMENT_SIZE: usize = 1024;

/// Maximum number of fragments per message
pub const MAX_FRAGMENTS: u16 = 65534;

/// Fragment timeout in seconds
#[cfg(feature = "std")]
pub const FRAGMENT_TIMEOUT_SECS: u64 = 60;

/// Maximum number of concurrent reassembly operations
pub const MAX_CONCURRENT_REASSEMBLIES: usize = 100;

/// Maximum total memory used for fragments (in bytes)
pub const MAX_FRAGMENT_MEMORY: usize = 10 * 1024 * 1024; // 10MB

// ----------------------------------------------------------------------------
// Fragment Header
// ----------------------------------------------------------------------------

/// Header for message fragments
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FragmentHeader {
    /// Unique message identifier
    pub message_id: Uuid,
    /// Fragment sequence number (0-based)
    pub fragment_number: u16,
    /// Total number of fragments in message
    pub total_fragments: u16,
    /// Size of the original complete message
    pub total_size: u32,
    /// CRC32 checksum of complete message
    pub checksum: u32,
}

impl FragmentHeader {
    /// Create a new fragment header
    pub fn new(
        message_id: Uuid,
        fragment_number: u16,
        total_fragments: u16,
        total_size: u32,
        checksum: u32,
    ) -> Self {
        Self {
            message_id,
            fragment_number,
            total_fragments,
            total_size,
            checksum,
        }
    }

    /// Check if this is the first fragment
    pub fn is_first(&self) -> bool {
        self.fragment_number == 0
    }

    /// Check if this is the last fragment
    pub fn is_last(&self) -> bool {
        self.fragment_number + 1 == self.total_fragments
    }
}

// ----------------------------------------------------------------------------
// Fragment
// ----------------------------------------------------------------------------

/// A single message fragment
#[derive(Debug, Clone)]
pub struct Fragment {
    /// Fragment header with metadata
    pub header: FragmentHeader,
    /// Fragment payload data
    pub data: Vec<u8>,
    /// Timestamp when fragment was created/received
    pub timestamp: Timestamp,
}

impl Fragment {
    /// Create a new fragment
    pub fn new(header: FragmentHeader, data: Vec<u8>) -> Self {
        Self {
            header,
            data,
            #[cfg(feature = "std")]
            timestamp: Timestamp::now(),
            #[cfg(not(feature = "std"))]
            timestamp: Timestamp::new(0),
        }
    }

    /// Convert fragment to BitChat packet
    pub fn to_packet(
        &self,
        sender_id: PeerId,
        recipient_id: Option<PeerId>,
    ) -> Result<BitchatPacket> {
        let message_type = if self.header.is_first() {
            MessageType::FragmentStart
        } else if self.header.is_last() {
            MessageType::FragmentEnd
        } else {
            MessageType::FragmentContinue
        };

        // Serialize header + data
        let header_bytes = bincode::serialize(&self.header)?;
        let mut payload = Vec::with_capacity(header_bytes.len() + self.data.len());
        payload.extend_from_slice(&header_bytes);
        payload.extend_from_slice(&self.data);

        let mut packet = BitchatPacket::new(message_type, sender_id, payload);
        if let Some(recipient) = recipient_id {
            packet = packet.with_recipient(recipient);
        }

        Ok(packet)
    }

    /// Create fragment from BitChat packet
    pub fn from_packet(packet: &BitchatPacket) -> Result<Self> {
        if !matches!(
            packet.message_type,
            MessageType::FragmentStart | MessageType::FragmentContinue | MessageType::FragmentEnd
        ) {
            return Err(crate::PacketError::UnknownMessageType {
                message_type: packet.message_type as u8,
            }
            .into());
        }

        // Deserialize header first
        let header: FragmentHeader = bincode::deserialize(&packet.payload)?;
        let header_size = bincode::serialized_size(&header)? as usize;

        if packet.payload.len() < header_size {
            return Err(crate::PacketError::PayloadTooSmall {
                expected: header_size,
                actual: packet.payload.len(),
            }
            .into());
        }

        let data = packet.payload[header_size..].to_vec();

        Ok(Self::new(header, data))
    }
}

// ----------------------------------------------------------------------------
// Message Fragmenter
// ----------------------------------------------------------------------------

/// Handles fragmentation of large messages
pub struct MessageFragmenter;

impl MessageFragmenter {
    /// Split a large message into fragments
    pub fn fragment_message(
        message_id: Uuid,
        data: &[u8],
        max_fragment_size: usize,
    ) -> Result<Vec<Fragment>> {
        if data.is_empty() {
            return Err(BitchatError::Fragmentation {
                message: "Cannot fragment empty message".to_string(),
            });
        }

        let total_size = data.len() as u32;
        let checksum = Self::calculate_checksum(data);

        // Calculate fragment payload size (excluding header overhead)
        let header = FragmentHeader::new(message_id, 0, 1, total_size, checksum);
        let header_size = bincode::serialized_size(&header)? as usize;

        if max_fragment_size <= header_size {
            return Err(BitchatError::Fragmentation {
                message: "Fragment size too small for header".to_string(),
            });
        }

        let fragment_payload_size = max_fragment_size - header_size;
        let total_fragments = data.len().div_ceil(fragment_payload_size) as u16;

        if total_fragments > MAX_FRAGMENTS {
            return Err(BitchatError::Fragmentation {
                message: "Message too large to fragment".to_string(),
            });
        }

        let mut fragments = Vec::with_capacity(total_fragments as usize);

        for i in 0..total_fragments {
            let start = i as usize * fragment_payload_size;
            let end = cmp::min(start + fragment_payload_size, data.len());
            let fragment_data = data[start..end].to_vec();

            let header = FragmentHeader::new(message_id, i, total_fragments, total_size, checksum);

            fragments.push(Fragment::new(header, fragment_data));
        }

        Ok(fragments)
    }

    /// Calculate CRC32 checksum using crc32fast
    fn calculate_checksum(data: &[u8]) -> u32 {
        let mut hasher = Hasher::new();
        hasher.update(data);
        hasher.finalize()
    }
}

// ----------------------------------------------------------------------------
// Fragment Reassembler
// ----------------------------------------------------------------------------

/// State of a message being reassembled
#[derive(Debug)]
struct ReassemblyState {
    /// Expected message metadata
    total_fragments: u16,
    total_size: u32,
    checksum: u32,
    /// Fragments received so far
    fragments: HashMap<u16, Vec<u8>>,
    /// Timestamp when first fragment was received
    first_received: Timestamp,
    /// Current memory usage for this reassembly
    memory_used: usize,
}

impl ReassemblyState {
    fn new(header: &FragmentHeader) -> Self {
        Self {
            total_fragments: header.total_fragments,
            total_size: header.total_size,
            checksum: header.checksum,
            fragments: HashMap::new(),
            #[cfg(feature = "std")]
            first_received: Timestamp::now(),
            #[cfg(not(feature = "std"))]
            first_received: Timestamp::new(0),
            memory_used: 0,
        }
    }

    fn is_complete(&self) -> bool {
        self.fragments.len() == self.total_fragments as usize
    }

    fn add_fragment(&mut self, fragment: &Fragment) -> Result<()> {
        // Validate fragment metadata matches
        if fragment.header.total_fragments != self.total_fragments
            || fragment.header.total_size != self.total_size
            || fragment.header.checksum != self.checksum
        {
            return Err(BitchatError::InvalidPacket(
                "Fragment metadata mismatch".into(),
            ));
        }

        // Check for duplicate fragments
        if self
            .fragments
            .contains_key(&fragment.header.fragment_number)
        {
            return Err(crate::PacketError::DuplicateFragment.into());
        }

        // Track memory usage
        self.memory_used += fragment.data.len();

        self.fragments
            .insert(fragment.header.fragment_number, fragment.data.clone());
        Ok(())
    }

    fn assemble(&self) -> Result<Vec<u8>> {
        if !self.is_complete() {
            return Err(BitchatError::InvalidPacket(
                "Incomplete fragment set".into(),
            ));
        }

        let mut assembled = Vec::with_capacity(self.total_size as usize);

        // Assemble fragments in order
        for i in 0..self.total_fragments {
            if let Some(fragment_data) = self.fragments.get(&i) {
                assembled.extend_from_slice(fragment_data);
            } else {
                return Err(crate::PacketError::FragmentSequenceError.into());
            }
        }

        // Verify assembled size
        if assembled.len() != self.total_size as usize {
            return Err(crate::PacketError::PayloadTooSmall {
                expected: self.total_size as usize,
                actual: assembled.len(),
            }
            .into());
        }

        // Verify checksum
        let calculated_checksum = MessageFragmenter::calculate_checksum(&assembled);
        if calculated_checksum != self.checksum {
            return Err(crate::PacketError::ChecksumFailed.into());
        }

        Ok(assembled)
    }

    #[cfg(feature = "std")]
    fn is_expired(&self) -> bool {
        use core::time::Duration;
        let now = Timestamp::now();
        let elapsed = Duration::from_millis(
            now.as_millis()
                .saturating_sub(self.first_received.as_millis()),
        );
        elapsed.as_secs() > FRAGMENT_TIMEOUT_SECS
    }
}

/// Reassembles fragmented messages
pub struct MessageReassembler {
    /// Ongoing reassembly operations by message ID
    reassembly_states: HashMap<Uuid, ReassemblyState>,
    /// Total memory currently used by all reassemblies
    total_memory_used: usize,
}

impl MessageReassembler {
    /// Create a new message reassembler
    pub fn new() -> Self {
        Self {
            reassembly_states: HashMap::new(),
            total_memory_used: 0,
        }
    }

    /// Process a fragment, potentially completing a message
    pub fn process_fragment(&mut self, fragment: Fragment) -> Result<Option<Vec<u8>>> {
        let message_id = fragment.header.message_id;

        // Validate fragment size before processing
        if fragment.data.len() > MAX_FRAGMENT_SIZE {
            return Err(crate::PacketError::PayloadTooLarge {
                max: MAX_FRAGMENT_SIZE,
                actual: fragment.data.len(),
            }
            .into());
        }

        // Validate total message size is reasonable
        if fragment.header.total_size > MAX_FRAGMENT_MEMORY as u32 {
            return Err(crate::PacketError::PayloadTooLarge {
                max: MAX_FRAGMENT_MEMORY,
                actual: fragment.header.total_size as usize,
            }
            .into());
        }

        // Get or create reassembly state with proper limit enforcement
        if !self.reassembly_states.contains_key(&message_id) {
            // Enforce concurrent reassembly limit BEFORE creating new state
            while self.reassembly_states.len() >= MAX_CONCURRENT_REASSEMBLIES {
                if !self.evict_oldest_reassembly() {
                    return Err(crate::PacketError::Generic {
                        message: "Cannot evict reassembly states to make room".to_string(),
                    }
                    .into());
                }
            }

            // Enforce memory limit BEFORE creating new state
            while self.total_memory_used + fragment.data.len() > MAX_FRAGMENT_MEMORY {
                if !self.evict_largest_reassembly() {
                    return Err(crate::PacketError::Generic {
                        message: "Cannot evict reassembly states to free memory".to_string(),
                    }
                    .into());
                }
            }

            let state = ReassemblyState::new(&fragment.header);
            self.reassembly_states.insert(message_id, state);
        }

        let fragment_size = fragment.data.len();
        let state = self.reassembly_states.get_mut(&message_id).ok_or_else(|| {
            crate::PacketError::Generic {
                message: "Reassembly state not found".to_string(),
            }
        })?;

        // Final check for existing reassembly: ensure adding this fragment won't exceed memory
        if self.total_memory_used + fragment_size > MAX_FRAGMENT_MEMORY {
            return Err(crate::PacketError::Generic {
                message: "Fragment would exceed memory limit".to_string(),
            }
            .into());
        }

        state.add_fragment(&fragment)?;
        self.total_memory_used += fragment_size;

        // Check if message is complete
        if state.is_complete() {
            let assembled = state.assemble()?;
            let state = self.reassembly_states.remove(&message_id).ok_or_else(|| {
                crate::PacketError::Generic {
                    message: "Reassembly state missing during completion".to_string(),
                }
            })?;
            self.total_memory_used = self.total_memory_used.saturating_sub(state.memory_used);
            Ok(Some(assembled))
        } else {
            Ok(None)
        }
    }

    /// Clean up expired reassembly states
    #[cfg(feature = "std")]
    pub fn cleanup_expired(&mut self) {
        let expired_ids: Vec<Uuid> = self
            .reassembly_states
            .iter()
            .filter_map(
                |(id, state)| {
                    if state.is_expired() {
                        Some(*id)
                    } else {
                        None
                    }
                },
            )
            .collect();

        for id in expired_ids {
            self.cancel_reassembly(&id);
        }
    }

    /// Clean up expired reassembly states (no_std compatible)
    /// Takes a current timestamp for comparison
    #[cfg(not(feature = "std"))]
    pub fn cleanup_expired_with_time(&mut self, current_time: Timestamp) {
        let expired_ids: Vec<Uuid> = self
            .reassembly_states
            .iter()
            .filter_map(|(id, state)| {
                let elapsed_ms = current_time
                    .as_millis()
                    .saturating_sub(state.first_received.as_millis());
                if elapsed_ms > FRAGMENT_TIMEOUT_SECS * 1000 {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();

        for id in expired_ids {
            self.cancel_reassembly(&id);
        }
    }

    /// Get count of active reassembly operations
    pub fn active_reassemblies(&self) -> usize {
        self.reassembly_states.len()
    }

    /// Remove incomplete reassembly state
    pub fn cancel_reassembly(&mut self, message_id: &Uuid) -> bool {
        if let Some(state) = self.reassembly_states.remove(message_id) {
            self.total_memory_used = self.total_memory_used.saturating_sub(state.memory_used);
            true
        } else {
            false
        }
    }

    /// Evict the oldest reassembly operation to make space
    /// Returns true if an eviction occurred, false if no states exist to evict
    fn evict_oldest_reassembly(&mut self) -> bool {
        if let Some((oldest_id, _)) = self
            .reassembly_states
            .iter()
            .min_by_key(|(_, state)| state.first_received.as_millis())
        {
            let oldest_id = *oldest_id;
            self.cancel_reassembly(&oldest_id)
        } else {
            false
        }
    }

    /// Evict the largest reassembly operation to free memory
    /// Returns true if an eviction occurred, false if no states exist to evict
    fn evict_largest_reassembly(&mut self) -> bool {
        if let Some((largest_id, _)) = self
            .reassembly_states
            .iter()
            .max_by_key(|(_, state)| state.memory_used)
        {
            let largest_id = *largest_id;
            self.cancel_reassembly(&largest_id)
        } else {
            false
        }
    }

    /// Get total memory usage across all reassemblies
    pub fn total_memory_used(&self) -> usize {
        self.total_memory_used
    }
}

impl Default for MessageReassembler {
    fn default() -> Self {
        Self::new()
    }
}

// ----------------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fragment_single_message() {
        let message_id = Uuid::new_v4();
        let data = b"Hello, world!";

        let fragments =
            MessageFragmenter::fragment_message(message_id, data, MAX_FRAGMENT_SIZE).unwrap();

        assert_eq!(fragments.len(), 1);
        assert!(fragments[0].header.is_first());
        assert!(fragments[0].header.is_last());
        assert_eq!(fragments[0].header.total_fragments, 1);
        assert_eq!(fragments[0].header.total_size, data.len() as u32);
    }

    #[test]
    fn test_fragment_large_message() {
        let message_id = Uuid::new_v4();
        let data = vec![0x42; 2048]; // 2KB message
        let fragment_size = 512; // 512 byte fragments

        let fragments =
            MessageFragmenter::fragment_message(message_id, &data, fragment_size).unwrap();

        // Should create multiple fragments
        assert!(fragments.len() > 1);
        assert_eq!(fragments[0].header.total_fragments, fragments.len() as u16);

        // Verify first and last fragment flags
        assert!(fragments[0].header.is_first());
        assert!(!fragments[0].header.is_last());
        assert!(!fragments[fragments.len() - 1].header.is_first());
        assert!(fragments[fragments.len() - 1].header.is_last());

        // All fragments should have same metadata
        for fragment in &fragments {
            assert_eq!(fragment.header.message_id, message_id);
            assert_eq!(fragment.header.total_size, data.len() as u32);
            assert_eq!(fragment.header.total_fragments, fragments.len() as u16);
        }
    }

    #[test]
    fn test_reassemble_single_fragment() {
        let message_id = Uuid::new_v4();
        let data = b"Hello, world!";

        let fragments =
            MessageFragmenter::fragment_message(message_id, data, MAX_FRAGMENT_SIZE).unwrap();

        let mut reassembler = MessageReassembler::new();
        let result = reassembler.process_fragment(fragments[0].clone()).unwrap();

        assert!(result.is_some());
        assert_eq!(result.unwrap(), data);
    }

    #[test]
    fn test_reassemble_multiple_fragments() {
        let message_id = Uuid::new_v4();
        let data = vec![0x42; 2048]; // 2KB message
        let fragment_size = 512;

        let fragments =
            MessageFragmenter::fragment_message(message_id, &data, fragment_size).unwrap();

        let mut reassembler = MessageReassembler::new();

        // Process all but last fragment
        for fragment in &fragments[..fragments.len() - 1] {
            let result = reassembler.process_fragment(fragment.clone()).unwrap();
            assert!(result.is_none()); // Should not be complete yet
        }

        // Process last fragment
        let result = reassembler
            .process_fragment(fragments[fragments.len() - 1].clone())
            .unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), data);
    }

    #[test]
    fn test_fragment_packet_conversion() {
        let message_id = Uuid::new_v4();
        let data = b"Hello, world!";
        let sender_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);

        let fragments =
            MessageFragmenter::fragment_message(message_id, data, MAX_FRAGMENT_SIZE).unwrap();

        let packet = fragments[0].to_packet(sender_id, None).unwrap();
        assert_eq!(packet.message_type, MessageType::FragmentStart);
        assert_eq!(packet.sender_id, sender_id);

        let reconstructed = Fragment::from_packet(&packet).unwrap();
        assert_eq!(reconstructed.header.message_id, message_id);
        assert_eq!(reconstructed.data, fragments[0].data);
    }

    #[test]
    fn test_checksum_verification() {
        let message_id = Uuid::new_v4();
        let data = b"Hello, world!";

        let fragments =
            MessageFragmenter::fragment_message(message_id, data, MAX_FRAGMENT_SIZE).unwrap();

        let mut reassembler = MessageReassembler::new();

        // Corrupt the fragment data
        let mut corrupted_fragment = fragments[0].clone();
        corrupted_fragment.data[0] ^= 0xFF;

        // Should fail checksum verification
        let result = reassembler.process_fragment(corrupted_fragment);
        assert!(result.is_err());
    }

    #[test]
    fn test_dos_protection_max_reassemblies() {
        let mut reassembler = MessageReassembler::new();

        // Create many incomplete reassemblies to test the limit
        for _i in 0..MAX_CONCURRENT_REASSEMBLIES + 10 {
            let message_id = Uuid::new_v4();
            // Use larger data to ensure multiple fragments
            let data = vec![0x42; 1000];

            let fragments = MessageFragmenter::fragment_message(
                message_id, &data, 400, // Smaller fragment size to ensure multiple fragments
            )
            .unwrap();

            // Only send first fragment to keep reassembly incomplete
            if fragments.len() > 1 {
                let result = reassembler.process_fragment(fragments[0].clone());
                assert!(result.is_ok());
                assert!(result.unwrap().is_none()); // Should not be complete
            }
        }

        // Should not exceed the maximum
        assert!(reassembler.active_reassemblies() <= MAX_CONCURRENT_REASSEMBLIES);
    }

    #[test]
    fn test_dos_protection_memory_limit() {
        let mut reassembler = MessageReassembler::new();

        // Create a large fragment that would exceed memory limits
        let large_data = vec![0x42; MAX_FRAGMENT_MEMORY + 1000];
        let message_id = Uuid::new_v4();

        let header = FragmentHeader::new(message_id, 0, 1, large_data.len() as u32, 0);

        let fragment = Fragment::new(header, large_data);

        // Should reject fragment that's too large
        let result = reassembler.process_fragment(fragment);
        assert!(result.is_err());
    }

    #[test]
    fn test_memory_accounting() {
        let mut reassembler = MessageReassembler::new();
        let message_id = Uuid::new_v4();
        let data = vec![0x42; 1000];

        let fragments = MessageFragmenter::fragment_message(message_id, &data, 512).unwrap();

        // Process first fragment
        assert_eq!(reassembler.total_memory_used(), 0);
        let result = reassembler.process_fragment(fragments[0].clone()).unwrap();
        assert!(result.is_none());
        assert!(reassembler.total_memory_used() > 0);

        let _memory_after_first = reassembler.total_memory_used();

        // Process remaining fragments
        for fragment in &fragments[1..] {
            let result = reassembler.process_fragment(fragment.clone()).unwrap();
            if result.is_some() {
                // Message completed, memory should be freed
                assert_eq!(reassembler.total_memory_used(), 0);
                break;
            }
        }
    }
}
