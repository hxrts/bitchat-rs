//! High-level advertising manager

use std::time::Duration;

use bitchat_core::{PeerId, Result as BitchatResult};
use bitchat_core::internal::IdentityKeyPair;
use tracing::{debug, info};

use crate::config::BleTransportConfig;

use super::{BleAdvertiser, PlatformAdvertiser};

// ----------------------------------------------------------------------------
// Advertising Manager
// ----------------------------------------------------------------------------

/// High-level advertising manager that handles platform-specific implementations
pub struct AdvertisingManager {
    advertiser: PlatformAdvertiser,
    current_peer_id: Option<PeerId>,
    #[allow(dead_code)]
    rotation_interval: Option<Duration>,
}

impl AdvertisingManager {
    /// Create a new advertising manager
    pub fn new() -> Self {
        Self {
            advertiser: PlatformAdvertiser::new(),
            current_peer_id: None,
            rotation_interval: None,
        }
    }

    /// Start advertising for the given peer
    pub async fn start(
        &mut self,
        peer_id: PeerId,
        identity: &IdentityKeyPair,
        config: &BleTransportConfig,
    ) -> BitchatResult<()> {
        self.advertiser.start_advertising(&peer_id, identity, config).await?;
        self.current_peer_id = Some(peer_id);
        info!("BLE advertising started for peer {}", peer_id);
        Ok(())
    }

    /// Stop advertising
    pub async fn stop(&mut self) -> BitchatResult<()> {
        self.advertiser.stop_advertising().await?;
        self.current_peer_id = None;
        info!("BLE advertising stopped");
        Ok(())
    }

    /// Check if currently advertising
    pub fn is_advertising(&self) -> bool {
        self.advertiser.is_advertising()
    }

    /// Enable periodic rotation of advertising data (for privacy)
    #[allow(dead_code)]
    pub fn enable_rotation(&mut self, interval: Duration) {
        self.rotation_interval = Some(interval);
    }

    /// Manually rotate advertising data
    pub async fn rotate(&mut self, identity: &IdentityKeyPair, config: &BleTransportConfig) -> BitchatResult<()> {
        if let Some(peer_id) = self.current_peer_id {
            self.advertiser
                .update_advertising_data(&peer_id, identity, config)
                .await?;
            debug!("Rotated BLE advertising data for peer {}", peer_id);
        }
        Ok(())
    }
}

impl Default for AdvertisingManager {
    fn default() -> Self {
        Self::new()
    }
}
