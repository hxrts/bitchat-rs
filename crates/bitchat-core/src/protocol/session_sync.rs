//! Multi-device session synchronization for BitChat
//!
//! This module implements synchronization of session state and message history
//! across multiple devices belonging to the same BitChat identity.

use alloc::{collections::BTreeMap, string::{String, ToString}, vec::Vec};
use serde::{Deserialize, Serialize};

use crate::protocol::message::NoisePayloadType;
use crate::types::{Fingerprint, PeerId, Timestamp};
use crate::{BitchatError, Result};

// ----------------------------------------------------------------------------
// Constants
// ----------------------------------------------------------------------------

/// Maximum number of devices per identity
pub const MAX_DEVICES_PER_IDENTITY: usize = 10;

/// Maximum size of session state data
pub const MAX_SESSION_STATE_SIZE: usize = 8192;

/// Maximum age of session sync data in milliseconds (24 hours)
pub const MAX_SESSION_SYNC_AGE: u64 = 24 * 60 * 60 * 1000;

/// Maximum number of message references to sync
pub const MAX_MESSAGE_REFS: usize = 1000;

// ----------------------------------------------------------------------------
// Core Types
// ----------------------------------------------------------------------------

/// Unique identifier for a device within an identity
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DeviceId(String);

impl DeviceId {
    /// Generate a new random device ID
    pub fn generate() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    /// Create from string
    pub fn from_string(id: String) -> Self {
        Self(id)
    }

    /// Get the string representation
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl core::fmt::Display for DeviceId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Device information for synchronization
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceInfo {
    /// Device unique identifier
    pub device_id: DeviceId,
    /// Device display name
    pub name: String,
    /// Device type (mobile, desktop, web, etc.)
    pub device_type: DeviceType,
    /// Last seen timestamp
    pub last_seen: Timestamp,
    /// Device capabilities
    pub capabilities: DeviceCapabilities,
    /// Device public key fingerprint
    pub fingerprint: Fingerprint,
}

impl DeviceInfo {
    /// Create new device info
    pub fn new(
        device_id: DeviceId,
        name: String,
        device_type: DeviceType,
        fingerprint: Fingerprint,
    ) -> Self {
        Self {
            device_id,
            name,
            device_type,
            last_seen: Timestamp::now(),
            capabilities: DeviceCapabilities::default(),
            fingerprint,
        }
    }

    /// Update last seen timestamp
    pub fn update_last_seen(&mut self) {
        self.last_seen = Timestamp::now();
    }

    /// Check if device is considered online (seen within last 5 minutes)
    pub fn is_online(&self) -> bool {
        let five_minutes = 5 * 60 * 1000; // 5 minutes in milliseconds
        Timestamp::now() - self.last_seen < five_minutes
    }
}

/// Types of devices that can participate in synchronization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceType {
    /// Mobile device (iOS/Android)
    Mobile,
    /// Desktop application
    Desktop,
    /// Web browser
    Web,
    /// Command line interface
    Cli,
    /// Unknown/other device type
    Unknown,
}

impl Default for DeviceType {
    fn default() -> Self {
        DeviceType::Unknown
    }
}

/// Device capabilities for synchronization coordination
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceCapabilities {
    /// Can store message history
    pub has_storage: bool,
    /// Can maintain persistent connections
    pub has_persistence: bool,
    /// Supports file transfer
    pub supports_files: bool,
    /// Supports location features
    pub supports_location: bool,
    /// Battery powered (affects sync frequency)
    pub battery_powered: bool,
}

impl Default for DeviceCapabilities {
    fn default() -> Self {
        Self {
            has_storage: true,
            has_persistence: true,
            supports_files: false,
            supports_location: false,
            battery_powered: false,
        }
    }
}

/// Session state data for synchronization
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionSyncState {
    /// Peer this session is with
    pub peer_id: PeerId,
    /// Current session status
    pub status: SessionStatus,
    /// Last message timestamp for this session
    pub last_message: Option<Timestamp>,
    /// Number of unread messages
    pub unread_count: u32,
    /// Session encryption fingerprint
    pub session_fingerprint: Option<Fingerprint>,
    /// Last sync timestamp
    pub last_sync: Timestamp,
}

impl SessionSyncState {
    /// Create new session state
    pub fn new(peer_id: PeerId, status: SessionStatus) -> Self {
        Self {
            peer_id,
            status,
            last_message: None,
            unread_count: 0,
            session_fingerprint: None,
            last_sync: Timestamp::now(),
        }
    }

    /// Mark as synchronized
    pub fn mark_synced(&mut self) {
        self.last_sync = Timestamp::now();
    }

    /// Check if session needs sync (older than 30 seconds)
    pub fn needs_sync(&self) -> bool {
        let sync_interval = 30 * 1000; // 30 seconds
        Timestamp::now() - self.last_sync > sync_interval
    }
}

/// Session status for synchronization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionStatus {
    /// Session is being established
    Handshaking,
    /// Session is active and established
    Active,
    /// Session is idle but can be resumed
    Idle,
    /// Session has been terminated
    Terminated,
}

/// Message reference for synchronization
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageRef {
    /// Message unique identifier
    pub message_id: String,
    /// Sender peer ID
    pub sender: PeerId,
    /// Recipient peer ID (if private message)
    pub recipient: Option<PeerId>,
    /// Message timestamp
    pub timestamp: Timestamp,
    /// Message hash for integrity verification
    pub content_hash: String,
    /// Whether message was read on this device
    pub read: bool,
}

impl MessageRef {
    /// Create new message reference
    pub fn new(
        message_id: String,
        sender: PeerId,
        recipient: Option<PeerId>,
        timestamp: Timestamp,
        content_hash: String,
    ) -> Self {
        Self {
            message_id,
            sender,
            recipient,
            timestamp,
            content_hash,
            read: false,
        }
    }

    /// Mark message as read
    pub fn mark_read(&mut self) {
        self.read = true;
    }
}

// ----------------------------------------------------------------------------
// Synchronization Messages
// ----------------------------------------------------------------------------

/// Device announcement for multi-device discovery
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceAnnouncement {
    /// Device information
    pub device_info: DeviceInfo,
    /// Identity fingerprint this device belongs to
    pub identity_fingerprint: Fingerprint,
    /// Proof of identity ownership (signature)
    pub identity_proof: Vec<u8>,
    /// Announcement timestamp
    pub timestamp: Timestamp,
}

impl DeviceAnnouncement {
    /// Create new device announcement
    pub fn new(
        device_info: DeviceInfo,
        identity_fingerprint: Fingerprint,
        identity_proof: Vec<u8>,
    ) -> Self {
        Self {
            device_info,
            identity_fingerprint,
            identity_proof,
            timestamp: Timestamp::now(),
        }
    }

    /// Check if announcement is still valid (not too old)
    pub fn is_valid(&self) -> bool {
        Timestamp::now() - self.timestamp < MAX_SESSION_SYNC_AGE
    }
}

/// Session synchronization request
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionSyncRequest {
    /// Requesting device ID
    pub device_id: DeviceId,
    /// Sessions the device knows about
    pub known_sessions: Vec<SessionSyncState>,
    /// Recent message references
    pub message_refs: Vec<MessageRef>,
    /// Request timestamp
    pub timestamp: Timestamp,
}

impl SessionSyncRequest {
    /// Create new sync request
    pub fn new(device_id: DeviceId, sessions: Vec<SessionSyncState>, messages: Vec<MessageRef>) -> Self {
        Self {
            device_id,
            known_sessions: sessions,
            message_refs: messages,
            timestamp: Timestamp::now(),
        }
    }
}

/// Session synchronization response
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionSyncResponse {
    /// Responding device ID
    pub device_id: DeviceId,
    /// Updated session states
    pub session_updates: Vec<SessionSyncState>,
    /// Missing message references the requestor should fetch
    pub missing_messages: Vec<MessageRef>,
    /// New messages to sync
    pub new_messages: Vec<MessageRef>,
    /// Response timestamp
    pub timestamp: Timestamp,
}

impl SessionSyncResponse {
    /// Create new sync response
    pub fn new(device_id: DeviceId) -> Self {
        Self {
            device_id,
            session_updates: Vec::new(),
            missing_messages: Vec::new(),
            new_messages: Vec::new(),
            timestamp: Timestamp::now(),
        }
    }

    /// Add session update
    pub fn with_session_update(mut self, session: SessionSyncState) -> Self {
        self.session_updates.push(session);
        self
    }

    /// Add missing message
    pub fn with_missing_message(mut self, message: MessageRef) -> Self {
        self.missing_messages.push(message);
        self
    }

    /// Add new message
    pub fn with_new_message(mut self, message: MessageRef) -> Self {
        self.new_messages.push(message);
        self
    }
}

/// Heartbeat message to indicate device is online
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceHeartbeat {
    /// Device ID sending heartbeat
    pub device_id: DeviceId,
    /// Current device status
    pub status: DeviceStatus,
    /// Timestamp of heartbeat
    pub timestamp: Timestamp,
}

impl DeviceHeartbeat {
    /// Create new heartbeat
    pub fn new(device_id: DeviceId, status: DeviceStatus) -> Self {
        Self {
            device_id,
            status,
            timestamp: Timestamp::now(),
        }
    }
}

/// Device status for heartbeat
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceStatus {
    /// Device is online and active
    Online,
    /// Device is idle but reachable
    Idle,
    /// Device is going offline
    GoingOffline,
}

// ----------------------------------------------------------------------------
// Session Sync Wrapper
// ----------------------------------------------------------------------------

/// All session synchronization message types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionSyncMessage {
    /// Device announcement
    Announcement(DeviceAnnouncement),
    /// Sync request
    SyncRequest(SessionSyncRequest),
    /// Sync response
    SyncResponse(SessionSyncResponse),
    /// Device heartbeat
    Heartbeat(DeviceHeartbeat),
}

impl SessionSyncMessage {
    /// Get the device ID for this message
    pub fn device_id(&self) -> &DeviceId {
        match self {
            SessionSyncMessage::Announcement(ann) => &ann.device_info.device_id,
            SessionSyncMessage::SyncRequest(req) => &req.device_id,
            SessionSyncMessage::SyncResponse(resp) => &resp.device_id,
            SessionSyncMessage::Heartbeat(hb) => &hb.device_id,
        }
    }

    /// Get the corresponding NoisePayloadType for this message
    pub fn payload_type(&self) -> NoisePayloadType {
        match self {
            SessionSyncMessage::Announcement(_) => NoisePayloadType::DeviceAnnouncement,
            SessionSyncMessage::SyncRequest(_) => NoisePayloadType::SessionSyncRequest,
            SessionSyncMessage::SyncResponse(_) => NoisePayloadType::SessionSyncResponse,
            SessionSyncMessage::Heartbeat(_) => NoisePayloadType::DeviceHeartbeat,
        }
    }
}

// ----------------------------------------------------------------------------
// Multi-Device Session Manager
// ----------------------------------------------------------------------------

/// Manages session synchronization across multiple devices
#[derive(Debug, Clone)]
pub struct MultiDeviceSessionManager {
    /// Local device information
    local_device: DeviceInfo,
    /// Known devices for this identity
    known_devices: BTreeMap<DeviceId, DeviceInfo>,
    /// Session states across all devices
    session_states: BTreeMap<PeerId, SessionSyncState>,
    /// Message references for synchronization
    message_refs: BTreeMap<String, MessageRef>,
    /// Last sync timestamp with each device
    device_sync_times: BTreeMap<DeviceId, Timestamp>,
}

impl MultiDeviceSessionManager {
    /// Create new multi-device session manager
    pub fn new(local_device: DeviceInfo) -> Self {
        let mut known_devices = BTreeMap::new();
        known_devices.insert(local_device.device_id.clone(), local_device.clone());

        Self {
            local_device,
            known_devices,
            session_states: BTreeMap::new(),
            message_refs: BTreeMap::new(),
            device_sync_times: BTreeMap::new(),
        }
    }

    /// Add or update a known device
    pub fn add_device(&mut self, device: DeviceInfo) -> Result<()> {
        if self.known_devices.len() >= MAX_DEVICES_PER_IDENTITY
            && !self.known_devices.contains_key(&device.device_id)
        {
            return Err(BitchatError::invalid_packet(
                "Maximum number of devices reached",
            ));
        }

        self.known_devices.insert(device.device_id.clone(), device);
        Ok(())
    }

    /// Remove a device
    pub fn remove_device(&mut self, device_id: &DeviceId) -> Option<DeviceInfo> {
        self.device_sync_times.remove(device_id);
        self.known_devices.remove(device_id)
    }

    /// Get all known devices
    pub fn get_devices(&self) -> Vec<&DeviceInfo> {
        self.known_devices.values().collect()
    }

    /// Get online devices
    pub fn get_online_devices(&self) -> Vec<&DeviceInfo> {
        self.known_devices
            .values()
            .filter(|device| device.is_online())
            .collect()
    }

    /// Update session state
    pub fn update_session(&mut self, session: SessionSyncState) {
        session.peer_id;
        self.session_states.insert(session.peer_id, session);
    }

    /// Get session state
    pub fn get_session(&self, peer_id: &PeerId) -> Option<&SessionSyncState> {
        self.session_states.get(peer_id)
    }

    /// Add message reference
    pub fn add_message_ref(&mut self, message_ref: MessageRef) -> Result<()> {
        if self.message_refs.len() >= MAX_MESSAGE_REFS {
            // Remove oldest message reference
            if let Some(oldest_id) = self
                .message_refs
                .iter()
                .min_by_key(|(_, msg)| msg.timestamp)
                .map(|(id, _)| id.clone())
            {
                self.message_refs.remove(&oldest_id);
            }
        }

        self.message_refs
            .insert(message_ref.message_id.clone(), message_ref);
        Ok(())
    }

    /// Create sync request for another device
    pub fn create_sync_request(&mut self) -> SessionSyncRequest {
        let sessions: Vec<SessionSyncState> = self.session_states.values().cloned().collect();
        let messages: Vec<MessageRef> = self.message_refs.values().cloned().collect();

        SessionSyncRequest::new(self.local_device.device_id.clone(), sessions, messages)
    }

    /// Process sync request and create response
    pub fn process_sync_request(
        &mut self,
        request: &SessionSyncRequest,
    ) -> Result<SessionSyncResponse> {
        let mut response = SessionSyncResponse::new(self.local_device.device_id.clone());

        // Find sessions that need updates
        for remote_session in &request.known_sessions {
            if let Some(local_session) = self.session_states.get(&remote_session.peer_id) {
                // If our session is newer, add it to updates
                if local_session.last_sync > remote_session.last_sync {
                    response = response.with_session_update(local_session.clone());
                }
            }
        }

        // Find missing messages in the request
        for local_message in self.message_refs.values() {
            if !request
                .message_refs
                .iter()
                .any(|msg| msg.message_id == local_message.message_id)
            {
                response = response.with_new_message(local_message.clone());
            }
        }

        // Find messages we're missing
        for remote_message in &request.message_refs {
            if !self.message_refs.contains_key(&remote_message.message_id) {
                response = response.with_missing_message(remote_message.clone());
            }
        }

        // Update sync time
        self.device_sync_times
            .insert(request.device_id.clone(), Timestamp::now());

        Ok(response)
    }

    /// Process sync response
    pub fn process_sync_response(&mut self, response: &SessionSyncResponse) -> Result<()> {
        // Apply session updates
        for session in &response.session_updates {
            self.session_states.insert(session.peer_id, session.clone());
        }

        // Add new messages
        for message in &response.new_messages {
            self.add_message_ref(message.clone())?;
        }

        // Update sync time
        self.device_sync_times
            .insert(response.device_id.clone(), Timestamp::now());

        Ok(())
    }

    /// Clean up old devices and data
    pub fn cleanup_old_data(&mut self) {
        let now = Timestamp::now();

        // Remove devices that haven't been seen for too long
        let offline_devices: Vec<DeviceId> = self
            .known_devices
            .iter()
            .filter_map(|(device_id, device)| {
                if device_id != &self.local_device.device_id
                    && now - device.last_seen > MAX_SESSION_SYNC_AGE
                {
                    Some(device_id.clone())
                } else {
                    None
                }
            })
            .collect();

        for device_id in offline_devices {
            self.remove_device(&device_id);
        }

        // Remove old message references
        let old_messages: Vec<String> = self
            .message_refs
            .iter()
            .filter_map(|(msg_id, msg)| {
                if now - msg.timestamp > MAX_SESSION_SYNC_AGE {
                    Some(msg_id.clone())
                } else {
                    None
                }
            })
            .collect();

        for msg_id in old_messages {
            self.message_refs.remove(&msg_id);
        }
    }

    /// Check if any sessions need synchronization
    pub fn needs_sync(&self) -> bool {
        self.session_states.values().any(|session| session.needs_sync())
    }

    /// Get devices that need sync (haven't synced recently)
    pub fn devices_needing_sync(&self) -> Vec<&DeviceInfo> {
        let sync_interval = 60 * 1000; // 1 minute
        let now = Timestamp::now();

        self.known_devices
            .values()
            .filter(|device| {
                device.device_id != self.local_device.device_id
                    && device.is_online()
                    && self
                        .device_sync_times
                        .get(&device.device_id)
                        .map_or(true, |last_sync| now - *last_sync > sync_interval)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_device(id: u8, name: &str) -> DeviceInfo {
        let device_id = DeviceId::from_string(format!("device-{}", id));
        let fingerprint = Fingerprint::new([id; 32]);
        DeviceInfo::new(device_id, name.to_string(), DeviceType::Desktop, fingerprint)
    }

    #[test]
    fn test_multi_device_manager_creation() {
        let device = create_test_device(1, "Test Device");
        let manager = MultiDeviceSessionManager::new(device.clone());

        assert_eq!(manager.get_devices().len(), 1);
        assert_eq!(manager.get_devices()[0].device_id, device.device_id);
    }

    #[test]
    fn test_device_management() {
        let device1 = create_test_device(1, "Device 1");
        let device2 = create_test_device(2, "Device 2");

        let mut manager = MultiDeviceSessionManager::new(device1.clone());

        // Add device
        manager.add_device(device2.clone()).unwrap();
        assert_eq!(manager.get_devices().len(), 2);

        // Remove device
        let removed = manager.remove_device(&device2.device_id);
        assert!(removed.is_some());
        assert_eq!(manager.get_devices().len(), 1);
    }

    #[test]
    fn test_session_synchronization() {
        let device = create_test_device(1, "Test Device");
        let mut manager = MultiDeviceSessionManager::new(device.clone());

        let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        let session = SessionSyncState::new(peer_id, SessionStatus::Active);

        // Update session
        manager.update_session(session.clone());
        assert_eq!(manager.get_session(&peer_id), Some(&session));

        // Create sync request
        let request = manager.create_sync_request();
        assert_eq!(request.device_id, device.device_id);
        assert_eq!(request.known_sessions.len(), 1);
    }

    #[test]
    fn test_message_reference_management() {
        let device = create_test_device(1, "Test Device");
        let mut manager = MultiDeviceSessionManager::new(device);

        let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        let message_ref = MessageRef::new(
            "msg-1".to_string(),
            peer_id,
            None,
            Timestamp::now(),
            "hash123".to_string(),
        );

        manager.add_message_ref(message_ref.clone()).unwrap();
        assert!(manager.message_refs.contains_key("msg-1"));
    }

    #[test]
    fn test_sync_request_processing() {
        let device1 = create_test_device(1, "Device 1");
        let device2 = create_test_device(2, "Device 2");
        let mut manager = MultiDeviceSessionManager::new(device1.clone());

        let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        let session = SessionSyncState::new(peer_id, SessionStatus::Active);
        manager.update_session(session);

        // Create an older version of the session to simulate sync
        let mut old_session = SessionSyncState::new(peer_id, SessionStatus::Active);
        old_session.last_sync = Timestamp::new(Timestamp::now().as_millis() - 1000); // 1 second ago
        
        let request = SessionSyncRequest::new(device2.device_id, vec![old_session], Vec::new());
        let response = manager.process_sync_request(&request).unwrap();

        assert_eq!(response.device_id, device1.device_id);
        // Now we should get 1 session update because our session is newer
        assert_eq!(response.session_updates.len(), 1);
    }

    #[test]
    fn test_device_status() {
        let mut device = create_test_device(1, "Test Device");

        assert!(device.is_online()); // Just created, should be online

        // Simulate old last_seen
        let ten_minutes_ago = Timestamp::now().as_millis() - (10 * 60 * 1000);
        device.last_seen = Timestamp::new(ten_minutes_ago);
        assert!(!device.is_online());
    }
}