//! Fallback advertising implementation for unsupported platforms

use bitchat_core::{PeerId, Result as BitchatResult};
use bitchat_core::internal::IdentityKeyPair;
use tracing::warn;

use crate::config::BleTransportConfig;
use crate::protocol::generate_device_name;

use super::BleAdvertiser;

// ----------------------------------------------------------------------------
// Fallback Implementation
// ----------------------------------------------------------------------------

/// Fallback advertising implementation for unsupported platforms
pub struct FallbackAdvertiser {
    #[allow(dead_code)]
    is_advertising: bool,
}

impl FallbackAdvertiser {
    pub fn new() -> Self {
        Self {
            is_advertising: false,
        }
    }
}

impl Default for FallbackAdvertiser {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl BleAdvertiser for FallbackAdvertiser {
    async fn start_advertising(
        &mut self,
        peer_id: &PeerId,
        _identity: &IdentityKeyPair,
        config: &BleTransportConfig,
    ) -> BitchatResult<()> {
        let device_name = generate_device_name(peer_id, &config.device_name_prefix);
        warn!(
            "BLE advertising not supported on this platform. Device '{}' will not be discoverable. \
            Consider using a supported platform (Linux with BlueZ or macOS) for full functionality.",
            device_name
        );
        Ok(())
    }

    async fn stop_advertising(&mut self) -> BitchatResult<()> {
        Ok(())
    }

    fn is_advertising(&self) -> bool {
        false
    }

    async fn update_advertising_data(
        &mut self,
        _peer_id: &PeerId,
        _identity: &IdentityKeyPair,
        _config: &BleTransportConfig,
    ) -> BitchatResult<()> {
        Ok(())
    }
}
