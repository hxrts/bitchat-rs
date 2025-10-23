//! Read receipts and delivery acknowledgments
//!
//! This module implements canonical-compatible read receipts and delivery acknowledgments
//! as specified in the Swift/iOS reference implementation. These provide reliable message
//! delivery confirmation and read status tracking across BitChat's dual transport architecture.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::protocol::message::NoisePayloadType;
use crate::protocol::message_store::MessageId;
use crate::types::{PeerId, Timestamp};
use crate::{BitchatError, Result};

// ----------------------------------------------------------------------------
// Receipt Types
// ----------------------------------------------------------------------------

/// Status of a message in the read receipt system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReceiptType {
    /// Message has been delivered to the recipient
    Delivered,
    /// Message has been read by the recipient
    Read,
}

// ----------------------------------------------------------------------------
// Delivery Acknowledgment
// ----------------------------------------------------------------------------

/// Delivery acknowledgment confirming message receipt
/// 
/// Sent automatically when a private message is received to confirm delivery.
/// Follows the canonical implementation pattern with message ID and timestamp.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeliveryAck {
    /// ID of the original message being acknowledged
    pub message_id: MessageId,
    /// Peer ID of the sender who will receive this acknowledgment
    pub to_peer_id: PeerId,
    /// Timestamp when the message was received and ack was created
    pub timestamp: Timestamp,
    /// Optional recipient nickname for display purposes
    pub recipient_nickname: Option<String>,
}

impl DeliveryAck {
    /// Create a new delivery acknowledgment
    pub fn new(
        message_id: MessageId,
        to_peer_id: PeerId,
        recipient_nickname: Option<String>,
    ) -> Self {
        Self {
            message_id,
            to_peer_id,
            timestamp: Timestamp::now(),
            recipient_nickname,
        }
    }

    /// Get the corresponding NoisePayloadType for this acknowledgment
    pub fn payload_type(&self) -> NoisePayloadType {
        NoisePayloadType::Delivered
    }

    /// Serialize to binary format for transmission
    pub fn to_binary(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(|e| BitchatError::serialization_error_with_message(e.to_string()))
    }

    /// Deserialize from binary format
    pub fn from_binary(data: &[u8]) -> Result<Self> {
        bincode::deserialize(data).map_err(|e| BitchatError::serialization_error_with_message(e.to_string()))
    }

    /// Validate the acknowledgment data
    pub fn validate(&self) -> Result<()> {
        if let Some(ref nickname) = self.recipient_nickname {
            if nickname.len() > 64 {
                return Err(BitchatError::invalid_packet(
                    "Recipient nickname too long in delivery ack",
                ));
            }
        }
        Ok(())
    }
}

// ----------------------------------------------------------------------------
// Read Receipt
// ----------------------------------------------------------------------------

/// Read receipt confirming message has been read
/// 
/// Sent when a user views a private message to indicate read status.
/// Follows the canonical implementation with message ID, reader info, and timestamp.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadReceipt {
    /// ID of the original message being acknowledged as read
    pub message_id: MessageId,
    /// Peer ID of the reader (recipient who read the message)
    pub reader_peer_id: PeerId,
    /// Peer ID of the sender who will receive this receipt
    pub to_peer_id: PeerId,
    /// Timestamp when the message was read
    pub timestamp: Timestamp,
    /// Optional reader nickname for display purposes
    pub reader_nickname: Option<String>,
}

impl ReadReceipt {
    /// Create a new read receipt
    pub fn new(
        message_id: MessageId,
        reader_peer_id: PeerId,
        to_peer_id: PeerId,
        reader_nickname: Option<String>,
    ) -> Self {
        Self {
            message_id,
            reader_peer_id,
            to_peer_id,
            timestamp: Timestamp::now(),
            reader_nickname,
        }
    }

    /// Get the corresponding NoisePayloadType for this receipt
    pub fn payload_type(&self) -> NoisePayloadType {
        NoisePayloadType::ReadReceipt
    }

    /// Serialize to binary format for transmission
    pub fn to_binary(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(|e| BitchatError::serialization_error_with_message(e.to_string()))
    }

    /// Deserialize from binary format
    pub fn from_binary(data: &[u8]) -> Result<Self> {
        bincode::deserialize(data).map_err(|e| BitchatError::serialization_error_with_message(e.to_string()))
    }

    /// Validate the read receipt data
    pub fn validate(&self) -> Result<()> {
        if let Some(ref nickname) = self.reader_nickname {
            if nickname.len() > 64 {
                return Err(BitchatError::invalid_packet(
                    "Reader nickname too long in read receipt",
                ));
            }
        }
        Ok(())
    }
}

// ----------------------------------------------------------------------------
// Receipt Manager
// ----------------------------------------------------------------------------

/// Manages read receipts and delivery acknowledgments
/// 
/// Integrates with the existing DeliveryTracker to provide canonical-compatible
/// receipt tracking and prevents duplicate acknowledgments.
#[derive(Debug, Clone)]
pub struct ReceiptManager {
    /// Set of message IDs for which delivery acks have been sent
    sent_delivery_acks: hashbrown::HashSet<MessageId>,
    /// Set of message IDs for which read receipts have been sent
    sent_read_receipts: hashbrown::HashSet<MessageId>,
    /// Privacy setting: whether to send read receipts
    send_read_receipts_enabled: bool,
    /// Privacy setting: whether to send delivery acks
    send_delivery_acks_enabled: bool,
}

impl ReceiptManager {
    /// Create a new receipt manager with default settings
    pub fn new() -> Self {
        Self {
            sent_delivery_acks: hashbrown::HashSet::new(),
            sent_read_receipts: hashbrown::HashSet::new(),
            send_read_receipts_enabled: true,
            send_delivery_acks_enabled: true,
        }
    }

    /// Check if a delivery acknowledgment should be sent for this message
    pub fn should_send_delivery_ack(&self, message_id: &MessageId) -> bool {
        self.send_delivery_acks_enabled && !self.sent_delivery_acks.contains(message_id)
    }

    /// Check if a read receipt should be sent for this message
    pub fn should_send_read_receipt(&self, message_id: &MessageId) -> bool {
        self.send_read_receipts_enabled && !self.sent_read_receipts.contains(message_id)
    }

    /// Mark that a delivery acknowledgment has been sent for this message
    pub fn mark_delivery_ack_sent(&mut self, message_id: MessageId) {
        self.sent_delivery_acks.insert(message_id);
    }

    /// Mark that a read receipt has been sent for this message
    pub fn mark_read_receipt_sent(&mut self, message_id: MessageId) {
        self.sent_read_receipts.insert(message_id);
    }

    /// Enable or disable sending read receipts (privacy control)
    pub fn set_read_receipts_enabled(&mut self, enabled: bool) {
        self.send_read_receipts_enabled = enabled;
    }

    /// Enable or disable sending delivery acknowledgments (privacy control)
    pub fn set_delivery_acks_enabled(&mut self, enabled: bool) {
        self.send_delivery_acks_enabled = enabled;
    }

    /// Check if read receipts are enabled
    pub fn read_receipts_enabled(&self) -> bool {
        self.send_read_receipts_enabled
    }

    /// Check if delivery acknowledgments are enabled
    pub fn delivery_acks_enabled(&self) -> bool {
        self.send_delivery_acks_enabled
    }

    /// Clean up old receipt tracking data to prevent unbounded memory growth
    /// 
    /// Removes tracking data for messages older than the specified age.
    /// Should be called periodically in long-running applications.
    pub fn cleanup_old_receipts(&mut self, max_entries: usize) {
        // Simple cleanup: keep only the most recent entries
        // In a production implementation, this might use timestamps
        if self.sent_delivery_acks.len() > max_entries {
            let excess = self.sent_delivery_acks.len() - max_entries;
            let to_remove: Vec<MessageId> = self
                .sent_delivery_acks
                .iter()
                .take(excess)
                .cloned()
                .collect();
            for id in to_remove {
                self.sent_delivery_acks.remove(&id);
            }
        }

        if self.sent_read_receipts.len() > max_entries {
            let excess = self.sent_read_receipts.len() - max_entries;
            let to_remove: Vec<MessageId> = self
                .sent_read_receipts
                .iter()
                .take(excess)
                .cloned()
                .collect();
            for id in to_remove {
                self.sent_read_receipts.remove(&id);
            }
        }
    }

    /// Get statistics about receipt tracking
    pub fn get_stats(&self) -> ReceiptStats {
        ReceiptStats {
            delivery_acks_sent_count: self.sent_delivery_acks.len(),
            read_receipts_sent_count: self.sent_read_receipts.len(),
            read_receipts_enabled: self.send_read_receipts_enabled,
            delivery_acks_enabled: self.send_delivery_acks_enabled,
        }
    }
}

impl Default for ReceiptManager {
    fn default() -> Self {
        Self::new()
    }
}

// ----------------------------------------------------------------------------
// Receipt Statistics
// ----------------------------------------------------------------------------

/// Statistics about receipt manager activity
#[derive(Debug, Clone)]
pub struct ReceiptStats {
    /// Number of delivery acknowledgments sent
    pub delivery_acks_sent_count: usize,
    /// Number of read receipts sent
    pub read_receipts_sent_count: usize,
    /// Whether read receipts are enabled
    pub read_receipts_enabled: bool,
    /// Whether delivery acknowledgments are enabled
    pub delivery_acks_enabled: bool,
}

// ----------------------------------------------------------------------------
// Integration with Delivery Status
// ----------------------------------------------------------------------------

/// Enhanced delivery status that includes read receipt information
/// 
/// Extends the basic delivery status with canonical read receipt tracking
/// to match the Swift/iOS implementation's DeliveryStatus enum.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EnhancedDeliveryStatus {
    /// Message is being sent
    Sending,
    /// Message was sent successfully
    Sent,
    /// Message was delivered to recipient (with delivery details)
    Delivered {
        /// Peer who received the message
        recipient: PeerId,
        /// When the delivery was confirmed
        timestamp: Timestamp,
        /// Optional recipient nickname
        recipient_nickname: Option<String>,
    },
    /// Message was read by recipient (with read details)
    Read {
        /// Peer who read the message
        reader: PeerId,
        /// When the message was read
        timestamp: Timestamp,
        /// Optional reader nickname
        reader_nickname: Option<String>,
    },
    /// Delivery failed with reason
    Failed {
        /// Reason for failure
        reason: String,
    },
}

impl EnhancedDeliveryStatus {
    /// Create a new delivered status from a delivery acknowledgment
    pub fn from_delivery_ack(ack: &DeliveryAck) -> Self {
        Self::Delivered {
            recipient: ack.to_peer_id,
            timestamp: ack.timestamp,
            recipient_nickname: ack.recipient_nickname.clone(),
        }
    }

    /// Create a new read status from a read receipt
    pub fn from_read_receipt(receipt: &ReadReceipt) -> Self {
        Self::Read {
            reader: receipt.reader_peer_id,
            timestamp: receipt.timestamp,
            reader_nickname: receipt.reader_nickname.clone(),
        }
    }

    /// Check if this status represents a successful state
    pub fn is_successful(&self) -> bool {
        matches!(self, Self::Delivered { .. } | Self::Read { .. })
    }

    /// Check if this status represents a failure
    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed { .. })
    }

    /// Check if this status represents a completed delivery
    pub fn is_delivered(&self) -> bool {
        matches!(self, Self::Delivered { .. } | Self::Read { .. })
    }

    /// Check if this status represents a read message
    pub fn is_read(&self) -> bool {
        matches!(self, Self::Read { .. })
    }
}

// ----------------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_message_id() -> MessageId {
        use sha2::{Digest, Sha256};
        let hash = Sha256::digest(b"test message content");
        MessageId::from_bytes(hash.into())
    }

    fn create_test_peer_id(id: u8) -> PeerId {
        PeerId::new([id, 0, 0, 0, 0, 0, 0, 0])
    }

    #[test]
    fn test_delivery_ack_creation() {
        let message_id = create_test_message_id();
        let peer_id = create_test_peer_id(1);
        let nickname = Some("Alice".to_string());

        let ack = DeliveryAck::new(message_id, peer_id, nickname.clone());

        assert_eq!(ack.message_id, message_id);
        assert_eq!(ack.to_peer_id, peer_id);
        assert_eq!(ack.recipient_nickname, nickname);
        assert_eq!(ack.payload_type(), NoisePayloadType::Delivered);

        ack.validate().unwrap();
    }

    #[test]
    fn test_read_receipt_creation() {
        let message_id = create_test_message_id();
        let reader_id = create_test_peer_id(1);
        let sender_id = create_test_peer_id(2);
        let nickname = Some("Bob".to_string());

        let receipt = ReadReceipt::new(message_id, reader_id, sender_id, nickname.clone());

        assert_eq!(receipt.message_id, message_id);
        assert_eq!(receipt.reader_peer_id, reader_id);
        assert_eq!(receipt.to_peer_id, sender_id);
        assert_eq!(receipt.reader_nickname, nickname);
        assert_eq!(receipt.payload_type(), NoisePayloadType::ReadReceipt);

        receipt.validate().unwrap();
    }

    #[test]
    fn test_delivery_ack_binary_roundtrip() {
        let message_id = create_test_message_id();
        let peer_id = create_test_peer_id(1);
        let ack = DeliveryAck::new(message_id, peer_id, Some("Charlie".to_string()));

        let binary = ack.to_binary().unwrap();
        let parsed = DeliveryAck::from_binary(&binary).unwrap();

        assert_eq!(ack, parsed);
    }

    #[test]
    fn test_read_receipt_binary_roundtrip() {
        let message_id = create_test_message_id();
        let reader_id = create_test_peer_id(1);
        let sender_id = create_test_peer_id(2);
        let receipt = ReadReceipt::new(message_id, reader_id, sender_id, Some("Dave".to_string()));

        let binary = receipt.to_binary().unwrap();
        let parsed = ReadReceipt::from_binary(&binary).unwrap();

        assert_eq!(receipt, parsed);
    }

    #[test]
    fn test_receipt_manager_duplicates() {
        let mut manager = ReceiptManager::new();
        let message_id = create_test_message_id();

        // Should allow first receipt
        assert!(manager.should_send_delivery_ack(&message_id));
        assert!(manager.should_send_read_receipt(&message_id));

        // Mark as sent
        manager.mark_delivery_ack_sent(message_id);
        manager.mark_read_receipt_sent(message_id);

        // Should prevent duplicates
        assert!(!manager.should_send_delivery_ack(&message_id));
        assert!(!manager.should_send_read_receipt(&message_id));
    }

    #[test]
    fn test_receipt_manager_privacy_controls() {
        let mut manager = ReceiptManager::new();
        let message_id = create_test_message_id();

        // Disable read receipts
        manager.set_read_receipts_enabled(false);
        assert!(!manager.should_send_read_receipt(&message_id));
        assert!(manager.should_send_delivery_ack(&message_id)); // Still enabled

        // Disable delivery acks
        manager.set_delivery_acks_enabled(false);
        assert!(!manager.should_send_delivery_ack(&message_id));
    }

    #[test]
    fn test_enhanced_delivery_status() {
        let message_id = create_test_message_id();
        let peer_id = create_test_peer_id(1);

        // Test delivery status
        let ack = DeliveryAck::new(message_id, peer_id, Some("Alice".to_string()));
        let delivered_status = EnhancedDeliveryStatus::from_delivery_ack(&ack);

        assert!(delivered_status.is_successful());
        assert!(delivered_status.is_delivered());
        assert!(!delivered_status.is_read());

        // Test read status
        let receipt = ReadReceipt::new(message_id, peer_id, peer_id, Some("Alice".to_string()));
        let read_status = EnhancedDeliveryStatus::from_read_receipt(&receipt);

        assert!(read_status.is_successful());
        assert!(read_status.is_delivered());
        assert!(read_status.is_read());

        // Test failed status
        let failed_status = EnhancedDeliveryStatus::Failed {
            reason: "Network timeout".to_string(),
        };

        assert!(failed_status.is_failed());
        assert!(!failed_status.is_delivered());
    }

    #[test]
    fn test_receipt_validation() {
        let message_id = create_test_message_id();
        let peer_id = create_test_peer_id(1);

        // Test valid acknowledgments
        let valid_ack = DeliveryAck::new(message_id, peer_id, Some("Alice".to_string()));
        valid_ack.validate().unwrap();

        let valid_receipt = ReadReceipt::new(message_id, peer_id, peer_id, Some("Bob".to_string()));
        valid_receipt.validate().unwrap();

        // Test invalid nickname length
        let long_nickname = "a".repeat(100);
        let invalid_ack = DeliveryAck::new(message_id, peer_id, Some(long_nickname.clone()));
        assert!(invalid_ack.validate().is_err());

        let invalid_receipt = ReadReceipt::new(message_id, peer_id, peer_id, Some(long_nickname));
        assert!(invalid_receipt.validate().is_err());
    }
}