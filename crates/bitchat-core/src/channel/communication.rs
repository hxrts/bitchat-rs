//! CSP Channel Communication Protocol Types
//!
//! This module defines the typed communication protocol.
//! All inter-task communication flows through these channel message types.

use crate::protocol::message_store::MessageId;
use crate::protocol::BitchatPacket;
use crate::PeerId;
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

/// Commands sent from UI and external systems to the Core Logic task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    /// Send a message to a specific peer
    SendMessage { recipient: PeerId, content: String },
    /// Initiate connection to a peer
    ConnectToPeer { peer_id: PeerId },
    /// Start peer discovery across all transports
    StartDiscovery,
    /// Stop peer discovery
    StopDiscovery,
    /// Disconnect from a specific peer
    DisconnectFromPeer { peer_id: PeerId },
    /// Shutdown the system gracefully
    Shutdown,
    /// Pause a specific transport
    PauseTransport { transport: TransportType },
    /// Resume a specific transport
    ResumeTransport { transport: TransportType },
    /// Request detailed system status report
    GetSystemStatus,
    /// Query the status of a specific message
    QueryMessageStatus { message_id: MessageId },
    /// Query the session state with a specific peer
    QueryPeerSession { peer_id: PeerId },
    /// Query the delivery status of messages to a peer
    QueryDeliveryStatus { peer_id: PeerId },
    /// Query the complete internal state for debugging
    QueryInternalState,
}

// ----------------------------------------------------------------------------
// Event: Transport → Core Logic
// ----------------------------------------------------------------------------

/// Events sent from Transport tasks to the Core Logic task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    /// A peer was discovered via transport
    PeerDiscovered {
        peer_id: PeerId,
        transport: TransportType,
        signal_strength: Option<i8>,
    },
    /// A message was received from a peer (legacy)
    MessageReceived {
        from: PeerId,
        content: String,
        transport: TransportType,
        message_id: Option<MessageId>,
        recipient: Option<PeerId>,
        timestamp: Option<u64>,
        sequence: Option<u64>,
    },
    /// A BitChat wire protocol packet was received
    BitchatPacketReceived {
        from: PeerId,
        packet: BitchatPacket,
        transport: TransportType,
    },
    /// Connection to peer was established
    ConnectionEstablished {
        peer_id: PeerId,
        transport: TransportType,
    },
    /// Connection to peer was lost
    ConnectionLost {
        peer_id: PeerId,
        transport: TransportType,
        reason: String,
    },
    /// Transport-specific error occurred
    TransportError {
        transport: TransportType,
        error: String,
    },
}

// ----------------------------------------------------------------------------
// Effect: Core Logic → Transport (External Side Effects Only)
// ----------------------------------------------------------------------------

/// Effects sent from Core Logic task to Transport tasks
/// Effects describe external side effects only - no UI knowledge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Effect {
    /// Send packet to peer via transport (legacy raw bytes)
    SendPacket {
        peer_id: PeerId,
        data: Vec<u8>,
        transport: TransportType,
    },
    /// Send BitChat wire protocol packet to peer
    SendBitchatPacket {
        peer_id: PeerId,
        packet: BitchatPacket,
        transport: TransportType,
    },
    /// Broadcast BitChat wire protocol packet
    BroadcastBitchatPacket {
        packet: BitchatPacket,
        transport: TransportType,
    },
    /// Initiate connection to peer
    InitiateConnection {
        peer_id: PeerId,
        transport: TransportType,
    },
    /// Start listening for connections
    StartListening { transport: TransportType },
    /// Stop listening for connections
    StopListening { transport: TransportType },
    /// Write data to persistent storage
    WriteToStorage { key: String, data: Vec<u8> },
    /// Schedule a retry operation
    ScheduleRetry { delay: Duration, command: Command },
    /// Start discovery for transport
    StartTransportDiscovery { transport: TransportType },
    /// Stop discovery for transport
    StopTransportDiscovery { transport: TransportType },
    /// Pause a specific transport
    PauseTransport { transport: TransportType },
    /// Resume a specific transport
    ResumeTransport { transport: TransportType },
}

// ----------------------------------------------------------------------------
// AppEvent: Core Logic → UI (State Changes Only)
// ----------------------------------------------------------------------------

/// Application events sent from Core Logic task to UI task
/// AppEvents describe state changes that UI components need to know about
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AppEvent {
    /// A message was received
    MessageReceived {
        from: PeerId,
        content: String,
        timestamp: u64,
    },
    /// A message was sent successfully
    MessageSent {
        to: PeerId,
        content: String,
        timestamp: u64,
    },
    /// Peer connection status changed
    PeerStatusChanged {
        peer_id: PeerId,
        status: ConnectionStatus,
        transport: Option<TransportType>,
    },
    /// Discovery state changed
    DiscoveryStateChanged {
        active: bool,
        transport: Option<TransportType>,
    },
    /// Conversation was updated
    ConversationUpdated {
        peer_id: PeerId,
        message_count: usize,
        last_message_time: u64,
    },
    /// System is busy processing
    SystemBusy { reason: String },
    /// System error occurred
    SystemError { error: String },
    /// System status report in response to GetSystemStatus command
    SystemStatusReport {
        peer_count: usize,
        active_connections: usize,
        message_count: u64,
        uptime_seconds: u64,
        transport_status: Vec<(TransportType, TransportStatus)>,
        memory_usage_bytes: Option<usize>,
    },
    /// Message status report in response to QueryMessageStatus command
    MessageStatusReport {
        message_id: MessageId,
        status: MessageDeliveryStatus,
        sent_at: Option<u64>,
        delivered_at: Option<u64>,
        retry_count: u32,
        last_error: Option<String>,
    },
    /// Peer session report in response to QueryPeerSession command
    PeerSessionReport {
        peer_id: PeerId,
        session_state: PeerSessionState,
        established_at: Option<u64>,
        last_activity: Option<u64>,
        messages_sent: u64,
        messages_received: u64,
        encryption_status: EncryptionStatus,
    },
    /// Delivery status report in response to QueryDeliveryStatus command
    DeliveryStatusReport {
        peer_id: PeerId,
        pending_messages: Vec<MessageId>,
        delivered_messages: u64,
        failed_messages: u64,
        avg_delivery_time_ms: Option<f64>,
    },
    /// Internal state report in response to QueryInternalState command
    InternalStateReport {
        peer_id: PeerId,
        active_sessions: usize,
        message_store_size: usize,
        pending_deliveries: usize,
        connection_states: Vec<(PeerId, ConnectionStatus)>,
        memory_usage_estimate: Option<usize>,
        uptime_ms: u64,
    },
}

// ----------------------------------------------------------------------------
// Supporting Types
// ----------------------------------------------------------------------------

/// Transport mechanism identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TransportType {
    Ble,
    Nostr,
    #[cfg(feature = "testing")]
    Mock,
}

impl fmt::Display for TransportType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransportType::Ble => write!(f, "BLE"),
            TransportType::Nostr => write!(f, "Nostr"),
            #[cfg(feature = "testing")]
            TransportType::Mock => write!(f, "Mock"),
        }
    }
}

/// Connection status for UI display
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionStatus {
    Disconnected,
    Discovering,
    Connecting,
    Connected,
    Error,
}

/// Transport operational status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransportStatus {
    /// Transport is active and functioning
    Active,
    /// Transport is paused by user request
    Paused,
    /// Transport is temporarily disabled due to error
    Disabled,
    /// Transport is not available on this platform
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

/// Message delivery status for query responses
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageDeliveryStatus {
    /// Message is pending delivery
    Pending,
    /// Message was delivered successfully
    Delivered,
    /// Message delivery failed permanently
    Failed,
    /// Message is being retried
    Retrying,
    /// Message delivery timed out
    TimedOut,
}

/// Peer session state for query responses
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PeerSessionState {
    /// No session exists
    None,
    /// Session is being established (handshaking)
    Establishing,
    /// Session is established and ready
    Established,
    /// Session failed or was terminated
    Failed,
    /// Session is being rekeyed
    Rekeying,
}

/// Encryption status for session reports
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EncryptionStatus {
    /// No encryption (cleartext)
    None,
    /// Noise Protocol encryption active
    NoiseProtocol,
    /// Encryption failed or compromised
    Failed,
    /// Encryption being negotiated
    Negotiating,
}

impl fmt::Display for MessageDeliveryStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MessageDeliveryStatus::Pending => write!(f, "Pending"),
            MessageDeliveryStatus::Delivered => write!(f, "Delivered"),
            MessageDeliveryStatus::Failed => write!(f, "Failed"),
            MessageDeliveryStatus::Retrying => write!(f, "Retrying"),
            MessageDeliveryStatus::TimedOut => write!(f, "TimedOut"),
        }
    }
}

impl fmt::Display for PeerSessionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PeerSessionState::None => write!(f, "None"),
            PeerSessionState::Establishing => write!(f, "Establishing"),
            PeerSessionState::Established => write!(f, "Established"),
            PeerSessionState::Failed => write!(f, "Failed"),
            PeerSessionState::Rekeying => write!(f, "Rekeying"),
        }
    }
}

impl fmt::Display for EncryptionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EncryptionStatus::None => write!(f, "None"),
            EncryptionStatus::NoiseProtocol => write!(f, "NoiseProtocol"),
            EncryptionStatus::Failed => write!(f, "Failed"),
            EncryptionStatus::Negotiating => write!(f, "Negotiating"),
        }
    }
}

// ----------------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transport_type_display() {
        assert_eq!(format!("{}", TransportType::Ble), "BLE");
        assert_eq!(format!("{}", TransportType::Nostr), "Nostr");
        #[cfg(feature = "testing")]
        assert_eq!(format!("{}", TransportType::Mock), "Mock");
    }

    #[test]
    fn test_connection_status_display() {
        assert_eq!(format!("{}", ConnectionStatus::Connected), "Connected");
        assert_eq!(
            format!("{}", ConnectionStatus::Disconnected),
            "Disconnected"
        );
    }

    #[test]
    fn test_command_serialization() {
        let cmd = Command::SendMessage {
            recipient: PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]),
            content: "test message".to_string(),
        };

        let serialized = bincode::serialize(&cmd).unwrap();
        let deserialized: Command = bincode::deserialize(&serialized).unwrap();

        match deserialized {
            Command::SendMessage { recipient, content } => {
                assert_eq!(recipient, PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]));
                assert_eq!(content, "test message");
            }
            _ => panic!("Wrong command type"),
        }
    }
}
