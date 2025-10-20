use alloc::{vec::Vec, string::String};
use core::convert::TryInto;
use serde::{Deserialize, Serialize};
use crate::errors::BitchatError;

/// TLV (Type-Length-Value) encoding types for BitChat protocol
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TlvType {
    Nickname = 0x01,
    NoisePublicKey = 0x02,
    SigningPublicKey = 0x03,
    DirectNeighbors = 0x04,
}

impl TlvType {
    pub fn from_u8(value: u8) -> Result<Self, BitchatError> {
        match value {
            0x01 => Ok(TlvType::Nickname),
            0x02 => Ok(TlvType::NoisePublicKey),
            0x03 => Ok(TlvType::SigningPublicKey),
            0x04 => Ok(TlvType::DirectNeighbors),
            _ => Err(BitchatError::InvalidTlvType(value)),
        }
    }
}

/// A single TLV entry
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlvEntry {
    pub tlv_type: TlvType,
    pub value: Vec<u8>,
}

impl TlvEntry {
    /// Create a new TLV entry
    pub fn new(tlv_type: TlvType, value: Vec<u8>) -> Self {
        Self { tlv_type, value }
    }

    /// Create a TLV entry for a nickname (UTF-8 string)
    pub fn nickname(nickname: &str) -> Result<Self, BitchatError> {
        let bytes = nickname.as_bytes();
        if bytes.len() > 255 {
            return Err(BitchatError::TlvValueTooLarge(bytes.len()));
        }
        Ok(Self::new(TlvType::Nickname, bytes.to_vec()))
    }

    /// Create a TLV entry for a Noise public key (32 bytes)
    pub fn noise_public_key(key: &[u8; 32]) -> Self {
        Self::new(TlvType::NoisePublicKey, key.to_vec())
    }

    /// Create a TLV entry for a signing public key (32 bytes)
    pub fn signing_public_key(key: &[u8; 32]) -> Self {
        Self::new(TlvType::SigningPublicKey, key.to_vec())
    }

    /// Create a TLV entry for direct neighbors (peer IDs)
    pub fn direct_neighbors(neighbors: &[&[u8; 32]]) -> Self {
        let mut value = Vec::new();
        for neighbor in neighbors {
            value.extend_from_slice(*neighbor);
        }
        Self::new(TlvType::DirectNeighbors, value)
    }

    /// Get the nickname as a UTF-8 string
    pub fn as_nickname(&self) -> Result<String, BitchatError> {
        if self.tlv_type != TlvType::Nickname {
            return Err(BitchatError::InvalidTlvType(self.tlv_type as u8));
        }
        String::from_utf8(self.value.clone())
            .map_err(|_| BitchatError::InvalidUtf8)
    }

    /// Get the value as a 32-byte key
    pub fn as_key(&self) -> Result<[u8; 32], BitchatError> {
        if self.value.len() != 32 {
            return Err(BitchatError::InvalidKeyLength(self.value.len()));
        }
        self.value.as_slice().try_into()
            .map_err(|_| BitchatError::InvalidKeyLength(self.value.len()))
    }

    /// Get direct neighbors as a list of peer IDs
    pub fn as_neighbors(&self) -> Result<Vec<[u8; 32]>, BitchatError> {
        if self.tlv_type != TlvType::DirectNeighbors {
            return Err(BitchatError::InvalidTlvType(self.tlv_type as u8));
        }
        
        if self.value.len() % 32 != 0 {
            return Err(BitchatError::InvalidDataLength(self.value.len()));
        }

        let mut neighbors = Vec::new();
        for chunk in self.value.chunks_exact(32) {
            let peer_id: [u8; 32] = chunk.try_into()
                .map_err(|_| BitchatError::InvalidKeyLength(chunk.len()))?;
            neighbors.push(peer_id);
        }
        Ok(neighbors)
    }

    /// Encode this TLV entry to bytes
    pub fn encode(&self) -> Vec<u8> {
        let mut encoded = Vec::new();
        
        // Type (1 byte)
        encoded.push(self.tlv_type as u8);
        
        // Length (2 bytes, big-endian)
        let length = self.value.len() as u16;
        encoded.extend_from_slice(&length.to_be_bytes());
        
        // Value
        encoded.extend_from_slice(&self.value);
        
        encoded
    }

    /// Decode a TLV entry from bytes
    pub fn decode(data: &[u8]) -> Result<(Self, usize), BitchatError> {
        if data.len() < 3 {
            return Err(BitchatError::InsufficientData(data.len()));
        }

        // Parse type
        let tlv_type = TlvType::from_u8(data[0])?;
        
        // Parse length (2 bytes, big-endian)
        let length = u16::from_be_bytes([data[1], data[2]]) as usize;
        
        // Check if we have enough data for the value
        let total_length = 3 + length;
        if data.len() < total_length {
            return Err(BitchatError::InsufficientData(data.len()));
        }
        
        // Extract value
        let value = data[3..3 + length].to_vec();
        
        let entry = TlvEntry::new(tlv_type, value);
        Ok((entry, total_length))
    }
}

/// TLV codec for encoding/decoding multiple TLV entries
#[derive(Debug, Clone, Default)]
pub struct TlvCodec {
    entries: Vec<TlvEntry>,
}

impl TlvCodec {
    /// Create a new empty TLV codec
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a TLV entry
    pub fn add_entry(&mut self, entry: TlvEntry) {
        self.entries.push(entry);
    }

    /// Get all entries
    pub fn entries(&self) -> &[TlvEntry] {
        &self.entries
    }

    /// Find the first entry of a specific type
    pub fn find_entry(&self, tlv_type: TlvType) -> Option<&TlvEntry> {
        self.entries.iter().find(|entry| entry.tlv_type == tlv_type)
    }

    /// Encode all TLV entries to bytes
    pub fn encode(&self) -> Vec<u8> {
        let mut encoded = Vec::new();
        for entry in &self.entries {
            encoded.extend_from_slice(&entry.encode());
        }
        encoded
    }

    /// Decode TLV entries from bytes
    pub fn decode(data: &[u8]) -> Result<Self, BitchatError> {
        let mut codec = TlvCodec::new();
        let mut offset = 0;

        while offset < data.len() {
            let (entry, bytes_consumed) = TlvEntry::decode(&data[offset..])?;
            codec.add_entry(entry);
            offset += bytes_consumed;
        }

        Ok(codec)
    }

    /// Validate that required TLV types are present
    pub fn validate_required(&self, required_types: &[TlvType]) -> Result<(), BitchatError> {
        for required_type in required_types {
            if self.find_entry(*required_type).is_none() {
                return Err(BitchatError::MissingRequiredTlv(*required_type as u8));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tlv_type_conversion() {
        assert_eq!(TlvType::from_u8(0x01).unwrap(), TlvType::Nickname);
        assert_eq!(TlvType::from_u8(0x02).unwrap(), TlvType::NoisePublicKey);
        assert_eq!(TlvType::from_u8(0x03).unwrap(), TlvType::SigningPublicKey);
        assert_eq!(TlvType::from_u8(0x04).unwrap(), TlvType::DirectNeighbors);
        assert!(TlvType::from_u8(0xFF).is_err());
    }

    #[test]
    fn test_nickname_entry() {
        let entry = TlvEntry::nickname("alice").unwrap();
        assert_eq!(entry.tlv_type, TlvType::Nickname);
        assert_eq!(entry.as_nickname().unwrap(), "alice");
    }

    #[test]
    fn test_key_entries() {
        let key = [0x42; 32];
        
        let noise_entry = TlvEntry::noise_public_key(&key);
        assert_eq!(noise_entry.tlv_type, TlvType::NoisePublicKey);
        assert_eq!(noise_entry.as_key().unwrap(), key);
        
        let signing_entry = TlvEntry::signing_public_key(&key);
        assert_eq!(signing_entry.tlv_type, TlvType::SigningPublicKey);
        assert_eq!(signing_entry.as_key().unwrap(), key);
    }

    #[test]
    fn test_neighbors_entry() {
        let neighbor1 = [0x11; 32];
        let neighbor2 = [0x22; 32];
        let neighbors = [&neighbor1, &neighbor2];
        
        let entry = TlvEntry::direct_neighbors(&neighbors);
        assert_eq!(entry.tlv_type, TlvType::DirectNeighbors);
        
        let decoded_neighbors = entry.as_neighbors().unwrap();
        assert_eq!(decoded_neighbors.len(), 2);
        assert_eq!(decoded_neighbors[0], neighbor1);
        assert_eq!(decoded_neighbors[1], neighbor2);
    }

    #[test]
    fn test_tlv_entry_encode_decode() {
        let entry = TlvEntry::nickname("test").unwrap();
        let encoded = entry.encode();
        
        let (decoded_entry, bytes_consumed) = TlvEntry::decode(&encoded).unwrap();
        assert_eq!(bytes_consumed, encoded.len());
        assert_eq!(decoded_entry, entry);
        assert_eq!(decoded_entry.as_nickname().unwrap(), "test");
    }

    #[test]
    fn test_tlv_codec_multiple_entries() {
        let mut codec = TlvCodec::new();
        
        codec.add_entry(TlvEntry::nickname("alice").unwrap());
        codec.add_entry(TlvEntry::noise_public_key(&[0x42; 32]));
        codec.add_entry(TlvEntry::signing_public_key(&[0x43; 32]));
        
        let encoded = codec.encode();
        let decoded_codec = TlvCodec::decode(&encoded).unwrap();
        
        assert_eq!(decoded_codec.entries().len(), 3);
        
        let nickname_entry = decoded_codec.find_entry(TlvType::Nickname).unwrap();
        assert_eq!(nickname_entry.as_nickname().unwrap(), "alice");
        
        let noise_entry = decoded_codec.find_entry(TlvType::NoisePublicKey).unwrap();
        assert_eq!(noise_entry.as_key().unwrap(), [0x42; 32]);
    }

    #[test]
    fn test_validation() {
        let mut codec = TlvCodec::new();
        codec.add_entry(TlvEntry::nickname("alice").unwrap());
        codec.add_entry(TlvEntry::noise_public_key(&[0x42; 32]));
        
        let required = [TlvType::Nickname, TlvType::NoisePublicKey];
        assert!(codec.validate_required(&required).is_ok());
        
        let required_with_missing = [TlvType::Nickname, TlvType::SigningPublicKey];
        assert!(codec.validate_required(&required_with_missing).is_err());
    }
}