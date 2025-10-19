//! Message delivery tracking and reliability for BitChat runtime
//!
//! This module contains the stateful DeliveryTracker that manages message
//! delivery attempts and retries.

use std::collections::HashMap;
use uuid::Uuid;
use alloc::vec::Vec;

use bitchat_core::{
    PeerId,
    internal::{
        DeliveryStatus, DeliveryConfig, TrackedMessage, TimeSource
    }
};

// ----------------------------------------------------------------------------
// Delivery Tracker
// ----------------------------------------------------------------------------

/// Tracks message delivery status and manages retries
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
            tracked_messages: HashMap::new(),
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

    /// Mark a message as confirmed
    pub fn mark_confirmed(&mut self, message_id: &Uuid) -> bool {
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

    /// Mark a message as cancelled
    pub fn mark_cancelled(&mut self, message_id: &Uuid) -> bool {
        if let Some(tracked) = self.tracked_messages.get_mut(message_id) {
            tracked.mark_cancelled();
            true
        } else {
            false
        }
    }

    /// Get a tracked message
    pub fn get_message(&self, message_id: &Uuid) -> Option<&TrackedMessage> {
        self.tracked_messages.get(message_id)
    }

    /// Get a mutable tracked message
    pub fn get_message_mut(&mut self, message_id: &Uuid) -> Option<&mut TrackedMessage> {
        self.tracked_messages.get_mut(message_id)
    }

    /// Get all messages ready for retry
    pub fn get_retry_ready_messages(&self) -> Vec<&TrackedMessage> {
        self.tracked_messages
            .values()
            .filter(|msg| msg.can_retry())
            .collect()
    }

    /// Get all tracked messages by recipient
    pub fn get_messages_for_peer(&self, peer_id: &PeerId) -> Vec<&TrackedMessage> {
        self.tracked_messages
            .values()
            .filter(|msg| &msg.recipient == peer_id)
            .collect()
    }

    /// Clean up completed and failed messages
    pub fn cleanup_completed(&mut self) {
        self.tracked_messages.retain(|_, msg| {
            !matches!(
                msg.status,
                DeliveryStatus::Confirmed
                    | DeliveryStatus::Failed
                    | DeliveryStatus::Cancelled
            )
        });
    }

    /// Get delivery statistics
    pub fn get_statistics(&self) -> DeliveryStatistics {
        let mut stats = DeliveryStatistics::default();

        for message in self.tracked_messages.values() {
            match message.status {
                DeliveryStatus::Pending => stats.pending += 1,
                DeliveryStatus::Sent => stats.sent += 1,
                DeliveryStatus::Confirmed => stats.confirmed += 1,
                DeliveryStatus::Failed => stats.failed += 1,
                DeliveryStatus::Cancelled => stats.cancelled += 1,
            }
            stats.total_attempts += message.attempt_count() as u64;
        }

        stats.total_messages = self.tracked_messages.len() as u64;
        stats
    }

    /// Remove a tracked message
    pub fn remove_message(&mut self, message_id: &Uuid) -> Option<TrackedMessage> {
        self.tracked_messages.remove(message_id)
    }

    /// Get the number of tracked messages
    pub fn tracked_count(&self) -> usize {
        self.tracked_messages.len()
    }
}

// ----------------------------------------------------------------------------
// Delivery Statistics
// ----------------------------------------------------------------------------

/// Statistics about message delivery
#[derive(Debug, Default, Clone)]
pub struct DeliveryStatistics {
    /// Total number of messages
    pub total_messages: u64,
    /// Number of pending messages
    pub pending: u64,
    /// Number of sent messages (awaiting confirmation)
    pub sent: u64,
    /// Number of confirmed messages
    pub confirmed: u64,
    /// Number of failed messages
    pub failed: u64,
    /// Number of cancelled messages
    pub cancelled: u64,
    /// Total delivery attempts across all messages
    pub total_attempts: u64,
}

impl DeliveryStatistics {
    /// Calculate success rate (confirmed / total)
    pub fn success_rate(&self) -> f64 {
        if self.total_messages == 0 {
            0.0
        } else {
            self.confirmed as f64 / self.total_messages as f64
        }
    }

    /// Calculate failure rate (failed / total)
    pub fn failure_rate(&self) -> f64 {
        if self.total_messages == 0 {
            0.0
        } else {
            self.failed as f64 / self.total_messages as f64
        }
    }

    /// Calculate average attempts per message
    pub fn average_attempts(&self) -> f64 {
        if self.total_messages == 0 {
            0.0
        } else {
            self.total_attempts as f64 / self.total_messages as f64
        }
    }
}

// ----------------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use bitchat_core::SystemTimeSource;

    #[cfg(feature = "std")]
    #[test]
    fn test_delivery_tracker() {
        let time_source = SystemTimeSource;
        let mut tracker = DeliveryTracker::new(time_source);

        let message_id = Uuid::new_v4();
        let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        let payload = b"Hello, world!".to_vec();

        // Track a message
        let tracked = tracker.track_message(message_id, peer_id, payload);
        assert_eq!(tracked.status, DeliveryStatus::Pending);

        // Mark as sent
        assert!(tracker.mark_sent(&message_id));
        let tracked = tracker.get_message(&message_id).unwrap();
        assert_eq!(tracked.status, DeliveryStatus::Sent);

        // Mark as confirmed
        assert!(tracker.mark_confirmed(&message_id));
        let tracked = tracker.get_message(&message_id).unwrap();
        assert_eq!(tracked.status, DeliveryStatus::Confirmed);

        // Get statistics
        let stats = tracker.get_statistics();
        assert_eq!(stats.total_messages, 1);
        assert_eq!(stats.confirmed, 1);
        assert_eq!(stats.success_rate(), 1.0);
    }
}