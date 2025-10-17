//! Message delivery tracking and reliability for BitChat
//!
//! This module provides reliable message delivery with automatic retries,
//! exponential backoff, and delivery confirmation tracking.

use alloc::{collections::BTreeMap, vec::Vec};
use core::time::Duration;
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

// ----------------------------------------------------------------------------
// Delivery Configuration
// ----------------------------------------------------------------------------

/// Configuration for message delivery behavior
#[derive(Debug, Clone)]
pub struct DeliveryConfig {
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Initial retry delay
    pub initial_retry_delay: Duration,
    /// Maximum retry delay (for exponential backoff)
    pub max_retry_delay: Duration,
    /// Exponential backoff multiplier
    pub backoff_multiplier: f32,
    /// Timeout for delivery confirmation
    pub confirmation_timeout: Duration,
}

impl Default for DeliveryConfig {
    fn default() -> Self {
        Self {
            max_retries: 5,
            initial_retry_delay: Duration::from_millis(500),
            max_retry_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            confirmation_timeout: Duration::from_secs(60),
        }
    }
}

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
pub struct DeliveryTracker<T: TimeSource> {
    /// Configuration for delivery behavior
    config: DeliveryConfig,
    /// Messages currently being tracked
    tracked_messages: BTreeMap<Uuid, TrackedMessage>,
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
            tracked_messages: BTreeMap::new(),
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
        self.tracked_messages.get_mut(&message_id).unwrap()
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
    pub fn cleanup(&mut self) -> (Vec<TrackedMessage>, Vec<TrackedMessage>) {
        let mut completed = Vec::new();
        let mut expired = Vec::new();
        let mut to_remove = Vec::new();

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
                _ => {}
            }
        }

        for id in to_remove {
            self.tracked_messages.remove(&id);
        }

        (completed, expired)
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
// Tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::StdTimeSource;

    #[cfg(feature = "std")]
    #[test]
    fn test_tracked_message_lifecycle() {
        let message_id = Uuid::new_v4();
        let recipient = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        let payload = b"test message".to_vec();
        let time_source = StdTimeSource;

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

    #[cfg(feature = "std")]
    #[test]
    fn test_delivery_tracker() {
        let time_source = StdTimeSource;
        let mut tracker = DeliveryTracker::new(time_source);
        let message_id = Uuid::new_v4();
        let recipient = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        let payload = b"test message".to_vec();

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

    #[cfg(feature = "std")]
    #[test]
    fn test_exponential_backoff() {
        let config = DeliveryConfig::default();
        let message_id = Uuid::new_v4();
        let recipient = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        let payload = b"test message".to_vec();
        let time_source = StdTimeSource;

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

    #[cfg(feature = "std")]
    #[test]
    fn test_delivery_stats() {
        let time_source = StdTimeSource;
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

    #[cfg(feature = "std")]
    #[test]
    fn test_max_retry_limit() {
        let message_id = Uuid::new_v4();
        let recipient = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        let payload = b"test message".to_vec();
        let time_source = StdTimeSource;

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
}
