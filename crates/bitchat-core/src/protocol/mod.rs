//! BitChat Protocol Module
//!
//! This module contains the core BitChat messaging protocol implementation:
//! - `crypto`: Cryptographic primitives and key management
//! - `session`: Noise session management and handshakes
//! - `delivery`: Message delivery tracking and reliability
//! - `message_store`: Content-addressed message storage
//! - `connection_state`: Connection state machine management
//! - `packet`: Binary wire protocol packet format
//! - `message`: Application layer message structures
//! - `wire`: Binary serialization and wire format utilities
//! - `fragmentation`: Message fragmentation and reassembly for MTU-limited transports
//! - `deduplication`: Message deduplication using Bloom filters
//! - `file_transfer`: Secure file transfer protocol with chunked delivery
//! - `group_messaging`: Group chat functionality with member management
//! - `session_sync`: Multi-device session synchronization
//! - `capabilities`: Capability detection and version negotiation
//! - `announce`: Peer discovery announce packets with TLV encoding
//! - `tlv`: Type-Length-Value encoding for structured data
//! - `acknowledgments`: Read receipts and delivery acknowledgments

pub mod acknowledgments;
pub mod announce;
pub mod connection_state;
pub mod crypto;
pub mod deduplication;
pub mod delivery;
pub mod fragmentation;
pub mod message;
pub mod message_store;
pub mod packet;
pub mod session;
pub mod tlv;
pub mod wire;

// Experimental features (not in canonical implementation)
#[cfg(feature = "experimental")]
pub mod file_transfer;

#[cfg(feature = "experimental")]
pub mod group_messaging;

#[cfg(feature = "experimental")]
pub mod session_sync;

#[cfg(feature = "experimental")]
pub mod capabilities;

// Re-export crypto types
pub use crypto::{
    generate_fingerprint, IdentityKeyPair, NoiseHandshake, NoiseKeyPair, NoiseTransport,
};

// Re-export session types
pub use session::{NoiseSession, SessionState};

// Re-export delivery types
pub use delivery::{
    DeliveryAttempt, DeliveryConfig, DeliveryStatus, DeliveryTracker, TrackedMessage,
    EnhancedDeliveryTracker, EnhancedDeliveryStats,
};

// Re-export message store types
pub use message_store::{
    ContentAddressedMessage, ConversationId, MessageId, MessageStore, MessageStoreStats,
};

// Re-export connection state types
pub use connection_state::{
    AuditEntry, ConnectionEvent, ConnectionState, SessionParams, StateTransition,
    StateTransitionError,
};

// Re-export packet types
pub use packet::{BitchatPacket, MessageType, PacketFlags, PacketHeader};

// Re-export message types
pub use message::{BitchatMessage, MessageFlags, NoisePayload, NoisePayloadType};

// Re-export wire format types
pub use wire::{Compression, Padding, WireFormat};

// Re-export TLV types
pub use tlv::{TlvCodec, TlvEntry, TlvType};

// Re-export announce types
pub use announce::{AnnouncePayload, DiscoveredPeer};

// Re-export fragmentation types
pub use fragmentation::{Fragment, FragmentHeader, MessageFragmenter, MessageReassembler};

// Re-export deduplication types
pub use deduplication::{BloomFilter, DeduplicationManager, DeduplicationStats, PacketId};

// Re-export acknowledgment types
pub use acknowledgments::{
    DeliveryAck, EnhancedDeliveryStatus, ReadReceipt, ReceiptManager, ReceiptStats, ReceiptType,
};

// Experimental re-exports (only available with experimental feature flag)
#[cfg(feature = "experimental")]
pub use file_transfer::{
    FileAccept, FileChunk, FileComplete, FileHash, FileMetadata, FileOffer, FileTransferId,
    FileTransferManager, FileTransferMessage, FileTransferSession, TransferStatus,
};

#[cfg(feature = "experimental")]
pub use group_messaging::{
    GroupCreate, GroupId, GroupInvite, GroupJoin, GroupKick, GroupLeave, GroupManager, GroupMember,
    GroupMessage, GroupMessagingMessage, GroupMetadata, GroupRole, GroupSettings, GroupUpdate,
};

#[cfg(feature = "experimental")]
pub use session_sync::{
    DeviceAnnouncement, DeviceCapabilities, DeviceHeartbeat, DeviceId, DeviceInfo, DeviceStatus,
    DeviceType, MessageRef, MultiDeviceSessionManager, SessionStatus, SessionSyncMessage,
    SessionSyncRequest, SessionSyncResponse, SessionSyncState,
};

#[cfg(feature = "experimental")]
pub use capabilities::{
    Capability, CapabilityId, CapabilityManager, CapabilityMessage, CapabilityRejection,
    ImplementationInfo, NegotiationStatus, ProtocolVersion, RejectionReason, VersionAck,
    VersionHello,
};
