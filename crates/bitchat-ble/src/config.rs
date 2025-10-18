//! BLE transport configuration

use std::time::Duration;

// ----------------------------------------------------------------------------
// Configuration
// ----------------------------------------------------------------------------

/// Configuration for BLE transport
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BleTransportConfig {
    /// Maximum time to wait for scanning
    pub scan_timeout: Duration,
    /// Maximum time to wait for connection
    pub connection_timeout: Duration,
    /// Maximum packet size (BLE has limitations)
    pub max_packet_size: usize,
    /// Device name prefix for peer discovery
    pub device_name_prefix: String,
    /// Whether to automatically reconnect on disconnection
    pub auto_reconnect: bool,
}

impl Default for BleTransportConfig {
    fn default() -> Self {
        Self {
            scan_timeout: Duration::from_secs(10),
            connection_timeout: Duration::from_secs(5),
            max_packet_size: 500, // Conservative limit for BLE
            device_name_prefix: "BitChat".to_string(),
            auto_reconnect: true,
        }
    }
}

impl BleTransportConfig {
    /// Create a new configuration with custom settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Set scan timeout
    pub fn with_scan_timeout(mut self, timeout: Duration) -> Self {
        self.scan_timeout = timeout;
        self
    }

    /// Set connection timeout
    pub fn with_connection_timeout(mut self, timeout: Duration) -> Self {
        self.connection_timeout = timeout;
        self
    }

    /// Set maximum packet size
    pub fn with_max_packet_size(mut self, size: usize) -> Self {
        self.max_packet_size = size;
        self
    }

    /// Set device name prefix
    pub fn with_device_name_prefix(mut self, prefix: String) -> Self {
        self.device_name_prefix = prefix;
        self
    }

    /// Enable or disable auto-reconnect
    pub fn with_auto_reconnect(mut self, enabled: bool) -> Self {
        self.auto_reconnect = enabled;
        self
    }
}
