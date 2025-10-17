//! Message fragmentation and reassembly for large BitChat messages
//!
//! This module handles splitting large messages into smaller fragments for transport
//! and reassembling them on the receiving end, following the BitChat protocol specification.

use alloc::{collections::BTreeMap, vec::Vec};
use core::cmp;
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
pub const MAX_FRAGMENTS: u16 = 65535;

/// Fragment timeout in seconds
#[cfg(feature = "std")]
pub const FRAGMENT_TIMEOUT_SECS: u64 = 60;

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
    pub fn to_packet(&self, sender_id: PeerId, recipient_id: Option<PeerId>) -> Result<BitchatPacket> {
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
        if !matches!(packet.message_type, 
            MessageType::FragmentStart | MessageType::FragmentContinue | MessageType::FragmentEnd) {
            return Err(BitchatError::InvalidPacket("Not a fragment packet".into()));
        }
        
        // Deserialize header first
        let header: FragmentHeader = bincode::deserialize(&packet.payload)?;
        let header_size = bincode::serialized_size(&header)? as usize;
        
        if packet.payload.len() < header_size {
            return Err(BitchatError::InvalidPacket("Fragment payload too small".into()));
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
            return Err(BitchatError::InvalidPacket("Cannot fragment empty message".into()));
        }
        
        let total_size = data.len() as u32;
        let checksum = Self::calculate_checksum(data);
        
        // Calculate fragment payload size (excluding header overhead)
        let header = FragmentHeader::new(message_id, 0, 1, total_size, checksum);
        let header_size = bincode::serialized_size(&header)? as usize;
        
        if max_fragment_size <= header_size {
            return Err(BitchatError::InvalidPacket("Fragment size too small for header".into()));
        }
        
        let fragment_payload_size = max_fragment_size - header_size;
        let total_fragments = ((data.len() + fragment_payload_size - 1) / fragment_payload_size) as u16;
        
        if total_fragments > MAX_FRAGMENTS {
            return Err(BitchatError::InvalidPacket("Message too large to fragment".into()));
        }
        
        let mut fragments = Vec::with_capacity(total_fragments as usize);
        
        for i in 0..total_fragments {
            let start = i as usize * fragment_payload_size;
            let end = cmp::min(start + fragment_payload_size, data.len());
            let fragment_data = data[start..end].to_vec();
            
            let header = FragmentHeader::new(
                message_id,
                i,
                total_fragments,
                total_size,
                checksum,
            );
            
            fragments.push(Fragment::new(header, fragment_data));
        }
        
        Ok(fragments)
    }
    
    /// Calculate CRC32 checksum
    fn calculate_checksum(data: &[u8]) -> u32 {
        // Simple CRC32 implementation for checksum verification
        let mut crc = 0xFFFFFFFF;
        for &byte in data {
            crc ^= byte as u32;
            for _ in 0..8 {
                if crc & 1 != 0 {
                    crc = (crc >> 1) ^ 0xEDB88320;
                } else {
                    crc >>= 1;
                }
            }
        }
        !crc
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
    fragments: BTreeMap<u16, Vec<u8>>,
    /// Timestamp when first fragment was received
    first_received: Timestamp,
}

impl ReassemblyState {
    fn new(header: &FragmentHeader) -> Self {
        Self {
            total_fragments: header.total_fragments,
            total_size: header.total_size,
            checksum: header.checksum,
            fragments: BTreeMap::new(),
            #[cfg(feature = "std")]
            first_received: Timestamp::now(),
            #[cfg(not(feature = "std"))]
            first_received: Timestamp::new(0),
        }
    }
    
    fn is_complete(&self) -> bool {
        self.fragments.len() == self.total_fragments as usize
    }
    
    fn add_fragment(&mut self, fragment: &Fragment) -> Result<()> {
        // Validate fragment metadata matches
        if fragment.header.total_fragments != self.total_fragments ||
           fragment.header.total_size != self.total_size ||
           fragment.header.checksum != self.checksum {
            return Err(BitchatError::InvalidPacket("Fragment metadata mismatch".into()));
        }
        
        // Check for duplicate fragments
        if self.fragments.contains_key(&fragment.header.fragment_number) {
            return Err(BitchatError::InvalidPacket("Duplicate fragment".into()));
        }
        
        self.fragments.insert(fragment.header.fragment_number, fragment.data.clone());
        Ok(())
    }
    
    fn assemble(&self) -> Result<Vec<u8>> {
        if !self.is_complete() {
            return Err(BitchatError::InvalidPacket("Incomplete fragment set".into()));
        }
        
        let mut assembled = Vec::with_capacity(self.total_size as usize);
        
        // Assemble fragments in order
        for i in 0..self.total_fragments {
            if let Some(fragment_data) = self.fragments.get(&i) {
                assembled.extend_from_slice(fragment_data);
            } else {
                return Err(BitchatError::InvalidPacket("Missing fragment".into()));
            }
        }
        
        // Verify assembled size
        if assembled.len() != self.total_size as usize {
            return Err(BitchatError::InvalidPacket("Assembled size mismatch".into()));
        }
        
        // Verify checksum
        let calculated_checksum = MessageFragmenter::calculate_checksum(&assembled);
        if calculated_checksum != self.checksum {
            return Err(BitchatError::InvalidPacket("Checksum verification failed".into()));
        }
        
        Ok(assembled)
    }
    
    #[cfg(feature = "std")]
    fn is_expired(&self) -> bool {
        use core::time::Duration;
        let now = Timestamp::now();
        let elapsed = Duration::from_millis(
            now.as_millis().saturating_sub(self.first_received.as_millis())
        );
        elapsed.as_secs() > FRAGMENT_TIMEOUT_SECS
    }
}

/// Reassembles fragmented messages
pub struct MessageReassembler {
    /// Ongoing reassembly operations by message ID
    reassembly_states: BTreeMap<Uuid, ReassemblyState>,
}

impl MessageReassembler {
    /// Create a new message reassembler
    pub fn new() -> Self {
        Self {
            reassembly_states: BTreeMap::new(),
        }
    }
    
    /// Process a fragment, potentially completing a message
    pub fn process_fragment(&mut self, fragment: Fragment) -> Result<Option<Vec<u8>>> {
        let message_id = fragment.header.message_id;
        
        // Get or create reassembly state
        if !self.reassembly_states.contains_key(&message_id) {
            let state = ReassemblyState::new(&fragment.header);
            self.reassembly_states.insert(message_id, state);
        }
        
        let state = self.reassembly_states.get_mut(&message_id).unwrap();
        state.add_fragment(&fragment)?;
        
        // Check if message is complete
        if state.is_complete() {
            let assembled = state.assemble()?;
            self.reassembly_states.remove(&message_id);
            Ok(Some(assembled))
        } else {
            Ok(None)
        }
    }
    
    /// Clean up expired reassembly states
    #[cfg(feature = "std")]
    pub fn cleanup_expired(&mut self) {
        let expired_ids: Vec<Uuid> = self.reassembly_states
            .iter()
            .filter_map(|(id, state)| {
                if state.is_expired() { Some(*id) } else { None }
            })
            .collect();
        
        for id in expired_ids {
            self.reassembly_states.remove(&id);
        }
    }
    
    /// Get count of active reassembly operations
    pub fn active_reassemblies(&self) -> usize {
        self.reassembly_states.len()
    }
    
    /// Remove incomplete reassembly state
    pub fn cancel_reassembly(&mut self, message_id: &Uuid) -> bool {
        self.reassembly_states.remove(message_id).is_some()
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
        
        let fragments = MessageFragmenter::fragment_message(
            message_id,
            data,
            MAX_FRAGMENT_SIZE,
        ).unwrap();
        
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
        
        let fragments = MessageFragmenter::fragment_message(
            message_id,
            &data,
            fragment_size,
        ).unwrap();
        
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
        
        let fragments = MessageFragmenter::fragment_message(
            message_id,
            data,
            MAX_FRAGMENT_SIZE,
        ).unwrap();
        
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
        
        let fragments = MessageFragmenter::fragment_message(
            message_id,
            &data,
            fragment_size,
        ).unwrap();
        
        let mut reassembler = MessageReassembler::new();
        
        // Process all but last fragment
        for fragment in &fragments[..fragments.len() - 1] {
            let result = reassembler.process_fragment(fragment.clone()).unwrap();
            assert!(result.is_none()); // Should not be complete yet
        }
        
        // Process last fragment
        let result = reassembler.process_fragment(fragments[fragments.len() - 1].clone()).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), data);
    }
    
    #[test]
    fn test_fragment_packet_conversion() {
        let message_id = Uuid::new_v4();
        let data = b"Hello, world!";
        let sender_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        
        let fragments = MessageFragmenter::fragment_message(
            message_id,
            data,
            MAX_FRAGMENT_SIZE,
        ).unwrap();
        
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
        
        let fragments = MessageFragmenter::fragment_message(
            message_id,
            data,
            MAX_FRAGMENT_SIZE,
        ).unwrap();
        
        let mut reassembler = MessageReassembler::new();
        
        // Corrupt the fragment data
        let mut corrupted_fragment = fragments[0].clone();
        corrupted_fragment.data[0] ^= 0xFF;
        
        // Should fail checksum verification
        let result = reassembler.process_fragment(corrupted_fragment);
        assert!(result.is_err());
    }
}