//! Rate Limiting for BitChat Core
//!
//! Provides rate limiting functionality to prevent DoS attacks by limiting the rate
//! of incoming messages, connections, and other operations per peer and globally.

use bitchat_core::{internal::RateLimitConfig, BitchatError, BitchatResult, PeerId};
use instant::Instant;

#[cfg(not(feature = "std"))]
use alloc::collections::{BTreeMap as HashMap, VecDeque};
#[cfg(feature = "std")]
use std::collections::{HashMap, VecDeque};

/// Time-based event tracking for rate limiting
#[derive(Debug)]
struct EventWindow {
    events: VecDeque<Instant>,
    window_duration_secs: u64,
}

impl EventWindow {
    fn new(window_duration_secs: u64) -> Self {
        Self {
            events: VecDeque::new(),
            window_duration_secs,
        }
    }

    /// Add a new event and return the current count in the window
    fn add_event(&mut self) -> u32 {
        let now = Instant::now();
        self.events.push_back(now);
        self.cleanup_old_events();
        self.events.len() as u32
    }

    /// Get current event count in the window without adding an event
    fn get_count(&mut self) -> u32 {
        self.cleanup_old_events();
        self.events.len() as u32
    }

    /// Remove events older than the window duration
    fn cleanup_old_events(&mut self) {
        let now = Instant::now();
        let window_duration = std::time::Duration::from_secs(self.window_duration_secs);

        while let Some(&front_time) = self.events.front() {
            if now.duration_since(front_time) > window_duration {
                self.events.pop_front();
            } else {
                break;
            }
        }
    }
}

/// Per-peer rate limiting state
#[derive(Debug)]
struct PeerLimits {
    message_window: EventWindow,
    connection_window: EventWindow,
    last_access: Instant,
}

impl PeerLimits {
    fn new(config: &RateLimitConfig) -> Self {
        Self {
            message_window: EventWindow::new(config.window_duration_secs),
            connection_window: EventWindow::new(config.window_duration_secs),
            last_access: Instant::now(),
        }
    }

    fn update_access_time(&mut self) {
        self.last_access = Instant::now();
    }
}

/// Main rate limiter implementation
pub struct RateLimiter {
    config: RateLimitConfig,
    peer_limits: HashMap<PeerId, PeerLimits>,
    global_message_window: EventWindow,
    global_connection_window: EventWindow,
}

impl RateLimiter {
    /// Create a new rate limiter with the given configuration
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            global_message_window: EventWindow::new(config.window_duration_secs),
            global_connection_window: EventWindow::new(config.window_duration_secs),
            config,
            peer_limits: HashMap::new(),
        }
    }

    /// Check if a message from the given peer should be allowed
    pub fn check_message_allowed(&mut self, peer_id: PeerId) -> BitchatResult<()> {
        // Check global message rate limit first
        let global_count = self.global_message_window.add_event();
        if global_count > self.config.global_messages_per_window {
            return Err(BitchatError::RateLimited {
                reason: format!(
                    "Global message rate limit exceeded: {} messages in {}s window",
                    global_count, self.config.window_duration_secs
                ),
            });
        }

        // Check per-peer message rate limit
        self.cleanup_old_peers();

        let peer_limits = self
            .peer_limits
            .entry(peer_id)
            .or_insert_with(|| PeerLimits::new(&self.config));

        peer_limits.update_access_time();
        let peer_count = peer_limits.message_window.add_event();

        if peer_count > self.config.messages_per_peer_per_window {
            return Err(BitchatError::RateLimited {
                reason: format!(
                    "Peer {} message rate limit exceeded: {} messages in {}s window",
                    peer_id, peer_count, self.config.window_duration_secs
                ),
            });
        }

        Ok(())
    }

    /// Check if a connection from the given peer should be allowed
    pub fn check_connection_allowed(&mut self, peer_id: PeerId) -> BitchatResult<()> {
        // Check global connection rate limit first
        let global_count = self.global_connection_window.add_event();
        if global_count > self.config.global_connections_per_window {
            return Err(BitchatError::RateLimited {
                reason: format!(
                    "Global connection rate limit exceeded: {} connections in {}s window",
                    global_count, self.config.window_duration_secs
                ),
            });
        }

        // Check per-peer connection rate limit
        self.cleanup_old_peers();

        let peer_limits = self
            .peer_limits
            .entry(peer_id)
            .or_insert_with(|| PeerLimits::new(&self.config));

        peer_limits.update_access_time();
        let peer_count = peer_limits.connection_window.add_event();

        if peer_count > self.config.connections_per_peer_per_window {
            return Err(BitchatError::RateLimited {
                reason: format!(
                    "Peer {} connection rate limit exceeded: {} connections in {}s window",
                    peer_id, peer_count, self.config.window_duration_secs
                ),
            });
        }

        Ok(())
    }

    /// Get current rate limiting statistics
    pub fn get_stats(&mut self) -> RateLimitStats {
        self.cleanup_old_peers();

        let global_messages = self.global_message_window.get_count();
        let global_connections = self.global_connection_window.get_count();
        let tracked_peers = self.peer_limits.len();

        RateLimitStats {
            global_messages_in_window: global_messages,
            global_connections_in_window: global_connections,
            tracked_peers,
            config: self.config.clone(),
        }
    }

    /// Remove tracking data for peers that haven't been active recently
    fn cleanup_old_peers(&mut self) {
        let now = Instant::now();
        let cleanup_threshold =
            std::time::Duration::from_secs(self.config.window_duration_secs * 2);

        // Remove peers that haven't been active recently
        self.peer_limits
            .retain(|_, limits| now.duration_since(limits.last_access) < cleanup_threshold);

        // If we still have too many peers, remove the least recently used ones
        if self.peer_limits.len() > self.config.max_tracked_peers {
            let mut peers_by_access: Vec<_> = self
                .peer_limits
                .iter()
                .map(|(peer_id, limits)| (*peer_id, limits.last_access))
                .collect();

            peers_by_access.sort_by_key(|(_, access_time)| *access_time);

            let excess_count = self.peer_limits.len() - self.config.max_tracked_peers;
            for (peer_id, _) in peers_by_access.into_iter().take(excess_count) {
                self.peer_limits.remove(&peer_id);
            }
        }
    }

    /// Reset all rate limiting state (for testing)
    #[cfg(test)]
    pub fn reset(&mut self) {
        self.peer_limits.clear();
        self.global_message_window = EventWindow::new(self.config.window_duration_secs);
        self.global_connection_window = EventWindow::new(self.config.window_duration_secs);
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new(RateLimitConfig::default())
    }
}

/// Rate limiting statistics
#[derive(Debug, Clone)]
pub struct RateLimitStats {
    pub global_messages_in_window: u32,
    pub global_connections_in_window: u32,
    pub tracked_peers: usize,
    pub config: RateLimitConfig,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_message_rate_limiting() {
        let config = RateLimitConfig {
            messages_per_peer_per_window: 3,
            window_duration_secs: 1,
            ..Default::default()
        };

        let mut limiter = RateLimiter::new(config);
        let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);

        // First 3 messages should be allowed
        assert!(limiter.check_message_allowed(peer_id).is_ok());
        assert!(limiter.check_message_allowed(peer_id).is_ok());
        assert!(limiter.check_message_allowed(peer_id).is_ok());

        // 4th message should be rate limited
        assert!(limiter.check_message_allowed(peer_id).is_err());
    }

    #[test]
    fn test_global_rate_limiting() {
        let config = RateLimitConfig {
            global_messages_per_window: 2,
            messages_per_peer_per_window: 10,
            window_duration_secs: 1,
            ..Default::default()
        };

        let mut limiter = RateLimiter::new(config);
        let peer1 = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        let peer2 = PeerId::new([8, 7, 6, 5, 4, 3, 2, 1]);

        // First 2 messages should be allowed (global limit)
        assert!(limiter.check_message_allowed(peer1).is_ok());
        assert!(limiter.check_message_allowed(peer2).is_ok());

        // 3rd message should be globally rate limited
        assert!(limiter.check_message_allowed(peer1).is_err());
    }

    #[test]
    fn test_connection_rate_limiting() {
        let config = RateLimitConfig {
            connections_per_peer_per_window: 2,
            window_duration_secs: 1,
            ..Default::default()
        };

        let mut limiter = RateLimiter::new(config);
        let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);

        // First 2 connections should be allowed
        assert!(limiter.check_connection_allowed(peer_id).is_ok());
        assert!(limiter.check_connection_allowed(peer_id).is_ok());

        // 3rd connection should be rate limited
        assert!(limiter.check_connection_allowed(peer_id).is_err());
    }

    #[test]
    fn test_rate_limit_stats() {
        let mut limiter = RateLimiter::default();
        let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);

        limiter.check_message_allowed(peer_id).unwrap();
        limiter.check_connection_allowed(peer_id).unwrap();

        let stats = limiter.get_stats();
        assert_eq!(stats.global_messages_in_window, 1);
        assert_eq!(stats.global_connections_in_window, 1);
        assert_eq!(stats.tracked_peers, 1);
    }

    #[test]
    fn test_peer_cleanup() {
        let config = RateLimitConfig {
            max_tracked_peers: 2,
            window_duration_secs: 1,
            ..Default::default()
        };

        let mut limiter = RateLimiter::new(config);

        let peer1 = PeerId::new([1, 0, 0, 0, 0, 0, 0, 0]);
        let peer2 = PeerId::new([2, 0, 0, 0, 0, 0, 0, 0]);
        let peer3 = PeerId::new([3, 0, 0, 0, 0, 0, 0, 0]);

        // Add 3 peers (exceeds max of 2)
        limiter.check_message_allowed(peer1).unwrap();
        limiter.check_message_allowed(peer2).unwrap();
        limiter.check_message_allowed(peer3).unwrap();

        let stats = limiter.get_stats();
        assert!(stats.tracked_peers <= 2);
    }

    #[test]
    fn test_window_expiry() {
        let config = RateLimitConfig {
            messages_per_peer_per_window: 1,
            window_duration_secs: 1,
            ..Default::default()
        };

        let mut limiter = RateLimiter::new(config);
        let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);

        // First message should be allowed
        assert!(limiter.check_message_allowed(peer_id).is_ok());

        // Second message should be rate limited
        assert!(limiter.check_message_allowed(peer_id).is_err());

        // Wait for window to expire
        thread::sleep(Duration::from_secs(2));

        // Message should be allowed again after window expiry
        assert!(limiter.check_message_allowed(peer_id).is_ok());
    }
}
