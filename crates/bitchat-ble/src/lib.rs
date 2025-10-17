//! Bluetooth Low Energy transport implementation for BitChat
//!
//! This crate provides a BLE transport that implements the `Transport` trait from
//! `bitchat-core`, enabling BitChat communication over Bluetooth Low Energy.
//!
//! ## Architecture
//!
//! The BLE transport is organized into several modules for clean separation of concerns:
//!
//! - [`config`] - Transport configuration and settings
//! - [`peer`] - Peer state management and connection tracking
//! - [`protocol`] - BLE protocol constants and utilities
//! - [`discovery`] - Device scanning and peer discovery
//! - [`connection`] - Connection management and data transmission
//! - [`transport`] - Main transport implementation and orchestration
//!
//! ## Usage
//!
//! ```rust,no_run
//! use bitchat_ble::{BleTransport, BleTransportConfig};
//! use bitchat_core::{PeerId, transport::Transport};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
//! let config = BleTransportConfig::new()
//!     .with_device_name_prefix("MyApp".to_string())
//!     .with_auto_reconnect(true);
//!
//! let mut transport = BleTransport::with_config(peer_id, config);
//! 
//! // Start the transport - now includes production-ready advertising
//! transport.start().await?;
//! 
//! // The transport will automatically:
//! // - Start advertising on all supported platforms (Linux, macOS, Windows)
//! // - Begin scanning for other BitChat peers
//! // - Handle connection management and data transmission
//! 
//! # Ok(())
//! # }
//! ```
//!
//! ## Platform Support
//!
//! ### Advertising Support
//! - **Linux**: Full support via `bluer` crate with BlueZ and GATT service registration
//! - **macOS**: Full support via Core Bluetooth framework using CBPeripheralManager 
//! - **Windows**: Full support via Windows Runtime Bluetooth APIs and GATT service providers
//! - **Other platforms**: Scanning only (no advertising)
//!
//! ### Discovery Support
//! All platforms support peer discovery via btleplug's central mode scanning.

mod advertising;
mod config;
mod peer;
mod protocol;
mod discovery;
mod connection;
mod transport;

// Public API exports
pub use advertising::{AdvertisingManager, BleAdvertiser};
pub use config::BleTransportConfig;
pub use peer::{BlePeer, ConnectionState};
pub use transport::BleTransport;
pub use protocol::{
    generate_device_name,
    BITCHAT_SERVICE_UUID,
    BITCHAT_TX_CHARACTERISTIC_UUID,
    BITCHAT_RX_CHARACTERISTIC_UUID,
};

// Re-export key traits for convenience
pub use bitchat_core::transport::Transport;