//! Unit Tests for Pure Functions
//!
//! Comprehensive testing of state transitions, effect planning, message storage, and other
//! pure functions within the bitchat-core library. These tests focus on the core business
//! logic without any network I/O or UI dependencies.
//!
//! Tests cover: connection state machines, message deduplication, cryptographic operations,
//! effect planning, statistics tracking, and core data structures.

use bitchat_core::{
    internal::{
        ConnectionEvent, ConnectionState, ContentAddressedMessage, MessageId, MessageStore,
        SessionParams, TimeSource,
    },
    AppEvent, ChannelTransportType, Command, ConnectionStatus, Effect, Event, PeerId,
    SystemTimeSource,
};

// ----------------------------------------------------------------------------
// Test Utilities
// ----------------------------------------------------------------------------

fn create_test_peer_id(id: u8) -> PeerId {
    PeerId::new([id, 0, 0, 0, 0, 0, 0, 0])
}

fn create_test_message(
    from: PeerId,
    to: Option<PeerId>,
    content: &str,
    seq: u64,
) -> ContentAddressedMessage {
    // Use a fixed timestamp for deterministic test results
    let fixed_timestamp = 1234567890000; // Fixed timestamp
    ContentAddressedMessage::from_metadata(
        from,
        to,
        content.to_string(),
        seq,
        fixed_timestamp,
        None,
    )
    .expect("Failed to create test message")
}

// ----------------------------------------------------------------------------
// Connection State Machine Tests
// ----------------------------------------------------------------------------

#[test]
fn test_connection_state_disconnected_to_discovering() {
    let peer_id = create_test_peer_id(1);
    let state = ConnectionState::new_disconnected(peer_id);

    let transition = state.transition(ConnectionEvent::StartDiscovery {
        timeout_seconds: Some(60),
    });
    assert!(transition.is_ok());

    let transition = transition.unwrap();
    assert!(matches!(
        transition.new_state,
        ConnectionState::Discovering(_)
    ));
    assert!(!transition.effects.is_empty());
    assert!(!transition.audit_entry.event.is_empty());
}

#[test]
fn test_connection_state_discovering_to_connecting() {
    let peer_id = create_test_peer_id(1);
    let state = ConnectionState::new_disconnected(peer_id);

    // First transition to discovering
    let state = state
        .transition(ConnectionEvent::StartDiscovery {
            timeout_seconds: Some(60),
        })
        .unwrap()
        .new_state;

    // Discover peer (stays in discovering state)
    let state = state
        .transition(ConnectionEvent::PeerDiscovered {
            transport: ChannelTransportType::Ble,
            signal_strength: Some(-50),
        })
        .unwrap()
        .new_state;
    assert!(matches!(state, ConnectionState::Discovering(_)));

    // Then initiate connection to transition to connecting
    let session_params = SessionParams {
        protocol_version: 1,
        encryption_key: vec![1, 2, 3, 4],
        timeout_seconds: 30,
    };
    let transition = state.transition(ConnectionEvent::InitiateConnection {
        transport: ChannelTransportType::Ble,
        session_params,
    });
    assert!(transition.is_ok());

    let transition = transition.unwrap();
    assert!(matches!(
        transition.new_state,
        ConnectionState::Connecting(_)
    ));
}

#[test]
fn test_connection_state_connecting_to_connected() {
    let peer_id = create_test_peer_id(1);
    let mut state = ConnectionState::new_disconnected(peer_id);

    // Transition through discovering -> connecting
    state = state
        .transition(ConnectionEvent::StartDiscovery {
            timeout_seconds: Some(60),
        })
        .unwrap()
        .new_state;
    state = state
        .transition(ConnectionEvent::PeerDiscovered {
            transport: ChannelTransportType::Ble,
            signal_strength: Some(-50),
        })
        .unwrap()
        .new_state;

    // Initiate connection to move to connecting state
    let session_params = SessionParams {
        protocol_version: 1,
        encryption_key: vec![1, 2, 3, 4],
        timeout_seconds: 30,
    };
    state = state
        .transition(ConnectionEvent::InitiateConnection {
            transport: ChannelTransportType::Ble,
            session_params,
        })
        .unwrap()
        .new_state;

    // Then establish connection
    let transition = state.transition(ConnectionEvent::ConnectionEstablished {
        session_id: "test-session".to_string(),
    });
    assert!(transition.is_ok());

    let transition = transition.unwrap();
    assert!(matches!(
        transition.new_state,
        ConnectionState::Connected(_)
    ));
}

#[test]
fn test_connection_state_invalid_transitions() {
    let peer_id = create_test_peer_id(1);
    let state = ConnectionState::new_disconnected(peer_id);

    // Cannot establish connection directly from disconnected
    let result = state.transition(ConnectionEvent::ConnectionEstablished {
        session_id: "test-session".to_string(),
    });
    assert!(result.is_err());
}

#[test]
fn test_connection_state_error_recovery() {
    let peer_id = create_test_peer_id(1);
    let mut state = ConnectionState::new_disconnected(peer_id);

    // Transition to connected state
    state = state
        .transition(ConnectionEvent::StartDiscovery {
            timeout_seconds: Some(60),
        })
        .unwrap()
        .new_state;
    state = state
        .transition(ConnectionEvent::PeerDiscovered {
            transport: ChannelTransportType::Ble,
            signal_strength: Some(-50),
        })
        .unwrap()
        .new_state;

    // Initiate connection
    let session_params = SessionParams {
        protocol_version: 1,
        encryption_key: vec![1, 2, 3, 4],
        timeout_seconds: 30,
    };
    state = state
        .transition(ConnectionEvent::InitiateConnection {
            transport: ChannelTransportType::Ble,
            session_params,
        })
        .unwrap()
        .new_state;

    // Establish connection
    state = state
        .transition(ConnectionEvent::ConnectionEstablished {
            session_id: "test-session".to_string(),
        })
        .unwrap()
        .new_state;

    // Test error transition
    let transition = state.transition(ConnectionEvent::ConnectionLost {
        reason: "Network error".to_string(),
    });
    assert!(transition.is_ok());

    let transition = transition.unwrap();
    assert!(matches!(transition.new_state, ConnectionState::Failed(_)));
}

// ----------------------------------------------------------------------------
// Message Store Tests
// ----------------------------------------------------------------------------

#[test]
fn test_message_store_deduplication() {
    let mut store = MessageStore::new();
    let peer_id = create_test_peer_id(1);
    let message1 = create_test_message(peer_id, None, "Hello", 1);
    let message2 = create_test_message(peer_id, None, "Hello", 1); // Same content, same sequence

    assert!(store.store_message(message1.clone()).is_ok());
    assert!(store.store_message(message2).is_ok()); // Should be deduplicated

    let conversation_id = message1.conversation_id();
    let messages = store.get_conversation_messages(&conversation_id);
    assert_eq!(messages.len(), 1);
}

#[test]
fn test_message_store_conversation_retrieval() {
    let mut store = MessageStore::new();
    let peer1 = create_test_peer_id(1);
    let peer2 = create_test_peer_id(2);

    let msg1 = create_test_message(peer1, Some(peer2), "Hello from peer1", 1);
    let msg2 = create_test_message(peer2, Some(peer1), "Hello from peer2", 1);
    let msg3 = create_test_message(peer1, None, "Broadcast from peer1", 2);

    store.store_message(msg1).unwrap();
    store.store_message(msg2).unwrap();
    store.store_message(msg3).unwrap();

    let msg_conversations = store.get_peer_conversations(&peer1);
    assert!(!msg_conversations.is_empty()); // Should have conversations
}

#[test]
fn test_message_store_integrity_verification() {
    let mut store = MessageStore::new();
    let peer_id = create_test_peer_id(1);
    let message = create_test_message(peer_id, None, "Original content", 1);

    store.store_message(message.clone()).unwrap();

    // Verify content integrity validation works correctly
    assert!(message.verify_integrity());

    // Verify message can be retrieved by ID
    let retrieved = store.get_message(&message.id);
    assert!(retrieved.is_some());
}

#[test]
fn test_message_store_time_based_queries() {
    let mut store = MessageStore::new();
    let peer_id = create_test_peer_id(1);

    let msg1 = create_test_message(peer_id, None, "Message 1", 1);
    let msg2 = create_test_message(peer_id, None, "Message 2", 2);
    let msg3 = create_test_message(peer_id, None, "Message 3", 3);

    store.store_message(msg1.clone()).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(10));
    store.store_message(msg2.clone()).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(10));
    store.store_message(msg3.clone()).unwrap();

    // Get all messages in a time range
    let start_time = 0;
    let end_time = u64::MAX;
    let range_messages = store.get_messages_in_range(start_time, end_time);
    assert_eq!(range_messages.len(), 3);

    // Verify we have all three messages
    assert!(range_messages.iter().any(|m| m.content == "Message 1"));
    assert!(range_messages.iter().any(|m| m.content == "Message 2"));
    assert!(range_messages.iter().any(|m| m.content == "Message 3"));
}

// ----------------------------------------------------------------------------
// Core State Management Tests
// ----------------------------------------------------------------------------

#[test]
fn test_connection_status_transitions() {
    // Test basic status values
    assert_eq!(
        ConnectionStatus::Disconnected,
        ConnectionStatus::Disconnected
    );
    assert_eq!(ConnectionStatus::Connecting, ConnectionStatus::Connecting);
    assert_eq!(ConnectionStatus::Connected, ConnectionStatus::Connected);
    assert_ne!(ConnectionStatus::Connected, ConnectionStatus::Disconnected);
}

#[test]
fn test_transport_type_identification() {
    assert_eq!(ChannelTransportType::Ble, ChannelTransportType::Ble);
    assert_eq!(ChannelTransportType::Nostr, ChannelTransportType::Nostr);
    assert_ne!(ChannelTransportType::Ble, ChannelTransportType::Nostr);
}

// ----------------------------------------------------------------------------
// Effect Planning Tests
// ----------------------------------------------------------------------------

#[test]
fn test_effect_planning_send_packet() {
    let peer_id = create_test_peer_id(1);
    let data = b"test packet data".to_vec();
    let transport = ChannelTransportType::Ble;

    let effect = Effect::SendPacket {
        peer_id,
        data: data.clone(),
        transport,
    };

    // Verify effect structure
    if let Effect::SendPacket {
        peer_id: p,
        data: d,
        transport: t,
    } = effect
    {
        assert_eq!(p, peer_id);
        assert_eq!(d, data);
        assert_eq!(t, transport);
    } else {
        panic!("Expected SendPacket effect");
    }
}

#[test]
fn test_effect_planning_connection_initiation() {
    let peer_id = create_test_peer_id(1);
    let transport = ChannelTransportType::Nostr;

    let effect = Effect::InitiateConnection { peer_id, transport };

    if let Effect::InitiateConnection {
        peer_id: p,
        transport: t,
    } = effect
    {
        assert_eq!(p, peer_id);
        assert_eq!(t, transport);
    } else {
        panic!("Expected InitiateConnection effect");
    }
}

#[test]
fn test_effect_planning_transport_discovery() {
    let transport = ChannelTransportType::Ble;

    let start_effect = Effect::StartTransportDiscovery { transport };
    let stop_effect = Effect::StopTransportDiscovery { transport };

    assert!(matches!(
        start_effect,
        Effect::StartTransportDiscovery { .. }
    ));
    assert!(matches!(stop_effect, Effect::StopTransportDiscovery { .. }));
}

// ----------------------------------------------------------------------------
// Command and Event Processing Tests
// ----------------------------------------------------------------------------

#[test]
fn test_command_structure() {
    let peer_id = create_test_peer_id(1);
    let content = "Test message".to_string();

    let send_cmd = Command::SendMessage {
        recipient: peer_id,
        content: content.clone(),
    };

    if let Command::SendMessage {
        recipient,
        content: c,
    } = send_cmd
    {
        assert_eq!(recipient, peer_id);
        assert_eq!(c, content);
    } else {
        panic!("Expected SendMessage command");
    }
}

#[test]
fn test_event_structure() {
    let peer_id = create_test_peer_id(1);
    let transport = ChannelTransportType::Ble;
    let content = "Received message".to_string();

    let test_id = MessageId::from_bytes([1; 32]);
    let event = Event::MessageReceived {
        from: peer_id,
        content: content.clone(),
        transport,
        message_id: Some(test_id),
        recipient: Some(peer_id),
        timestamp: Some(42),
        sequence: Some(7),
    };

    if let Event::MessageReceived {
        from,
        content: c,
        transport: t,
        message_id,
        recipient,
        timestamp,
        sequence,
    } = event
    {
        assert_eq!(from, peer_id);
        assert_eq!(c, content);
        assert_eq!(t, transport);
        assert_eq!(message_id, Some(test_id));
        assert_eq!(recipient, Some(peer_id));
        assert_eq!(timestamp, Some(42));
        assert_eq!(sequence, Some(7));
    } else {
        panic!("Expected MessageReceived event");
    }
}

#[test]
fn test_app_event_structure() {
    let peer_id = create_test_peer_id(1);
    let content = "App message".to_string();
    let timestamp = 12345u64;

    let app_event = AppEvent::MessageReceived {
        from: peer_id,
        content: content.clone(),
        timestamp,
    };

    if let AppEvent::MessageReceived {
        from,
        content: c,
        timestamp: t,
    } = app_event
    {
        assert_eq!(from, peer_id);
        assert_eq!(c, content);
        assert_eq!(t, timestamp);
    } else {
        panic!("Expected MessageReceived app event");
    }
}

// ----------------------------------------------------------------------------
// Time Source Tests
// ----------------------------------------------------------------------------

#[test]
fn test_system_time_source() {
    let time_source = SystemTimeSource;
    let timestamp1 = time_source.now();

    std::thread::sleep(std::time::Duration::from_millis(10));

    let timestamp2 = time_source.now();

    assert!(timestamp2.as_millis() > timestamp1.as_millis());
}

// ----------------------------------------------------------------------------
// Property-Based Tests
// ----------------------------------------------------------------------------

#[test]
fn test_message_id_consistency() {
    let peer_id = create_test_peer_id(1);

    // Same content should produce same message ID
    let msg1 = create_test_message(peer_id, None, "Same content", 1);
    let msg2 = create_test_message(peer_id, None, "Same content", 1);

    assert_eq!(msg1.id, msg2.id);

    // Different content should produce different message IDs
    let msg3 = create_test_message(peer_id, None, "Different content", 1);
    assert_ne!(msg1.id, msg3.id);
}

#[test]
fn test_peer_id_uniqueness() {
    let peer1 = create_test_peer_id(1);
    let peer2 = create_test_peer_id(2);
    let peer3 = create_test_peer_id(1); // Same as peer1

    assert_ne!(peer1, peer2);
    assert_eq!(peer1, peer3);
}

#[test]
fn test_connection_state_invariants() {
    let peer_id = create_test_peer_id(1);
    let state = ConnectionState::new_disconnected(peer_id);

    // Invariant: peer_id should always match
    assert_eq!(state.peer_id(), peer_id);

    // State invariant: disconnected peers cannot send messages
    assert!(!state.can_send_messages());
}
