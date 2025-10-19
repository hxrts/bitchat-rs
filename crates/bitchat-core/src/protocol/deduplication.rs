//! Message deduplication using Bloom filters
//!
//! This module provides efficient duplicate detection for mesh networking
//! using probabilistic Bloom filters to prevent message forwarding loops.

use alloc::vec::Vec;
use core::hash::Hash;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::types::{PeerId, Timestamp};
// BitchatError and Result used in tests only

// ----------------------------------------------------------------------------
// Constants
// ----------------------------------------------------------------------------

/// Default number of hash functions for the Bloom filter
pub const DEFAULT_HASH_FUNCTIONS: usize = 3;

/// Default bit array size for the Bloom filter (8KB)
pub const DEFAULT_BLOOM_SIZE: usize = 65536; // 64K bits = 8KB

/// Time-to-live for Bloom filter entries (5 minutes)
pub const BLOOM_TTL_MS: u64 = 300_000;

/// Maximum packet ID size for deduplication
pub const MAX_PACKET_ID_SIZE: usize = 64;

// ----------------------------------------------------------------------------
// Packet ID Generation
// ----------------------------------------------------------------------------

/// Unique identifier for packets used in deduplication
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PacketId(pub [u8; 32]);

impl PacketId {
    /// Create a packet ID from sender, timestamp, and content hash
    pub fn new(sender: PeerId, timestamp: u64, content_hash: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(sender.as_bytes());
        hasher.update(timestamp.to_be_bytes());
        hasher.update(content_hash);

        let result = hasher.finalize();
        let mut packet_id = [0u8; 32];
        packet_id.copy_from_slice(&result);

        Self(packet_id)
    }

    /// Create a packet ID from raw packet data
    pub fn from_packet_data(sender: PeerId, timestamp: u64, packet_data: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(packet_data);
        let content_hash = hasher.finalize();

        Self::new(sender, timestamp, &content_hash)
    }

    /// Get the raw bytes of the packet ID
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Generate hash for Bloom filter with given seed
    pub fn hash_with_seed(&self, seed: u32) -> u64 {
        let mut hasher = Sha256::new();
        hasher.update(seed.to_be_bytes());
        hasher.update(self.0);

        let result = hasher.finalize();
        u64::from_be_bytes([
            result[0], result[1], result[2], result[3], result[4], result[5], result[6], result[7],
        ])
    }
}

// ----------------------------------------------------------------------------
// Bloom Filter Implementation
// ----------------------------------------------------------------------------

/// Probabilistic data structure for efficient duplicate detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BloomFilter {
    /// Bit array for the Bloom filter
    bits: Vec<u8>,
    /// Number of hash functions
    hash_functions: usize,
    /// Total size in bits
    bit_size: usize,
    /// Creation timestamp for aging
    created_at: Timestamp,
}

impl BloomFilter {
    /// Create a new Bloom filter with specified parameters
    pub fn new(bit_size: usize, hash_functions: usize) -> Self {
        let byte_size = (bit_size + 7) / 8; // Round up to nearest byte
        Self {
            bits: vec![0u8; byte_size],
            hash_functions,
            bit_size,
            created_at: Timestamp::now(),
        }
    }

    /// Create a Bloom filter with default parameters
    pub fn with_default_parameters() -> Self {
        Self::new(DEFAULT_BLOOM_SIZE, DEFAULT_HASH_FUNCTIONS)
    }

    /// Create a Bloom filter optimized for mesh networking
    pub fn for_mesh_routing(expected_elements: usize, false_positive_rate: f64) -> Self {
        let bit_size = Self::optimal_bit_size(expected_elements, false_positive_rate);
        let hash_functions = Self::optimal_hash_functions(bit_size, expected_elements);
        Self::new(bit_size, hash_functions)
    }

    /// Calculate optimal bit array size for given parameters
    fn optimal_bit_size(expected_elements: usize, false_positive_rate: f64) -> usize {
        let n = expected_elements as f64;
        let p = false_positive_rate;
        let m = -(n * p.ln()) / (2.0_f64.ln().powi(2));
        m.ceil() as usize
    }

    /// Calculate optimal number of hash functions
    fn optimal_hash_functions(bit_size: usize, expected_elements: usize) -> usize {
        let m = bit_size as f64;
        let n = expected_elements as f64;
        let k = (m / n) * 2.0_f64.ln();
        k.round() as usize
    }

    /// Add a packet ID to the Bloom filter
    pub fn add(&mut self, packet_id: &PacketId) {
        for i in 0..self.hash_functions {
            let hash = packet_id.hash_with_seed(i as u32);
            let bit_index = (hash as usize) % self.bit_size;
            self.set_bit(bit_index);
        }
    }

    /// Check if a packet ID might be in the Bloom filter
    /// Returns true if possibly present, false if definitely not present
    pub fn contains(&self, packet_id: &PacketId) -> bool {
        for i in 0..self.hash_functions {
            let hash = packet_id.hash_with_seed(i as u32);
            let bit_index = (hash as usize) % self.bit_size;
            if !self.get_bit(bit_index) {
                return false;
            }
        }
        true
    }

    /// Set a bit at the specified index
    fn set_bit(&mut self, bit_index: usize) {
        if bit_index < self.bit_size {
            let byte_index = bit_index / 8;
            let bit_offset = bit_index % 8;
            self.bits[byte_index] |= 1u8 << bit_offset;
        }
    }

    /// Get a bit at the specified index
    fn get_bit(&self, bit_index: usize) -> bool {
        if bit_index < self.bit_size {
            let byte_index = bit_index / 8;
            let bit_offset = bit_index % 8;
            (self.bits[byte_index] & (1u8 << bit_offset)) != 0
        } else {
            false
        }
    }

    /// Check if the Bloom filter has expired
    pub fn is_expired(&self) -> bool {
        let current_time = Timestamp::now();
        current_time
            .as_millis()
            .saturating_sub(self.created_at.as_millis())
            > BLOOM_TTL_MS
    }

    /// Clear all bits in the Bloom filter
    pub fn clear(&mut self) {
        self.bits.fill(0);
        self.created_at = Timestamp::now();
    }

    /// Get the estimated false positive rate
    pub fn estimated_false_positive_rate(&self, elements_added: usize) -> f64 {
        let m = self.bit_size as f64;
        let k = self.hash_functions as f64;
        let n = elements_added as f64;

        let exponent = -(k * n) / m;
        (1.0 - exponent.exp()).powf(k)
    }

    /// Get memory usage in bytes
    pub fn memory_usage(&self) -> usize {
        self.bits.len()
    }

    /// Get the fill ratio (percentage of bits set)
    pub fn fill_ratio(&self) -> f64 {
        let set_bits = self
            .bits
            .iter()
            .map(|byte| byte.count_ones() as usize)
            .sum::<usize>();
        (set_bits as f64) / (self.bit_size as f64)
    }
}

impl Default for BloomFilter {
    fn default() -> Self {
        Self::new(DEFAULT_BLOOM_SIZE, DEFAULT_HASH_FUNCTIONS)
    }
}

// ----------------------------------------------------------------------------
// Deduplication Manager
// ----------------------------------------------------------------------------

/// Statistics for deduplication performance
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DeduplicationStats {
    /// Total packets processed
    pub packets_processed: u64,
    /// Packets detected as duplicates
    pub duplicates_detected: u64,
    /// False positive detections (if known)
    pub false_positives: u64,
    /// Number of Bloom filter rotations
    pub filter_rotations: u64,
}

impl DeduplicationStats {
    /// Calculate duplicate detection rate
    pub fn duplicate_rate(&self) -> f64 {
        if self.packets_processed == 0 {
            0.0
        } else {
            (self.duplicates_detected as f64) / (self.packets_processed as f64)
        }
    }

    /// Calculate false positive rate
    pub fn false_positive_rate(&self) -> f64 {
        if self.duplicates_detected == 0 {
            0.0
        } else {
            (self.false_positives as f64) / (self.duplicates_detected as f64)
        }
    }
}

/// Manages deduplication of packets using rotating Bloom filters
pub struct DeduplicationManager {
    /// Primary Bloom filter for recent packets
    primary_filter: BloomFilter,
    /// Secondary Bloom filter for aging out old packets
    secondary_filter: Option<BloomFilter>,
    /// Configuration parameters
    expected_packets_per_period: usize,
    false_positive_rate: f64,
    /// Performance statistics
    stats: DeduplicationStats,
}

impl DeduplicationManager {
    /// Create a new deduplication manager
    pub fn new(expected_packets_per_period: usize, false_positive_rate: f64) -> Self {
        let primary_filter =
            BloomFilter::for_mesh_routing(expected_packets_per_period, false_positive_rate);

        Self {
            primary_filter,
            secondary_filter: None,
            expected_packets_per_period,
            false_positive_rate,
            stats: DeduplicationStats::default(),
        }
    }

    /// Create a deduplication manager with default parameters
    pub fn with_default_parameters() -> Self {
        Self::new(10000, 0.01) // 10K packets per period, 1% false positive rate
    }

    /// Create a deduplication manager optimized for BLE mesh networking
    pub fn for_ble_mesh() -> Self {
        Self::new(1000, 0.005) // 1K packets per period, 0.5% false positive rate
    }

    /// Check if a packet is a duplicate and add it to the filter
    /// Returns true if this is a duplicate packet
    pub fn check_and_add(&mut self, packet_id: PacketId) -> bool {
        self.stats.packets_processed += 1;

        // Check primary filter first
        let is_duplicate = self.primary_filter.contains(&packet_id);

        // Check secondary filter if it exists
        let is_duplicate = is_duplicate
            || self
                .secondary_filter
                .as_ref()
                .map(|filter| filter.contains(&packet_id))
                .unwrap_or(false);

        if is_duplicate {
            self.stats.duplicates_detected += 1;
        }

        // Add to primary filter regardless
        self.primary_filter.add(&packet_id);

        // Rotate filters if primary is getting full or expired
        if self.should_rotate_filters() {
            self.rotate_filters();
        }

        is_duplicate
    }

    /// Check if a packet is a duplicate without adding it
    pub fn is_duplicate(&self, packet_id: &PacketId) -> bool {
        let in_primary = self.primary_filter.contains(packet_id);
        let in_secondary = self
            .secondary_filter
            .as_ref()
            .map(|filter| filter.contains(packet_id))
            .unwrap_or(false);

        in_primary || in_secondary
    }

    /// Force rotation of Bloom filters
    pub fn rotate_filters(&mut self) {
        self.stats.filter_rotations += 1;

        // Move primary to secondary, create new primary
        let new_primary = BloomFilter::for_mesh_routing(
            self.expected_packets_per_period,
            self.false_positive_rate,
        );

        let old_primary = core::mem::replace(&mut self.primary_filter, new_primary);
        self.secondary_filter = Some(old_primary);
    }

    /// Check if filters should be rotated
    fn should_rotate_filters(&self) -> bool {
        // Rotate if primary filter is expired
        if self.primary_filter.is_expired() {
            return true;
        }

        // Rotate if primary filter is getting too full (>70% bits set)
        if self.primary_filter.fill_ratio() > 0.7 {
            return true;
        }

        false
    }

    /// Perform periodic maintenance
    pub fn maintain(&mut self) {
        // Remove expired secondary filter
        if let Some(ref secondary) = self.secondary_filter {
            if secondary.is_expired() {
                self.secondary_filter = None;
            }
        }

        // Rotate if needed
        if self.should_rotate_filters() {
            self.rotate_filters();
        }
    }

    /// Get deduplication statistics
    pub fn stats(&self) -> &DeduplicationStats {
        &self.stats
    }

    /// Get estimated memory usage in bytes
    pub fn memory_usage(&self) -> usize {
        let primary_usage = self.primary_filter.memory_usage();
        let secondary_usage = self
            .secondary_filter
            .as_ref()
            .map(|f| f.memory_usage())
            .unwrap_or(0);
        primary_usage + secondary_usage
    }

    /// Clear all filters and reset statistics
    pub fn clear(&mut self) {
        self.primary_filter.clear();
        self.secondary_filter = None;
        self.stats = DeduplicationStats::default();
    }
}

impl Default for DeduplicationManager {
    fn default() -> Self {
        Self::new(10000, 0.01) // 10K packets per period, 1% false positive rate
    }
}

// ----------------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_peer_id(id: u8) -> PeerId {
        PeerId::new([id, 0, 0, 0, 0, 0, 0, 0])
    }

    #[test]
    fn test_packet_id_generation() {
        let sender = create_test_peer_id(1);
        let timestamp = 1234567890;
        let content = b"test content";

        let id1 = PacketId::from_packet_data(sender, timestamp, content);
        let id2 = PacketId::from_packet_data(sender, timestamp, content);

        // Same inputs should produce same ID
        assert_eq!(id1, id2);

        // Different content should produce different ID
        let id3 = PacketId::from_packet_data(sender, timestamp, b"different content");
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_bloom_filter_basic_operations() {
        let mut filter = BloomFilter::new(1000, 3);
        let sender = create_test_peer_id(1);

        let packet_id1 = PacketId::from_packet_data(sender, 1000, b"packet 1");
        let packet_id2 = PacketId::from_packet_data(sender, 2000, b"packet 2");

        // Should not contain packets initially
        assert!(!filter.contains(&packet_id1));
        assert!(!filter.contains(&packet_id2));

        // Add first packet
        filter.add(&packet_id1);
        assert!(filter.contains(&packet_id1));
        assert!(!filter.contains(&packet_id2));

        // Add second packet
        filter.add(&packet_id2);
        assert!(filter.contains(&packet_id1));
        assert!(filter.contains(&packet_id2));
    }

    #[test]
    fn test_bloom_filter_optimal_parameters() {
        let filter = BloomFilter::for_mesh_routing(1000, 0.01);
        assert!(filter.bit_size > 0);
        assert!(filter.hash_functions > 0);
        assert!(filter.hash_functions <= 10); // Reasonable upper bound
    }

    #[test]
    fn test_deduplication_manager() {
        let mut manager = DeduplicationManager::new(100, 0.01);
        let sender = create_test_peer_id(1);

        let packet_id1 = PacketId::from_packet_data(sender, 1000, b"packet 1");
        let packet_id2 = PacketId::from_packet_data(sender, 2000, b"packet 2");

        // First time should not be duplicate
        assert!(!manager.check_and_add(packet_id1.clone()));
        assert_eq!(manager.stats().packets_processed, 1);
        assert_eq!(manager.stats().duplicates_detected, 0);

        // Second time should be duplicate
        assert!(manager.check_and_add(packet_id1));
        assert_eq!(manager.stats().packets_processed, 2);
        assert_eq!(manager.stats().duplicates_detected, 1);

        // Different packet should not be duplicate
        assert!(!manager.check_and_add(packet_id2));
        assert_eq!(manager.stats().packets_processed, 3);
        assert_eq!(manager.stats().duplicates_detected, 1);
    }

    #[test]
    fn test_filter_rotation() {
        let mut manager = DeduplicationManager::new(10, 0.01);
        let sender = create_test_peer_id(1);

        // Fill up the primary filter
        for i in 0..50 {
            let packet_id = PacketId::from_packet_data(sender, i, &[i as u8]);
            manager.check_and_add(packet_id);
        }

        let initial_rotations = manager.stats().filter_rotations;

        // Force rotation
        manager.rotate_filters();
        assert_eq!(manager.stats().filter_rotations, initial_rotations + 1);
        assert!(manager.secondary_filter.is_some());
    }

    #[test]
    fn test_deduplication_stats() {
        let stats = DeduplicationStats {
            packets_processed: 100,
            duplicates_detected: 10,
            false_positives: 1,
            filter_rotations: 2,
        };

        assert_eq!(stats.duplicate_rate(), 0.1);
        assert_eq!(stats.false_positive_rate(), 0.1);
    }

    #[test]
    fn test_bloom_filter_memory_usage() {
        let filter = BloomFilter::new(8192, 3); // 8K bits = 1KB
        assert_eq!(filter.memory_usage(), 1024);
    }

    #[test]
    fn test_deduplication_manager_for_ble() {
        let manager = DeduplicationManager::for_ble_mesh();
        assert_eq!(manager.expected_packets_per_period, 1000);
        assert_eq!(manager.false_positive_rate, 0.005);
    }
}
