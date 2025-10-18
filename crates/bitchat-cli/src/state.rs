//! State persistence for the BitChat CLI

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::error::{CliError, Result};
use bitchat_core::{transport::TransportType, BitchatMessage, PeerId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppState {
    /// User's identity keys (serialized)
    pub identity_keys: Option<String>,
    /// Discovered peers with their transport types
    pub discovered_peers: HashMap<String, PeerInfo>,
    /// Recent messages for persistence
    pub recent_messages: Vec<StoredMessage>,
    /// Application statistics
    pub stats: AppStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    /// Peer ID as hex string
    pub peer_id: String,
    /// Transport types this peer is available on
    pub transport_types: Vec<TransportType>,
    /// Last seen timestamp
    pub last_seen: u64,
    /// Display name if known
    pub display_name: Option<String>,
    /// Nostr public key if available
    pub nostr_pubkey: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredMessage {
    /// Message ID
    pub id: String,
    /// Sender peer ID
    pub sender_id: String,
    /// Sender display name
    pub sender_name: String,
    /// Message content
    pub content: String,
    /// Timestamp
    pub timestamp: u64,
    /// Whether this is a private message
    pub is_private: bool,
    /// Recipient if private message
    pub recipient_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppStats {
    /// Total messages sent
    pub messages_sent: u64,
    /// Total messages received
    pub messages_received: u64,
    /// Total peers discovered
    pub peers_discovered: u64,
    /// Application startup count
    pub startup_count: u64,
    /// Total runtime in seconds
    pub total_runtime: u64,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            identity_keys: None,
            discovered_peers: HashMap::new(),
            recent_messages: Vec::new(),
            stats: AppStats::default(),
        }
    }
}

impl AppState {
    /// Load state from file
    pub fn load_from_file(path: &PathBuf) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let state_str = std::fs::read_to_string(path)
            .map_err(|e| CliError::StatePersistence(format!("Failed to read state file: {}", e)))?;

        serde_json::from_str(&state_str)
            .map_err(|e| CliError::StatePersistence(format!("Failed to parse state file: {}", e)))
    }

    /// Save state to file
    pub fn save_to_file(&self, path: &PathBuf) -> Result<()> {
        let state_str = serde_json::to_string_pretty(self)
            .map_err(|e| CliError::StatePersistence(format!("Failed to serialize state: {}", e)))?;

        std::fs::write(path, state_str)
            .map_err(|e| CliError::StatePersistence(format!("Failed to write state file: {}", e)))
    }

    /// Add a discovered peer
    pub fn add_peer(
        &mut self,
        peer_id: PeerId,
        transport_type: TransportType,
        display_name: Option<String>,
    ) {
        let peer_id_str = peer_id.to_string();

        let peer_info = self
            .discovered_peers
            .entry(peer_id_str.clone())
            .or_insert_with(|| {
                self.stats.peers_discovered += 1;
                PeerInfo {
                    peer_id: peer_id_str,
                    transport_types: Vec::new(),
                    last_seen: current_timestamp(),
                    display_name: display_name.clone(),
                    nostr_pubkey: None,
                }
            });

        if !peer_info.transport_types.contains(&transport_type) {
            peer_info.transport_types.push(transport_type);
        }

        peer_info.last_seen = current_timestamp();

        if let Some(name) = display_name {
            peer_info.display_name = Some(name);
        }
    }

    /// Add a received message
    pub fn add_message(&mut self, sender_id: PeerId, message: &BitchatMessage, is_received: bool) {
        let stored_message = StoredMessage {
            id: message.id.to_string(),
            sender_id: sender_id.to_string(),
            sender_name: message.sender.clone(),
            content: message.content.clone(),
            timestamp: message.timestamp.as_millis(),
            is_private: message.flags.is_private,
            recipient_id: message
                .recipient_nickname
                .as_ref()
                .map(|_| sender_id.to_string()),
        };

        self.recent_messages.push(stored_message);

        // Keep only the last 100 messages
        if self.recent_messages.len() > 100 {
            self.recent_messages.remove(0);
        }

        if is_received {
            self.stats.messages_received += 1;
        } else {
            self.stats.messages_sent += 1;
        }
    }

    /// Get peers by transport type
    pub fn get_peers_by_transport(&self, transport_type: TransportType) -> Vec<&PeerInfo> {
        self.discovered_peers
            .values()
            .filter(|peer| peer.transport_types.contains(&transport_type))
            .collect()
    }

    /// Update startup statistics
    pub fn record_startup(&mut self) {
        self.stats.startup_count += 1;
    }

    /// Update runtime statistics
    pub fn update_runtime(&mut self, runtime_seconds: u64) {
        self.stats.total_runtime += runtime_seconds;
    }

    /// Clean up old data
    pub fn cleanup_old_data(&mut self, max_age_seconds: u64) {
        let cutoff_time = current_timestamp() - max_age_seconds * 1000;

        // Remove old peers
        self.discovered_peers
            .retain(|_, peer| peer.last_seen > cutoff_time);

        // Remove old messages
        self.recent_messages
            .retain(|msg| msg.timestamp > cutoff_time);
    }
}

impl PeerInfo {
    /// Check if peer was seen recently
    pub fn is_recent(&self, max_age_seconds: u64) -> bool {
        let cutoff_time = current_timestamp() - max_age_seconds * 1000;
        self.last_seen > cutoff_time
    }
}

/// Get current timestamp in milliseconds
fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// State manager for handling persistence operations
pub struct StateManager {
    state: AppState,
    file_path: PathBuf,
    last_save: std::time::Instant,
    auto_save_interval: std::time::Duration,
}

impl StateManager {
    /// Create a new state manager
    pub fn new(state_dir: PathBuf, auto_save_interval: std::time::Duration) -> Result<Self> {
        let file_path = state_dir.join("app_state.json");
        let state = AppState::load_from_file(&file_path)?;

        Ok(Self {
            state,
            file_path,
            last_save: std::time::Instant::now(),
            auto_save_interval,
        })
    }

    /// Get mutable reference to state
    pub fn state_mut(&mut self) -> &mut AppState {
        &mut self.state
    }

    /// Get immutable reference to state
    pub fn state(&self) -> &AppState {
        &self.state
    }

    /// Save state if auto-save interval has passed
    pub fn maybe_auto_save(&mut self) -> Result<()> {
        if self.last_save.elapsed() >= self.auto_save_interval {
            self.save()?;
        }
        Ok(())
    }

    /// Force save state to file
    pub fn save(&mut self) -> Result<()> {
        self.state.save_to_file(&self.file_path)?;
        self.last_save = std::time::Instant::now();
        Ok(())
    }
}
