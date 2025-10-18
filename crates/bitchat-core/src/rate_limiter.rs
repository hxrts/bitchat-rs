//! Rate limiting for BitChat transports
//!
//! This module provides rate limiting functionality to prevent DoS attacks
//! and manage resource usage in BitChat transports.

use alloc::collections::BTreeMap;

use crate::types::{PeerId, TimeSource, Timestamp};
use crate::{BitchatError, Result};

// ----------------------------------------------------------------------------
// Rate Limiting Configuration
// ----------------------------------------------------------------------------

/// Configuration for rate limiting
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum messages per peer per time window
    pub max_messages_per_peer: u32,
    /// Maximum connections per peer per time window  
    pub max_connections_per_peer: u32,
    /// Time window for rate limiting (in milliseconds)
    pub time_window_ms: u64,
    /// Maximum total incoming messages per time window
    pub max_total_messages: u32,
    /// Maximum total connections per time window
    pub max_total_connections: u32,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_messages_per_peer: 10,   // 10 messages per peer per minute
            max_connections_per_peer: 3, // 3 connections per peer per minute
            time_window_ms: 60_000,      // 1 minute window
            max_total_messages: 100,     // 100 total messages per minute
            max_total_connections: 20,   // 20 total connections per minute
        }
    }
}

// ----------------------------------------------------------------------------
// Rate Limiter State
// ----------------------------------------------------------------------------

/// Tracks activity for a specific peer
#[derive(Debug, Clone)]
struct PeerActivity {
    /// Message timestamps within current window
    message_times: Vec<Timestamp>,
    /// Connection timestamps within current window
    connection_times: Vec<Timestamp>,
}

impl PeerActivity {
    fn new() -> Self {
        Self {
            message_times: Vec::new(),
            connection_times: Vec::new(),
        }
    }

    /// Clean up old entries outside the time window
    fn cleanup<T: TimeSource>(&mut self, time_source: &T, window_ms: u64) {
        let now = time_source.now();
        let cutoff_time = now.as_millis().saturating_sub(window_ms);

        self.message_times
            .retain(|&ts| ts.as_millis() >= cutoff_time);
        self.connection_times
            .retain(|&ts| ts.as_millis() >= cutoff_time);
    }

    /// Add a message timestamp
    fn add_message<T: TimeSource>(&mut self, time_source: &T) {
        self.message_times.push(time_source.now());
    }

    /// Add a connection timestamp
    fn add_connection<T: TimeSource>(&mut self, time_source: &T) {
        self.connection_times.push(time_source.now());
    }

    /// Get message count in current window
    fn message_count(&self) -> u32 {
        self.message_times.len() as u32
    }

    /// Get connection count in current window
    fn connection_count(&self) -> u32 {
        self.connection_times.len() as u32
    }
}

// ----------------------------------------------------------------------------
// Rate Limiter
// ----------------------------------------------------------------------------

/// Rate limiter for transport operations
pub struct RateLimiter<T: TimeSource> {
    config: RateLimitConfig,
    peer_activity: BTreeMap<PeerId, PeerActivity>,
    time_source: T,
    total_messages: Vec<Timestamp>,
    total_connections: Vec<Timestamp>,
}

impl<T: TimeSource> RateLimiter<T> {
    /// Create a new rate limiter with default configuration
    pub fn new(time_source: T) -> Self {
        Self::with_config(RateLimitConfig::default(), time_source)
    }

    /// Create a new rate limiter with custom configuration
    pub fn with_config(config: RateLimitConfig, time_source: T) -> Self {
        Self {
            config,
            peer_activity: BTreeMap::new(),
            time_source,
            total_messages: Vec::new(),
            total_connections: Vec::new(),
        }
    }

    /// Check if a message from a peer should be allowed
    pub fn check_message_allowed(&mut self, peer_id: &PeerId) -> Result<()> {
        self.cleanup_expired();

        // Check total message limit
        if self.total_messages.len() as u32 >= self.config.max_total_messages {
            return Err(BitchatError::InvalidPacket(
                "Global message rate limit exceeded".into(),
            ));
        }

        // Check per-peer message limit
        let activity = self
            .peer_activity
            .entry(*peer_id)
            .or_insert_with(PeerActivity::new);
        if activity.message_count() >= self.config.max_messages_per_peer {
            return Err(BitchatError::InvalidPacket(
                "Per-peer message rate limit exceeded".into(),
            ));
        }

        Ok(())
    }

    /// Record a message from a peer (call after check_message_allowed succeeds)
    pub fn record_message(&mut self, peer_id: &PeerId) {
        self.total_messages.push(self.time_source.now());

        let activity = self
            .peer_activity
            .entry(*peer_id)
            .or_insert_with(PeerActivity::new);
        activity.add_message(&self.time_source);
    }

    /// Check if a connection from a peer should be allowed
    pub fn check_connection_allowed(&mut self, peer_id: &PeerId) -> Result<()> {
        self.cleanup_expired();

        // Check total connection limit
        if self.total_connections.len() as u32 >= self.config.max_total_connections {
            return Err(BitchatError::InvalidPacket(
                "Global connection rate limit exceeded".into(),
            ));
        }

        // Check per-peer connection limit
        let activity = self
            .peer_activity
            .entry(*peer_id)
            .or_insert_with(PeerActivity::new);
        if activity.connection_count() >= self.config.max_connections_per_peer {
            return Err(BitchatError::InvalidPacket(
                "Per-peer connection rate limit exceeded".into(),
            ));
        }

        Ok(())
    }

    /// Record a connection from a peer (call after check_connection_allowed succeeds)
    pub fn record_connection(&mut self, peer_id: &PeerId) {
        self.total_connections.push(self.time_source.now());

        let activity = self
            .peer_activity
            .entry(*peer_id)
            .or_insert_with(PeerActivity::new);
        activity.add_connection(&self.time_source);
    }

    /// Clean up expired entries
    fn cleanup_expired(&mut self) {
        let window_ms = self.config.time_window_ms;
        let now = self.time_source.now();
        let cutoff_time = now.as_millis().saturating_sub(window_ms);

        // Clean up total counters
        self.total_messages
            .retain(|&ts| ts.as_millis() >= cutoff_time);
        self.total_connections
            .retain(|&ts| ts.as_millis() >= cutoff_time);

        // Clean up per-peer activity
        for activity in self.peer_activity.values_mut() {
            activity.cleanup(&self.time_source, window_ms);
        }

        // Remove peers with no recent activity
        self.peer_activity
            .retain(|_, activity| activity.message_count() > 0 || activity.connection_count() > 0);
    }

    /// Get current configuration
    pub fn config(&self) -> &RateLimitConfig {
        &self.config
    }

    /// Update configuration
    pub fn set_config(&mut self, config: RateLimitConfig) {
        self.config = config;
    }

    /// Get statistics about current usage
    pub fn get_stats(&mut self) -> RateLimitStats {
        self.cleanup_expired();

        RateLimitStats {
            total_messages: self.total_messages.len() as u32,
            total_connections: self.total_connections.len() as u32,
            active_peers: self.peer_activity.len() as u32,
            config: self.config.clone(),
        }
    }
}

// ----------------------------------------------------------------------------
// Rate Limit Statistics
// ----------------------------------------------------------------------------

/// Statistics about current rate limiting state
#[derive(Debug, Clone)]
pub struct RateLimitStats {
    /// Current total messages in window
    pub total_messages: u32,
    /// Current total connections in window
    pub total_connections: u32,
    /// Number of peers with recent activity
    pub active_peers: u32,
    /// Current configuration
    pub config: RateLimitConfig,
}

impl RateLimitStats {
    /// Calculate percentage of message limit used
    pub fn message_usage_percent(&self) -> f32 {
        if self.config.max_total_messages == 0 {
            0.0
        } else {
            (self.total_messages as f32 / self.config.max_total_messages as f32) * 100.0
        }
    }

    /// Calculate percentage of connection limit used
    pub fn connection_usage_percent(&self) -> f32 {
        if self.config.max_total_connections == 0 {
            0.0
        } else {
            (self.total_connections as f32 / self.config.max_total_connections as f32) * 100.0
        }
    }
}

// ----------------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::StdTimeSource;

    #[cfg(feature = "std")]
    #[test]
    fn test_rate_limiter_allows_within_limits() {
        let time_source = StdTimeSource;
        let mut limiter = RateLimiter::new(time_source);
        let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);

        // Should allow messages within limit
        for _ in 0..limiter.config.max_messages_per_peer {
            assert!(limiter.check_message_allowed(&peer_id).is_ok());
            limiter.record_message(&peer_id);
        }

        // Should reject when limit exceeded
        assert!(limiter.check_message_allowed(&peer_id).is_err());
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_rate_limiter_global_limits() {
        let time_source = StdTimeSource;
        let mut limiter = RateLimiter::new(time_source);

        // Fill up global message limit with different peers
        for i in 0..limiter.config.max_total_messages {
            let peer_id = PeerId::new([i as u8, 0, 0, 0, 0, 0, 0, 0]);
            assert!(limiter.check_message_allowed(&peer_id).is_ok());
            limiter.record_message(&peer_id);
        }

        // Should reject new message from any peer
        let new_peer = PeerId::new([255, 0, 0, 0, 0, 0, 0, 0]);
        assert!(limiter.check_message_allowed(&new_peer).is_err());
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_rate_limiter_statistics() {
        let time_source = StdTimeSource;
        let mut limiter = RateLimiter::new(time_source);
        let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);

        // Record some activity
        limiter.check_message_allowed(&peer_id).unwrap();
        limiter.record_message(&peer_id);

        limiter.check_connection_allowed(&peer_id).unwrap();
        limiter.record_connection(&peer_id);

        let stats = limiter.get_stats();
        assert_eq!(stats.total_messages, 1);
        assert_eq!(stats.total_connections, 1);
        assert_eq!(stats.active_peers, 1);
        assert!(stats.message_usage_percent() > 0.0);
        assert!(stats.connection_usage_percent() > 0.0);
    }
}
