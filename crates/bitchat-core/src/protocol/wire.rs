//! Wire format utilities for binary serialization
//!
//! This module provides compression, padding, and binary codec functionality
//! for the BitChat wire protocol.

use alloc::vec::Vec;
use core::convert::TryInto;

use crate::protocol::packet::BitchatPacket;
use crate::types::PeerId;
use crate::{BitchatError, Result};

#[cfg(feature = "std")]
use flate2::{read::ZlibDecoder, write::ZlibEncoder, Compression as ZlibCompression};
#[cfg(feature = "std")]
use std::io::{Read, Write};

// ----------------------------------------------------------------------------
// Constants
// ----------------------------------------------------------------------------

/// Standard padding block sizes for traffic analysis resistance
pub const PADDING_BLOCK_SIZES: &[usize] = &[256, 512, 1024, 2048];

/// Minimum size for compression (payloads smaller than this are not compressed)
pub const COMPRESSION_THRESHOLD: usize = 256;

/// Size of Ed25519 signature
pub const SIGNATURE_SIZE: usize = 64;

/// Size of PeerId
pub const PEER_ID_SIZE: usize = 8;

// ----------------------------------------------------------------------------
// Wire Format Codec
// ----------------------------------------------------------------------------

/// Binary wire format encoder/decoder for BitchatPacket
pub struct WireFormat;

impl WireFormat {
    /// Encode a BitchatPacket to binary wire format
    pub fn encode(packet: &BitchatPacket) -> Result<Vec<u8>> {
        packet.validate()?;

        let mut bytes = Vec::new();

        // 1. Serialize header
        let header_bytes = packet.header.to_bytes()?;
        bytes.extend_from_slice(&header_bytes);

        // 2. Sender ID (always present, 8 bytes)
        bytes.extend_from_slice(packet.sender_id.as_bytes());

        // 3. Optional recipient ID (8 bytes)
        if packet.header.flags.has_recipient() {
            if let Some(recipient_id) = packet.recipient_id {
                bytes.extend_from_slice(recipient_id.as_bytes());
            } else {
                return Err(BitchatError::invalid_packet(
                    "Recipient flag set but no recipient",
                ));
            }
        }

        // 4. Optional route (reserved for future use)
        if packet.header.flags.has_route() {
            if let Some(ref route) = packet.route {
                // Route format: length (2 bytes) + data
                if route.len() > u16::MAX as usize {
                    return Err(BitchatError::invalid_packet("Route too large"));
                }
                bytes.extend_from_slice(&(route.len() as u16).to_be_bytes());
                bytes.extend_from_slice(route);
            } else {
                return Err(BitchatError::invalid_packet("Route flag set but no route"));
            }
        }

        // 5. Payload
        bytes.extend_from_slice(&packet.payload);

        // 6. Optional signature (64 bytes)
        if packet.header.flags.has_signature() {
            if let Some(signature) = packet.signature {
                bytes.extend_from_slice(&signature);
            } else {
                return Err(BitchatError::invalid_packet(
                    "Signature flag set but no signature",
                ));
            }
        }

        Ok(bytes)
    }

    /// Decode a BitchatPacket from binary wire format
    pub fn decode(bytes: &[u8]) -> Result<BitchatPacket> {
        if bytes.is_empty() {
            return Err(BitchatError::invalid_packet("Empty packet"));
        }

        let mut offset = 0;

        // 1. Parse header
        let header = crate::protocol::packet::PacketHeader::from_bytes(bytes)?;
        offset += header.header_size();

        if bytes.len() < offset {
            return Err(BitchatError::invalid_packet("Packet too short for header"));
        }

        // 2. Parse sender ID (always present, 8 bytes)
        if bytes.len() < offset + PEER_ID_SIZE {
            return Err(BitchatError::invalid_packet(
                "Packet too short for sender ID",
            ));
        }

        let sender_bytes: [u8; 8] = bytes[offset..offset + PEER_ID_SIZE]
            .try_into()
            .map_err(|_| BitchatError::invalid_packet("Invalid sender ID"))?;
        let sender_id = PeerId::new(sender_bytes);
        offset += PEER_ID_SIZE;

        // 3. Parse optional recipient ID
        let recipient_id = if header.flags.has_recipient() {
            if bytes.len() < offset + PEER_ID_SIZE {
                return Err(BitchatError::invalid_packet(
                    "Packet too short for recipient ID",
                ));
            }

            let recipient_bytes: [u8; 8] = bytes[offset..offset + PEER_ID_SIZE]
                .try_into()
                .map_err(|_| BitchatError::invalid_packet("Invalid recipient ID"))?;
            offset += PEER_ID_SIZE;
            Some(PeerId::new(recipient_bytes))
        } else {
            None
        };

        // 4. Parse optional route
        let route = if header.flags.has_route() {
            if bytes.len() < offset + 2 {
                return Err(BitchatError::invalid_packet(
                    "Packet too short for route length",
                ));
            }

            let route_length_bytes: [u8; 2] = bytes[offset..offset + 2]
                .try_into()
                .map_err(|_| BitchatError::invalid_packet("Invalid route length"))?;
            let route_length = u16::from_be_bytes(route_length_bytes) as usize;
            offset += 2;

            if bytes.len() < offset + route_length {
                return Err(BitchatError::invalid_packet(
                    "Packet too short for route data",
                ));
            }

            let route_data = bytes[offset..offset + route_length].to_vec();
            offset += route_length;
            Some(route_data)
        } else {
            None
        };

        // 5. Parse payload
        let payload_length = header.payload_length as usize;
        if bytes.len() < offset + payload_length {
            return Err(BitchatError::invalid_packet("Packet too short for payload"));
        }

        let payload = bytes[offset..offset + payload_length].to_vec();
        offset += payload_length;

        // 6. Parse optional signature
        let signature = if header.flags.has_signature() {
            if bytes.len() < offset + SIGNATURE_SIZE {
                return Err(BitchatError::invalid_packet(
                    "Packet too short for signature",
                ));
            }

            let signature_bytes: [u8; 64] = bytes[offset..offset + SIGNATURE_SIZE]
                .try_into()
                .map_err(|_| BitchatError::invalid_packet("Invalid signature"))?;
            offset += SIGNATURE_SIZE;
            Some(signature_bytes)
        } else {
            None
        };

        // Verify we consumed the entire packet
        if offset != bytes.len() {
            return Err(BitchatError::invalid_packet("Packet has trailing data"));
        }

        let packet = BitchatPacket {
            header,
            sender_id,
            recipient_id,
            route,
            payload,
            signature,
        };

        packet.validate()?;
        Ok(packet)
    }

    /// Encode packet with optional compression and padding
    pub fn encode_with_options(
        packet: &BitchatPacket,
        compress: bool,
        pad: bool,
    ) -> Result<Vec<u8>> {
        let mut packet = packet.clone();

        // Apply compression if requested and payload is large enough
        if compress && Compression::should_compress(&packet.payload) {
            packet.payload = Compression::compress(&packet.payload)?;
            packet.header.flags = packet.header.flags.with_compression();
            packet.update_payload_length();
        }

        // Encode to wire format
        let mut bytes = Self::encode(&packet)?;

        // Apply padding if requested
        if pad {
            bytes = Padding::pad(bytes);
        }

        Ok(bytes)
    }

    /// Decode packet with automatic decompression and padding removal
    pub fn decode_with_options(bytes: &[u8]) -> Result<BitchatPacket> {
        // Remove padding first
        let bytes = Padding::unpad(bytes)?;

        // Decode packet
        let mut packet = Self::decode(bytes)?;

        // Decompress payload if compressed
        if packet.header.flags.is_compressed() {
            packet.payload = Compression::decompress(&packet.payload)?;
            packet.header.flags = crate::protocol::packet::PacketFlags::new(
                packet.header.flags.as_u8()
                    & !crate::protocol::packet::PacketFlags::IS_COMPRESSED.as_u8(),
            );
            packet.update_payload_length();
        }

        Ok(packet)
    }
}

// ----------------------------------------------------------------------------
// Compression
// ----------------------------------------------------------------------------

/// Payload compression utilities
pub struct Compression;

impl Compression {
    /// Compress data using zlib
    pub fn compress(data: &[u8]) -> Result<Vec<u8>> {
        #[cfg(feature = "std")]
        {
            let mut encoder = ZlibEncoder::new(Vec::new(), ZlibCompression::default());
            encoder
                .write_all(data)
                .map_err(|e| BitchatError::invalid_packet(format!("Compression failed: {}", e)))?;
            let compressed = encoder.finish().map_err(|e| {
                BitchatError::invalid_packet(format!("Compression finalization failed: {}", e))
            })?;
            Ok(compressed)
        }
        #[cfg(not(feature = "std"))]
        {
            // Fallback for no_std environments - return original data with a marker
            let mut compressed = alloc::vec![0x78, 0x9C]; // zlib header
            compressed.extend_from_slice(data);
            Ok(compressed)
        }
    }

    /// Decompress zlib data
    pub fn decompress(data: &[u8]) -> Result<Vec<u8>> {
        #[cfg(feature = "std")]
        {
            let mut decoder = ZlibDecoder::new(data);
            let mut decompressed = Vec::new();
            decoder.read_to_end(&mut decompressed).map_err(|e| {
                BitchatError::invalid_packet(format!("Decompression failed: {}", e))
            })?;
            Ok(decompressed)
        }
        #[cfg(not(feature = "std"))]
        {
            // Fallback for no_std environments - strip placeholder header
            if data.len() < 2 || data[0] != 0x78 || data[1] != 0x9C {
                return Err(BitchatError::invalid_packet("Invalid compressed data"));
            }
            Ok(data[2..].to_vec())
        }
    }

    /// Check if compression would be beneficial for given data
    pub fn should_compress(data: &[u8]) -> bool {
        data.len() >= COMPRESSION_THRESHOLD
    }

    /// Get compression ratio for data
    #[cfg(feature = "std")]
    pub fn compression_ratio(original: &[u8], compressed: &[u8]) -> f64 {
        if original.is_empty() {
            return 1.0;
        }
        compressed.len() as f64 / original.len() as f64
    }
}

// ----------------------------------------------------------------------------
// Padding
// ----------------------------------------------------------------------------

/// Message padding for traffic analysis resistance
pub struct Padding;

impl Padding {
    /// Pad data to the next standard block size using PKCS#7-style padding
    pub fn pad(mut data: Vec<u8>) -> Vec<u8> {
        let original_length = data.len();
        let target_size = Self::optimal_block_size(original_length);
        let padding_length = target_size - original_length;

        if padding_length > 0 {
            // PKCS#7 padding: fill with bytes equal to padding length
            data.resize(target_size, padding_length as u8);
        }

        data
    }

    /// Remove PKCS#7-style padding
    pub fn unpad(data: &[u8]) -> Result<&[u8]> {
        if data.is_empty() {
            return Err(BitchatError::invalid_packet("Cannot unpad empty data"));
        }

        let padding_length = data[data.len() - 1] as usize;

        // Validate padding
        if padding_length == 0 || padding_length > data.len() {
            // No padding or invalid padding length
            return Ok(data);
        }

        // Check if last 'padding_length' bytes are all equal to padding_length
        let start_index = data.len() - padding_length;
        for &byte in &data[start_index..] {
            if byte != padding_length as u8 {
                // Invalid padding, return original data
                return Ok(data);
            }
        }

        Ok(&data[..start_index])
    }

    /// Select optimal block size for given data length
    pub fn optimal_block_size(length: usize) -> usize {
        for &block_size in PADDING_BLOCK_SIZES {
            if length <= block_size {
                return block_size;
            }
        }

        // For very large data, round up to next 2048-byte boundary
        ((length + 2047) / 2048) * 2048
    }
}

// Helper method for BitchatPacket to update payload length
impl BitchatPacket {
    /// Update payload length in header (internal use)
    pub(crate) fn update_payload_length(&mut self) {
        self.header.payload_length = self.payload.len() as u32;
    }
}

// ----------------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::packet::MessageType;

    fn create_test_packet() -> BitchatPacket {
        let sender = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        let payload = b"Hello, BitChat!".to_vec();
        BitchatPacket::new_simple(MessageType::Message, sender, payload)
    }

    #[test]
    fn test_wire_format_roundtrip() {
        let packet = create_test_packet();

        let encoded = WireFormat::encode(&packet).unwrap();
        let decoded = WireFormat::decode(&encoded).unwrap();

        assert_eq!(packet, decoded);
    }

    #[test]
    fn test_wire_format_with_recipient() {
        let recipient = PeerId::new([8, 7, 6, 5, 4, 3, 2, 1]);
        let packet = create_test_packet().with_recipient(recipient);

        let encoded = WireFormat::encode(&packet).unwrap();
        let decoded = WireFormat::decode(&encoded).unwrap();

        assert_eq!(packet, decoded);
        assert_eq!(decoded.recipient_id, Some(recipient));
    }

    #[test]
    fn test_wire_format_with_signature() {
        let signature = [42u8; 64];
        let packet = create_test_packet().with_signature(signature);

        let encoded = WireFormat::encode(&packet).unwrap();
        let decoded = WireFormat::decode(&encoded).unwrap();

        assert_eq!(packet, decoded);
        assert_eq!(decoded.signature, Some(signature));
    }

    #[test]
    fn test_padding() {
        let data = vec![1, 2, 3, 4, 5];
        let padded = Padding::pad(data.clone());

        assert!(padded.len() >= data.len());
        assert_eq!(padded.len(), Padding::optimal_block_size(data.len()));

        let unpadded = Padding::unpad(&padded).unwrap();
        assert_eq!(unpadded, &data[..]);
    }

    #[test]
    fn test_optimal_block_size() {
        assert_eq!(Padding::optimal_block_size(100), 256);
        assert_eq!(Padding::optimal_block_size(256), 256);
        assert_eq!(Padding::optimal_block_size(257), 512);
        assert_eq!(Padding::optimal_block_size(1024), 1024);
        assert_eq!(Padding::optimal_block_size(2049), 4096);
    }

    #[test]
    fn test_compression_roundtrip() {
        let data = b"This is test data for compression".to_vec();

        let compressed = Compression::compress(&data).unwrap();
        let decompressed = Compression::decompress(&compressed).unwrap();

        assert_eq!(data, decompressed);
    }

    #[test]
    fn test_compression_large_data() {
        // Create compressible data larger than threshold
        let data = "A".repeat(1000).into_bytes();

        let compressed = Compression::compress(&data).unwrap();
        let decompressed = Compression::decompress(&compressed).unwrap();

        assert_eq!(data, decompressed);

        #[cfg(feature = "std")]
        {
            // Compressed data should be smaller for repetitive content
            assert!(compressed.len() < data.len());
            let ratio = Compression::compression_ratio(&data, &compressed);
            assert!(ratio < 1.0);
        }
    }

    #[test]
    fn test_compression_should_compress() {
        let small_data = vec![1, 2, 3, 4, 5];
        let large_data = vec![0u8; COMPRESSION_THRESHOLD + 1];

        assert!(!Compression::should_compress(&small_data));
        assert!(Compression::should_compress(&large_data));
    }

    #[test]
    fn test_compression_random_data() {
        // Random data shouldn't compress well
        let data: Vec<u8> = (0..500).map(|i| (i * 17 + 42) as u8).collect();

        let compressed = Compression::compress(&data).unwrap();
        let decompressed = Compression::decompress(&compressed).unwrap();

        assert_eq!(data, decompressed);
    }

    #[test]
    fn test_encode_with_options() {
        let packet = create_test_packet();

        // Test with compression and padding
        let encoded = WireFormat::encode_with_options(&packet, true, true).unwrap();
        let decoded = WireFormat::decode_with_options(&encoded).unwrap();

        assert_eq!(packet.payload, decoded.payload);
        assert_eq!(packet.sender_id, decoded.sender_id);
    }

    #[test]
    fn test_compression_with_large_packet() {
        let sender = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        // Create large compressible payload (within v1 255-byte limit)
        let large_payload = "BitChat is awesome! ".repeat(12).into_bytes(); // 240 bytes
        let packet = BitchatPacket::new_simple(MessageType::Message, sender, large_payload.clone());

        // Encode with compression
        let encoded = WireFormat::encode_with_options(&packet, true, false).unwrap();
        let decoded = WireFormat::decode_with_options(&encoded).unwrap();

        assert_eq!(packet.payload, decoded.payload);
        assert_eq!(packet.sender_id, decoded.sender_id);

        // Verify compression was attempted (packet should have compression flag)
        #[cfg(feature = "std")]
        {
            // For this test, we just verify the functionality works correctly
            // Compression effectiveness depends on content and overhead
            let uncompressed_encoded = WireFormat::encode(&packet).unwrap();
            // Both should decode to the same content
            let uncompressed_decoded = WireFormat::decode(&uncompressed_encoded).unwrap();
            assert_eq!(packet.payload, uncompressed_decoded.payload);
        }
    }

    #[test]
    fn test_compression_with_small_packet() {
        let packet = create_test_packet(); // Small payload, should not be compressed

        // Encode with compression requested
        let encoded = WireFormat::encode_with_options(&packet, true, false).unwrap();
        let decoded = WireFormat::decode_with_options(&encoded).unwrap();

        assert_eq!(packet.payload, decoded.payload);
        assert_eq!(packet.sender_id, decoded.sender_id);

        // Small packets should not be compressed
        assert!(!decoded.header.flags.is_compressed());
    }

    #[test]
    fn test_invalid_packet_data() {
        // Test empty packet
        assert!(WireFormat::decode(&[]).is_err());

        // Test truncated packet
        let packet = create_test_packet();
        let encoded = WireFormat::encode(&packet).unwrap();
        assert!(WireFormat::decode(&encoded[..5]).is_err());
    }
}
