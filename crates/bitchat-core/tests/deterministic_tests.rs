//! Deterministic tests demonstrating the improved testing infrastructure
//!
//! These tests showcase how to use the new testing utilities for
//! deterministic, fast, and reliable testing of BitChat components.

use bitchat_core::crypto::{IdentityKeyPair, NoiseKeyPair};
use bitchat_core::packet::{BitchatMessage, BitchatPacket, MessageType};
use bitchat_core::transport::Transport;

mod test_utils;
use test_utils::{
    create_test_environment, create_test_peers, DeterministicRng, MockTimeSource, MockTransport,
};

use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::test]
async fn test_deterministic_time_and_packets() {
    // Create deterministic test environment
    let time_source = MockTimeSource::new();

    // Test packet creation with controlled time
    time_source.set_time(1000);
    let peer_id = create_test_peers(1)[0];
    let payload = b"test message".to_vec();

    let packet1 =
        BitchatPacket::new_with_time(MessageType::Message, peer_id, payload.clone(), &time_source);

    // Advance time and create another packet
    time_source.advance(500);
    let packet2 =
        BitchatPacket::new_with_time(MessageType::Message, peer_id, payload.clone(), &time_source);

    // Verify deterministic timestamps
    assert_eq!(packet1.timestamp.as_millis(), 1000);
    assert_eq!(packet2.timestamp.as_millis(), 1500);

    // Test message creation with controlled time
    let msg1 =
        BitchatMessage::new_with_time("Alice".to_string(), "Hello World".to_string(), &time_source);

    time_source.advance(1000);
    let msg2 =
        BitchatMessage::new_with_time("Bob".to_string(), "Hi Alice".to_string(), &time_source);

    assert_eq!(msg1.timestamp.as_millis(), 1500);
    assert_eq!(msg2.timestamp.as_millis(), 2500);
}

#[tokio::test]
async fn test_deterministic_crypto() {
    // Create deterministic RNG
    let mut rng = DeterministicRng::new();

    // Generate identity keys with deterministic RNG
    let identity1 = IdentityKeyPair::generate_with_rng(&mut rng).unwrap();
    let identity2 = IdentityKeyPair::generate_with_rng(&mut rng).unwrap();

    // Reset RNG to same state and generate again
    let mut rng_reset = DeterministicRng::new();
    let identity1_repeat = IdentityKeyPair::generate_with_rng(&mut rng_reset).unwrap();
    let identity2_repeat = IdentityKeyPair::generate_with_rng(&mut rng_reset).unwrap();

    // Verify deterministic key generation
    assert_eq!(
        identity1.public_key_bytes(),
        identity1_repeat.public_key_bytes()
    );
    assert_eq!(
        identity2.public_key_bytes(),
        identity2_repeat.public_key_bytes()
    );

    // Test Noise key pairs
    let mut rng = DeterministicRng::new();
    let noise1 = NoiseKeyPair::generate_with_rng(&mut rng);
    let noise2 = NoiseKeyPair::generate_with_rng(&mut rng);

    // Reset and generate again
    let mut rng_reset = DeterministicRng::new();
    let noise1_repeat = NoiseKeyPair::generate_with_rng(&mut rng_reset);
    let noise2_repeat = NoiseKeyPair::generate_with_rng(&mut rng_reset);

    assert_eq!(noise1.public_key_bytes(), noise1_repeat.public_key_bytes());
    assert_eq!(noise2.public_key_bytes(), noise2_repeat.public_key_bytes());
}

#[tokio::test]
async fn test_mock_network_simulation() {
    // Create test environment
    let (time_source, network) = create_test_environment();
    let peers = create_test_peers(3);

    // Create transports for each peer
    let transport1 = Arc::new(Mutex::new(
        MockTransport::new(peers[0]).with_network(network.clone()),
    ));
    let transport2 = Arc::new(Mutex::new(
        MockTransport::new(peers[1]).with_network(network.clone()),
    ));
    let transport3 = Arc::new(Mutex::new(
        MockTransport::new(peers[2]).with_network(network.clone()),
    ));

    // Add transports to network
    {
        let network = network.lock().await;
        network.add_transport(transport1.clone()).await;
        network.add_transport(transport2.clone()).await;
        network.add_transport(transport3.clone()).await;
    }

    // Start all transports
    transport1.lock().await.start().await.unwrap();
    transport2.lock().await.start().await.unwrap();
    transport3.lock().await.start().await.unwrap();

    // Create and send a packet from peer 1 to peer 2
    let packet = BitchatPacket::new_with_time(
        MessageType::Message,
        peers[0],
        b"Hello peer 2!".to_vec(),
        &time_source,
    );

    transport1
        .lock()
        .await
        .send_to(peers[1], packet.clone())
        .await
        .unwrap();

    // Process network (deliver packets)
    network.lock().await.tick().await.unwrap();

    // Peer 2 should receive the packet
    let (sender, received_packet) = transport2.lock().await.receive().await.unwrap();
    assert_eq!(sender, peers[0]);
    assert_eq!(received_packet.sender_id, peers[0]);
    assert_eq!(received_packet.payload, b"Hello peer 2!");
}

#[tokio::test]
async fn test_network_simulation_with_delays() {
    // Create test environment
    let (time_source, network) = create_test_environment();
    let peers = create_test_peers(2);

    // Configure network with delays
    {
        let mut network = network.lock().await;
        let mut config = test_utils::NetworkSimConfig::default();
        config.max_delay = 100; // 100ms max delay
        network.configure(config);
    }

    // Create transports
    let transport1 = Arc::new(Mutex::new(
        MockTransport::new(peers[0]).with_network(network.clone()),
    ));
    let transport2 = Arc::new(Mutex::new(
        MockTransport::new(peers[1]).with_network(network.clone()),
    ));

    {
        let network = network.lock().await;
        network.add_transport(transport1.clone()).await;
        network.add_transport(transport2.clone()).await;
    }

    transport1.lock().await.start().await.unwrap();
    transport2.lock().await.start().await.unwrap();

    // Send packet at time 0
    time_source.set_time(0);
    let packet = BitchatPacket::new_with_time(
        MessageType::Message,
        peers[0],
        b"Delayed message".to_vec(),
        &time_source,
    );

    transport1
        .lock()
        .await
        .send_to(peers[1], packet)
        .await
        .unwrap();

    // Process network - packet should be queued for future delivery
    network.lock().await.tick().await.unwrap();

    // Try to receive - should fail because packet is delayed
    // We'll use a timeout-based approach since we can't use now_or_never without futures
    let result = tokio::time::timeout(
        tokio::time::Duration::from_millis(10),
        transport2.lock().await.receive(),
    )
    .await;
    assert!(result.is_err()); // Timeout means no packet received

    // Advance time past the delay and process network again
    time_source.advance(150);
    network.lock().await.tick().await.unwrap();

    // Now the packet should be delivered
    let (sender, received_packet) = transport2.lock().await.receive().await.unwrap();
    assert_eq!(sender, peers[0]);
    assert_eq!(received_packet.payload, b"Delayed message");
}

#[tokio::test]
async fn test_network_simulation_with_packet_loss() {
    // Create test environment
    let (time_source, network) = create_test_environment();
    let peers = create_test_peers(2);

    // Configure network with 100% packet loss for testing
    {
        let mut network = network.lock().await;
        let mut config = test_utils::NetworkSimConfig::default();
        config.packet_loss_rate = 1.0; // 100% packet loss
        network.configure(config);
    }

    // Create transports
    let transport1 = Arc::new(Mutex::new(
        MockTransport::new(peers[0]).with_network(network.clone()),
    ));
    let transport2 = Arc::new(Mutex::new(
        MockTransport::new(peers[1]).with_network(network.clone()),
    ));

    {
        let network = network.lock().await;
        network.add_transport(transport1.clone()).await;
        network.add_transport(transport2.clone()).await;
    }

    transport1.lock().await.start().await.unwrap();
    transport2.lock().await.start().await.unwrap();

    // Send packet that should be lost
    let packet = BitchatPacket::new_with_time(
        MessageType::Message,
        peers[0],
        b"Lost message".to_vec(),
        &time_source,
    );

    transport1
        .lock()
        .await
        .send_to(peers[1], packet)
        .await
        .unwrap();

    // Process network
    network.lock().await.tick().await.unwrap();

    // Packet should be lost, so receive should not get anything
    let result = tokio::time::timeout(
        tokio::time::Duration::from_millis(10),
        transport2.lock().await.receive(),
    )
    .await;
    assert!(result.is_err()); // Timeout means no packet received
}
