//! BLE device discovery, scanning, and advertising
//!
//! This module provides comprehensive BLE functionality including:
//! - Device scanning and peer discovery using btleplug
//! - Cross-platform BLE advertising with platform-specific implementations
//! - Production-ready peripheral mode support on supported platforms

use std::collections::HashMap;
use std::sync::Arc;

use bitchat_core::internal::{IdentityKeyPair, TransportError};
use bitchat_core::{BitchatError, PeerId, Result as BitchatResult};
use btleplug::api::{Central, CentralEvent, Manager as _, Peripheral, ScanFilter};
use btleplug::platform::{Adapter, Manager};
use tokio::sync::RwLock;
use tracing::{debug, info};

use crate::advertising::AdvertisingManager;
use crate::config::BleTransportConfig;
use crate::peer::BlePeer;
use crate::protocol::{extract_and_verify_peer_id, BITCHAT_SERVICE_UUID};

// ----------------------------------------------------------------------------
// Discovery Implementation
// ----------------------------------------------------------------------------

/// Handles BLE device discovery and scanning
pub struct BleDiscovery {
    config: BleTransportConfig,
    adapter: Option<Adapter>,
    advertising_manager: AdvertisingManager,
}

impl BleDiscovery {
    /// Create a new discovery manager
    pub fn new(config: BleTransportConfig) -> Self {
        Self {
            config,
            adapter: None,
            advertising_manager: AdvertisingManager::new(),
        }
    }

    /// Initialize BLE adapter
    #[allow(dead_code)]
    pub async fn initialize_adapter(&mut self) -> BitchatResult<()> {
        let manager = Manager::new().await.map_err(|e| {
            BitchatError::Transport(TransportError::InvalidConfiguration {
                reason: format!("Failed to create BLE manager: {}", e),
            })
        })?;

        let adapters = manager.adapters().await.map_err(|e| {
            BitchatError::Transport(TransportError::InvalidConfiguration {
                reason: format!("Failed to get BLE adapters: {}", e),
            })
        })?;

        if adapters.is_empty() {
            return Err(BitchatError::Transport(
                TransportError::TransportUnavailable {
                    transport_type: "BLE".to_string(),
                },
            ));
        }

        self.adapter = Some(adapters[0].clone());
        info!("BLE adapter initialized");
        Ok(())
    }

    /// Get adapter reference
    #[allow(dead_code)]
    pub fn adapter(&self) -> Option<&Adapter> {
        self.adapter.as_ref()
    }

    /// Start scanning for BitChat peers
    pub async fn start_scanning(&self) -> BitchatResult<()> {
        let adapter = self.adapter.as_ref().ok_or_else(|| {
            BitchatError::Transport(TransportError::TransportUnavailable {
                transport_type: "BLE adapter not initialized".to_string(),
            })
        })?;

        let scan_filter = ScanFilter {
            services: vec![BITCHAT_SERVICE_UUID],
        };

        adapter.start_scan(scan_filter).await.map_err(|e| {
            BitchatError::Transport(TransportError::InvalidConfiguration {
                reason: format!("Failed to start BLE scan: {}", e),
            })
        })?;

        info!("Started BLE scanning for BitChat peers");
        Ok(())
    }

    /// Stop scanning for peers
    pub async fn stop_scanning(&self) -> BitchatResult<()> {
        if let Some(adapter) = &self.adapter {
            adapter.stop_scan().await.map_err(|e| {
                BitchatError::Transport(TransportError::InvalidConfiguration {
                    reason: format!("Failed to stop BLE scan: {}", e),
                })
            })?;
        }
        Ok(())
    }

    /// Process discovery events from BLE adapter
    ///
    /// Uses cryptographic verification to prevent peer impersonation attacks.
    /// Only devices with valid signed PeerAnnouncements will be accepted.
    #[allow(dead_code)]
    pub async fn process_discovery_event(
        &self,
        event: CentralEvent,
        peers: &Arc<RwLock<HashMap<PeerId, BlePeer>>>,
        cached_peers: &Arc<RwLock<Vec<PeerId>>>,
    ) -> BitchatResult<()> {
        match event {
            CentralEvent::DeviceDiscovered(id) => {
                if let Some(adapter) = &self.adapter {
                    if let Ok(peripheral) = adapter.peripheral(&id).await {
                        if let Ok(Some(properties)) = peripheral.properties().await {
                            if let Some(name) = &properties.local_name {
                                if name.starts_with(&self.config.device_name_prefix) {
                                    // Get manufacturer-specific data for secure verification
                                    let manufacturer_data = properties
                                        .manufacturer_data
                                        .get(&0xFFFF) // Use company ID 0xFFFF for test/development
                                        .cloned()
                                        .unwrap_or_default();

                                    if manufacturer_data.is_empty() {
                                        debug!("Rejecting device '{}' - missing secure advertising data", name);
                                        return Ok(());
                                    }

                                    match extract_and_verify_peer_id(
                                        name,
                                        &manufacturer_data,
                                        &self.config.device_name_prefix,
                                        60, // 60 second max age
                                    ) {
                                        Ok(Some(peer_id)) => {
                                            let ble_peer =
                                                BlePeer::new(peer_id, peripheral, name.clone());

                                            let mut peers_lock = peers.write().await;
                                            if let std::collections::hash_map::Entry::Vacant(e) =
                                                peers_lock.entry(peer_id)
                                            {
                                                debug!(
                                                    "Discovered secure BitChat peer: {} ({})",
                                                    peer_id, name
                                                );
                                                e.insert(ble_peer);

                                                // Update cached peers list
                                                let mut cached = cached_peers.write().await;
                                                cached.push(peer_id);
                                            }
                                        }
                                        Ok(None) => {
                                            debug!(
                                                "Cryptographic verification failed for device: {}",
                                                name
                                            );
                                        }
                                        Err(e) => {
                                            debug!(
                                                "Invalid advertising data from device '{}': {}",
                                                name, e
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            CentralEvent::DeviceDisconnected(id) => {
                // Find peer by peripheral ID and mark as disconnected
                let mut peers_lock = peers.write().await;
                for peer in peers_lock.values_mut() {
                    if peer.peripheral_id() == id {
                        peer.mark_disconnected();
                        debug!("Peer {} disconnected", peer.peer_id);
                        break;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Start advertising as a BitChat peer
    ///
    /// Uses platform-specific implementations to provide full BLE peripheral mode support:
    /// - Linux: Uses bluer crate with BlueZ for complete GATT service implementation
    /// - macOS: Uses Core Bluetooth framework via Objective-C bindings for CBPeripheralManager
    /// - Other platforms: Logs warning about lack of support
    pub async fn start_advertising(
        &mut self,
        peer_id: PeerId,
        identity: &IdentityKeyPair,
    ) -> BitchatResult<()> {
        self.advertising_manager
            .start(peer_id, identity, &self.config)
            .await?;
        info!("Started BLE advertising for peer {}", peer_id);
        Ok(())
    }

    /// Stop advertising
    pub async fn stop_advertising(&mut self) -> BitchatResult<()> {
        self.advertising_manager.stop().await?;
        info!("Stopped BLE advertising");
        Ok(())
    }

    /// Check if currently advertising
    #[allow(dead_code)]
    pub fn is_advertising(&self) -> bool {
        self.advertising_manager.is_advertising()
    }

    /// Rotate advertising data for privacy
    #[allow(dead_code)]
    pub async fn rotate_advertising(&mut self, identity: &IdentityKeyPair) -> BitchatResult<()> {
        self.advertising_manager
            .rotate(identity, &self.config)
            .await?;
        debug!("Rotated BLE advertising data");
        Ok(())
    }
}
