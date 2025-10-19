//! Linux BLE advertising implementation using bluer (BlueZ)

use bitchat_core::internal::{IdentityKeyPair, TransportError};
use bitchat_core::{BitchatError, PeerId, Result as BitchatResult};
use tracing::{debug, info};

use crate::config::BleTransportConfig;
use crate::protocol::{
    generate_advertising_data, generate_device_name, BITCHAT_RX_CHARACTERISTIC_UUID,
    BITCHAT_SERVICE_UUID, BITCHAT_TX_CHARACTERISTIC_UUID,
};

use super::BleAdvertiser;

// ----------------------------------------------------------------------------
// Linux Implementation
// ----------------------------------------------------------------------------

pub struct LinuxAdvertiser {
    session: Option<bluer::Session>,
    adapter: Option<bluer::Adapter>,
    advertisement_handle: Option<bluer::adv::Advertisement>,
    is_advertising: bool,
}

impl LinuxAdvertiser {
    pub fn new() -> Self {
        Self {
            session: None,
            adapter: None,
            advertisement_handle: None,
            is_advertising: false,
        }
    }

    async fn initialize(&mut self) -> BitchatResult<()> {
        if self.session.is_some() {
            return Ok(());
        }

        let session = bluer::Session::new().await.map_err(|e| {
            BitchatError::Transport(TransportError::TransportUnavailable {
                transport_type: format!("BlueZ session: {}", e),
            })
        })?;

        let adapter = session.default_adapter().await.map_err(|e| {
            BitchatError::Transport(TransportError::TransportUnavailable {
                transport_type: format!("BLE adapter: {}", e),
            })
        })?;

        // Enable adapter if needed
        if !adapter.is_powered().await.unwrap_or(false) {
            adapter.set_powered(true).await.map_err(|e| {
                BitchatError::Transport(TransportError::InvalidConfiguration {
                    reason: format!("Failed to power on adapter: {}", e),
                })
            })?;
        }

        self.session = Some(session);
        self.adapter = Some(adapter);
        info!("Linux BLE adapter initialized for advertising");
        Ok(())
    }
}

#[async_trait::async_trait]
impl BleAdvertiser for LinuxAdvertiser {
    async fn start_advertising(
        &mut self,
        peer_id: &PeerId,
        identity: &IdentityKeyPair,
        config: &BleTransportConfig,
    ) -> BitchatResult<()> {
        self.initialize().await?;

        let adapter = self.adapter.as_ref().unwrap();
        let device_name = generate_device_name(peer_id, &config.device_name_prefix);

        // Create GATT service
        let app = bluer::gatt::local::Application::new();
        let service_handle = app.service(BITCHAT_SERVICE_UUID).unwrap();

        // Add TX characteristic (write)
        let _tx_char = service_handle
            .characteristic(BITCHAT_TX_CHARACTERISTIC_UUID)
            .write(|data, _| {
                debug!("Received data on TX characteristic: {} bytes", data.len());
                Ok(())
            })
            .build();

        // Add RX characteristic (read/notify)
        let _rx_char = service_handle
            .characteristic(BITCHAT_RX_CHARACTERISTIC_UUID)
            .read(|_| Ok(vec![]))
            .notify()
            .build();

        // Register GATT application
        let app_handle = adapter.serve_gatt_application(app).await.map_err(|e| {
            BitchatError::Transport(TransportError::InvalidConfiguration {
                reason: format!("Failed to register GATT service: {}", e),
            })
        })?;

        // Generate secure advertising data
        let secure_advertising_data = generate_advertising_data(*peer_id, identity, &device_name)?;

        // Create manufacturer data map
        let mut manufacturer_data = std::collections::HashMap::new();
        manufacturer_data.insert(0xFFFF, secure_advertising_data); // Use 0xFFFF for test/development

        // Create advertisement
        let advertisement = bluer::adv::Advertisement {
            advertisement_type: bluer::adv::Type::Peripheral,
            local_name: Some(device_name.clone()),
            services: vec![BITCHAT_SERVICE_UUID].into_iter().collect(),
            manufacturer_data,
            discoverable: Some(true),
            connectable: Some(true),
            ..Default::default()
        };

        let advertisement_handle = adapter.advertise(advertisement).await.map_err(|e| {
            BitchatError::Transport(TransportError::InvalidConfiguration {
                reason: format!("Failed to start advertising: {}", e),
            })
        })?;

        self.advertisement_handle = Some(advertisement_handle);
        self.is_advertising = true;

        info!("Started BLE advertising as '{}'", device_name);
        Ok(())
    }

    async fn stop_advertising(&mut self) -> BitchatResult<()> {
        if let Some(handle) = self.advertisement_handle.take() {
            drop(handle); // Dropping the handle stops advertising
            self.is_advertising = false;
            info!("Stopped BLE advertising");
        }
        Ok(())
    }

    fn is_advertising(&self) -> bool {
        self.is_advertising
    }

    async fn update_advertising_data(
        &mut self,
        peer_id: &PeerId,
        identity: &IdentityKeyPair,
        config: &BleTransportConfig,
    ) -> BitchatResult<()> {
        if self.is_advertising {
            self.stop_advertising().await?;
            self.start_advertising(peer_id, identity, config).await?;
        }
        Ok(())
    }
}
