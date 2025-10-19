//! Cross-platform advertising trait and platform detection

pub mod fallback;
#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
pub mod manager;

// Re-export manager types
pub use manager::AdvertisingManager;

use bitchat_core::{PeerId, Result as BitchatResult};
use bitchat_core::internal::IdentityKeyPair;

use crate::config::BleTransportConfig;

// ----------------------------------------------------------------------------
// Cross-platform Advertising Trait
// ----------------------------------------------------------------------------

/// Trait for BLE advertising functionality across different platforms
#[async_trait::async_trait]
pub trait BleAdvertiser: Send + Sync {
    /// Start advertising with the given configuration
    async fn start_advertising(
        &mut self,
        peer_id: &PeerId,
        identity: &IdentityKeyPair,
        config: &BleTransportConfig,
    ) -> BitchatResult<()>;

    /// Stop advertising
    async fn stop_advertising(&mut self) -> BitchatResult<()>;

    /// Check if currently advertising
    fn is_advertising(&self) -> bool;

    /// Update advertising data (e.g., for rotating peer announcements)
    async fn update_advertising_data(
        &mut self,
        peer_id: &PeerId,
        identity: &IdentityKeyPair,
        config: &BleTransportConfig,
    ) -> BitchatResult<()>;
}

// ----------------------------------------------------------------------------
// Platform Detection and Factory
// ----------------------------------------------------------------------------

/// Platform-specific advertiser enum
pub enum PlatformAdvertiser {
    #[cfg(target_os = "linux")]
    Linux(linux::LinuxAdvertiser),
    #[cfg(target_os = "macos")]
    MacOS(macos::MacOSAdvertiser),
    #[allow(dead_code)]
    Fallback(fallback::FallbackAdvertiser),
}

impl PlatformAdvertiser {
    /// Create the appropriate advertiser for the current platform
    pub fn new() -> Self {
        #[cfg(target_os = "linux")]
        {
            Self::Linux(linux::LinuxAdvertiser::new())
        }
        #[cfg(target_os = "macos")]
        {
            Self::MacOS(macos::MacOSAdvertiser::new())
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            Self::Fallback(fallback::FallbackAdvertiser::new())
        }
    }
}

impl Default for PlatformAdvertiser {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl BleAdvertiser for PlatformAdvertiser {
    async fn start_advertising(
        &mut self,
        peer_id: &PeerId,
        identity: &IdentityKeyPair,
        config: &BleTransportConfig,
    ) -> BitchatResult<()> {
        match self {
            #[cfg(target_os = "linux")]
            Self::Linux(ref mut advertiser) => advertiser.start_advertising(peer_id, identity, config).await,
            #[cfg(target_os = "macos")]
            Self::MacOS(ref mut advertiser) => advertiser.start_advertising(peer_id, identity, config).await,
            Self::Fallback(ref mut advertiser) => {
                advertiser.start_advertising(peer_id, identity, config).await
            }
        }
    }

    async fn stop_advertising(&mut self) -> BitchatResult<()> {
        match self {
            #[cfg(target_os = "linux")]
            Self::Linux(ref mut advertiser) => advertiser.stop_advertising().await,
            #[cfg(target_os = "macos")]
            Self::MacOS(ref mut advertiser) => advertiser.stop_advertising().await,
            Self::Fallback(ref mut advertiser) => advertiser.stop_advertising().await,
        }
    }

    fn is_advertising(&self) -> bool {
        match self {
            #[cfg(target_os = "linux")]
            Self::Linux(ref advertiser) => advertiser.is_advertising(),
            #[cfg(target_os = "macos")]
            Self::MacOS(ref advertiser) => advertiser.is_advertising(),
            Self::Fallback(ref advertiser) => advertiser.is_advertising(),
        }
    }

    async fn update_advertising_data(
        &mut self,
        peer_id: &PeerId,
        identity: &IdentityKeyPair,
        config: &BleTransportConfig,
    ) -> BitchatResult<()> {
        match self {
            #[cfg(target_os = "linux")]
            Self::Linux(ref mut advertiser) => {
                advertiser.update_advertising_data(peer_id, identity, config).await
            }
            #[cfg(target_os = "macos")]
            Self::MacOS(ref mut advertiser) => {
                advertiser.update_advertising_data(peer_id, identity, config).await
            }
            Self::Fallback(ref mut advertiser) => {
                advertiser.update_advertising_data(peer_id, identity, config).await
            }
        }
    }
}
