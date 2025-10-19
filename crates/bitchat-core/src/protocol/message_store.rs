//! Content-Addressed Message Storage
//!
//! Provides immutable message storage with cryptographic identifiers for
//! automatic deduplication and integrity verification.

use crate::types::Timestamp;
use crate::{BitchatError, PeerId, Result as BitchatResult};
use alloc::collections::BTreeMap;
use hashbrown::HashMap;
use serde::{Deserialize, Serialize};

cfg_if::cfg_if! {
    if #[cfg(feature = "std")] {
        use std::fmt;
    } else {
        use core::fmt;
        use alloc::string::{String, ToString};
        use alloc::vec::Vec;
        use alloc::format;
        use alloc::boxed::Box;
    }
}
use sha2::{Digest, Sha256};

// ----------------------------------------------------------------------------
// Message Types
// ----------------------------------------------------------------------------

/// Cryptographic identifier for content-addressed messages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId([u8; 32]);

impl MessageId {
    /// Create MessageId from hash bytes
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Get the raw bytes of the message ID
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Convert to hex string for display
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    /// Parse from hex string
    pub fn from_hex(hex_str: &str) -> Result<Self, hex::FromHexError> {
        let bytes = hex::decode(hex_str)?;
        if bytes.len() != 32 {
            return Err(hex::FromHexError::InvalidStringLength);
        }
        let mut array = [0u8; 32];
        array.copy_from_slice(&bytes);
        Ok(Self(array))
    }
}

impl fmt::Display for MessageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

/// Immutable message with content-addressed identity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentAddressedMessage {
    /// Cryptographic hash serving as both ID and integrity proof
    pub id: MessageId,
    /// Sender's peer ID
    pub sender: PeerId,
    /// Optional recipient (None for broadcast messages)
    pub recipient: Option<PeerId>,
    /// Message content
    pub content: String,
    /// Timestamp when message was created
    pub timestamp: u64,
    /// Message sequence number for ordering within conversations
    pub sequence: u64,
}

impl ContentAddressedMessage {
    /// Create new message with content-addressed ID
    pub fn new(sender: PeerId, recipient: Option<PeerId>, content: String, sequence: u64) -> Self {
        let timestamp = {
            cfg_if::cfg_if! {
                if #[cfg(any(feature = "std", feature = "wasm"))] {
                    Timestamp::now().as_millis()
                } else {
                    0  // Fallback when no time features enabled
                }
            }
        };

        let id = Self::compute_id(&sender, &recipient, &content, timestamp, sequence);

        Self {
            id,
            sender,
            recipient,
            content,
            timestamp,
            sequence,
        }
    }

    /// Create message from externally supplied metadata (e.g., received from peer)
    pub fn from_metadata(
        sender: PeerId,
        recipient: Option<PeerId>,
        content: String,
        sequence: u64,
        timestamp: u64,
        message_id: Option<MessageId>,
    ) -> BitchatResult<Self> {
        let normalized_timestamp = if timestamp < 1_000_000_000_000 {
            timestamp.saturating_mul(1_000)
        } else {
            timestamp
        };

        let computed_id = Self::compute_id(
            &sender,
            &recipient,
            &content,
            normalized_timestamp,
            sequence,
        );

        if let Some(provided_id) = message_id {
            if provided_id != computed_id {
                return Err(BitchatError::invalid_packet(
                    "Provided message ID does not match computed content hash",
                ));
            }

            Ok(Self {
                id: provided_id,
                sender,
                recipient,
                content,
                timestamp: normalized_timestamp,
                sequence,
            })
        } else {
            Ok(Self {
                id: computed_id,
                sender,
                recipient,
                content,
                timestamp: normalized_timestamp,
                sequence,
            })
        }
    }

    /// Compute content-addressed ID from message components
    fn compute_id(
        sender: &PeerId,
        recipient: &Option<PeerId>,
        content: &str,
        timestamp: u64,
        sequence: u64,
    ) -> MessageId {
        let mut hasher = Sha256::new();

        // Hash sender
        hasher.update(sender.as_bytes());

        // Hash recipient (use zeros for None)
        match recipient {
            Some(peer_id) => hasher.update(peer_id.as_bytes()),
            None => hasher.update([0u8; 8]),
        }

        // Hash content
        hasher.update(content.as_bytes());

        // Hash timestamp
        hasher.update(timestamp.to_be_bytes());

        // Hash sequence
        hasher.update(sequence.to_be_bytes());

        let hash = hasher.finalize();
        MessageId::from_bytes(hash.into())
    }

    /// Verify message integrity by recomputing hash
    pub fn verify_integrity(&self) -> bool {
        let computed_id = Self::compute_id(
            &self.sender,
            &self.recipient,
            &self.content,
            self.timestamp,
            self.sequence,
        );
        computed_id == self.id
    }

    /// Get conversation ID (pair of sender/recipient)
    pub fn conversation_id(&self) -> ConversationId {
        match self.recipient {
            Some(recipient) => {
                // Ensure consistent ordering for conversation ID
                if self.sender.as_bytes() < recipient.as_bytes() {
                    ConversationId::new(self.sender, recipient)
                } else {
                    ConversationId::new(recipient, self.sender)
                }
            }
            None => ConversationId::broadcast(),
        }
    }
}

/// Conversation identifier for grouping messages
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConversationId {
    /// Direct conversation between two peers
    Direct { peer1: PeerId, peer2: PeerId },
    /// Broadcast conversation (all peers)
    Broadcast,
}

impl ConversationId {
    pub fn new(peer1: PeerId, peer2: PeerId) -> Self {
        // Ensure consistent ordering
        if peer1.as_bytes() < peer2.as_bytes() {
            Self::Direct { peer1, peer2 }
        } else {
            Self::Direct {
                peer1: peer2,
                peer2: peer1,
            }
        }
    }

    pub fn broadcast() -> Self {
        Self::Broadcast
    }

    pub fn involves_peer(&self, peer_id: &PeerId) -> bool {
        match self {
            Self::Direct { peer1, peer2 } => peer1 == peer_id || peer2 == peer_id,
            Self::Broadcast => true,
        }
    }
}

impl fmt::Display for ConversationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Direct { peer1, peer2 } => write!(f, "{}↔{}", peer1, peer2),
            Self::Broadcast => write!(f, "broadcast"),
        }
    }
}

// ----------------------------------------------------------------------------
// Message Store Implementation
// ----------------------------------------------------------------------------

use crate::config::MessageStoreConfig;

/// Content-addressed message store with automatic deduplication
#[derive(Debug)]
pub struct MessageStore {
    /// Messages indexed by content-addressed ID
    messages: HashMap<MessageId, ContentAddressedMessage>,
    /// Conversations indexed by conversation ID
    conversations: HashMap<ConversationId, Vec<MessageId>>,
    /// Messages indexed by timestamp for time-based queries
    time_index: BTreeMap<u64, Vec<MessageId>>,
    /// Configuration for validation and limits
    config: MessageStoreConfig,
    /// Statistics
    stats: MessageStoreStats,
}

#[derive(Debug, Clone)]
pub struct MessageStoreStats {
    pub total_messages: usize,
    pub unique_conversations: usize,
    pub deduplication_saves: usize,
    pub integrity_failures: usize,
}

impl MessageStore {
    /// Create new empty message store with default configuration
    pub fn new() -> Self {
        Self::with_config(MessageStoreConfig::default())
    }

    /// Create new empty message store with specified configuration
    pub fn with_config(config: MessageStoreConfig) -> Self {
        Self {
            messages: HashMap::default(),
            conversations: HashMap::default(),
            time_index: BTreeMap::new(),
            config,
            stats: MessageStoreStats {
                total_messages: 0,
                unique_conversations: 0,
                deduplication_saves: 0,
                integrity_failures: 0,
            },
        }
    }

    /// Validate message input against configuration limits
    fn validate_message_input(&self, message: &ContentAddressedMessage) -> BitchatResult<()> {
        // Check content length limit (in characters)
        let char_count = message.content.chars().count();
        if char_count > self.config.max_content_length {
            return Err(BitchatError::InvalidPacket(
                format!(
                    "Message content exceeds maximum length of {} characters",
                    self.config.max_content_length
                )
                .into(),
            ));
        }

        // Check message size limit (in bytes) - use serialized size
        let message_size =
            bincode::serialized_size(message).map_err(|_| BitchatError::serialization_error())?;

        if message_size as usize > self.config.max_message_size {
            return Err(BitchatError::InvalidPacket(
                format!(
                    "Message size {} bytes exceeds maximum of {} bytes",
                    message_size, self.config.max_message_size
                )
                .into(),
            ));
        }

        // Strict content validation if enabled
        if self.config.strict_content_validation {
            self.validate_content_safety(&message.content)?;
        }

        Ok(())
    }

    /// Validate content for potential security issues
    fn validate_content_safety(&self, content: &str) -> BitchatResult<()> {
        // Check for null bytes (can cause issues in some contexts)
        if content.contains('\0') {
            return Err(BitchatError::InvalidPacket(
                "Message content contains null bytes".into(),
            ));
        }

        // Check for excessive whitespace that could be a DoS attempt
        let total_chars = content.chars().count();
        if total_chars > 0 {
            let whitespace_count = content.chars().filter(|c| c.is_whitespace()).count();
            let whitespace_ratio = whitespace_count as f32 / total_chars as f32;

            if whitespace_ratio > 0.9 {
                return Err(BitchatError::InvalidPacket(
                    "Message content is mostly whitespace".into(),
                ));
            }
        }

        // Check for control characters (except common ones like \n, \r, \t)
        for c in content.chars() {
            if c.is_control() && !matches!(c, '\n' | '\r' | '\t') {
                return Err(BitchatError::InvalidPacket(
                    "Message content contains invalid control characters".into(),
                ));
            }
        }

        Ok(())
    }

    /// Enforce store capacity limits
    fn enforce_capacity_limits(
        &mut self,
        new_message: &ContentAddressedMessage,
    ) -> BitchatResult<()> {
        // Check total message limit
        if self.stats.total_messages >= self.config.max_total_messages {
            // Try to clean up old messages first
            let removed = self.cleanup_old_messages();
            if removed == 0 && self.stats.total_messages >= self.config.max_total_messages {
                return Err(BitchatError::InvalidPacket(
                    "Message store at maximum capacity".into(),
                ));
            }
        }

        // Check per-conversation limit
        let conversation_id = new_message.conversation_id();
        if let Some(messages_in_conv) = self.conversations.get(&conversation_id) {
            if messages_in_conv.len() >= self.config.max_messages_per_conversation {
                return Err(BitchatError::InvalidPacket(
                    format!(
                        "Conversation has reached maximum of {} messages",
                        self.config.max_messages_per_conversation
                    )
                    .into(),
                ));
            }
        }

        Ok(())
    }

    /// Clean up old messages based on age
    fn cleanup_old_messages(&mut self) -> usize {
        let current_time: u64 = {
            cfg_if::cfg_if! {
                if #[cfg(any(feature = "std", feature = "wasm"))] {
                    Timestamp::now().as_millis() / 1000  // Convert millis to seconds
                } else {
                    0u64  // Fallback when no time features enabled - no cleanup
                }
            }
        };
        let cutoff_time = current_time.saturating_sub(self.config.max_message_age_secs);

        let mut to_remove = Vec::new();

        for (message_id, message) in &self.messages {
            if message.timestamp < cutoff_time {
                to_remove.push(*message_id);
            }
        }

        let removed_count = to_remove.len();
        for message_id in to_remove {
            self.remove_message_completely(&message_id);
        }

        removed_count
    }

    /// Remove a message completely from all indices
    fn remove_message_completely(&mut self, message_id: &MessageId) {
        if let Some(message) = self.messages.remove(message_id) {
            // Remove from conversation index
            let conversation_id = message.conversation_id();
            if let Some(conv_messages) = self.conversations.get_mut(&conversation_id) {
                conv_messages.retain(|id| id != message_id);
                if conv_messages.is_empty() {
                    self.conversations.remove(&conversation_id);
                    self.stats.unique_conversations =
                        self.stats.unique_conversations.saturating_sub(1);
                }
            }

            // Remove from time index
            if let Some(time_messages) = self.time_index.get_mut(&message.timestamp) {
                time_messages.retain(|id| id != message_id);
                if time_messages.is_empty() {
                    self.time_index.remove(&message.timestamp);
                }
            }

            self.stats.total_messages = self.stats.total_messages.saturating_sub(1);
        }
    }

    /// Store message with automatic deduplication and comprehensive validation
    pub fn store_message(&mut self, message: ContentAddressedMessage) -> BitchatResult<bool> {
        // 1. Input validation based on configuration
        self.validate_message_input(&message)?;

        // 2. Verify message integrity
        if !message.verify_integrity() {
            self.stats.integrity_failures += 1;
            return Err(BitchatError::InvalidPacket(
                "Message integrity verification failed".into(),
            ));
        }

        // 3. Check for deduplication
        if self.messages.contains_key(&message.id) {
            self.stats.deduplication_saves += 1;
            return Ok(false); // Message already exists
        }

        // 4. Enforce store capacity limits
        self.enforce_capacity_limits(&message)?;

        let conversation_id = message.conversation_id();
        let message_id = message.id;
        let timestamp = message.timestamp;

        // Store message
        self.messages.insert(message_id, message);

        // Update conversation index
        if !self.conversations.contains_key(&conversation_id) {
            self.stats.unique_conversations += 1;
        }
        self.conversations
            .entry(conversation_id)
            .or_default()
            .push(message_id);

        // Update time index
        self.time_index
            .entry(timestamp)
            .or_default()
            .push(message_id);

        self.stats.total_messages += 1;
        Ok(true) // New message stored
    }

    /// Get message by ID
    pub fn get_message(&self, id: &MessageId) -> Option<&ContentAddressedMessage> {
        self.messages.get(id)
    }

    /// Get messages in a conversation, ordered by timestamp
    pub fn get_conversation_messages(
        &self,
        conversation_id: &ConversationId,
    ) -> Vec<&ContentAddressedMessage> {
        let message_ids = match self.conversations.get(conversation_id) {
            Some(ids) => ids,
            None => return Vec::new(),
        };

        let mut messages: Vec<_> = message_ids
            .iter()
            .filter_map(|id| self.messages.get(id))
            .collect();

        // Sort by timestamp, then by sequence
        messages.sort_by(|a, b| {
            a.timestamp
                .cmp(&b.timestamp)
                .then(a.sequence.cmp(&b.sequence))
        });

        messages
    }

    /// Get messages in time range
    pub fn get_messages_in_range(
        &self,
        start_time: u64,
        end_time: u64,
    ) -> Vec<&ContentAddressedMessage> {
        let mut messages = Vec::new();

        for (&_timestamp, message_ids) in self.time_index.range(start_time..=end_time) {
            for message_id in message_ids {
                if let Some(message) = self.messages.get(message_id) {
                    messages.push(message);
                }
            }
        }

        // Sort by timestamp, then by sequence
        messages.sort_by(|a, b| {
            a.timestamp
                .cmp(&b.timestamp)
                .then(a.sequence.cmp(&b.sequence))
        });

        messages
    }

    /// Get all conversations involving a peer
    pub fn get_peer_conversations(&self, peer_id: &PeerId) -> Vec<&ConversationId> {
        self.conversations
            .keys()
            .filter(|conv_id| conv_id.involves_peer(peer_id))
            .collect()
    }

    /// Get store statistics
    pub fn stats(&self) -> &MessageStoreStats {
        &self.stats
    }

    /// Get total number of stored messages
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// Check if message exists
    pub fn contains_message(&self, id: &MessageId) -> bool {
        self.messages.contains_key(id)
    }

    /// Get conversation message count
    pub fn conversation_message_count(&self, conversation_id: &ConversationId) -> usize {
        self.conversations
            .get(conversation_id)
            .map(|ids| ids.len())
            .unwrap_or(0)
    }
}

impl Default for MessageStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_peer_id(id: u8) -> PeerId {
        PeerId::new([id, 0, 0, 0, 0, 0, 0, 0])
    }

    #[test]
    fn test_message_id_creation() {
        let peer1 = create_test_peer_id(1);
        let peer2 = create_test_peer_id(2);

        let message =
            ContentAddressedMessage::new(peer1, Some(peer2), "Hello World".to_string(), 1);

        // Verify message integrity
        assert!(message.verify_integrity());

        // Different content should produce different ID
        let message2 =
            ContentAddressedMessage::new(peer1, Some(peer2), "Different message".to_string(), 1);

        // IDs should be different due to different content
        assert_ne!(message.id, message2.id);
    }

    #[test]
    fn test_message_store_deduplication() {
        let mut store = MessageStore::new();
        let peer1 = create_test_peer_id(1);
        let peer2 = create_test_peer_id(2);

        let message =
            ContentAddressedMessage::new(peer1, Some(peer2), "Test message".to_string(), 1);

        // First store should succeed
        let stored = store.store_message(message.clone()).unwrap();
        assert!(stored);
        assert_eq!(store.message_count(), 1);

        // Second store should detect duplicate
        let stored = store.store_message(message).unwrap();
        assert!(!stored);
        assert_eq!(store.message_count(), 1);
        assert_eq!(store.stats().deduplication_saves, 1);
    }

    #[test]
    fn test_conversation_queries() {
        let mut store = MessageStore::new();
        let peer1 = create_test_peer_id(1);
        let peer2 = create_test_peer_id(2);
        let peer3 = create_test_peer_id(3);

        // Add messages to peer1 ↔ peer2 conversation
        let msg1 = ContentAddressedMessage::new(peer1, Some(peer2), "Message 1".to_string(), 1);
        let msg2 = ContentAddressedMessage::new(peer2, Some(peer1), "Message 2".to_string(), 2);

        // Add message to peer1 ↔ peer3 conversation
        let msg3 = ContentAddressedMessage::new(peer1, Some(peer3), "Message 3".to_string(), 1);

        store.store_message(msg1).unwrap();
        store.store_message(msg2).unwrap();
        store.store_message(msg3).unwrap();

        // Test conversation queries
        let conv_id_12 = ConversationId::new(peer1, peer2);
        let messages = store.get_conversation_messages(&conv_id_12);
        assert_eq!(messages.len(), 2);

        let conv_id_13 = ConversationId::new(peer1, peer3);
        let messages = store.get_conversation_messages(&conv_id_13);
        assert_eq!(messages.len(), 1);

        // Test peer conversation queries
        let peer1_conversations = store.get_peer_conversations(&peer1);
        assert_eq!(peer1_conversations.len(), 2);
    }

    #[test]
    fn test_conversation_id_ordering() {
        let peer1 = create_test_peer_id(1);
        let peer2 = create_test_peer_id(2);

        let conv1 = ConversationId::new(peer1, peer2);
        let conv2 = ConversationId::new(peer2, peer1);

        assert_eq!(conv1, conv2); // Should be the same regardless of order
    }

    #[test]
    fn test_message_integrity() {
        let peer1 = create_test_peer_id(1);
        let peer2 = create_test_peer_id(2);

        let mut message =
            ContentAddressedMessage::new(peer1, Some(peer2), "Original content".to_string(), 1);

        assert!(message.verify_integrity());

        // Tamper with content
        message.content = "Tampered content".to_string();
        assert!(!message.verify_integrity());
    }
}
