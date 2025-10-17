//! Message handlers for the BitChat protocol
//!
//! This module provides trait-based message handling with clean dispatch mechanisms
//! for processing different types of BitChat messages.

use alloc::{string::String, vec::Vec};
use uuid::Uuid;

use crate::packet::{BitchatMessage, BitchatPacket, DeliveryAck, MessageType, ReadReceipt};
use crate::types::PeerId;
use crate::Result;

// ----------------------------------------------------------------------------
// Message Handler Trait
// ----------------------------------------------------------------------------

/// Trait for handling different types of BitChat messages
pub trait MessageHandler {
    /// Handle a regular chat message
    fn handle_message(&mut self, packet: &BitchatPacket, message: &BitchatMessage) -> Result<()>;
    
    /// Handle a delivery acknowledgment
    fn handle_delivery_ack(&mut self, packet: &BitchatPacket, ack: &DeliveryAck) -> Result<()>;
    
    /// Handle a read receipt
    fn handle_read_receipt(&mut self, packet: &BitchatPacket, receipt: &ReadReceipt) -> Result<()>;
    
    /// Handle a Noise handshake initiation
    fn handle_handshake_init(&mut self, packet: &BitchatPacket) -> Result<()>;
    
    /// Handle a Noise handshake response
    fn handle_handshake_response(&mut self, packet: &BitchatPacket) -> Result<()>;
    
    /// Handle a Noise handshake finalization
    fn handle_handshake_finalize(&mut self, packet: &BitchatPacket) -> Result<()>;
    
    /// Handle an identity announcement
    fn handle_announce(&mut self, packet: &BitchatPacket) -> Result<()>;
    
    /// Handle a sync request
    fn handle_request_sync(&mut self, packet: &BitchatPacket) -> Result<()>;
    
    /// Handle a fragment start
    fn handle_fragment_start(&mut self, packet: &BitchatPacket) -> Result<()>;
    
    /// Handle a fragment continuation
    fn handle_fragment_continue(&mut self, packet: &BitchatPacket) -> Result<()>;
    
    /// Handle a fragment end
    fn handle_fragment_end(&mut self, packet: &BitchatPacket) -> Result<()>;
}

// ----------------------------------------------------------------------------
// Message Dispatcher
// ----------------------------------------------------------------------------

/// Dispatches BitChat packets to appropriate message handlers
pub struct MessageDispatcher;

impl MessageDispatcher {
    /// Dispatch a packet to the appropriate handler method
    pub fn dispatch<H: MessageHandler>(handler: &mut H, packet: &BitchatPacket) -> Result<()> {
        match packet.message_type {
            MessageType::Message => {
                let message: BitchatMessage = bincode::deserialize(&packet.payload)?;
                handler.handle_message(packet, &message)
            }
            MessageType::DeliveryAck => {
                let ack: DeliveryAck = bincode::deserialize(&packet.payload)?;
                handler.handle_delivery_ack(packet, &ack)
            }
            MessageType::ReadReceipt => {
                let receipt: ReadReceipt = bincode::deserialize(&packet.payload)?;
                handler.handle_read_receipt(packet, &receipt)
            }
            MessageType::NoiseHandshakeInit => {
                handler.handle_handshake_init(packet)
            }
            MessageType::NoiseHandshakeResponse => {
                handler.handle_handshake_response(packet)
            }
            MessageType::NoiseHandshakeFinalize => {
                handler.handle_handshake_finalize(packet)
            }
            MessageType::Announce => {
                handler.handle_announce(packet)
            }
            MessageType::RequestSync => {
                handler.handle_request_sync(packet)
            }
            MessageType::FragmentStart => {
                handler.handle_fragment_start(packet)
            }
            MessageType::FragmentContinue => {
                handler.handle_fragment_continue(packet)
            }
            MessageType::FragmentEnd => {
                handler.handle_fragment_end(packet)
            }
        }
    }
}

// ----------------------------------------------------------------------------
// Default Handler Implementation
// ----------------------------------------------------------------------------

/// Default no-op message handler for testing and demonstration
pub struct DefaultHandler;

impl MessageHandler for DefaultHandler {
    fn handle_message(&mut self, _packet: &BitchatPacket, _message: &BitchatMessage) -> Result<()> {
        Ok(())
    }
    
    fn handle_delivery_ack(&mut self, _packet: &BitchatPacket, _ack: &DeliveryAck) -> Result<()> {
        Ok(())
    }
    
    fn handle_read_receipt(&mut self, _packet: &BitchatPacket, _receipt: &ReadReceipt) -> Result<()> {
        Ok(())
    }
    
    fn handle_handshake_init(&mut self, _packet: &BitchatPacket) -> Result<()> {
        Ok(())
    }
    
    fn handle_handshake_response(&mut self, _packet: &BitchatPacket) -> Result<()> {
        Ok(())
    }
    
    fn handle_handshake_finalize(&mut self, _packet: &BitchatPacket) -> Result<()> {
        Ok(())
    }
    
    fn handle_announce(&mut self, _packet: &BitchatPacket) -> Result<()> {
        Ok(())
    }
    
    fn handle_request_sync(&mut self, _packet: &BitchatPacket) -> Result<()> {
        Ok(())
    }
    
    fn handle_fragment_start(&mut self, _packet: &BitchatPacket) -> Result<()> {
        Ok(())
    }
    
    fn handle_fragment_continue(&mut self, _packet: &BitchatPacket) -> Result<()> {
        Ok(())
    }
    
    fn handle_fragment_end(&mut self, _packet: &BitchatPacket) -> Result<()> {
        Ok(())
    }
}

// ----------------------------------------------------------------------------
// Message Builder
// ----------------------------------------------------------------------------

/// Helper for building various message types
pub struct MessageBuilder;

impl MessageBuilder {
    /// Create a regular chat message packet
    pub fn create_message(
        sender_id: PeerId,
        sender_name: String,
        content: String,
        recipient_id: Option<PeerId>,
    ) -> Result<BitchatPacket> {
        let message = BitchatMessage::new(sender_name, content);
        let payload = bincode::serialize(&message)?;
        
        let mut packet = BitchatPacket::new(MessageType::Message, sender_id, payload);
        if let Some(recipient) = recipient_id {
            packet = packet.with_recipient(recipient);
        }
        
        Ok(packet)
    }
    
    /// Create a delivery acknowledgment packet
    pub fn create_delivery_ack(
        sender_id: PeerId,
        message_id: Uuid,
        recipient_id: Option<PeerId>,
    ) -> Result<BitchatPacket> {
        let ack = DeliveryAck::new(message_id);
        let payload = bincode::serialize(&ack)?;
        
        let mut packet = BitchatPacket::new(MessageType::DeliveryAck, sender_id, payload);
        if let Some(recipient) = recipient_id {
            packet = packet.with_recipient(recipient);
        }
        
        Ok(packet)
    }
    
    /// Create a read receipt packet
    pub fn create_read_receipt(
        sender_id: PeerId,
        message_id: Uuid,
        recipient_id: Option<PeerId>,
    ) -> Result<BitchatPacket> {
        let receipt = ReadReceipt::new(message_id);
        let payload = bincode::serialize(&receipt)?;
        
        let mut packet = BitchatPacket::new(MessageType::ReadReceipt, sender_id, payload);
        if let Some(recipient) = recipient_id {
            packet = packet.with_recipient(recipient);
        }
        
        Ok(packet)
    }
    
    /// Create a handshake initiation packet
    pub fn create_handshake_init(
        sender_id: PeerId,
        handshake_payload: Vec<u8>,
        recipient_id: PeerId,
    ) -> BitchatPacket {
        BitchatPacket::new(MessageType::NoiseHandshakeInit, sender_id, handshake_payload)
            .with_recipient(recipient_id)
    }
    
    /// Create a handshake response packet
    pub fn create_handshake_response(
        sender_id: PeerId,
        handshake_payload: Vec<u8>,
        recipient_id: PeerId,
    ) -> BitchatPacket {
        BitchatPacket::new(MessageType::NoiseHandshakeResponse, sender_id, handshake_payload)
            .with_recipient(recipient_id)
    }
    
    /// Create a handshake finalization packet
    pub fn create_handshake_finalize(
        sender_id: PeerId,
        handshake_payload: Vec<u8>,
        recipient_id: PeerId,
    ) -> BitchatPacket {
        BitchatPacket::new(MessageType::NoiseHandshakeFinalize, sender_id, handshake_payload)
            .with_recipient(recipient_id)
    }
    
    /// Create an announcement packet
    pub fn create_announce(
        sender_id: PeerId,
        announcement_data: Vec<u8>,
    ) -> BitchatPacket {
        BitchatPacket::new(MessageType::Announce, sender_id, announcement_data)
    }
    
    /// Create a sync request packet
    pub fn create_request_sync(
        sender_id: PeerId,
        sync_data: Vec<u8>,
    ) -> BitchatPacket {
        BitchatPacket::new(MessageType::RequestSync, sender_id, sync_data)
    }
}

// ----------------------------------------------------------------------------
// Event-based Handler
// ----------------------------------------------------------------------------

/// Event types that can be emitted during message processing
#[derive(Debug, Clone)]
pub enum BitchatEvent {
    /// New message received
    MessageReceived {
        from: PeerId,
        message: BitchatMessage,
    },
    /// Message delivery confirmed
    DeliveryConfirmed {
        message_id: Uuid,
        confirmed_by: PeerId,
    },
    /// Message read by recipient
    MessageRead {
        message_id: Uuid,
        read_by: PeerId,
    },
    /// New peer announced
    PeerAnnounced {
        peer_id: PeerId,
        announcement: Vec<u8>,
    },
    /// Handshake completed
    HandshakeCompleted {
        peer_id: PeerId,
    },
    /// Handshake failed
    HandshakeFailed {
        peer_id: PeerId,
        reason: String,
    },
}

/// Trait for handling BitChat events
pub trait EventHandler {
    /// Handle a BitChat event
    fn handle_event(&mut self, event: BitchatEvent);
}

/// Message handler that emits events
pub struct EventEmittingHandler<E: EventHandler> {
    event_handler: E,
}

impl<E: EventHandler> EventEmittingHandler<E> {
    /// Create a new event-emitting handler
    pub fn new(event_handler: E) -> Self {
        Self { event_handler }
    }
    
    /// Get a reference to the event handler
    pub fn event_handler(&self) -> &E {
        &self.event_handler
    }
    
    /// Get a mutable reference to the event handler
    pub fn event_handler_mut(&mut self) -> &mut E {
        &mut self.event_handler
    }
}

impl<E: EventHandler> MessageHandler for EventEmittingHandler<E> {
    fn handle_message(&mut self, packet: &BitchatPacket, message: &BitchatMessage) -> Result<()> {
        let event = BitchatEvent::MessageReceived {
            from: packet.sender_id,
            message: message.clone(),
        };
        self.event_handler.handle_event(event);
        Ok(())
    }
    
    fn handle_delivery_ack(&mut self, packet: &BitchatPacket, ack: &DeliveryAck) -> Result<()> {
        let event = BitchatEvent::DeliveryConfirmed {
            message_id: ack.message_id,
            confirmed_by: packet.sender_id,
        };
        self.event_handler.handle_event(event);
        Ok(())
    }
    
    fn handle_read_receipt(&mut self, packet: &BitchatPacket, receipt: &ReadReceipt) -> Result<()> {
        let event = BitchatEvent::MessageRead {
            message_id: receipt.message_id,
            read_by: packet.sender_id,
        };
        self.event_handler.handle_event(event);
        Ok(())
    }
    
    fn handle_handshake_init(&mut self, _packet: &BitchatPacket) -> Result<()> {
        Ok(())
    }
    
    fn handle_handshake_response(&mut self, _packet: &BitchatPacket) -> Result<()> {
        Ok(())
    }
    
    fn handle_handshake_finalize(&mut self, packet: &BitchatPacket) -> Result<()> {
        let event = BitchatEvent::HandshakeCompleted {
            peer_id: packet.sender_id,
        };
        self.event_handler.handle_event(event);
        Ok(())
    }
    
    fn handle_announce(&mut self, packet: &BitchatPacket) -> Result<()> {
        let event = BitchatEvent::PeerAnnounced {
            peer_id: packet.sender_id,
            announcement: packet.payload.clone(),
        };
        self.event_handler.handle_event(event);
        Ok(())
    }
    
    fn handle_request_sync(&mut self, _packet: &BitchatPacket) -> Result<()> {
        Ok(())
    }
    
    fn handle_fragment_start(&mut self, _packet: &BitchatPacket) -> Result<()> {
        Ok(())
    }
    
    fn handle_fragment_continue(&mut self, _packet: &BitchatPacket) -> Result<()> {
        Ok(())
    }
    
    fn handle_fragment_end(&mut self, _packet: &BitchatPacket) -> Result<()> {
        Ok(())
    }
}

// ----------------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    struct TestEventHandler {
        events: Vec<BitchatEvent>,
    }
    
    impl TestEventHandler {
        fn new() -> Self {
            Self { events: Vec::new() }
        }
    }
    
    impl EventHandler for TestEventHandler {
        fn handle_event(&mut self, event: BitchatEvent) {
            self.events.push(event);
        }
    }

    #[test]
    fn test_message_builder() {
        let sender_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        let recipient_id = Some(PeerId::new([8, 7, 6, 5, 4, 3, 2, 1]));
        
        let packet = MessageBuilder::create_message(
            sender_id,
            "alice".to_string(),
            "Hello, world!".to_string(),
            recipient_id,
        ).unwrap();
        
        assert_eq!(packet.message_type, MessageType::Message);
        assert_eq!(packet.sender_id, sender_id);
        assert_eq!(packet.recipient_id, recipient_id);
        
        // Verify payload can be deserialized
        let message: BitchatMessage = bincode::deserialize(&packet.payload).unwrap();
        assert_eq!(message.sender, "alice");
        assert_eq!(message.content, "Hello, world!");
    }
    
    #[test]
    fn test_message_dispatcher() {
        let mut handler = DefaultHandler;
        
        let sender_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        let packet = MessageBuilder::create_message(
            sender_id,
            "alice".to_string(),
            "Hello, world!".to_string(),
            None,
        ).unwrap();
        
        // Should not panic
        MessageDispatcher::dispatch(&mut handler, &packet).unwrap();
    }
    
    #[test]
    fn test_event_emitting_handler() {
        let event_handler = TestEventHandler::new();
        let mut handler = EventEmittingHandler::new(event_handler);
        
        let sender_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        let packet = MessageBuilder::create_message(
            sender_id,
            "alice".to_string(),
            "Hello, world!".to_string(),
            None,
        ).unwrap();
        
        MessageDispatcher::dispatch(&mut handler, &packet).unwrap();
        
        assert_eq!(handler.event_handler().events.len(), 1);
        match &handler.event_handler().events[0] {
            BitchatEvent::MessageReceived { from, message } => {
                assert_eq!(*from, sender_id);
                assert_eq!(message.sender, "alice");
                assert_eq!(message.content, "Hello, world!");
            }
            _ => panic!("Expected MessageReceived event"),
        }
    }
    
    #[test]
    fn test_delivery_ack_flow() {
        let sender_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        let message_id = Uuid::new_v4();
        
        let packet = MessageBuilder::create_delivery_ack(
            sender_id,
            message_id,
            None,
        ).unwrap();
        
        assert_eq!(packet.message_type, MessageType::DeliveryAck);
        
        let ack: DeliveryAck = bincode::deserialize(&packet.payload).unwrap();
        assert_eq!(ack.message_id, message_id);
    }
}