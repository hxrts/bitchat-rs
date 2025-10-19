//! Test to verify our binary format matches the canonical BitChat implementation

use bitchat_core::protocol::{BitchatPacket, MessageType, PacketHeader, PacketFlags};
use bitchat_core::protocol::WireFormat;
use bitchat_core::types::{PeerId, Timestamp, Ttl};

#[test]
fn test_canonical_header_size() {
    // Canonical expects exactly 13 bytes for version 1 headers
    let header = PacketHeader::new(
        MessageType::Message,
        Ttl::new(7),
        Timestamp::now(),
        PacketFlags::new(0),
        100, // payload length
    );
    
    let header_bytes = header.to_bytes().unwrap();
    assert_eq!(header_bytes.len(), 13, "Header must be exactly 13 bytes for canonical compatibility");
    
    // Verify structure: Version(1) + Type(1) + TTL(1) + Timestamp(8) + Flags(1) + PayloadLength(1) = 13
    assert_eq!(header_bytes[0], 1); // Version
    assert_eq!(header_bytes[1], 0x02); // Message type (0x02)
    assert_eq!(header_bytes[2], 7); // TTL
    // bytes 3-10: timestamp (8 bytes)
    assert_eq!(header_bytes[11], 0); // Flags
    assert_eq!(header_bytes[12], 100); // Payload length (1 byte for v1)
}

#[test]
fn test_canonical_message_types() {
    // Test all canonical message type values match specification
    assert_eq!(MessageType::Announce as u8, 0x01);
    assert_eq!(MessageType::Message as u8, 0x02); 
    assert_eq!(MessageType::Leave as u8, 0x03);
    assert_eq!(MessageType::NoiseHandshake as u8, 0x10); // Single handshake type (not split)
    assert_eq!(MessageType::NoiseEncrypted as u8, 0x11); // FIXED: was 0x12
    assert_eq!(MessageType::Fragment as u8, 0x20);
    assert_eq!(MessageType::RequestSync as u8, 0x21);
    assert_eq!(MessageType::FileTransfer as u8, 0x22); // ADDED: was missing
}

#[test]
fn test_canonical_fragment_format() {
    use bitchat_core::protocol::fragmentation::FragmentHeader;
    
    // Test canonical fragment header: FragmentID(8) + Index(2) + Total(2) + OriginalType(1) = 13 bytes
    let fragment_header = FragmentHeader::new(
        0x1234567890ABCDEF_u64, // 8-byte fragment ID
        5,                      // index
        10,                     // total
        MessageType::Message,   // original type
    );
    
    let header_bytes = fragment_header.to_bytes().unwrap();
    assert_eq!(header_bytes.len(), 13, "Fragment header must be 13 bytes");
    
    // Verify structure matches canonical format exactly
    assert_eq!(u64::from_be_bytes(header_bytes[0..8].try_into().unwrap()), 0x1234567890ABCDEF);
    assert_eq!(u16::from_be_bytes(header_bytes[8..10].try_into().unwrap()), 5);
    assert_eq!(u16::from_be_bytes(header_bytes[10..12].try_into().unwrap()), 10);
    assert_eq!(header_bytes[12], MessageType::Message as u8);
}

#[test]
fn test_canonical_wire_format_compatibility() {
    let sender = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
    let payload = b"Hello, BitChat!".to_vec();
    let packet = BitchatPacket::new(MessageType::Message, sender, payload);
    
    // Test encoding/decoding works correctly
    let encoded = WireFormat::encode(&packet).unwrap();
    let decoded = WireFormat::decode(&encoded).unwrap();
    
    // Verify all fields match
    assert_eq!(packet.header.version, decoded.header.version);
    assert_eq!(packet.header.message_type as u8, decoded.header.message_type as u8);
    assert_eq!(packet.sender_id, decoded.sender_id);
    assert_eq!(packet.payload, decoded.payload);
    
    // Verify wire format structure
    assert_eq!(encoded[0], 1); // Version
    assert_eq!(encoded[1], MessageType::Message as u8); // Type
    
    // For a simple message with 15-byte payload + 8-byte sender ID = total packet ~36 bytes
    assert!(encoded.len() < 50, "Encoded packet should be reasonably small");
}

#[test]
fn test_max_payload_size_v1() {
    // Version 1 should support max 255 bytes payload
    let sender = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
    let max_payload = vec![0u8; 255];
    let packet = BitchatPacket::new(MessageType::Message, sender, max_payload);
    
    // Should encode successfully
    let encoded = WireFormat::encode(&packet).unwrap();
    let decoded = WireFormat::decode(&encoded).unwrap();
    assert_eq!(decoded.payload.len(), 255);
    
    // Payload larger than 255 should fail for v1
    let oversized_payload = vec![0u8; 256];
    let oversized_packet = BitchatPacket::new(MessageType::Message, sender, oversized_payload);
    assert!(WireFormat::encode(&oversized_packet).is_err(), "v1 should reject payloads > 255 bytes");
}