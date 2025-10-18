//! Integration tests for Nostr transport

use bitchat_core::packet::MessageType;
use bitchat_core::transport::{LatencyClass, ReliabilityClass, Transport, TransportType};
use bitchat_core::{BitchatPacket, PeerId};
use bitchat_nostr::{create_local_relay_config, BitchatNostrMessage, NostrTransport};
use std::time::Duration;

#[test]
fn test_bitchat_nostr_message() {
    let sender_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
    let recipient_id = Some(PeerId::new([8, 7, 6, 5, 4, 3, 2, 1]));

    let packet = BitchatPacket::new(MessageType::Message, sender_id, b"test message".to_vec());

    let nostr_msg = BitchatNostrMessage::new(sender_id, recipient_id, &packet).unwrap();

    assert_eq!(nostr_msg.sender_peer_id, sender_id);
    assert_eq!(nostr_msg.recipient_peer_id, recipient_id);

    let reconstructed_packet = nostr_msg.to_packet().unwrap();
    assert_eq!(reconstructed_packet.sender_id, sender_id);
    assert_eq!(reconstructed_packet.payload, b"test message");
}

#[test]
fn test_transport_capabilities() {
    let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
    let _config = create_local_relay_config();
    let transport = NostrTransport::new(peer_id).expect("Failed to create transport");
    let caps = transport.capabilities();

    assert_eq!(caps.transport_type, TransportType::Nostr);
    assert!(caps.supports_discovery);
    assert!(caps.supports_broadcast);
    assert!(caps.requires_internet);
    assert_eq!(caps.latency_class, LatencyClass::Medium);
    assert_eq!(caps.reliability_class, ReliabilityClass::High);
}

#[test]
fn test_local_relay_config() {
    let config = create_local_relay_config();
    assert_eq!(config.relay_urls, vec!["ws://localhost:7777"]);
    assert_eq!(config.connection_timeout, Duration::from_secs(5));
}
