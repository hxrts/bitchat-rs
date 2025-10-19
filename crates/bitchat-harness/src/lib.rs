#![cfg_attr(not(feature = "std"), no_std)]
//! BitChat Harness - Comprehensive Testing Framework
//!
//! Provides the canonical channel/message schema, shared runtime utilities,
//! and comprehensive testing infrastructure for BitChat protocol development.
//!
//! # Overview
//!
//! The BitChat Harness is designed to make testing BitChat protocols as simple
//! and powerful as possible. It provides:
//!
//! - **High-level TestHarness**: Encapsulates all boilerplate setup
//! - **Realistic Network Simulation**: MockTransport with real-world conditions
//! - **Comprehensive Network Models**: Ideal, lossy, high-latency, mobile, adversarial
//! - **Advanced Features**: Packet loss, duplication, corruption, reordering, bandwidth limits
//!
//! # Quick Start
//!
//! ```rust,ignore
//! // This example requires the "testing" feature to be enabled
//! use bitchat_harness::TestHarness;
//! use bitchat_core::PeerId;
//! use std::time::Duration;
//!
//! #[tokio::test]
//! async fn test_basic_messaging() {
//!     let mut harness = TestHarness::new().await;
//!     
//!     // Add a peer to the network
//!     let peer_id = PeerId::new([9, 10, 11, 12, 13, 14, 15, 16]);
//!     harness.network.add_peer(peer_id).await.unwrap();
//!     
//!     // Send a message to the peer
//!     harness.send_message_to_peer(peer_id, b"hello world".to_vec()).await.unwrap();
//!
//!     // Verify the message was sent to the network
//!     if let Some(outgoing_packet) = harness.network.expect_outgoing_timeout(Duration::from_secs(1)).await {
//!         assert_eq!(outgoing_packet.to, peer_id);
//!         assert!(!outgoing_packet.payload.is_empty());
//!     }
//!     
//!     harness.shutdown().await.unwrap();
//! }
//! ```
//!
//! # Network Simulation Features
//!
//! The MockTransport simulates real-world network conditions:
//!
//! - **Latency & Jitter**: Variable delivery times with realistic jitter
//! - **Packet Loss**: Random and burst packet loss patterns
//! - **Duplication**: Simulate network-level packet duplication
//! - **Corruption**: Random bit flips in packet payloads
//! - **Reordering**: Out-of-order packet delivery
//! - **Bandwidth Limits**: Throughput throttling
//! - **Connection Issues**: Random disconnections and reconnections

extern crate alloc;

pub mod messages;
pub mod transport;

#[cfg(feature = "testing")]
pub mod mock_transport;

#[cfg(feature = "testing")]
mod test_harness;

pub use messages::{
    InboundMessage, OutboundMessage, RawInboundMessage, RawOutboundMessage, TransportMetadata,
};
pub use transport::{
    HeartbeatConfig, HeartbeatManager, MessageProcessor, ReconnectConfig, ReconnectManager,
    TransportBuilder, TransportHandle, TransportLifecycle, TransportState,
};

#[cfg(feature = "testing")]
pub use mock_transport::{MockTransport, MockTransportConfig, MockTransportStats};

#[cfg(feature = "testing")]
pub use test_harness::{MockNetworkHandle, NetworkPacket, PacketMetadata, TestHarness};
