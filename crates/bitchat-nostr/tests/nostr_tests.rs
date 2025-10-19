//! Integration tests for Nostr transport task

use bitchat_core::{
    internal::{create_effect_channel, create_event_channel, ChannelConfig},
    ChannelTransportType, PeerId, TransportTask,
};
use bitchat_harness::{TransportBuilder, TransportHandle};
use bitchat_nostr::{BitchatNostrMessage, NostrConfig, NostrTransportTask};
use std::time::Duration;

#[test]
fn test_bitchat_nostr_message() {
    let sender_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
    let recipient_id = Some(PeerId::new([8, 7, 6, 5, 4, 3, 2, 1]));
    let test_data = b"test message".to_vec();

    let nostr_msg = BitchatNostrMessage::new(sender_id, recipient_id, test_data.clone());

    assert_eq!(nostr_msg.sender_peer_id, sender_id);
    assert_eq!(nostr_msg.recipient_peer_id, recipient_id);

    let reconstructed_data = nostr_msg.to_data().unwrap();
    assert_eq!(reconstructed_data, test_data);
}

#[test]
fn test_nostr_message_serialization() {
    let sender_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
    let test_data = b"hello world".to_vec();

    let nostr_msg = BitchatNostrMessage::new(sender_id, None, test_data.clone());

    // Test serialization to Nostr event content
    let content = nostr_msg.to_nostr_content().unwrap();
    assert!(!content.is_empty());

    // Test deserialization from Nostr event content
    let deserialized_msg = BitchatNostrMessage::from_nostr_content(&content).unwrap();
    assert_eq!(deserialized_msg.sender_peer_id, sender_id);
    assert_eq!(deserialized_msg.recipient_peer_id, None);

    let recovered_data = deserialized_msg.to_data().unwrap();
    assert_eq!(recovered_data, test_data);
}

#[test]
fn test_nostr_message_helpers() {
    let sender_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
    let recipient_id = PeerId::new([8, 7, 6, 5, 4, 3, 2, 1]);
    let test_data = b"test".to_vec();

    // Test direct message
    let direct_msg = BitchatNostrMessage::new(sender_id, Some(recipient_id), test_data.clone());
    assert!(direct_msg.is_for_peer(&recipient_id));
    assert!(!direct_msg.is_for_peer(&sender_id));
    assert!(!direct_msg.is_broadcast());

    // Test broadcast message
    let broadcast_msg = BitchatNostrMessage::new(sender_id, None, test_data);
    assert!(broadcast_msg.is_for_peer(&recipient_id));
    assert!(broadcast_msg.is_for_peer(&sender_id));
    assert!(broadcast_msg.is_broadcast());
}

#[tokio::test]
async fn test_nostr_transport_task_creation() {
    let config = ChannelConfig::default();
    let (event_sender, _event_receiver) = create_event_channel(&config);
    let (_effect_sender, effect_receiver) = create_effect_channel(&config);
    let nostr_config = NostrConfig::local_development();

    let mut task = NostrTransportTask::new(nostr_config).unwrap();
    task.attach_channels(event_sender, effect_receiver).unwrap();

    // Test basic properties
    assert_eq!(task.transport_type(), ChannelTransportType::Nostr);

    // Note: The new CSP architecture doesn't have is_running/start/stop methods
    // The transport task runs continuously via the run() method from TransportTask trait
    // This is a design improvement - the task lifecycle is managed by the runtime
}

#[tokio::test]
async fn test_nostr_transport_with_harness_builder() {
    let config = ChannelConfig::default();
    let (event_sender, _event_receiver) = create_event_channel(&config);
    let (_effect_sender, effect_receiver) = create_effect_channel(&config);

    // Use TransportBuilder to create transport infrastructure
    let builder = TransportBuilder::new(ChannelTransportType::Nostr)
        .with_reconnect(bitchat_harness::ReconnectConfig::default());

    let _processor = builder.build_message_processor(event_sender.clone());
    let _reconnect_manager = builder.build_reconnect_manager();

    // Create transport handle using harness utilities
    let mut handle = TransportHandle::new(event_sender, effect_receiver);

    // Verify handle creation
    assert!(handle.take_effect_receiver().is_some());
}

#[test]
fn test_nostr_config() {
    let config = NostrConfig::default();
    assert!(!config.relays.is_empty());
    assert_eq!(config.connection_timeout, Duration::from_secs(10));
    assert_eq!(config.max_data_size, 64000);
    assert!(config.auto_reconnect);

    let local_config = NostrConfig::local_development();
    assert_eq!(local_config.relays.len(), 1);
    assert_eq!(local_config.relays[0].url, "ws://localhost:7777");
    assert_eq!(local_config.connection_timeout, Duration::from_secs(5));
    assert_eq!(local_config.reconnect_interval, Duration::from_secs(2));
}

#[test]
fn test_nostr_config_builder() {
    let mut config = NostrConfig::default();

    // Test adding relay
    config.add_relay("wss://relay.example.com".to_string());
    assert!(config
        .relays
        .iter()
        .any(|r| r.url == "wss://relay.example.com"));

    // Test with specific private key
    let keys = nostr_sdk::Keys::generate();
    let config_with_key = NostrConfig::with_private_key(keys.clone());
    // Keys comparison - compare public keys since private keys can't be directly compared
    assert_eq!(
        config_with_key.private_key.as_ref().unwrap().public_key(),
        keys.public_key()
    );
}

#[test]
fn test_nostr_relay_config() {
    let relay_config = bitchat_nostr::NostrRelayConfig::new("wss://relay.test.com".to_string());
    assert_eq!(relay_config.url, "wss://relay.test.com");
    assert_eq!(relay_config.connection_timeout, Duration::from_secs(10));
    assert!(!relay_config.read_only);
}
