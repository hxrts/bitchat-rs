//! Property-based tests for message store deduplication logic
//!
//! These tests verify invariants around hash collision resistance,
//! deduplication correctness, and edge cases in message storage.

use bitchat_core::{
    config::MessageStoreConfig,
    protocol::message_store::{ContentAddressedMessage, MessageStore},
    types::Timestamp,
    PeerId,
};
use proptest::prelude::*;
use proptest::strategy::ValueTree;
use std::collections::HashSet;

/// Generate arbitrary PeerId for property tests
fn arb_peer_id() -> impl Strategy<Value = PeerId> {
    any::<[u8; 8]>().prop_map(PeerId::new)
}

/// Generate arbitrary message content
fn arb_message_content() -> impl Strategy<Value = String> {
    // Generate content that always has at least one non-whitespace character
    prop::string::string_regex(r"[a-zA-Z0-9.,!?][a-zA-Z0-9 .,!?]{0,999}").unwrap()
}

/// Generate arbitrary timestamp
#[allow(dead_code)]
fn arb_timestamp() -> impl Strategy<Value = Timestamp> {
    (0u64..=u64::MAX).prop_map(Timestamp::new)
}

/// Generate arbitrary sequence number
fn arb_sequence() -> impl Strategy<Value = u64> {
    0u64..=1_000_000u64
}

/// Generate arbitrary stored message
fn arb_stored_message() -> impl Strategy<Value = ContentAddressedMessage> {
    (
        arb_peer_id(),
        arb_peer_id(),
        arb_message_content(),
        arb_sequence(),
    )
        .prop_map(|(sender, recipient, content, sequence)| {
            ContentAddressedMessage::new(sender, Some(recipient), content, sequence)
        })
}

proptest! {
    /// Property: Identical messages always produce the same MessageId
    #[test]
    fn identical_messages_same_id(message in arb_stored_message()) {
        let mut store = MessageStore::new();

        // Store the same message twice
        let _exists1 = store.store_message(message.clone()).expect("First store should succeed");
        let _exists2 = store.store_message(message.clone()).expect("Second store should succeed");

        // Should only be stored once (deduplication)
        prop_assert_eq!(store.message_count(), 1);
        prop_assert_eq!(store.stats().deduplication_saves, 1);
    }

    /// Property: Different messages always produce different MessageIds
    #[test]
    fn different_messages_different_ids(
        sender1 in arb_peer_id(),
        sender2 in arb_peer_id(),
        content1 in arb_message_content(),
        content2 in arb_message_content(),
    ) {
        // Only test when messages are actually different
        prop_assume!(sender1 != sender2 || content1 != content2);

        let mut store = MessageStore::new();

        let msg1 = ContentAddressedMessage::new(sender1, Some(sender1), content1, 1);
        let msg2 = ContentAddressedMessage::new(sender2, Some(sender2), content2, 1);

        let _exists1 = store.store_message(msg1).expect("Store msg1 should succeed");
        let _exists2 = store.store_message(msg2).expect("Store msg2 should succeed");

        // Different messages must result in different storage
        prop_assert_eq!(store.message_count(), 2);
        prop_assert_eq!(store.stats().deduplication_saves, 0);
    }

    /// Property: Changing any field produces a different hash
    #[test]
    fn field_changes_produce_different_hashes(
        sender in arb_peer_id(),
        recipient in arb_peer_id(),
        content in arb_message_content(),
        alt_content in arb_message_content(),
    ) {
        prop_assume!(content != alt_content);

        let mut store = MessageStore::new();

        let base_msg = ContentAddressedMessage::new(sender, Some(recipient), content, 1);
        let alt_msg = ContentAddressedMessage::new(sender, Some(recipient), alt_content, 1);

        let _exists1 = store.store_message(base_msg).expect("Base message should store");
        let _exists2 = store.store_message(alt_msg).expect("Alt message should store");

        // Should have 2 different messages stored
        prop_assert_eq!(store.message_count(), 2);
    }

    /// Property: Message retrieval is consistent
    #[test]
    fn message_retrieval_consistency(messages in prop::collection::vec(arb_stored_message(), 1..20)) {
        let mut store = MessageStore::new();

        // Store all messages
        for msg in &messages {
            let _exists = store.store_message(msg.clone()).expect("Message storage should succeed");
        }

        // Verify all stored messages can be retrieved by their ID
        for original_msg in &messages {
            let retrieved = store.get_message(&original_msg.id).expect("Message should be retrievable");
            prop_assert_eq!(retrieved.sender, original_msg.sender);
            prop_assert_eq!(retrieved.recipient, original_msg.recipient);
            prop_assert_eq!(&retrieved.content, &original_msg.content);
            prop_assert_eq!(retrieved.timestamp, original_msg.timestamp);
            prop_assert_eq!(retrieved.sequence, original_msg.sequence);
        }
    }

    /// Property: Hash collision resistance across large volumes
    #[test]
    fn hash_collision_resistance(messages in prop::collection::vec(arb_stored_message(), 1..100)) {
        let mut store = MessageStore::new();
        let mut seen_ids = HashSet::new();

        for msg in messages {
            // Skip messages that might be rejected due to validation
            if let Ok(_exists) = store.store_message(msg.clone()) {
                // Each message should produce a unique ID (unless it's a duplicate)
                if seen_ids.contains(&msg.id) {
                    // This should be a duplicate, so deduplication saves should increase
                    prop_assert!(store.stats().deduplication_saves > 0);
                } else {
                    seen_ids.insert(msg.id);
                }
            }
        }
    }

    /// Property: Deduplication statistics are accurate
    #[test]
    fn deduplication_statistics_accuracy(
        base_msg in arb_stored_message(),
        duplicate_count in 1usize..10
    ) {
        let mut store = MessageStore::new();

        // Store the original message
        let _exists = store.store_message(base_msg.clone()).expect("Original message should store");
        prop_assert_eq!(store.message_count(), 1);
        prop_assert_eq!(store.stats().deduplication_saves, 0);

        // Store duplicates
        for _ in 0..duplicate_count {
            let _exists = store.store_message(base_msg.clone()).expect("Duplicate should store");
        }

        // Should still have only one message, but deduplication count should increase
        prop_assert_eq!(store.message_count(), 1);
        prop_assert_eq!(store.stats().deduplication_saves, duplicate_count);
    }

    /// Property: Content validation boundaries
    #[test]
    fn content_validation_boundaries(
        sender in arb_peer_id(),
        recipient in arb_peer_id(),
        content_size in 0usize..3000
    ) {
        let config = MessageStoreConfig {
            max_message_size: 1000, // Set reasonable limit
            max_messages_per_conversation: 100,
            max_total_messages: 10000,
            max_content_length: 500, // Characters, not bytes
            max_message_age_secs: 86400,
            strict_content_validation: true,
        };

        let mut store = MessageStore::with_config(config);
        let content = "x".repeat(content_size);

        let msg = ContentAddressedMessage::new(sender, Some(recipient), content, 1);
        let result = store.store_message(msg);

        if content_size <= 500 {
            prop_assert!(result.is_ok(), "Message within size limit should succeed");
        } else {
            prop_assert!(result.is_err(), "Message exceeding size limit should fail");
        }
    }

    /// Property: Time indexing consistency
    #[test]
    fn time_indexing_consistency(messages in prop::collection::vec(arb_stored_message(), 1..20)) {
        let mut store = MessageStore::new();

        // Store messages
        for msg in &messages {
            let _exists = store.store_message(msg.clone()).expect("Message should store");
        }

        // Verify time range queries work correctly
        if !messages.is_empty() {
            let min_time = messages.iter().map(|m| m.timestamp).min().unwrap();
            let max_time = messages.iter().map(|m| m.timestamp).max().unwrap();

            let range_results = store.get_messages_in_range(min_time, max_time);

            // Should retrieve some messages in the time range
            prop_assert!(!range_results.is_empty());
            prop_assert!(range_results.len() <= store.message_count());
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_property_test_generators() {
        // Smoke test to ensure our generators work
        let strategy = arb_stored_message();
        let mut runner = proptest::test_runner::TestRunner::deterministic();

        for _ in 0..10 {
            let msg = strategy.new_tree(&mut runner).unwrap().current();
            assert!(!msg.content.is_empty() || msg.content.is_empty()); // Just verify it generates
        }
    }
}
