//! Message delivery tracking and reliability for BitChat
//!
//! This module provides reliable message delivery with automatic retries,
//! exponential backoff, and delivery confirmation tracking.

use alloc::vec::Vec;
use core::time::Duration;
use hashbrown::HashMap;
use uuid::Uuid;

use crate::types::{PeerId, TimeSource, Timestamp};

// ----------------------------------------------------------------------------
// Delivery Status
// ----------------------------------------------------------------------------

/// Status of a message delivery attempt
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryStatus {
    /// Message is pending delivery
    Pending,
    /// Message was sent, awaiting acknowledgment
    Sent,
    /// Delivery confirmed by recipient
    Confirmed,
    /// Delivery failed after all retries
    Failed,
    /// Delivery was cancelled
    Cancelled,
}

// Import delivery configuration from the centralized config module
pub use crate::config::DeliveryConfig;

// Import acknowledgment types for integration
use crate::protocol::acknowledgments::{DeliveryAck, ReadReceipt, ReceiptManager, EnhancedDeliveryStatus};
use crate::protocol::message_store::MessageId;

// ----------------------------------------------------------------------------
// Delivery Attempt
// ----------------------------------------------------------------------------

/// Information about a single delivery attempt
#[derive(Debug, Clone)]
pub struct DeliveryAttempt {
    /// Attempt number (1-based)
    pub attempt_number: u32,
    /// Timestamp of the attempt
    pub timestamp: Timestamp,
    /// Delay until next retry (if applicable)
    pub next_retry_delay: Duration,
}

impl DeliveryAttempt {
    /// Create a new delivery attempt
    pub fn new<T: TimeSource>(
        attempt_number: u32,
        next_retry_delay: Duration,
        time_source: &T,
    ) -> Self {
        Self {
            attempt_number,
            timestamp: time_source.now(),
            next_retry_delay,
        }
    }
}

// ----------------------------------------------------------------------------
// Tracked Message
// ----------------------------------------------------------------------------

/// A message being tracked for delivery
#[derive(Debug, Clone)]
pub struct TrackedMessage {
    /// Message ID
    pub message_id: Uuid,
    /// Recipient peer ID
    pub recipient: PeerId,
    /// Current delivery status
    pub status: DeliveryStatus,
    /// Message payload (for retries)
    pub payload: Vec<u8>,
    /// Delivery attempts made
    pub attempts: Vec<DeliveryAttempt>,
    /// Timestamp when tracking started
    pub created_at: Timestamp,
    /// Timestamp when delivery was confirmed (if applicable)
    pub confirmed_at: Option<Timestamp>,
    /// Maximum number of retries allowed
    pub max_retries: u32,
}

impl TrackedMessage {
    /// Create a new tracked message
    pub fn new<T: TimeSource>(
        message_id: Uuid,
        recipient: PeerId,
        payload: Vec<u8>,
        max_retries: u32,
        time_source: &T,
    ) -> Self {
        Self {
            message_id,
            recipient,
            status: DeliveryStatus::Pending,
            payload,
            attempts: Vec::new(),
            created_at: time_source.now(),
            confirmed_at: None,
            max_retries,
        }
    }

    /// Get the number of delivery attempts made
    pub fn attempt_count(&self) -> u32 {
        self.attempts.len() as u32
    }

    /// Check if more retries are allowed
    pub fn can_retry(&self) -> bool {
        self.attempt_count() < self.max_retries
            && matches!(self.status, DeliveryStatus::Pending | DeliveryStatus::Sent)
    }

    /// Mark message as sent
    pub fn mark_sent<T: TimeSource>(&mut self, next_retry_delay: Duration, time_source: &T) {
        let attempt = DeliveryAttempt::new(self.attempt_count() + 1, next_retry_delay, time_source);
        self.attempts.push(attempt);
        self.status = DeliveryStatus::Sent;
    }

    /// Mark message as confirmed
    pub fn mark_confirmed<T: TimeSource>(&mut self, time_source: &T) {
        self.status = DeliveryStatus::Confirmed;
        self.confirmed_at = Some(time_source.now());
    }

    /// Mark message as failed
    pub fn mark_failed(&mut self) {
        self.status = DeliveryStatus::Failed;
    }

    /// Mark message as cancelled
    pub fn mark_cancelled(&mut self) {
        self.status = DeliveryStatus::Cancelled;
    }

    /// Get the next retry delay for this message
    pub fn next_retry_delay(&self, config: &DeliveryConfig) -> Duration {
        let base_delay = config.initial_retry_delay.as_millis() as f32;
        let exponent = self.attempt_count() as i32; // 0 for first attempt, 1 for second, etc.
        let multiplier = config.backoff_multiplier.powi(exponent);
        let delay_ms = (base_delay * multiplier) as u64;
        let delay = Duration::from_millis(delay_ms);

        if delay > config.max_retry_delay {
            config.max_retry_delay
        } else {
            delay
        }
    }

    /// Check if this message has timed out
    pub fn is_timed_out<T: TimeSource>(&self, config: &DeliveryConfig, time_source: &T) -> bool {
        let now = time_source.now();
        let elapsed =
            Duration::from_millis(now.as_millis().saturating_sub(self.created_at.as_millis()));
        elapsed > config.confirmation_timeout
    }

    /// Check if this message is ready for retry
    pub fn is_ready_for_retry<T: TimeSource>(&self, time_source: &T) -> bool {
        if !self.can_retry() || self.attempts.is_empty() {
            return false;
        }

        let last_attempt = &self.attempts[self.attempts.len() - 1];
        let now = time_source.now();
        let elapsed = Duration::from_millis(
            now.as_millis()
                .saturating_sub(last_attempt.timestamp.as_millis()),
        );

        elapsed >= last_attempt.next_retry_delay
    }
}

// ----------------------------------------------------------------------------
// Delivery Tracker
// ----------------------------------------------------------------------------

/// Tracks message delivery and handles retries
#[derive(Debug)]
pub struct DeliveryTracker<T: TimeSource> {
    /// Configuration for delivery behavior
    config: DeliveryConfig,
    /// Messages currently being tracked
    tracked_messages: HashMap<Uuid, TrackedMessage>,
    /// Time source for generating timestamps
    time_source: T,
}

impl<T: TimeSource> DeliveryTracker<T> {
    /// Create a new delivery tracker with default configuration
    pub fn new(time_source: T) -> Self {
        Self::with_config(DeliveryConfig::default(), time_source)
    }

    /// Create a new delivery tracker with custom configuration
    pub fn with_config(config: DeliveryConfig, time_source: T) -> Self {
        Self {
            config,
            tracked_messages: HashMap::default(),
            time_source,
        }
    }

    /// Start tracking a message for delivery
    pub fn track_message(
        &mut self,
        message_id: Uuid,
        recipient: PeerId,
        payload: Vec<u8>,
    ) -> &mut TrackedMessage {
        let tracked = TrackedMessage::new(
            message_id,
            recipient,
            payload,
            self.config.max_retries,
            &self.time_source,
        );

        self.tracked_messages.insert(message_id, tracked);
        self.tracked_messages
            .get_mut(&message_id)
            .expect("Message must exist as it was just inserted")
    }

    /// Mark a message as sent
    pub fn mark_sent(&mut self, message_id: &Uuid) -> bool {
        if let Some(tracked) = self.tracked_messages.get_mut(message_id) {
            let next_delay = tracked.next_retry_delay(&self.config);
            tracked.mark_sent(next_delay, &self.time_source);
            true
        } else {
            false
        }
    }

    /// Confirm delivery of a message
    pub fn confirm_delivery(&mut self, message_id: &Uuid) -> bool {
        if let Some(tracked) = self.tracked_messages.get_mut(message_id) {
            tracked.mark_confirmed(&self.time_source);
            true
        } else {
            false
        }
    }

    /// Mark a message as failed
    pub fn mark_failed(&mut self, message_id: &Uuid) -> bool {
        if let Some(tracked) = self.tracked_messages.get_mut(message_id) {
            tracked.mark_failed();
            true
        } else {
            false
        }
    }

    /// Cancel tracking of a message
    pub fn cancel_tracking(&mut self, message_id: &Uuid) -> Option<TrackedMessage> {
        if let Some(mut tracked) = self.tracked_messages.remove(message_id) {
            tracked.mark_cancelled();
            Some(tracked)
        } else {
            None
        }
    }

    /// Get a tracked message
    pub fn get_tracked(&self, message_id: &Uuid) -> Option<&TrackedMessage> {
        self.tracked_messages.get(message_id)
    }

    /// Get a mutable reference to a tracked message
    pub fn get_tracked_mut(&mut self, message_id: &Uuid) -> Option<&mut TrackedMessage> {
        self.tracked_messages.get_mut(message_id)
    }

    /// Get all messages ready for retry
    pub fn get_ready_for_retry(&self) -> Vec<&TrackedMessage> {
        self.tracked_messages
            .values()
            .filter(|tracked| tracked.is_ready_for_retry(&self.time_source))
            .collect()
    }

    /// Get all timed out messages
    pub fn get_timed_out(&self) -> Vec<&TrackedMessage> {
        self.tracked_messages
            .values()
            .filter(|tracked| tracked.is_timed_out(&self.config, &self.time_source))
            .collect()
    }

    /// Clean up completed and expired messages
    /// Also marks messages as failed if they exceed max retries
    pub fn cleanup(&mut self) -> (Vec<TrackedMessage>, Vec<TrackedMessage>) {
        let mut completed = Vec::new();
        let mut expired = Vec::new();
        let mut to_remove = Vec::new();
        let mut to_mark_failed = Vec::new();

        for (id, tracked) in &self.tracked_messages {
            match tracked.status {
                DeliveryStatus::Confirmed | DeliveryStatus::Failed | DeliveryStatus::Cancelled => {
                    completed.push(tracked.clone());
                    to_remove.push(*id);
                }
                _ if tracked.is_timed_out(&self.config, &self.time_source) => {
                    expired.push(tracked.clone());
                    to_remove.push(*id);
                }
                // Check for messages that have exhausted retry attempts
                DeliveryStatus::Pending | DeliveryStatus::Sent if !tracked.can_retry() => {
                    to_mark_failed.push(*id);
                }
                _ => {}
            }
        }

        // Mark exhausted messages as failed and add to completed list
        for id in to_mark_failed {
            if let Some(tracked) = self.tracked_messages.get_mut(&id) {
                tracked.mark_failed();
                completed.push(tracked.clone());
                to_remove.push(id);
            }
        }

        // Remove all completed, expired, and failed messages
        for id in to_remove {
            self.tracked_messages.remove(&id);
        }

        (completed, expired)
    }

    /// Aggressive cleanup for long-running applications
    /// Removes all messages older than the specified age, regardless of status
    /// This prevents unbounded memory growth in applications that run for extended periods
    pub fn cleanup_by_age(&mut self, max_age: core::time::Duration) -> usize {
        let now = self.time_source.now();
        let mut to_remove = Vec::new();

        for (id, tracked) in &self.tracked_messages {
            let age = core::time::Duration::from_millis(
                now.as_millis()
                    .saturating_sub(tracked.created_at.as_millis()),
            );

            if age > max_age {
                to_remove.push(*id);
            }
        }

        let removed_count = to_remove.len();
        for id in to_remove {
            self.tracked_messages.remove(&id);
        }

        removed_count
    }

    /// Force cleanup to maintain maximum number of tracked messages
    /// Removes oldest messages first when the limit is exceeded
    pub fn cleanup_by_count(&mut self, max_count: usize) -> usize {
        if self.tracked_messages.len() <= max_count {
            return 0;
        }

        // Sort by creation time to remove oldest first
        let mut messages_by_age: Vec<_> = self
            .tracked_messages
            .iter()
            .map(|(id, tracked)| (*id, tracked.created_at))
            .collect();

        messages_by_age.sort_by_key(|(_, created_at)| *created_at);

        let to_remove_count = self.tracked_messages.len() - max_count;
        let mut removed_count = 0;

        for (id, _) in messages_by_age.into_iter().take(to_remove_count) {
            self.tracked_messages.remove(&id);
            removed_count += 1;
        }

        removed_count
    }

    /// Get delivery statistics
    pub fn get_stats(&self) -> DeliveryStats {
        let mut stats = DeliveryStats::default();

        for tracked in self.tracked_messages.values() {
            stats.total += 1;

            match tracked.status {
                DeliveryStatus::Pending => stats.pending += 1,
                DeliveryStatus::Sent => stats.sent += 1,
                DeliveryStatus::Confirmed => stats.confirmed += 1,
                DeliveryStatus::Failed => stats.failed += 1,
                DeliveryStatus::Cancelled => stats.cancelled += 1,
            }

            stats.total_attempts += tracked.attempt_count();
        }

        stats
    }

    /// Get current configuration
    pub fn config(&self) -> &DeliveryConfig {
        &self.config
    }

    /// Update configuration
    pub fn set_config(&mut self, config: DeliveryConfig) {
        self.config = config;
    }
}

// Note: No Default implementation since we need a TimeSource parameter
// Users should use DeliveryTracker::new(time_source) instead

// ----------------------------------------------------------------------------
// Delivery Statistics
// ----------------------------------------------------------------------------

/// Statistics about message delivery
#[derive(Debug, Default, Clone)]
pub struct DeliveryStats {
    /// Total number of tracked messages
    pub total: u32,
    /// Messages pending delivery
    pub pending: u32,
    /// Messages sent (awaiting confirmation)
    pub sent: u32,
    /// Messages confirmed delivered
    pub confirmed: u32,
    /// Messages that failed delivery
    pub failed: u32,
    /// Messages that were cancelled
    pub cancelled: u32,
    /// Total delivery attempts across all messages
    pub total_attempts: u32,
}

impl DeliveryStats {
    /// Calculate delivery success rate (0.0 to 1.0)
    pub fn success_rate(&self) -> f32 {
        let completed = self.confirmed + self.failed + self.cancelled;
        if completed == 0 {
            0.0
        } else {
            self.confirmed as f32 / completed as f32
        }
    }

    /// Calculate average attempts per message
    pub fn average_attempts(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            self.total_attempts as f32 / self.total as f32
        }
    }
}

// ----------------------------------------------------------------------------
// Enhanced Delivery Tracker with Receipt Integration
// ----------------------------------------------------------------------------

/// Enhanced delivery tracker that integrates read receipts and delivery acknowledgments
/// 
/// This extends the basic DeliveryTracker with canonical receipt management,
/// providing comprehensive message delivery and read status tracking.
pub struct EnhancedDeliveryTracker<T: TimeSource> {
    /// Core delivery tracking
    delivery_tracker: DeliveryTracker<T>,
    /// Receipt management
    receipt_manager: ReceiptManager,
    /// Enhanced delivery statuses keyed by message UUID
    enhanced_statuses: HashMap<Uuid, EnhancedDeliveryStatus>,
    /// Content-addressed message ID mapping (UUID -> MessageId for receipts)
    message_id_mapping: HashMap<Uuid, MessageId>,
}

impl<T: TimeSource> EnhancedDeliveryTracker<T> {
    /// Create a new enhanced delivery tracker
    pub fn new(time_source: T) -> Self {
        Self {
            delivery_tracker: DeliveryTracker::new(time_source),
            receipt_manager: ReceiptManager::new(),
            enhanced_statuses: HashMap::new(),
            message_id_mapping: HashMap::new(),
        }
    }

    /// Create with custom configuration
    pub fn with_config(config: DeliveryConfig, time_source: T) -> Self {
        Self {
            delivery_tracker: DeliveryTracker::with_config(config, time_source),
            receipt_manager: ReceiptManager::new(),
            enhanced_statuses: HashMap::new(),
            message_id_mapping: HashMap::new(),
        }
    }

    /// Track a message with both UUID and content-addressed MessageId
    pub fn track_message_with_id(
        &mut self,
        message_uuid: Uuid,
        message_id: MessageId,
        recipient: PeerId,
        payload: Vec<u8>,
    ) -> &mut TrackedMessage {
        // Track in the core delivery tracker
        let tracked = self.delivery_tracker.track_message(message_uuid, recipient, payload);
        
        // Store the mapping and initial status
        self.message_id_mapping.insert(message_uuid, message_id);
        self.enhanced_statuses.insert(message_uuid, EnhancedDeliveryStatus::Sending);
        
        tracked
    }

    /// Process a received delivery acknowledgment
    pub fn process_delivery_ack(&mut self, ack: &DeliveryAck) -> bool {
        // Find the UUID corresponding to this MessageId
        let message_uuid = self.message_id_mapping
            .iter()
            .find(|(_, &id)| id == ack.message_id)
            .map(|(&uuid, _)| uuid);

        if let Some(uuid) = message_uuid {
            // Update core delivery tracker
            let success = self.delivery_tracker.confirm_delivery(&uuid);
            
            if success {
                // Update enhanced status
                let enhanced_status = EnhancedDeliveryStatus::from_delivery_ack(ack);
                self.enhanced_statuses.insert(uuid, enhanced_status);
            }
            
            success
        } else {
            false
        }
    }

    /// Process a received read receipt
    pub fn process_read_receipt(&mut self, receipt: &ReadReceipt) -> bool {
        // Find the UUID corresponding to this MessageId
        let message_uuid = self.message_id_mapping
            .iter()
            .find(|(_, &id)| id == receipt.message_id)
            .map(|(&uuid, _)| uuid);

        if let Some(uuid) = message_uuid {
            // Update enhanced status (read receipt supersedes delivery ack)
            let enhanced_status = EnhancedDeliveryStatus::from_read_receipt(receipt);
            self.enhanced_statuses.insert(uuid, enhanced_status);
            true
        } else {
            false
        }
    }

    /// Check if a delivery acknowledgment should be sent for a received message
    pub fn should_send_delivery_ack(&self, message_id: &MessageId) -> bool {
        self.receipt_manager.should_send_delivery_ack(message_id)
    }

    /// Check if a read receipt should be sent for a received message
    pub fn should_send_read_receipt(&self, message_id: &MessageId) -> bool {
        self.receipt_manager.should_send_read_receipt(message_id)
    }

    /// Mark that a delivery acknowledgment has been sent
    pub fn mark_delivery_ack_sent(&mut self, message_id: MessageId) {
        self.receipt_manager.mark_delivery_ack_sent(message_id);
    }

    /// Mark that a read receipt has been sent
    pub fn mark_read_receipt_sent(&mut self, message_id: MessageId) {
        self.receipt_manager.mark_read_receipt_sent(message_id);
    }

    /// Get the enhanced delivery status for a message
    pub fn get_enhanced_status(&self, message_uuid: &Uuid) -> Option<&EnhancedDeliveryStatus> {
        self.enhanced_statuses.get(message_uuid)
    }

    /// Get the content-addressed MessageId for a tracked message
    pub fn get_message_id(&self, message_uuid: &Uuid) -> Option<&MessageId> {
        self.message_id_mapping.get(message_uuid)
    }

    /// Configure receipt privacy settings
    pub fn configure_receipts(&mut self, send_delivery_acks: bool, send_read_receipts: bool) {
        self.receipt_manager.set_delivery_acks_enabled(send_delivery_acks);
        self.receipt_manager.set_read_receipts_enabled(send_read_receipts);
    }

    /// Get receipt manager for direct access
    pub fn receipt_manager(&self) -> &ReceiptManager {
        &self.receipt_manager
    }

    /// Get mutable receipt manager for direct access
    pub fn receipt_manager_mut(&mut self) -> &mut ReceiptManager {
        &mut self.receipt_manager
    }

    /// Get core delivery tracker for direct access
    pub fn delivery_tracker(&self) -> &DeliveryTracker<T> {
        &self.delivery_tracker
    }

    /// Get mutable core delivery tracker for direct access  
    pub fn delivery_tracker_mut(&mut self) -> &mut DeliveryTracker<T> {
        &mut self.delivery_tracker
    }

    /// Clean up old data to prevent memory growth
    pub fn cleanup(&mut self) -> (Vec<TrackedMessage>, Vec<TrackedMessage>) {
        // Clean up core delivery tracker
        let (completed, expired) = self.delivery_tracker.cleanup();
        
        // Clean up enhanced statuses for completed/expired messages
        for message in &completed {
            self.enhanced_statuses.remove(&message.message_id);
            self.message_id_mapping.remove(&message.message_id);
        }
        
        for message in &expired {
            self.enhanced_statuses.remove(&message.message_id);
            self.message_id_mapping.remove(&message.message_id);
        }
        
        // Clean up receipt manager
        self.receipt_manager.cleanup_old_receipts(1000); // Keep last 1000 receipts
        
        (completed, expired)
    }

    /// Get comprehensive delivery statistics
    pub fn get_enhanced_stats(&self) -> EnhancedDeliveryStats {
        let delivery_stats = self.delivery_tracker.get_stats();
        let receipt_stats = self.receipt_manager.get_stats();
        
        // Count enhanced statuses
        let mut delivered_count = 0;
        let mut read_count = 0;
        
        for status in self.enhanced_statuses.values() {
            if status.is_delivered() {
                delivered_count += 1;
            }
            if status.is_read() {
                read_count += 1;
            }
        }
        
        EnhancedDeliveryStats {
            basic_stats: delivery_stats,
            receipt_stats,
            delivered_count,
            read_count,
            tracked_messages_count: self.enhanced_statuses.len(),
        }
    }
}

/// Enhanced delivery statistics combining core delivery and receipt data
#[derive(Debug, Clone)]
pub struct EnhancedDeliveryStats {
    /// Basic delivery statistics
    pub basic_stats: DeliveryStats,
    /// Receipt manager statistics
    pub receipt_stats: crate::protocol::acknowledgments::ReceiptStats,
    /// Number of messages confirmed delivered
    pub delivered_count: usize,
    /// Number of messages confirmed read
    pub read_count: usize,
    /// Total number of tracked messages with enhanced status
    pub tracked_messages_count: usize,
}

impl EnhancedDeliveryStats {
    /// Calculate read rate (read messages / delivered messages)
    pub fn read_rate(&self) -> f32 {
        if self.delivered_count == 0 {
            0.0
        } else {
            self.read_count as f32 / self.delivered_count as f32
        }
    }

    /// Calculate delivery confirmation rate (delivered / sent)
    pub fn delivery_confirmation_rate(&self) -> f32 {
        if self.basic_stats.sent == 0 {
            0.0
        } else {
            self.delivered_count as f32 / self.basic_stats.sent as f32
        }
    }
}

// ----------------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SystemTimeSource;

    #[cfg(any(feature = "std", target_arch = "wasm32"))]
    #[test]
    fn test_tracked_message_lifecycle() {
        let message_id = Uuid::new_v4();
        let recipient = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        let payload = b"test_message".to_vec();
        let time_source = SystemTimeSource;

        let mut tracked = TrackedMessage::new(message_id, recipient, payload, 3, &time_source);

        assert_eq!(tracked.status, DeliveryStatus::Pending);
        assert_eq!(tracked.attempt_count(), 0);
        assert!(tracked.can_retry());

        // Mark as sent
        tracked.mark_sent(Duration::from_secs(1), &time_source);
        assert_eq!(tracked.status, DeliveryStatus::Sent);
        assert_eq!(tracked.attempt_count(), 1);

        // Mark as confirmed
        tracked.mark_confirmed(&time_source);
        assert_eq!(tracked.status, DeliveryStatus::Confirmed);
        assert!(!tracked.can_retry());
    }

    #[cfg(any(feature = "std", target_arch = "wasm32"))]
    #[test]
    fn test_delivery_tracker() {
        let time_source = SystemTimeSource;
        let mut tracker = DeliveryTracker::new(time_source);
        let message_id = Uuid::new_v4();
        let recipient = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        let payload = b"test_message".to_vec();

        // Track message
        tracker.track_message(message_id, recipient, payload);

        let tracked = tracker.get_tracked(&message_id).unwrap();
        assert_eq!(tracked.status, DeliveryStatus::Pending);

        // Mark as sent
        assert!(tracker.mark_sent(&message_id));
        let tracked = tracker.get_tracked(&message_id).unwrap();
        assert_eq!(tracked.status, DeliveryStatus::Sent);

        // Confirm delivery
        assert!(tracker.confirm_delivery(&message_id));
        let tracked = tracker.get_tracked(&message_id).unwrap();
        assert_eq!(tracked.status, DeliveryStatus::Confirmed);
    }

    #[cfg(any(feature = "std", target_arch = "wasm32"))]
    #[test]
    fn test_exponential_backoff() {
        let config = DeliveryConfig::default();
        let message_id = Uuid::new_v4();
        let recipient = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        let payload = b"test_message".to_vec();
        let time_source = SystemTimeSource;

        let mut tracked = TrackedMessage::new(message_id, recipient, payload, 5, &time_source);

        // First retry should use initial delay
        let delay1 = tracked.next_retry_delay(&config);
        assert_eq!(delay1, config.initial_retry_delay);

        // Simulate failed attempt
        tracked.mark_sent(delay1, &time_source);

        // Second retry should be longer
        let delay2 = tracked.next_retry_delay(&config);
        assert!(delay2 > delay1);

        // Verify exponential growth
        let expected = Duration::from_millis(
            (config.initial_retry_delay.as_millis() as f32 * config.backoff_multiplier) as u64,
        );
        assert_eq!(delay2, expected);
    }

    #[cfg(any(feature = "std", target_arch = "wasm32"))]
    #[test]
    fn test_delivery_stats() {
        let time_source = SystemTimeSource;
        let mut tracker = DeliveryTracker::new(time_source);

        // Add some test messages
        for i in 0..5 {
            let message_id = Uuid::new_v4();
            let recipient = PeerId::new([i, 0, 0, 0, 0, 0, 0, 0]);
            tracker.track_message(message_id, recipient, vec![i]);
        }

        let stats = tracker.get_stats();
        assert_eq!(stats.total, 5);
        assert_eq!(stats.pending, 5);
        assert_eq!(stats.confirmed, 0);

        // Confirm some deliveries
        let message_ids: Vec<Uuid> = tracker.tracked_messages.keys().cloned().collect();
        tracker.confirm_delivery(&message_ids[0]);
        tracker.confirm_delivery(&message_ids[1]);
        tracker.mark_failed(&message_ids[2]);

        let stats = tracker.get_stats();
        assert_eq!(stats.confirmed, 2);
        assert_eq!(stats.failed, 1);
        assert_eq!(stats.pending, 2);
        assert_eq!(stats.success_rate(), 2.0 / 3.0); // 2 confirmed out of 3 completed
    }

    #[cfg(any(feature = "std", target_arch = "wasm32"))]
    #[test]
    fn test_max_retry_limit() {
        let message_id = Uuid::new_v4();
        let recipient = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        let payload = b"test_message".to_vec();
        let time_source = SystemTimeSource;

        let mut tracked = TrackedMessage::new(message_id, recipient, payload, 2, &time_source);

        // Should allow retries initially
        assert!(tracked.can_retry());

        // First attempt
        tracked.mark_sent(Duration::from_secs(1), &time_source);
        assert!(tracked.can_retry());

        // Second attempt (reaches max)
        tracked.mark_sent(Duration::from_secs(1), &time_source);
        assert!(!tracked.can_retry());
    }

    #[cfg(any(feature = "std", target_arch = "wasm32"))]
    #[test]
    fn test_enhanced_delivery_tracker_integration() {
        use crate::protocol::acknowledgments::{DeliveryAck, ReadReceipt};
        use crate::protocol::message_store::MessageId;
        use sha2::{Digest, Sha256};

        let time_source = SystemTimeSource;
        let mut tracker = EnhancedDeliveryTracker::new(time_source);

        // Create test IDs
        let message_uuid = Uuid::new_v4();
        let hash = Sha256::digest(b"test message");
        let message_id = MessageId::from_bytes(hash.into());
        let recipient = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        let payload = b"test message".to_vec();

        // Track a message
        tracker.track_message_with_id(message_uuid, message_id, recipient, payload);

        // Verify initial status
        let status = tracker.get_enhanced_status(&message_uuid).unwrap();
        assert!(matches!(status, EnhancedDeliveryStatus::Sending));

        // Create and process delivery acknowledgment
        let delivery_ack = DeliveryAck::new(message_id, recipient, Some("Alice".to_string()));
        assert!(tracker.process_delivery_ack(&delivery_ack));

        // Verify delivered status
        let status = tracker.get_enhanced_status(&message_uuid).unwrap();
        assert!(status.is_delivered());
        assert!(!status.is_read());

        // Create and process read receipt
        let read_receipt = ReadReceipt::new(message_id, recipient, recipient, Some("Alice".to_string()));
        assert!(tracker.process_read_receipt(&read_receipt));

        // Verify read status
        let status = tracker.get_enhanced_status(&message_uuid).unwrap();
        assert!(status.is_delivered());
        assert!(status.is_read());

        // Check statistics
        let stats = tracker.get_enhanced_stats();
        assert_eq!(stats.delivered_count, 1);
        assert_eq!(stats.read_count, 1);
        assert_eq!(stats.read_rate(), 1.0);
    }

    #[cfg(any(feature = "std", target_arch = "wasm32"))]
    #[test]
    fn test_receipt_deduplication() {
        use crate::protocol::message_store::MessageId;
        use sha2::{Digest, Sha256};

        let time_source = SystemTimeSource;
        let tracker = EnhancedDeliveryTracker::new(time_source);

        let hash = Sha256::digest(b"test message");
        let message_id = MessageId::from_bytes(hash.into());

        // Should allow first receipt
        assert!(tracker.should_send_delivery_ack(&message_id));
        assert!(tracker.should_send_read_receipt(&message_id));

        // Configure privacy settings
        let mut tracker = tracker;
        tracker.configure_receipts(false, true); // Disable delivery acks, enable read receipts

        assert!(!tracker.should_send_delivery_ack(&message_id));
        assert!(tracker.should_send_read_receipt(&message_id));
    }
}
