//! Integration tests for BitChat core protocol
//!
//! These tests verify the interaction between different components of the BitChat
//! protocol, including session management, message handling, fragmentation, and
//! delivery tracking in multi-peer scenarios.

use bitchat_core::*;
use bitchat_core::packet::DeliveryAck;
use uuid::Uuid;

// ----------------------------------------------------------------------------
// Test Utilities
// ----------------------------------------------------------------------------

/// Test peer that combines all BitChat components
struct TestPeer {
    peer_id: PeerId,
    session_manager: NoiseSessionManager,
    delivery_tracker: DeliveryTracker,
    fragmenter: MessageFragmenter,
    reassembler: MessageReassembler,
    received_messages: Vec<BitchatMessage>,
    received_events: Vec<BitchatEvent>,
}

impl TestPeer {
    fn new() -> Self {
        let noise_key = bitchat_core::crypto::NoiseKeyPair::generate();
        let peer_id = PeerId::from_bytes(&noise_key.public_key_bytes());
        
        Self {
            peer_id,
            session_manager: NoiseSessionManager::new(noise_key),
            delivery_tracker: DeliveryTracker::new(),
            fragmenter: MessageFragmenter,
            reassembler: MessageReassembler::new(),
            received_messages: Vec::new(),
            received_events: Vec::new(),
        }
    }
    
    fn peer_id(&self) -> PeerId {
        self.peer_id
    }
    
    /// Create a session with another peer
    async fn create_session_with(&mut self, other_peer: &mut TestPeer) -> bitchat_core::Result<()> {
        let my_id = self.peer_id();
        let other_id = other_peer.peer_id();
        
        // Create sessions on both sides
        self.session_manager.get_or_create_outbound(other_id)?;
        other_peer.session_manager.create_inbound(my_id)?;
        
        // Perform handshake
        let mut messages = Vec::new();
        
        // Step 1: Initiator creates first message
        {
            let session = self.session_manager.get_session_mut(&other_id).unwrap();
            let msg1 = session.create_handshake_message(b"")?;
            messages.push(msg1);
        }
        
        // Step 2: Responder processes and responds
        {
            let other_session = other_peer.session_manager.get_session_mut(&my_id).unwrap();
            let response1 = other_session.process_handshake_message(&messages[0])?;
            if let Some(resp) = response1 {
                messages.push(resp);
            } else {
                let msg2 = other_session.create_handshake_message(b"")?;
                messages.push(msg2);
            }
        }
        
        // Step 3: Initiator processes and finalizes
        {
            let session = self.session_manager.get_session_mut(&other_id).unwrap();
            let response2 = session.process_handshake_message(&messages[1])?;
            if let Some(resp) = response2 {
                messages.push(resp);
            } else if !session.is_established() {
                let msg3 = session.create_handshake_message(b"")?;
                messages.push(msg3);
            }
        }
        
        // Step 4: Final processing if needed
        if messages.len() > 2 {
            let other_session = other_peer.session_manager.get_session_mut(&my_id).unwrap();
            other_session.process_handshake_message(&messages[2])?;
        }
        
        Ok(())
    }
    
    /// Complete a handshake between two sessions (deprecated - use create_session_with instead)
    #[allow(dead_code)]
    async fn complete_handshake(
        &mut self,
        session: &mut NoiseSession,
        other_session: &mut NoiseSession,
    ) -> bitchat_core::Result<()> {
        // Step 1: Initiator creates first message
        let msg1 = session.create_handshake_message(b"")?;
        let response1 = other_session.process_handshake_message(&msg1)?;
        
        // Step 2: Responder responds
        let msg2 = if let Some(resp) = response1 {
            resp
        } else {
            other_session.create_handshake_message(b"")?
        };
        let response2 = session.process_handshake_message(&msg2)?;
        
        // Step 3: Initiator finalizes
        let msg3 = if let Some(resp) = response2 {
            resp
        } else {
            session.create_handshake_message(b"")?
        };
        other_session.process_handshake_message(&msg3)?;
        
        Ok(())
    }
    
    /// Send a message to another peer
    async fn send_message_to(
        &mut self,
        recipient_id: PeerId,
        content: String,
    ) -> bitchat_core::Result<Uuid> {
        let message = BitchatMessage::new("sender".to_string(), content);
        let message_id = message.id;
        let payload = bincode::serialize(&message)?;
        
        // Track message for delivery
        self.delivery_tracker.track_message(message_id, recipient_id, payload.clone());
        
        // Create packet
        let packet = MessageBuilder::create_message(
            self.peer_id,
            "sender".to_string(),
            message.content.clone(),
            Some(recipient_id),
        )?;
        
        // Mark as sent
        self.delivery_tracker.mark_sent(&message_id);
        
        Ok(message_id)
    }
    
    /// Receive and process a packet
    async fn receive_packet(&mut self, packet: BitchatPacket) -> bitchat_core::Result<()> {
        match packet.message_type {
            MessageType::Message => {
                let message: BitchatMessage = bincode::deserialize(&packet.payload)?;
                self.received_messages.push(message.clone());
                
                // Send delivery acknowledgment
                let _ack = MessageBuilder::create_delivery_ack(
                    self.peer_id,
                    message.id,
                    Some(packet.sender_id),
                )?;
                
                self.received_events.push(BitchatEvent::MessageReceived {
                    from: packet.sender_id,
                    message,
                });
            }
            MessageType::DeliveryAck => {
                let ack: DeliveryAck = bincode::deserialize(&packet.payload)?;
                self.delivery_tracker.confirm_delivery(&ack.message_id);
                
                self.received_events.push(BitchatEvent::DeliveryConfirmed {
                    message_id: ack.message_id,
                    confirmed_by: packet.sender_id,
                });
            }
            MessageType::FragmentStart | MessageType::FragmentContinue | MessageType::FragmentEnd => {
                let fragment = Fragment::from_packet(&packet)?;
                if let Some(assembled) = self.reassembler.process_fragment(fragment)? {
                    // Reassembly complete - process the assembled message as a regular message
                    let message: BitchatMessage = bincode::deserialize(&assembled)?;
                    self.received_messages.push(message.clone());
                    
                    self.received_events.push(BitchatEvent::MessageReceived {
                        from: packet.sender_id,
                        message,
                    });
                }
            }
            _ => {
                // Handle other message types as needed
            }
        }
        
        Ok(())
    }
    
    /// Send a large message that requires fragmentation
    async fn send_large_message_to(
        &mut self,
        recipient_id: PeerId,
        content: String,
    ) -> bitchat_core::Result<Vec<BitchatPacket>> {
        let message = BitchatMessage::new("sender".to_string(), content);
        let payload = bincode::serialize(&message)?;
        
        // Fragment the message
        let fragments = MessageFragmenter::fragment_message(
            message.id,
            &payload,
            512, // Small fragment size for testing
        )?;
        
        // Convert fragments to packets
        let mut packets = Vec::new();
        for fragment in fragments {
            let packet = fragment.to_packet(self.peer_id, Some(recipient_id))?;
            packets.push(packet);
        }
        
        Ok(packets)
    }
}

// ----------------------------------------------------------------------------
// Integration Tests
// ----------------------------------------------------------------------------

#[tokio::test]
async fn test_basic_message_exchange() {
    let mut alice = TestPeer::new();
    let mut bob = TestPeer::new();
    
    // Create session between Alice and Bob
    alice.create_session_with(&mut bob).await.unwrap();
    
    // Alice sends message to Bob
    let message_id = alice.send_message_to(bob.peer_id(), "Hello, Bob!".to_string()).await.unwrap();
    
    // Simulate message delivery - create the actual packet
    let packet = MessageBuilder::create_message(
        alice.peer_id(),
        "Alice".to_string(),
        "Hello, Bob!".to_string(),
        Some(bob.peer_id()),
    ).unwrap();
    
    // Bob receives the message
    bob.receive_packet(packet).await.unwrap();
    
    // Verify Bob received the message
    assert_eq!(bob.received_messages.len(), 1);
    assert_eq!(bob.received_messages[0].content, "Hello, Bob!");
    assert_eq!(bob.received_events.len(), 1);
    
    // Verify Alice's delivery tracking
    let tracked = alice.delivery_tracker.get_tracked(&message_id).unwrap();
    assert_eq!(tracked.status, DeliveryStatus::Sent);
}

#[tokio::test]
async fn test_delivery_confirmation() {
    let mut alice = TestPeer::new();
    let mut bob = TestPeer::new();
    
    // Alice sends message to Bob
    let message_id = alice.send_message_to(bob.peer_id(), "Hello, Bob!".to_string()).await.unwrap();
    
    // Get the actual message that Alice is tracking
    let tracked = alice.delivery_tracker.get_tracked(&message_id).unwrap();
    let message: BitchatMessage = bincode::deserialize(&tracked.payload).unwrap();
    
    // Create the packet with the same message that Alice is tracking
    let packet = MessageBuilder::create_message(
        alice.peer_id(),
        "Alice".to_string(),
        message.content.clone(),
        Some(bob.peer_id()),
    ).unwrap();
    
    // Patch the packet to have the same message ID as Alice's tracked message
    let mut packet = packet;
    let mut patched_message = message;
    patched_message.id = message_id; // Use Alice's tracked message ID
    packet.payload = bincode::serialize(&patched_message).unwrap();
    
    bob.receive_packet(packet).await.unwrap();
    
    // Create delivery acknowledgment using the correct message ID
    let ack_packet = MessageBuilder::create_delivery_ack(
        bob.peer_id(),
        message_id, // Use the same message ID Alice is tracking
        Some(alice.peer_id()),
    ).unwrap();
    
    // Alice receives the acknowledgment
    alice.receive_packet(ack_packet).await.unwrap();
    
    // Verify delivery was confirmed
    let tracked = alice.delivery_tracker.get_tracked(&message_id).unwrap();
    assert_eq!(tracked.status, DeliveryStatus::Confirmed);
    
    assert_eq!(alice.received_events.len(), 1);
    match &alice.received_events[0] {
        BitchatEvent::DeliveryConfirmed { message_id: id, confirmed_by } => {
            assert_eq!(*id, message_id);
            assert_eq!(*confirmed_by, bob.peer_id());
        }
        _ => panic!("Expected DeliveryConfirmed event"),
    }
}

#[tokio::test]
async fn test_message_fragmentation() {
    let mut alice = TestPeer::new();
    let mut bob = TestPeer::new();
    
    // Create a large message that will be fragmented
    let large_content = "A".repeat(2000); // 2KB message
    let packets = alice.send_large_message_to(bob.peer_id(), large_content.clone()).await.unwrap();
    
    // Verify multiple fragments were created
    assert!(packets.len() > 1);
    
    // Verify fragment types
    assert_eq!(packets[0].message_type, MessageType::FragmentStart);
    assert_eq!(packets[packets.len() - 1].message_type, MessageType::FragmentEnd);
    
    // Bob receives all fragments
    for packet in packets {
        bob.receive_packet(packet).await.unwrap();
    }
    
    // Verify message was reassembled correctly
    assert_eq!(bob.received_messages.len(), 1);
    assert_eq!(bob.received_messages[0].content, large_content);
}

#[tokio::test]
async fn test_multi_peer_scenario() {
    let mut alice = TestPeer::new();
    let mut bob = TestPeer::new();
    let mut charlie = TestPeer::new();
    
    // Create sessions between all peers
    alice.create_session_with(&mut bob).await.unwrap();
    alice.create_session_with(&mut charlie).await.unwrap();
    bob.create_session_with(&mut charlie).await.unwrap();
    
    // Alice sends messages to both Bob and Charlie
    let msg_to_bob = alice.send_message_to(bob.peer_id(), "Hello, Bob!".to_string()).await.unwrap();
    let msg_to_charlie = alice.send_message_to(charlie.peer_id(), "Hello, Charlie!".to_string()).await.unwrap();
    
    // Create and send packets
    let packet_to_bob = MessageBuilder::create_message(
        alice.peer_id(),
        "Alice".to_string(),
        "Hello, Bob!".to_string(),
        Some(bob.peer_id()),
    ).unwrap();
    
    let packet_to_charlie = MessageBuilder::create_message(
        alice.peer_id(),
        "Alice".to_string(),
        "Hello, Charlie!".to_string(),
        Some(charlie.peer_id()),
    ).unwrap();
    
    // Deliver messages
    bob.receive_packet(packet_to_bob).await.unwrap();
    charlie.receive_packet(packet_to_charlie).await.unwrap();
    
    // Verify both peers received their messages
    assert_eq!(bob.received_messages.len(), 1);
    assert_eq!(bob.received_messages[0].content, "Hello, Bob!");
    
    assert_eq!(charlie.received_messages.len(), 1);
    assert_eq!(charlie.received_messages[0].content, "Hello, Charlie!");
    
    // Verify Alice is tracking both deliveries
    assert!(alice.delivery_tracker.get_tracked(&msg_to_bob).is_some());
    assert!(alice.delivery_tracker.get_tracked(&msg_to_charlie).is_some());
}

#[tokio::test]
async fn test_session_state_management() {
    let mut alice = TestPeer::new();
    let mut bob = TestPeer::new();
    
    // Initial state
    let (handshaking, established, failed) = alice.session_manager.session_counts();
    assert_eq!((handshaking, established, failed), (0, 0, 0));
    
    // Create session (should start in handshaking state)
    let bob_id = bob.peer_id();
    alice.session_manager.get_or_create_outbound(bob_id).unwrap();
    
    // Check session state
    {
        let session = alice.session_manager.get_session(&bob_id).unwrap();
        assert_eq!(session.state(), SessionState::Handshaking);
    }
    
    let (handshaking, established, failed) = alice.session_manager.session_counts();
    assert_eq!((handshaking, established, failed), (1, 0, 0));
    
    // Complete handshake using the helper method
    alice.create_session_with(&mut bob).await.unwrap();
    
    // Both sessions should now be established
    let alice_session = alice.session_manager.get_session(&bob_id).unwrap();
    let bob_session = bob.session_manager.get_session(&alice.peer_id()).unwrap();
    
    assert!(alice_session.is_established());
    assert!(bob_session.is_established());
    assert!(alice_session.peer_fingerprint().is_some());
    assert!(bob_session.peer_fingerprint().is_some());
}

#[tokio::test]
async fn test_delivery_retry_mechanism() {
    let mut alice = TestPeer::new();
    let bob_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
    
    // Send message
    let message_id = alice.send_message_to(bob_id, "Test message".to_string()).await.unwrap();
    
    // Verify message is being tracked
    let tracked = alice.delivery_tracker.get_tracked(&message_id).unwrap();
    assert_eq!(tracked.status, DeliveryStatus::Sent);
    assert_eq!(tracked.attempt_count(), 1);
    
    // Simulate retry
    alice.delivery_tracker.mark_sent(&message_id);
    let tracked = alice.delivery_tracker.get_tracked(&message_id).unwrap();
    assert_eq!(tracked.attempt_count(), 2);
    
    // Simulate delivery confirmation
    alice.delivery_tracker.confirm_delivery(&message_id);
    let tracked = alice.delivery_tracker.get_tracked(&message_id).unwrap();
    assert_eq!(tracked.status, DeliveryStatus::Confirmed);
}

#[tokio::test]
async fn test_concurrent_fragmented_messages() {
    let mut alice = TestPeer::new();
    let mut bob = TestPeer::new();
    
    // Send two large messages concurrently
    let message1_content = "Message 1: ".to_string() + &"A".repeat(1500);
    let message2_content = "Message 2: ".to_string() + &"B".repeat(1500);
    
    let packets1 = alice.send_large_message_to(bob.peer_id(), message1_content.clone()).await.unwrap();
    let packets2 = alice.send_large_message_to(bob.peer_id(), message2_content.clone()).await.unwrap();
    
    // Interleave the fragments to simulate concurrent transmission
    let mut all_packets = Vec::new();
    let max_len = packets1.len().max(packets2.len());
    
    for i in 0..max_len {
        if i < packets1.len() {
            all_packets.push(packets1[i].clone());
        }
        if i < packets2.len() {
            all_packets.push(packets2[i].clone());
        }
    }
    
    // Bob receives all fragments in interleaved order
    for packet in all_packets {
        bob.receive_packet(packet).await.unwrap();
    }
    
    // Verify both messages were reassembled correctly
    assert_eq!(bob.received_messages.len(), 2);
    
    // Messages might arrive in any order due to concurrent processing
    let mut received_contents: Vec<String> = bob.received_messages
        .iter()
        .map(|m| m.content.clone())
        .collect();
    received_contents.sort();
    
    let mut expected_contents = vec![message1_content, message2_content];
    expected_contents.sort();
    
    assert_eq!(received_contents, expected_contents);
}

#[tokio::test]
async fn test_delivery_statistics() {
    let mut alice = TestPeer::new();
    let bob_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
    let charlie_id = PeerId::new([8, 7, 6, 5, 4, 3, 2, 1]);
    
    // Send multiple messages
    let msg1 = alice.send_message_to(bob_id, "Message 1".to_string()).await.unwrap();
    let msg2 = alice.send_message_to(charlie_id, "Message 2".to_string()).await.unwrap();
    let msg3 = alice.send_message_to(bob_id, "Message 3".to_string()).await.unwrap();
    
    // Check initial stats
    let stats = alice.delivery_tracker.get_stats();
    assert_eq!(stats.total, 3);
    assert_eq!(stats.sent, 3);
    assert_eq!(stats.confirmed, 0);
    
    // Confirm some deliveries
    alice.delivery_tracker.confirm_delivery(&msg1);
    alice.delivery_tracker.confirm_delivery(&msg2);
    alice.delivery_tracker.mark_failed(&msg3);
    
    // Check final stats
    let stats = alice.delivery_tracker.get_stats();
    assert_eq!(stats.confirmed, 2);
    assert_eq!(stats.failed, 1);
    assert_eq!(stats.success_rate(), 2.0 / 3.0); // 2 out of 3 completed messages succeeded
    assert_eq!(stats.average_attempts(), 1.0); // Each message had 1 attempt
}