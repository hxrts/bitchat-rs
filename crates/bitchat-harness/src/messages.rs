//! Canonical BitChat message definitions (mirrors legacy channel types)
//!
//! These types are shared across transports, runtime, and UI layers. They will evolve
//! as the refactor proceeds but start by matching the original `bitchat-core` channel
//! schema to minimise churn while we migrate crates to the harness.

use bitchat_core::{
    internal::MessageId,
    PeerId,
};
use serde::{Deserialize, Serialize};

cfg_if::cfg_if! {
    if #[cfg(feature = "std")] {
        use std::fmt;
        use std::time::Duration;
    } else {
        use core::fmt;
        use core::time::Duration;
        use alloc::string::String;
        use alloc::vec::Vec;
    }
}

// ----------------------------------------------------------------------------
// Command: UI/External → Core Logic
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    SendMessage { recipient: PeerId, content: String },
    ConnectToPeer { peer_id: PeerId },
    StartDiscovery,
    StopDiscovery,
    DisconnectFromPeer { peer_id: PeerId },
    Shutdown,
    PauseTransport { transport: ChannelTransportType },
    ResumeTransport { transport: ChannelTransportType },
    GetSystemStatus,
}

// ----------------------------------------------------------------------------
// Event: Transport → Core Logic
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    PeerDiscovered {
        peer_id: PeerId,
        transport: ChannelTransportType,
        signal_strength: Option<i8>,
    },
    MessageReceived {
        from: PeerId,
        content: String,
        transport: ChannelTransportType,
        message_id: Option<MessageId>,
        recipient: Option<PeerId>,
        timestamp: Option<u64>,
        sequence: Option<u64>,
    },
    ConnectionEstablished { peer_id: PeerId, transport: ChannelTransportType },
    ConnectionLost { peer_id: PeerId, transport: ChannelTransportType, reason: String },
    TransportError { transport: ChannelTransportType, error: String },
}

// ----------------------------------------------------------------------------
// Effect: Core Logic → Transport
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Effect {
    SendPacket { peer_id: PeerId, data: Vec<u8>, transport: ChannelTransportType },
    InitiateConnection { peer_id: PeerId, transport: ChannelTransportType },
    StartListening { transport: ChannelTransportType },
    StopListening { transport: ChannelTransportType },
    WriteToStorage { key: String, data: Vec<u8> },
    ScheduleRetry { delay: Duration, command: Command },
    StartTransportDiscovery { transport: ChannelTransportType },
    StopTransportDiscovery { transport: ChannelTransportType },
    PauseTransport { transport: ChannelTransportType },
    ResumeTransport { transport: ChannelTransportType },
}

// ----------------------------------------------------------------------------
// AppEvent: Core Logic → UI
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AppEvent {
    MessageReceived { from: PeerId, content: String, timestamp: u64 },
    MessageSent { to: PeerId, content: String, timestamp: u64 },
    PeerStatusChanged { peer_id: PeerId, status: ConnectionStatus, transport: Option<ChannelTransportType> },
    DiscoveryStateChanged { active: bool, transport: Option<ChannelTransportType> },
    ConversationUpdated { peer_id: PeerId, message_count: usize, last_message_time: u64 },
    SystemBusy { reason: String },
    SystemError { error: String },
    SystemStatusReport {
        peer_count: usize,
        active_connections: usize,
        message_count: u64,
        uptime_seconds: u64,
        transport_status: Vec<(ChannelTransportType, TransportStatus)>,
        memory_usage_bytes: Option<usize>,
    },
}

// ----------------------------------------------------------------------------
// Supporting Types
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChannelTransportType {
    Ble,
    Nostr,
}

impl fmt::Display for ChannelTransportType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChannelTransportType::Ble => write!(f, "BLE"),
            ChannelTransportType::Nostr => write!(f, "Nostr"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionStatus {
    Disconnected,
    Discovering,
    Connecting,
    Connected,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransportStatus {
    Active,
    Paused,
    Disabled,
    Unavailable,
}

impl fmt::Display for ConnectionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectionStatus::Disconnected => write!(f, "Disconnected"),
            ConnectionStatus::Discovering => write!(f, "Discovering"),
            ConnectionStatus::Connecting => write!(f, "Connecting"),
            ConnectionStatus::Connected => write!(f, "Connected"),
            ConnectionStatus::Error => write!(f, "Error"),
        }
    }
}

impl fmt::Display for TransportStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransportStatus::Active => write!(f, "Active"),
            TransportStatus::Paused => write!(f, "Paused"),
            TransportStatus::Disabled => write!(f, "Disabled"),
            TransportStatus::Unavailable => write!(f, "Unavailable"),
        }
    }
}
#![cfg_attr(not(feature = "std"), allow(unused_imports))]

#[cfg(not(feature = "std"))]
extern crate alloc;
