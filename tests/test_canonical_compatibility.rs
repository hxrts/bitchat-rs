//! Test to verify our binary format matches the canonical BitChat implementation
//!
//! This test creates a BitChat packet and verifies that:
//! 1. Header is exactly 13 bytes for version 1
//! 2. Message types match canonical values
//! 3. Fragment format matches canonical specification

use bitchat_core::protocol::{BitchatPacket, MessageType, PacketHeader, PacketFlags};
use bitchat_core::protocol::WireFormat;
use bitchat_core::types::{PeerId, Timestamp, Ttl};

fn main() {
    println!("Testing canonical BitChat compatibility...\n");

    test_header_size();
    test_message_types();
    test_fragment_format();
    test_wire_format_compatibility();
    
    println!("[OK] All canonical compatibility tests passed!");
}

fn test_header_size() {
    println!("Testing header size...");
    
    let header = PacketHeader::new(
        MessageType::Message,
        Ttl::new(7),
        Timestamp::now(),
        PacketFlags::new(0),
        100, // payload length
    );
    
    let header_bytes = header.to_bytes().unwrap();
    println!("  Header size: {} bytes", header_bytes.len());
    
    // Canonical expects exactly 13 bytes
    assert_eq!(header_bytes.len(), 13, "Header must be exactly 13 bytes for canonical compatibility");
    
    // Verify structure: Version(1) + Type(1) + TTL(1) + Timestamp(8) + Flags(1) + PayloadLength(1) = 13
    assert_eq!(header_bytes[0], 1); // Version
    assert_eq!(header_bytes[1], 0x02); // Message type (0x02)
    assert_eq!(header_bytes[2], 7); // TTL
    // bytes 3-10: timestamp (8 bytes)
    assert_eq!(header_bytes[11], 0); // Flags
    assert_eq!(header_bytes[12], 100); // Payload length (1 byte for v1)
    
    println!("  [OK] Header size and structure correct");
}

fn test_message_types() {
    println!("Testing message type values...");
    
    // Test canonical message type values
    assert_eq!(MessageType::Announce as u8, 0x01);
    assert_eq!(MessageType::Message as u8, 0x02); 
    assert_eq!(MessageType::Leave as u8, 0x03);
    assert_eq!(MessageType::NoiseHandshake as u8, 0x10); // Single handshake type
    assert_eq!(MessageType::NoiseEncrypted as u8, 0x11); // FIXED: was 0x12
    assert_eq!(MessageType::Fragment as u8, 0x20);
    assert_eq!(MessageType::RequestSync as u8, 0x21);
    assert_eq!(MessageType::FileTransfer as u8, 0x22); // ADDED
    
    println!("  [OK] Message type values match canonical specification");
}

fn test_fragment_format() {
    println!("Testing fragment format...");
    
    use bitchat_core::protocol::fragmentation::{FragmentHeader, MessageFragmenter};
    
    // Test canonical fragment header: FragmentID(8) + Index(2) + Total(2) + OriginalType(1) = 13 bytes
    let fragment_header = FragmentHeader::new(
        0x1234567890ABCDEF_u64, // 8-byte fragment ID
        5,                      // index
        10,                     // total
        MessageType::Message,   // original type
    );
    
    let header_bytes = fragment_header.to_bytes().unwrap();
    println!("  Fragment header size: {} bytes", header_bytes.len());
    
    assert_eq!(header_bytes.len(), 13, "Fragment header must be 13 bytes");
    
    // Verify structure
    assert_eq!(u64::from_be_bytes(header_bytes[0..8].try_into().unwrap()), 0x1234567890ABCDEF);
    assert_eq!(u16::from_be_bytes(header_bytes[8..10].try_into().unwrap()), 5);
    assert_eq!(u16::from_be_bytes(header_bytes[10..12].try_into().unwrap()), 10);
    assert_eq!(header_bytes[12], MessageType::Message as u8);
    
    println!("  [OK] Fragment format matches canonical specification");
}

fn test_wire_format_compatibility() {
    println!("Testing wire format compatibility...");
    
    let sender = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
    let payload = b"Hello, BitChat!".to_vec();
    let packet = BitchatPacket::new(MessageType::Message, sender, payload);
    
    // Test encoding/decoding
    let encoded = WireFormat::encode(&packet).unwrap();
    let decoded = WireFormat::decode(&encoded).unwrap();
    
    assert_eq!(packet.header.version, decoded.header.version);
    assert_eq!(packet.header.message_type as u8, decoded.header.message_type as u8);
    assert_eq!(packet.sender_id, decoded.sender_id);
    assert_eq!(packet.payload, decoded.payload);
    
    println!("  Wire format packet size: {} bytes", encoded.len());
    println!("  [OK] Wire format round-trip successful");
}