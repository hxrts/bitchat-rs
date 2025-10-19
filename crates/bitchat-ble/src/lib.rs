//! Bluetooth Low Energy transport implementation for BitChat
//!
//! This crate provides a BLE transport that implements the `Transport` trait from
//! `bitchat-core`, enabling BitChat communication over Bluetooth Low Energy.
//!
//! ## Architecture
//!
//! The BLE transport is organized into several modules:
//!
//! - [`config`] - Transport configuration and settings
//! - [`error`] - Error types specific to BLE transport
//! - [`protocol`] - BLE protocol constants and utilities
//! - [`peer`] - Peer state management and connection tracking
//! - [`discovery`] - Device scanning and peer discovery
//! - [`connection`] - Connection management and data transmission
//! - [`transport`] - Main transport implementation
//!
//! ## Usage
//!
//! ```rust,no_run
//! use bitchat_ble::BleTransportTask;
//! use bitchat_core::TransportTask;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create channels for communication with Core Logic
//! let (event_sender, _event_receiver) = tokio::sync::mpsc::channel(100);
//! let (_effect_sender, effect_receiver) = tokio::sync::broadcast::channel(100);
//!
//! let mut transport = BleTransportTask::new();
//! transport.attach_channels(event_sender, effect_receiver)?;
//!
//! // Start the transport task - includes production-ready advertising
//! transport.run().await?;
//!
//! // The transport will automatically:
//! // - Start advertising on all supported platforms (Linux, macOS)
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
//! - **Other platforms**: Scanning only (no advertising)
//!
//! ### Discovery Support
//! Linux and macOS support peer discovery via btleplug's central mode scanning.

mod advertising;
mod config;
mod connection;
mod discovery;
mod error;
mod peer;
mod protocol;
mod transport;

// Public API exports
pub use advertising::{AdvertisingManager, BleAdvertiser};
pub use config::BleTransportConfig;
pub use error::BleTransportError;
pub use peer::{BlePeer, ConnectionState};
pub use protocol::{
    generate_device_name, BITCHAT_RX_CHARACTERISTIC_UUID, BITCHAT_SERVICE_UUID,
    BITCHAT_TX_CHARACTERISTIC_UUID,
};
pub use transport::BleTransportTask;

// Re-export TransportTask trait for convenience
pub use bitchat_core::TransportTask;
