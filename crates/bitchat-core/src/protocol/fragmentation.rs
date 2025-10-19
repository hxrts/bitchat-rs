//! Message fragmentation and reassembly for MTU-limited transports
//!
//! This module handles splitting large messages into fragments for transport
//! over connections with limited Maximum Transmission Unit (MTU) sizes, such as BLE.
//!
//! ## Canonical Fragment Format
//!
//! Fragments use a canonical 13-byte header format:
//! - FragmentID: 8 bytes (u64, big-endian)
//! - Index: 2 bytes (u16, big-endian)
//! - Total: 2 bytes (u16, big-endian)
//! - OriginalType: 1 byte (u8)
//! - Data: remaining bytes

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::cmp;
use serde::{Deserialize, Serialize};

use crate::types::{PeerId, Timestamp};
use crate::{BitchatError, Result};

// ----------------------------------------------------------------------------
// Constants
// ----------------------------------------------------------------------------

/// Maximum fragment size for BLE transport (conservative estimate)
pub const BLE_MAX_FRAGMENT_SIZE: usize = 244;

/// Maximum fragment size for general use
pub const DEFAULT_MAX_FRAGMENT_SIZE: usize = 1024;

/// Maximum number of fragments per message
pub const MAX_FRAGMENTS_PER_MESSAGE: u16 = 256;

/// Fragment timeout in milliseconds (5 minutes)
pub const FRAGMENT_TIMEOUT_MS: u64 = 300_000;

// ----------------------------------------------------------------------------
// Fragment Header
// ----------------------------------------------------------------------------

/// Fragment header for message reconstruction
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FragmentHeader {
    /// Unique message ID for this fragmented message (8 bytes)
    pub fragment_id: u64,
    /// Fragment sequence number (0-based)
    pub fragment_index: u16,
    /// Total number of fragments for this message
    pub total_fragments: u16,
    /// Original message type before fragmentation
    pub original_type: u8,
}

impl FragmentHeader {
    /// Create a new fragment header
    pub fn new(
        fragment_id: u64,
        fragment_index: u16,
        total_fragments: u16,
        original_type: u8,
    ) -> Self {
        Self {
            fragment_id,
            fragment_index,
            total_fragments,
            original_type,
        }
    }

    /// Check if this is the last fragment
    pub fn is_last_fragment(&self) -> bool {
        self.fragment_index + 1 == self.total_fragments
    }

    /// Validate fragment header
    pub fn validate(&self) -> Result<()> {
        if self.total_fragments == 0 {
            return Err(BitchatError::invalid_packet(
                "Total fragments cannot be zero",
            ));
        }

        if self.fragment_index >= self.total_fragments {
            return Err(BitchatError::invalid_packet("Fragment index out of bounds"));
        }

        if self.total_fragments > MAX_FRAGMENTS_PER_MESSAGE {
            return Err(BitchatError::invalid_packet("Too many fragments"));
        }

        Ok(())
    }

    /// Serialize fragment header to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(13);
        bytes.extend_from_slice(&self.fragment_id.to_be_bytes());
        bytes.extend_from_slice(&self.fragment_index.to_be_bytes());
        bytes.extend_from_slice(&self.total_fragments.to_be_bytes());
        bytes.push(self.original_type);
        bytes
    }

    /// Deserialize fragment header from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 13 {
            return Err(BitchatError::invalid_packet("Fragment header too short"));
        }

        let fragment_id = u64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]);
        let fragment_index = u16::from_be_bytes([bytes[8], bytes[9]]);
        let total_fragments = u16::from_be_bytes([bytes[10], bytes[11]]);
        let original_type = bytes[12];

        let header = Self::new(fragment_id, fragment_index, total_fragments, original_type);
        header.validate()?;
        Ok(header)
    }
}

// ----------------------------------------------------------------------------
// Fragment
// ----------------------------------------------------------------------------

/// A message fragment containing header and payload data
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
            timestamp: Timestamp::now(),
        }
    }

    /// Get the fragment's data size for reassembly
    pub fn data_size(&self) -> usize {
        self.data.len()
    }

    /// Serialize fragment to wire format
    pub fn to_wire_format(&self) -> Vec<u8> {
        let mut bytes = self.header.to_bytes();
        bytes.extend_from_slice(&self.data);
        bytes
    }

    /// Deserialize fragment from wire format
    pub fn from_wire_format(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 13 {
            return Err(BitchatError::invalid_packet("Fragment too short"));
        }

        let header = FragmentHeader::from_bytes(&bytes[0..13])?;
        let data = bytes[13..].to_vec();

        Ok(Self::new(header, data))
    }
}

// ----------------------------------------------------------------------------
// Message Fragmenter
// ----------------------------------------------------------------------------

/// Handles fragmentation of large messages
pub struct MessageFragmenter {
    /// Maximum size per fragment
    max_fragment_size: usize,
    /// Next fragment ID to assign
    next_fragment_id: u64,
}

impl MessageFragmenter {
    /// Create a new fragmenter with the specified maximum fragment size
    pub fn new(max_fragment_size: usize) -> Self {
        Self {
            max_fragment_size,
            next_fragment_id: 1,
        }
    }

    /// Create a fragmenter optimized for BLE transport
    pub fn for_ble() -> Self {
        Self::new(BLE_MAX_FRAGMENT_SIZE)
    }

    /// Check if a message needs fragmentation
    pub fn needs_fragmentation(&self, message_size: usize) -> bool {
        message_size > self.max_fragment_size
    }

    /// Fragment a message into multiple fragments
    pub fn fragment_message(&mut self, data: &[u8], original_type: u8) -> Result<Vec<Fragment>> {
        if data.is_empty() {
            return Err(BitchatError::invalid_packet(
                "Cannot fragment empty message",
            ));
        }

        let fragment_id = self.next_fragment_id;
        self.next_fragment_id = self.next_fragment_id.wrapping_add(1);

        // Calculate fragments needed
        let fragment_data_size = self.max_fragment_size.saturating_sub(13); // Account for header
        let total_fragments = data.len().div_ceil(fragment_data_size);

        if total_fragments > MAX_FRAGMENTS_PER_MESSAGE as usize {
            return Err(BitchatError::invalid_packet(
                "Message too large to fragment",
            ));
        }

        let mut fragments = Vec::with_capacity(total_fragments);

        for i in 0..total_fragments {
            let start = i * fragment_data_size;
            let end = cmp::min(start + fragment_data_size, data.len());
            let fragment_data = data[start..end].to_vec();

            let header =
                FragmentHeader::new(fragment_id, i as u16, total_fragments as u16, original_type);

            fragments.push(Fragment::new(header, fragment_data));
        }

        Ok(fragments)
    }

    /// Get the current fragment ID (for testing)
    pub fn current_fragment_id(&self) -> u64 {
        self.next_fragment_id
    }
}

// ----------------------------------------------------------------------------
// Fragment Reassembler
// ----------------------------------------------------------------------------

/// Tracks incomplete messages being reassembled
#[derive(Debug, Clone)]
struct IncompleteMessage {
    /// Expected number of fragments
    total_fragments: u16,
    /// Original message type before fragmentation
    original_type: u8,
    /// Fragments received so far
    fragments: BTreeMap<u16, Vec<u8>>,
    /// Timestamp of first fragment received
    first_fragment_time: Timestamp,
    /// Sender of the message
    _sender: PeerId,
}

impl IncompleteMessage {
    fn new(header: &FragmentHeader, sender: PeerId) -> Self {
        Self {
            total_fragments: header.total_fragments,
            original_type: header.original_type,
            fragments: BTreeMap::new(),
            first_fragment_time: Timestamp::now(),
            _sender: sender,
        }
    }

    fn add_fragment(&mut self, fragment: &Fragment) -> bool {
        let index = fragment.header.fragment_index;
        self.fragments
            .entry(index)
            .or_insert_with(|| fragment.data.clone());
        self.is_complete()
    }

    fn is_complete(&self) -> bool {
        self.fragments.len() == self.total_fragments as usize
    }

    fn is_expired(&self, current_time: Timestamp) -> bool {
        current_time
            .as_millis()
            .saturating_sub(self.first_fragment_time.as_millis())
            > FRAGMENT_TIMEOUT_MS
    }

    fn reassemble(&self) -> Result<Vec<u8>> {
        if !self.is_complete() {
            return Err(BitchatError::invalid_packet("Message is incomplete"));
        }

        let mut reassembled = Vec::new();

        // Reassemble fragments in order
        for i in 0..self.total_fragments {
            if let Some(fragment_data) = self.fragments.get(&i) {
                reassembled.extend_from_slice(fragment_data);
            } else {
                return Err(BitchatError::invalid_packet("Missing fragment"));
            }
        }

        Ok(reassembled)
    }

    fn get_original_type(&self) -> u8 {
        self.original_type
    }
}

/// Handles reassembly of fragmented messages
pub struct MessageReassembler {
    /// Incomplete messages being reassembled, keyed by (sender, fragment_id)
    incomplete_messages: BTreeMap<(PeerId, u64), IncompleteMessage>,
}

impl MessageReassembler {
    /// Create a new reassembler
    pub fn new() -> Self {
        Self {
            incomplete_messages: BTreeMap::new(),
        }
    }

    /// Process a received fragment
    /// Returns Some((data, original_type)) if the message is complete, None if more fragments are needed
    pub fn add_fragment(
        &mut self,
        fragment: Fragment,
        sender: PeerId,
    ) -> Result<Option<(Vec<u8>, u8)>> {
        fragment.header.validate()?;

        let key = (sender, fragment.header.fragment_id);

        // Check if this is a single-fragment message
        if fragment.header.total_fragments == 1 {
            return Ok(Some((fragment.data, fragment.header.original_type)));
        }

        // Get or create incomplete message
        let incomplete = self
            .incomplete_messages
            .entry(key)
            .or_insert_with(|| IncompleteMessage::new(&fragment.header, sender));

        // Validate fragment consistency
        if incomplete.total_fragments != fragment.header.total_fragments
            || incomplete.original_type != fragment.header.original_type
        {
            return Err(BitchatError::invalid_packet("Fragment header mismatch"));
        }

        // Add fragment and check for completion
        let is_complete = incomplete.add_fragment(&fragment);

        if is_complete {
            let reassembled = incomplete.reassemble()?;
            let original_type = incomplete.get_original_type();
            self.incomplete_messages.remove(&key);
            Ok(Some((reassembled, original_type)))
        } else {
            Ok(None)
        }
    }

    /// Clean up expired incomplete messages
    pub fn cleanup_expired(&mut self) {
        let current_time = Timestamp::now();
        self.incomplete_messages
            .retain(|_, incomplete| !incomplete.is_expired(current_time));
    }

    /// Get the number of incomplete messages
    pub fn incomplete_count(&self) -> usize {
        self.incomplete_messages.len()
    }

    /// Clear all incomplete messages
    pub fn clear(&mut self) {
        self.incomplete_messages.clear();
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

    fn create_test_peer_id(id: u8) -> PeerId {
        PeerId::new([id, 0, 0, 0, 0, 0, 0, 0])
    }

    #[test]
    fn test_fragment_header_serialization() {
        let header = FragmentHeader::new(12345678901234, 5, 10, 0x02);
        let bytes = header.to_bytes();
        assert_eq!(bytes.len(), 13);

        let parsed = FragmentHeader::from_bytes(&bytes).unwrap();
        assert_eq!(parsed, header);
    }

    #[test]
    fn test_fragment_header_validation() {
        // Valid header
        let header = FragmentHeader::new(1, 0, 5, 0x02);
        assert!(header.validate().is_ok());

        // Invalid: zero fragments
        let header = FragmentHeader::new(1, 0, 0, 0x02);
        assert!(header.validate().is_err());

        // Invalid: fragment index out of bounds
        let header = FragmentHeader::new(1, 5, 5, 0x02);
        assert!(header.validate().is_err());

        // Invalid: too many fragments
        let header = FragmentHeader::new(1, 0, MAX_FRAGMENTS_PER_MESSAGE + 1, 0x02);
        assert!(header.validate().is_err());
    }

    #[test]
    fn test_message_fragmentation() {
        let mut fragmenter = MessageFragmenter::new(100);
        let data = vec![0u8; 250]; // Will need 3 fragments with header overhead

        let fragments = fragmenter.fragment_message(&data, 0x02).unwrap();
        assert_eq!(fragments.len(), 3);

        // Check fragment consistency
        for (i, fragment) in fragments.iter().enumerate() {
            assert_eq!(fragment.header.fragment_id, 1);
            assert_eq!(fragment.header.fragment_index, i as u16);
            assert_eq!(fragment.header.total_fragments, 3);
            assert_eq!(fragment.header.original_type, 0x02);
        }
    }

    #[test]
    fn test_message_reassembly() {
        let mut fragmenter = MessageFragmenter::new(100);
        let mut reassembler = MessageReassembler::new();
        let sender = create_test_peer_id(1);

        let original_data = b"Hello, this is a test message that will be fragmented".to_vec();
        let fragments = fragmenter.fragment_message(&original_data, 0x02).unwrap();

        // Add fragments out of order
        let mut reassembled_result = None;
        for fragment in fragments.iter().rev() {
            if let Some((data, original_type)) =
                reassembler.add_fragment(fragment.clone(), sender).unwrap()
            {
                reassembled_result = Some((data, original_type));
                break;
            }
        }

        let (reassembled_data, original_type) = reassembled_result.unwrap();
        assert_eq!(reassembled_data, original_data);
        assert_eq!(original_type, 0x02);
    }

    #[test]
    fn test_single_fragment_message() {
        let mut reassembler = MessageReassembler::new();
        let sender = create_test_peer_id(1);

        let header = FragmentHeader::new(1, 0, 1, 0x02);
        let fragment = Fragment::new(header, b"hello".to_vec());

        let result = reassembler.add_fragment(fragment, sender).unwrap();
        let (data, original_type) = result.unwrap();
        assert_eq!(data, b"hello");
        assert_eq!(original_type, 0x02);
    }

    #[test]
    fn test_fragment_wire_format() {
        let header = FragmentHeader::new(123, 2, 5, 0x02);
        let fragment = Fragment::new(header, b"test data".to_vec());

        let wire_format = fragment.to_wire_format();
        let parsed = Fragment::from_wire_format(&wire_format).unwrap();

        assert_eq!(parsed.header, fragment.header);
        assert_eq!(parsed.data, fragment.data);
    }

    #[test]
    fn test_fragmenter_for_ble() {
        let fragmenter = MessageFragmenter::for_ble();
        assert_eq!(fragmenter.max_fragment_size, BLE_MAX_FRAGMENT_SIZE);

        // Should not need fragmentation for small messages
        assert!(!fragmenter.needs_fragmentation(100));

        // Should need fragmentation for large messages
        assert!(fragmenter.needs_fragmentation(1000));
    }

    #[test]
    fn test_reassembler_cleanup() {
        let mut reassembler = MessageReassembler::new();
        let sender = create_test_peer_id(1);

        // Add a partial message
        let header = FragmentHeader::new(1, 0, 2, 0x02);
        let fragment = Fragment::new(header, b"partial".to_vec());

        assert!(reassembler
            .add_fragment(fragment, sender)
            .unwrap()
            .is_none());
        assert_eq!(reassembler.incomplete_count(), 1);

        // Cleanup shouldn't remove recent messages
        reassembler.cleanup_expired();
        assert_eq!(reassembler.incomplete_count(), 1);

        // Clear all messages
        reassembler.clear();
        assert_eq!(reassembler.incomplete_count(), 0);
    }

    #[test]
    fn test_canonical_fragment_format() {
        // Test the canonical format: FragmentID(8) + Index(2) + Total(2) + OriginalType(1) + Data
        let fragment_id = 0x123456789ABCDEF0u64;
        let index = 5u16;
        let total = 10u16; // Use a valid number of fragments
        let original_type = 0x42u8;
        let data = b"test payload data".to_vec();

        let header = FragmentHeader::new(fragment_id, index, total, original_type);
        let fragment = Fragment::new(header, data.clone());

        // Test wire format
        let wire_bytes = fragment.to_wire_format();

        // Verify the byte layout
        assert_eq!(wire_bytes.len(), 13 + data.len());

        // Check fragment ID (8 bytes, big endian)
        assert_eq!(&wire_bytes[0..8], &fragment_id.to_be_bytes());

        // Check index (2 bytes, big endian)
        assert_eq!(&wire_bytes[8..10], &index.to_be_bytes());

        // Check total (2 bytes, big endian)
        assert_eq!(&wire_bytes[10..12], &total.to_be_bytes());

        // Check original type (1 byte)
        assert_eq!(wire_bytes[12], original_type);

        // Check data
        assert_eq!(&wire_bytes[13..], &data);

        // Test round-trip parsing
        let parsed_fragment = Fragment::from_wire_format(&wire_bytes).unwrap();
        assert_eq!(parsed_fragment.header.fragment_id, fragment_id);
        assert_eq!(parsed_fragment.header.fragment_index, index);
        assert_eq!(parsed_fragment.header.total_fragments, total);
        assert_eq!(parsed_fragment.header.original_type, original_type);
        assert_eq!(parsed_fragment.data, data);
    }
}
