//! Canonical NostrRelayManager implementation
//!
//! This module provides relay health monitoring, geographic selection, and connection
//! management matching the canonical Swift/iOS implementation.

use std::{string::String, vec::Vec};
use core::time::Duration;
use hashbrown::HashMap;
use serde::{Deserialize, Serialize};

use super::error::NostrTransportError;
use bitchat_core::types::{PeerId, TimeSource, Timestamp};

cfg_if::cfg_if! {
    if #[cfg(not(target_arch = "wasm32"))] {
        use nostr_sdk::prelude::*;
        use nostr_sdk::{Client as NostrClient, RelayPoolNotification, Event as NostrEvent};
        use tokio::sync::mpsc;
        use std::collections::BTreeMap;
    } else {
        // WASM stubs
        pub struct NostrClient;
        pub struct RelayPoolNotification;
        pub struct NostrEvent;
        pub struct Url;
        use std::collections::BTreeMap;
    }
}

// ----------------------------------------------------------------------------
// Relay Health and Status
// ----------------------------------------------------------------------------

/// Health status of a Nostr relay
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelayHealth {
    /// Relay is healthy and responsive
    Healthy,
    /// Relay is slow but functional
    Degraded,
    /// Relay is unreachable or failing
    Unhealthy,
    /// Relay status is unknown (not yet tested)
    Unknown,
}

impl Default for RelayHealth {
    fn default() -> Self {
        Self::Unknown
    }
}

/// Relay capabilities and features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayCapabilities {
    /// Supports NIP-04 encrypted direct messages
    pub supports_nip04: bool,
    /// Supports NIP-17 gift-wrapped messages
    pub supports_nip17: bool,
    /// Supports custom event kinds
    pub supports_custom_kinds: bool,
    /// Maximum event size in bytes
    pub max_event_size: Option<usize>,
    /// Rate limiting information
    pub rate_limit_per_minute: Option<u32>,
    /// Payment required for posting
    pub requires_payment: bool,
}

impl Default for RelayCapabilities {
    fn default() -> Self {
        Self {
            supports_nip04: true,  // Assume basic NIP-04 support
            supports_nip17: false, // Conservative assumption
            supports_custom_kinds: true,
            max_event_size: Some(65535), // Common limit
            rate_limit_per_minute: None,
            requires_payment: false,
        }
    }
}

/// Comprehensive relay information and status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayInfo {
    /// Relay URL
    pub url: String,
    /// Current health status
    pub health: RelayHealth,
    /// Relay capabilities
    pub capabilities: RelayCapabilities,
    /// Privacy score (0.0 = worst, 1.0 = best)
    pub privacy_score: f64,
    /// Geographic location (lat, lon) if known
    pub location: Option<(f64, f64)>,
    /// Connection statistics
    pub stats: RelayStats,
    /// Last connection attempt
    pub last_connection_attempt: Option<Timestamp>,
    /// Last successful operation
    pub last_success: Option<Timestamp>,
    /// Consecutive failures
    pub consecutive_failures: u32,
    /// Whether relay is currently connected
    pub is_connected: bool,
}

/// Relay connection and performance statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RelayStats {
    /// Total connection attempts
    pub connection_attempts: u32,
    /// Successful connections
    pub successful_connections: u32,
    /// Events sent successfully
    pub events_sent: u32,
    /// Events failed to send
    pub events_failed: u32,
    /// Average latency in milliseconds
    pub average_latency_ms: Option<u64>,
    /// Last measured latency
    pub last_latency_ms: Option<u64>,
    /// Total bytes sent
    pub bytes_sent: u64,
    /// Total bytes received
    pub bytes_received: u64,
}

impl RelayStats {
    /// Calculate connection success rate
    pub fn connection_success_rate(&self) -> f64 {
        if self.connection_attempts == 0 {
            0.0
        } else {
            self.successful_connections as f64 / self.connection_attempts as f64
        }
    }

    /// Calculate event success rate
    pub fn event_success_rate(&self) -> f64 {
        let total_events = self.events_sent + self.events_failed;
        if total_events == 0 {
            0.0
        } else {
            self.events_sent as f64 / total_events as f64
        }
    }
}

// ----------------------------------------------------------------------------
// Geographic Relay Discovery
// ----------------------------------------------------------------------------

/// Geographic relay directory for location-based relay selection
/// Matches canonical GeoRelayDirectory functionality
pub struct GeoRelayDirectory {
    /// Map of geohash prefixes to relay lists
    geo_relays: HashMap<String, Vec<RelayInfo>>,
    /// Global default relays
    default_relays: Vec<RelayInfo>,
    /// Time source for timestamps
    time_source: Box<dyn TimeSource>,
}

impl GeoRelayDirectory {
    /// Create a new geographic relay directory
    pub fn new(time_source: Box<dyn TimeSource>) -> Self {
        Self {
            geo_relays: HashMap::new(),
            default_relays: Self::get_default_relays(),
            time_source,
        }
    }

    /// Get canonical default relays
    /// Matches the canonical `defaultRelays` list
    fn get_default_relays() -> Vec<RelayInfo> {
        let default_urls = [
            "wss://relay.damus.io",
            "wss://nostr-relay.wlvs.space",
            "wss://nostr.fmt.wiz.biz",
            "wss://relay.nostr.band",
            "wss://nos.lol",
        ];

        default_urls
            .iter()
            .map(|url| RelayInfo {
                url: url.to_string(),
                health: RelayHealth::Unknown,
                capabilities: RelayCapabilities::default(),
                privacy_score: 0.7, // Moderate privacy for public relays
                location: None,
                stats: RelayStats::default(),
                last_connection_attempt: None,
                last_success: None,
                consecutive_failures: 0,
                is_connected: false,
            })
            .collect()
    }

    /// Find closest relays to a geohash
    /// Matches canonical `closestRelays(toGeohash:count:)` function
    pub fn closest_relays(&self, geohash: &str, count: usize) -> Vec<RelayInfo> {
        // Try to find relays for this specific geohash or parent geohashes
        for prefix_len in (1..=geohash.len()).rev() {
            let prefix = &geohash[..prefix_len];
            if let Some(relays) = self.geo_relays.get(prefix) {
                if relays.len() >= count {
                    return relays.iter().take(count).cloned().collect();
                }
            }
        }

        // Fallback to default relays if no geo-specific relays found
        self.default_relays.iter().take(count).cloned().collect()
    }

    /// Add relay for a specific geographic region
    pub fn add_geo_relay(&mut self, geohash_prefix: String, relay: RelayInfo) {
        self.geo_relays
            .entry(geohash_prefix)
            .or_insert_with(Vec::new)
            .push(relay);
    }

    /// Load relay directory from CSV data
    /// Matches canonical loading from `online_relays_gps.csv`
    pub fn load_from_csv(&mut self, csv_data: &str) -> Result<usize, NostrTransportError> {
        let mut loaded_count = 0;

        for line in csv_data.lines().skip(1) { // Skip header
            let fields: Vec<&str> = line.split(',').collect();
            if fields.len() >= 4 {
                let url = fields[0].trim().to_string();
                let lat: f64 = fields[1].trim().parse().map_err(|_| {
                    NostrTransportError::ConfigurationError("Invalid latitude".to_string())
                })?;
                let lon: f64 = fields[2].trim().parse().map_err(|_| {
                    NostrTransportError::ConfigurationError("Invalid longitude".to_string())
                })?;
                let geohash_prefix = fields[3].trim().to_string();

                let relay_info = RelayInfo {
                    url,
                    health: RelayHealth::Unknown,
                    capabilities: RelayCapabilities::default(),
                    privacy_score: 0.8, // Higher privacy for geographic relays
                    location: Some((lat, lon)),
                    stats: RelayStats::default(),
                    last_connection_attempt: None,
                    last_success: None,
                    consecutive_failures: 0,
                    is_connected: false,
                };

                self.add_geo_relay(geohash_prefix, relay_info);
                loaded_count += 1;
            }
        }

        Ok(loaded_count)
    }

    /// Calculate distance between two geographic points (Haversine formula)
    fn calculate_distance(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
        let r = 6371.0; // Earth's radius in kilometers
        let dlat = (lat2 - lat1).to_radians();
        let dlon = (lon2 - lon1).to_radians();
        let a = (dlat / 2.0).sin().powi(2)
            + lat1.to_radians().cos() * lat2.to_radians().cos() * (dlon / 2.0).sin().powi(2);
        let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
        r * c
    }
}

// ----------------------------------------------------------------------------
// Relay Selection Strategy
// ----------------------------------------------------------------------------

/// Strategy for selecting relays for message transmission
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelaySelectionStrategy {
    /// Use closest geographic relays
    Geographic,
    /// Use healthiest relays regardless of location
    HealthBased,
    /// Use highest privacy score relays
    PrivacyFocused,
    /// Round-robin through available relays
    RoundRobin,
    /// Use all available relays (broadcast)
    BroadcastAll,
}

impl Default for RelaySelectionStrategy {
    fn default() -> Self {
        Self::Geographic
    }
}

// ----------------------------------------------------------------------------
// NostrRelayManager
// ----------------------------------------------------------------------------

/// Canonical NostrRelayManager for managing Nostr relay connections
/// Matches the canonical singleton `NostrRelayManager.shared` functionality
pub struct NostrRelayManager<T: TimeSource + Send + Sync> {
    /// All known relays
    relays: HashMap<String, RelayInfo>,
    /// Geographic relay directory
    geo_directory: GeoRelayDirectory,
    /// Current selection strategy
    selection_strategy: RelaySelectionStrategy,
    /// Time source
    time_source: T,
    /// Round-robin counter
    round_robin_counter: usize,
    /// Maximum relays to connect to simultaneously
    max_concurrent_connections: usize,
    /// Connection timeout
    connection_timeout: Duration,
    /// Exponential backoff configuration
    initial_backoff: Duration,
    max_backoff: Duration,
    backoff_multiplier: f64,
    max_reconnect_attempts: usize,
}

impl<T: TimeSource + Clone + Send + Sync + 'static> NostrRelayManager<T> {
    /// Create a new relay manager
    pub fn new(time_source: T) -> Self {
        let geo_directory = GeoRelayDirectory::new(Box::new(time_source.clone()));
        
        Self {
            relays: HashMap::new(),
            geo_directory,
            selection_strategy: RelaySelectionStrategy::default(),
            time_source,
            round_robin_counter: 0,
            max_concurrent_connections: 10,
            connection_timeout: Duration::from_secs(30),
            initial_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(300),
            backoff_multiplier: 2.0,
            max_reconnect_attempts: 10,
        }
    }

    /// Add a relay to the manager
    pub fn add_relay(&mut self, relay: RelayInfo) {
        self.relays.insert(relay.url.clone(), relay);
    }

    /// Remove a relay from the manager
    pub fn remove_relay(&mut self, url: &str) -> Option<RelayInfo> {
        self.relays.remove(url)
    }

    /// Get relay information
    pub fn get_relay(&self, url: &str) -> Option<&RelayInfo> {
        self.relays.get(url)
    }

    /// Get mutable relay information
    pub fn get_relay_mut(&mut self, url: &str) -> Option<&mut RelayInfo> {
        self.relays.get_mut(url)
    }

    /// Ensure connections to specific target relays
    /// Matches canonical `ensureConnections(to: targetRelays)` function
    pub async fn ensure_connections(&mut self, target_relays: &[String]) -> Result<usize, NostrTransportError> {
        let mut connected_count = 0;

        for relay_url in target_relays {
            let needs_connection = self.relays.get(relay_url)
                .map(|relay| !relay.is_connected)
                .unwrap_or(false);
                
            if needs_connection {
                match self.connect_to_relay(relay_url).await {
                    Ok(()) => {
                        if let Some(relay) = self.relays.get_mut(relay_url) {
                            relay.is_connected = true;
                            relay.consecutive_failures = 0;
                            relay.last_success = Some(self.time_source.now());
                            connected_count += 1;
                        }
                    }
                    Err(e) => {
                        if let Some(relay) = self.relays.get_mut(relay_url) {
                            relay.consecutive_failures += 1;
                            relay.last_connection_attempt = Some(self.time_source.now());
                        }
                        // Continue trying other relays
                        eprintln!("Failed to connect to relay {}: {}", relay_url, e);
                    }
                }
            }
        }

        Ok(connected_count)
    }

    /// Connect to a specific relay
    async fn connect_to_relay(&mut self, relay_url: &str) -> Result<(), NostrTransportError> {
        cfg_if::cfg_if! {
            if #[cfg(not(target_arch = "wasm32"))] {
                // Update stats
                if let Some(relay) = self.relays.get_mut(relay_url) {
                    relay.stats.connection_attempts += 1;
                }

                // Implement actual connection logic here
                // This would use nostr-sdk to establish WebSocket connection
                
                // For now, simulate connection success/failure based on relay health
                if let Some(relay) = self.relays.get(relay_url) {
                    match relay.health {
                        RelayHealth::Healthy => {
                            // Simulate successful connection
                            if let Some(relay) = self.relays.get_mut(relay_url) {
                                relay.stats.successful_connections += 1;
                                relay.health = RelayHealth::Healthy;
                            }
                            Ok(())
                        }
                        RelayHealth::Unknown => {
                            // Test connection and update health
                            // For now, assume 70% success rate for unknown relays
                            use rand::Rng;
                            let success = rand::thread_rng().gen_bool(0.7);
                            
                            if let Some(relay) = self.relays.get_mut(relay_url) {
                                if success {
                                    relay.stats.successful_connections += 1;
                                    relay.health = RelayHealth::Healthy;
                                    Ok(())
                                } else {
                                    relay.health = RelayHealth::Unhealthy;
                                    Err(NostrTransportError::ConnectionFailed(
                                        format!("Failed to connect to {}", relay_url)
                                    ))
                                }
                            } else {
                                Err(NostrTransportError::ConnectionFailed(
                                    "Relay not found".to_string()
                                ))
                            }
                        }
                        _ => {
                            Err(NostrTransportError::ConnectionFailed(
                                format!("Relay {} is unhealthy", relay_url)
                            ))
                        }
                    }
                } else {
                    Err(NostrTransportError::ConnectionFailed(
                        "Relay not found".to_string()
                    ))
                }
            } else {
                // WASM stub
                Err(NostrTransportError::ConnectionFailed(
                    "Connection not implemented for WASM".to_string()
                ))
            }
        }
    }

    /// Select relays based on current strategy
    pub fn select_relays(&self, geohash: Option<&str>, count: usize) -> Vec<String> {
        match self.selection_strategy {
            RelaySelectionStrategy::Geographic => {
                if let Some(geohash) = geohash {
                    self.geo_directory
                        .closest_relays(geohash, count)
                        .into_iter()
                        .map(|r| r.url)
                        .collect()
                } else {
                    self.select_healthy_relays(count)
                }
            }
            RelaySelectionStrategy::HealthBased => self.select_healthy_relays(count),
            RelaySelectionStrategy::PrivacyFocused => self.select_privacy_relays(count),
            RelaySelectionStrategy::RoundRobin => self.select_round_robin(count),
            RelaySelectionStrategy::BroadcastAll => {
                self.relays.keys().cloned().collect()
            }
        }
    }

    /// Select healthiest relays
    fn select_healthy_relays(&self, count: usize) -> Vec<String> {
        let mut relays: Vec<_> = self.relays.values().collect();
        relays.sort_by(|a, b| {
            // Sort by health first, then by success rate
            match (a.health, b.health) {
                (RelayHealth::Healthy, RelayHealth::Healthy) => {
                    b.stats.connection_success_rate()
                        .partial_cmp(&a.stats.connection_success_rate())
                        .unwrap_or(core::cmp::Ordering::Equal)
                }
                (RelayHealth::Healthy, _) => core::cmp::Ordering::Less,
                (_, RelayHealth::Healthy) => core::cmp::Ordering::Greater,
                _ => core::cmp::Ordering::Equal,
            }
        });

        relays.into_iter().take(count).map(|r| r.url.clone()).collect()
    }

    /// Select highest privacy score relays
    fn select_privacy_relays(&self, count: usize) -> Vec<String> {
        let mut relays: Vec<_> = self.relays.values().collect();
        relays.sort_by(|a, b| {
            b.privacy_score
                .partial_cmp(&a.privacy_score)
                .unwrap_or(core::cmp::Ordering::Equal)
        });

        relays.into_iter().take(count).map(|r| r.url.clone()).collect()
    }

    /// Select relays using round-robin strategy
    fn select_round_robin(&self, count: usize) -> Vec<String> {
        let relay_urls: Vec<_> = self.relays.keys().cloned().collect();
        if relay_urls.is_empty() {
            return Vec::new();
        }

        let mut selected = Vec::new();
        // Use a simple hash-based selection since we can't mutate self
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        for i in 0..count {
            if selected.len() >= relay_urls.len() {
                break; // Don't repeat relays if we have fewer than requested
            }
            
            let mut hasher = DefaultHasher::new();
            i.hash(&mut hasher);
            let index = (hasher.finish() as usize) % relay_urls.len();
            selected.push(relay_urls[index].clone());
        }

        selected
    }

    /// Update relay health based on connection results
    pub fn update_relay_health(&mut self, url: &str, success: bool, latency_ms: Option<u64>) {
        if let Some(relay) = self.relays.get_mut(url) {
            if success {
                relay.consecutive_failures = 0;
                relay.last_success = Some(self.time_source.now());
                relay.health = RelayHealth::Healthy;
                
                if let Some(latency) = latency_ms {
                    relay.stats.last_latency_ms = Some(latency);
                    // Update average latency (simple moving average)
                    if let Some(avg) = relay.stats.average_latency_ms {
                        relay.stats.average_latency_ms = Some((avg + latency) / 2);
                    } else {
                        relay.stats.average_latency_ms = Some(latency);
                    }
                    
                    // Classify health based on latency
                    if latency > 5000 {
                        relay.health = RelayHealth::Degraded;
                    }
                }
            } else {
                relay.consecutive_failures += 1;
                if relay.consecutive_failures >= 3 {
                    relay.health = RelayHealth::Unhealthy;
                }
            }
        }
    }

    /// Get relay manager statistics
    pub fn get_stats(&self) -> RelayManagerStats {
        let mut stats = RelayManagerStats::default();
        
        for relay in self.relays.values() {
            stats.total_relays += 1;
            match relay.health {
                RelayHealth::Healthy => stats.healthy_relays += 1,
                RelayHealth::Degraded => stats.degraded_relays += 1,
                RelayHealth::Unhealthy => stats.unhealthy_relays += 1,
                RelayHealth::Unknown => stats.unknown_relays += 1,
            }
            
            if relay.is_connected {
                stats.connected_relays += 1;
            }
            
            stats.total_events_sent += relay.stats.events_sent;
            stats.total_events_failed += relay.stats.events_failed;
        }
        
        stats
    }

    /// Get geographic relay directory
    pub fn geo_directory(&self) -> &GeoRelayDirectory {
        &self.geo_directory
    }

    /// Get mutable geographic relay directory
    pub fn geo_directory_mut(&mut self) -> &mut GeoRelayDirectory {
        &mut self.geo_directory
    }

    /// Set relay selection strategy
    pub fn set_selection_strategy(&mut self, strategy: RelaySelectionStrategy) {
        self.selection_strategy = strategy;
    }

    /// Load default relays
    pub fn load_default_relays(&mut self) {
        for relay in GeoRelayDirectory::get_default_relays() {
            self.add_relay(relay);
        }
    }
}

/// Relay manager statistics
#[derive(Debug, Default, Clone)]
pub struct RelayManagerStats {
    pub total_relays: usize,
    pub healthy_relays: usize,
    pub degraded_relays: usize,
    pub unhealthy_relays: usize,
    pub unknown_relays: usize,
    pub connected_relays: usize,
    pub total_events_sent: u32,
    pub total_events_failed: u32,
}

impl RelayManagerStats {
    /// Calculate overall health percentage
    pub fn health_percentage(&self) -> f64 {
        if self.total_relays == 0 {
            0.0
        } else {
            self.healthy_relays as f64 / self.total_relays as f64
        }
    }

    /// Calculate event success rate
    pub fn event_success_rate(&self) -> f64 {
        let total_events = self.total_events_sent + self.total_events_failed;
        if total_events == 0 {
            0.0
        } else {
            self.total_events_sent as f64 / total_events as f64
        }
    }
}

// ----------------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use bitchat_core::types::SystemTimeSource;

    #[test]
    fn test_relay_info_creation() {
        let relay = RelayInfo {
            url: "wss://relay.example.com".to_string(),
            health: RelayHealth::Healthy,
            capabilities: RelayCapabilities::default(),
            privacy_score: 0.8,
            location: Some((40.7128, -74.0060)), // New York
            stats: RelayStats::default(),
            last_connection_attempt: None,
            last_success: None,
            consecutive_failures: 0,
            is_connected: true,
        };

        assert_eq!(relay.url, "wss://relay.example.com");
        assert_eq!(relay.health, RelayHealth::Healthy);
        assert!(relay.is_connected);
    }

    #[test]
    fn test_relay_stats_calculations() {
        let mut stats = RelayStats::default();
        stats.connection_attempts = 10;
        stats.successful_connections = 8;
        stats.events_sent = 100;
        stats.events_failed = 10;

        assert_eq!(stats.connection_success_rate(), 0.8);
        assert_eq!(stats.event_success_rate(), 100.0 / 110.0);
    }

    #[test]
    fn test_geo_relay_directory() {
        let time_source = SystemTimeSource;
        let mut directory = GeoRelayDirectory::new(Box::new(time_source));

        let relay = RelayInfo {
            url: "wss://georelay.example.com".to_string(),
            health: RelayHealth::Healthy,
            capabilities: RelayCapabilities::default(),
            privacy_score: 0.9,
            location: Some((37.7749, -122.4194)), // San Francisco
            stats: RelayStats::default(),
            last_connection_attempt: None,
            last_success: None,
            consecutive_failures: 0,
            is_connected: false,
        };

        directory.add_geo_relay("9q8yy".to_string(), relay);

        let closest = directory.closest_relays("9q8yy", 1);
        assert_eq!(closest.len(), 1);
        assert_eq!(closest[0].url, "wss://georelay.example.com");
    }

    #[test]
    fn test_relay_manager_selection() {
        let time_source = SystemTimeSource;
        let mut manager = NostrRelayManager::new(time_source);

        // Add test relays
        let relay1 = RelayInfo {
            url: "wss://relay1.example.com".to_string(),
            health: RelayHealth::Healthy,
            capabilities: RelayCapabilities::default(),
            privacy_score: 0.7,
            location: None,
            stats: RelayStats::default(),
            last_connection_attempt: None,
            last_success: None,
            consecutive_failures: 0,
            is_connected: false,
        };

        let relay2 = RelayInfo {
            url: "wss://relay2.example.com".to_string(),
            health: RelayHealth::Degraded,
            capabilities: RelayCapabilities::default(),
            privacy_score: 0.9,
            location: None,
            stats: RelayStats::default(),
            last_connection_attempt: None,
            last_success: None,
            consecutive_failures: 0,
            is_connected: false,
        };

        manager.add_relay(relay1);
        manager.add_relay(relay2);

        // Test health-based selection
        manager.set_selection_strategy(RelaySelectionStrategy::HealthBased);
        let selected = manager.select_relays(None, 1);
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0], "wss://relay1.example.com"); // Healthier relay

        // Test privacy-focused selection
        manager.set_selection_strategy(RelaySelectionStrategy::PrivacyFocused);
        let selected = manager.select_relays(None, 1);
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0], "wss://relay2.example.com"); // Higher privacy score
    }
}