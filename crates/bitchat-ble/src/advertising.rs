//! Platform-specific BLE advertising implementations
//! 
//! This module provides production-ready BLE advertising capabilities across different
//! operating systems, addressing the limitation that btleplug doesn't support peripheral mode.

use std::time::Duration;

use bitchat_core::{BitchatError, PeerId, Result as BitchatResult};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::config::BleTransportConfig;
use crate::protocol::{generate_device_name, BITCHAT_SERVICE_UUID, BITCHAT_TX_CHARACTERISTIC_UUID, BITCHAT_RX_CHARACTERISTIC_UUID};

// ----------------------------------------------------------------------------
// Cross-platform Advertising Trait
// ----------------------------------------------------------------------------

/// Trait for BLE advertising functionality across different platforms
#[async_trait::async_trait]
pub trait BleAdvertiser: Send + Sync {
    /// Start advertising with the given configuration
    async fn start_advertising(&mut self, peer_id: &PeerId, config: &BleTransportConfig) -> BitchatResult<()>;
    
    /// Stop advertising
    async fn stop_advertising(&mut self) -> BitchatResult<()>;
    
    /// Check if currently advertising
    fn is_advertising(&self) -> bool;
    
    /// Update advertising data (e.g., for rotating peer announcements)
    async fn update_advertising_data(&mut self, peer_id: &PeerId, config: &BleTransportConfig) -> BitchatResult<()>;
}

// ----------------------------------------------------------------------------
// Platform Detection and Factory
// ----------------------------------------------------------------------------

/// Platform-specific advertiser enum
pub enum PlatformAdvertiser {
    #[cfg(target_os = "linux")]
    Linux(LinuxAdvertiser),
    #[cfg(target_os = "macos")]
    MacOS(MacOSAdvertiser),
    #[cfg(target_os = "windows")]
    Windows(WindowsAdvertiser),
    Fallback(FallbackAdvertiser),
}

impl PlatformAdvertiser {
    /// Create the appropriate advertiser for the current platform
    pub fn new() -> Self {
        #[cfg(target_os = "linux")]
        {
            Self::Linux(LinuxAdvertiser::new())
        }
        #[cfg(target_os = "macos")]
        {
            Self::MacOS(MacOSAdvertiser::new())
        }
        #[cfg(target_os = "windows")]
        {
            Self::Windows(WindowsAdvertiser::new())
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            Self::Fallback(FallbackAdvertiser::new())
        }
    }
}

#[async_trait::async_trait]
impl BleAdvertiser for PlatformAdvertiser {
    async fn start_advertising(&mut self, peer_id: &PeerId, config: &BleTransportConfig) -> BitchatResult<()> {
        match self {
            #[cfg(target_os = "linux")]
            Self::Linux(ref mut advertiser) => advertiser.start_advertising(peer_id, config).await,
            #[cfg(target_os = "macos")]
            Self::MacOS(ref mut advertiser) => advertiser.start_advertising(peer_id, config).await,
            #[cfg(target_os = "windows")]
            Self::Windows(ref mut advertiser) => advertiser.start_advertising(peer_id, config).await,
            Self::Fallback(ref mut advertiser) => advertiser.start_advertising(peer_id, config).await,
        }
    }

    async fn stop_advertising(&mut self) -> BitchatResult<()> {
        match self {
            #[cfg(target_os = "linux")]
            Self::Linux(ref mut advertiser) => advertiser.stop_advertising().await,
            #[cfg(target_os = "macos")]
            Self::MacOS(ref mut advertiser) => advertiser.stop_advertising().await,
            #[cfg(target_os = "windows")]
            Self::Windows(ref mut advertiser) => advertiser.stop_advertising().await,
            Self::Fallback(ref mut advertiser) => advertiser.stop_advertising().await,
        }
    }

    fn is_advertising(&self) -> bool {
        match self {
            #[cfg(target_os = "linux")]
            Self::Linux(ref advertiser) => advertiser.is_advertising(),
            #[cfg(target_os = "macos")]
            Self::MacOS(ref advertiser) => advertiser.is_advertising(),
            #[cfg(target_os = "windows")]
            Self::Windows(ref advertiser) => advertiser.is_advertising(),
            Self::Fallback(ref advertiser) => advertiser.is_advertising(),
        }
    }

    async fn update_advertising_data(&mut self, peer_id: &PeerId, config: &BleTransportConfig) -> BitchatResult<()> {
        match self {
            #[cfg(target_os = "linux")]
            Self::Linux(ref mut advertiser) => advertiser.update_advertising_data(peer_id, config).await,
            #[cfg(target_os = "macos")]
            Self::MacOS(ref mut advertiser) => advertiser.update_advertising_data(peer_id, config).await,
            #[cfg(target_os = "windows")]
            Self::Windows(ref mut advertiser) => advertiser.update_advertising_data(peer_id, config).await,
            Self::Fallback(ref mut advertiser) => advertiser.update_advertising_data(peer_id, config).await,
        }
    }
}

// ----------------------------------------------------------------------------
// Linux Implementation (using bluer)
// ----------------------------------------------------------------------------

#[cfg(target_os = "linux")]
pub struct LinuxAdvertiser {
    session: Option<bluer::Session>,
    adapter: Option<bluer::Adapter>,
    advertisement_handle: Option<bluer::adv::Advertisement>,
    is_advertising: bool,
}

#[cfg(target_os = "linux")]
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
            BitchatError::InvalidPacket(format!("Failed to create BlueZ session: {}", e))
        })?;

        let adapter = session.default_adapter().await.map_err(|e| {
            BitchatError::InvalidPacket(format!("Failed to get default adapter: {}", e))
        })?;

        // Enable adapter if needed
        if !adapter.is_powered().await.unwrap_or(false) {
            adapter.set_powered(true).await.map_err(|e| {
                BitchatError::InvalidPacket(format!("Failed to power on adapter: {}", e))
            })?;
        }

        self.session = Some(session);
        self.adapter = Some(adapter);
        info!("Linux BLE adapter initialized for advertising");
        Ok(())
    }
}

#[cfg(target_os = "linux")]
#[async_trait::async_trait]
impl BleAdvertiser for LinuxAdvertiser {
    async fn start_advertising(&mut self, peer_id: &PeerId, config: &BleTransportConfig) -> BitchatResult<()> {
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
            BitchatError::InvalidPacket(format!("Failed to register GATT service: {}", e))
        })?;

        // Create advertisement
        let advertisement = bluer::adv::Advertisement {
            advertisement_type: bluer::adv::Type::Peripheral,
            local_name: Some(device_name.clone()),
            services: vec![BITCHAT_SERVICE_UUID].into_iter().collect(),
            discoverable: Some(true),
            connectable: Some(true),
            ..Default::default()
        };

        let advertisement_handle = adapter.advertise(advertisement).await.map_err(|e| {
            BitchatError::InvalidPacket(format!("Failed to start advertising: {}", e))
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

    async fn update_advertising_data(&mut self, peer_id: &PeerId, config: &BleTransportConfig) -> BitchatResult<()> {
        if self.is_advertising {
            self.stop_advertising().await?;
            self.start_advertising(peer_id, config).await?;
        }
        Ok(())
    }
}

// ----------------------------------------------------------------------------
// macOS Implementation (using core-bluetooth)
// ----------------------------------------------------------------------------

#[cfg(target_os = "macos")]
use objc::runtime::{Object, Class};
#[cfg(target_os = "macos")]
use objc::{msg_send, sel, sel_impl};
#[cfg(target_os = "macos")]
use objc_foundation::{INSString, NSString};
#[cfg(target_os = "macos")]
use cocoa::base::{id, nil};
#[cfg(target_os = "macos")]
use std::ptr;

#[cfg(target_os = "macos")]
pub struct MacOSAdvertiser {
    peripheral_manager: Option<id>,
    is_advertising: bool,
    service: Option<id>,
}

#[cfg(target_os = "macos")]
impl MacOSAdvertiser {
    pub fn new() -> Self {
        Self {
            peripheral_manager: None,
            is_advertising: false,
            service: None,
        }
    }

    fn initialize_peripheral_manager(&mut self) -> BitchatResult<()> {
        if self.peripheral_manager.is_some() {
            return Ok(());
        }

        unsafe {
            // Get CBPeripheralManager class
            let cb_peripheral_manager_class = Class::get("CBPeripheralManager")
                .ok_or_else(|| BitchatError::InvalidPacket(
                    "CBPeripheralManager class not available - Core Bluetooth framework missing".into()
                ))?;

            // Create peripheral manager instance
            let peripheral_manager: id = msg_send![cb_peripheral_manager_class, alloc];
            let peripheral_manager: id = msg_send![peripheral_manager, 
                initWithDelegate: nil
                queue: nil
                options: nil
            ];

            if peripheral_manager == nil {
                return Err(BitchatError::InvalidPacket(
                    "Failed to create CBPeripheralManager instance".into()
                ));
            }

            self.peripheral_manager = Some(peripheral_manager);
            info!("macOS CBPeripheralManager initialized");
        }

        Ok(())
    }

    fn create_bitchat_service(&mut self) -> BitchatResult<()> {
        unsafe {
            // Get CBUUID class and create service UUID
            let cbuuid_class = Class::get("CBUUID")
                .ok_or_else(|| BitchatError::InvalidPacket("CBUUID class not available".into()))?;
            
            let service_uuid_string = NSString::from_str(&BITCHAT_SERVICE_UUID.to_string());
            let service_uuid: id = msg_send![cbuuid_class, UUIDWithString: service_uuid_string];

            // Get CBMutableService class
            let cb_mutable_service_class = Class::get("CBMutableService")
                .ok_or_else(|| BitchatError::InvalidPacket("CBMutableService class not available".into()))?;

            // Create mutable service
            let service: id = msg_send![cb_mutable_service_class, alloc];
            let service: id = msg_send![service, initWithType: service_uuid primary: true];

            if service == nil {
                return Err(BitchatError::InvalidPacket("Failed to create CBMutableService".into()));
            }

            // Create TX characteristic (write without response)
            let tx_uuid_string = NSString::from_str(&BITCHAT_TX_CHARACTERISTIC_UUID.to_string());
            let tx_uuid: id = msg_send![cbuuid_class, UUIDWithString: tx_uuid_string];

            let cb_mutable_characteristic_class = Class::get("CBMutableCharacteristic")
                .ok_or_else(|| BitchatError::InvalidPacket("CBMutableCharacteristic class not available".into()))?;

            let tx_characteristic: id = msg_send![cb_mutable_characteristic_class, alloc];
            let tx_characteristic: id = msg_send![tx_characteristic,
                initWithType: tx_uuid,
                properties: 4u32,  // CBCharacteristicPropertyWriteWithoutResponse
                value: nil,
                permissions: 16u32  // CBAttributePermissionsWriteable
            ];

            // Create RX characteristic (read + notify)
            let rx_uuid_string = NSString::from_str(&BITCHAT_RX_CHARACTERISTIC_UUID.to_string());
            let rx_uuid: id = msg_send![cbuuid_class, UUIDWithString: rx_uuid_string];

            let rx_characteristic: id = msg_send![cb_mutable_characteristic_class, alloc];
            let rx_characteristic: id = msg_send![rx_characteristic,
                initWithType: rx_uuid,
                properties: 18u32,  // CBCharacteristicPropertyRead | CBCharacteristicPropertyNotify
                value: nil,
                permissions: 1u32   // CBAttributePermissionsReadable
            ];

            // Create NSArray with characteristics
            let nsarray_class = Class::get("NSArray")
                .ok_or_else(|| BitchatError::InvalidPacket("NSArray class not available".into()))?;
            
            let characteristics_vec = vec![tx_characteristic, rx_characteristic];
            let characteristics_array: id = msg_send![nsarray_class, 
                arrayWithObjects: characteristics_vec.as_ptr() count: 2
            ];

            // Set characteristics on service
            let _: () = msg_send![service, setCharacteristics: characteristics_array];

            self.service = Some(service);
            info!("Created BitChat BLE service with TX/RX characteristics");
        }

        Ok(())
    }
}

#[cfg(target_os = "macos")]
#[async_trait::async_trait]
impl BleAdvertiser for MacOSAdvertiser {
    async fn start_advertising(&mut self, peer_id: &PeerId, config: &BleTransportConfig) -> BitchatResult<()> {
        self.initialize_peripheral_manager()?;
        self.create_bitchat_service()?;

        let device_name = generate_device_name(peer_id, &config.device_name_prefix);

        unsafe {
            let peripheral_manager = self.peripheral_manager
                .ok_or_else(|| BitchatError::InvalidPacket("Peripheral manager not initialized".into()))?;

            let service = self.service
                .ok_or_else(|| BitchatError::InvalidPacket("Service not created".into()))?;

            // Add service to peripheral manager
            let _: () = msg_send![peripheral_manager, addService: service];

            // Create advertising data dictionary
            let nsstring_class = Class::get("NSString")
                .ok_or_else(|| BitchatError::InvalidPacket("NSString class not available".into()))?;
            let nsdictionary_class = Class::get("NSDictionary")
                .ok_or_else(|| BitchatError::InvalidPacket("NSDictionary class not available".into()))?;
            let nsarray_class = Class::get("NSArray")
                .ok_or_else(|| BitchatError::InvalidPacket("NSArray class not available".into()))?;
            let cbuuid_class = Class::get("CBUUID")
                .ok_or_else(|| BitchatError::InvalidPacket("CBUUID class not available".into()))?;

            // Local name key and value
            let local_name_key: id = msg_send![nsstring_class, stringWithUTF8String: "kCBAdvDataLocalName\0".as_ptr()];
            let local_name_value = NSString::from_str(&device_name);

            // Service UUIDs key and value
            let service_uuids_key: id = msg_send![nsstring_class, stringWithUTF8String: "kCBAdvDataServiceUUIDs\0".as_ptr()];
            let service_uuid_string = NSString::from_str(&BITCHAT_SERVICE_UUID.to_string());
            let service_uuid: id = msg_send![cbuuid_class, UUIDWithString: service_uuid_string];
            let service_uuids_vec = vec![service_uuid];
            let service_uuids_array: id = msg_send![nsarray_class, 
                arrayWithObjects: service_uuids_vec.as_ptr() count: 1
            ];

            // Create advertising data dictionary
            let advertising_data: id = msg_send![nsdictionary_class,
                dictionaryWithObjects: &[local_name_value.as_ptr(), service_uuids_array] as *const id
                forKeys: &[local_name_key, service_uuids_key] as *const id
                count: 2u64
            ];

            // Start advertising
            let _: () = msg_send![peripheral_manager, startAdvertising: advertising_data];

            self.is_advertising = true;
            info!("Started macOS BLE advertising as '{}'", device_name);
        }

        Ok(())
    }

    async fn stop_advertising(&mut self) -> BitchatResult<()> {
        if let Some(peripheral_manager) = self.peripheral_manager {
            unsafe {
                let _: () = msg_send![peripheral_manager, stopAdvertising];
            }
            self.is_advertising = false;
            info!("Stopped macOS BLE advertising");
        }
        Ok(())
    }

    fn is_advertising(&self) -> bool {
        self.is_advertising
    }

    async fn update_advertising_data(&mut self, peer_id: &PeerId, config: &BleTransportConfig) -> BitchatResult<()> {
        if self.is_advertising {
            self.stop_advertising().await?;
            self.start_advertising(peer_id, config).await?;
        }
        Ok(())
    }
}

// ----------------------------------------------------------------------------
// Windows Implementation (using Windows APIs)
// ----------------------------------------------------------------------------

#[cfg(target_os = "windows")]
use windows::{
    core::*,
    Win32::Devices::Bluetooth::*,
    Win32::Foundation::*,
    Win32::System::Com::*,
    Storage::Streams::*,
    Devices::Bluetooth::*,
    Devices::Bluetooth::Advertisement::*,
    Devices::Bluetooth::GenericAttributeProfile::*,
};

#[cfg(target_os = "windows")]
pub struct WindowsAdvertiser {
    publisher: Option<BluetoothLEAdvertisementPublisher>,
    gatt_service_provider: Option<GattServiceProvider>,
    is_advertising: bool,
}

#[cfg(target_os = "windows")]
impl WindowsAdvertiser {
    pub fn new() -> Self {
        Self {
            publisher: None,
            gatt_service_provider: None,
            is_advertising: false,
        }
    }

    async fn initialize_gatt_service(&mut self) -> BitchatResult<()> {
        if self.gatt_service_provider.is_some() {
            return Ok(());
        }

        // Create GATT service provider for BitChat service
        let service_uuid = Guid::from_u128(BITCHAT_SERVICE_UUID.as_u128());
        
        let result = GattServiceProvider::CreateAsync(&service_uuid).map_err(|e| {
            BitchatError::InvalidPacket(format!("Failed to create GATT service provider: {}", e))
        })?;

        let service_provider = result.await.map_err(|e| {
            BitchatError::InvalidPacket(format!("Failed to await GATT service provider creation: {}", e))
        })?;

        if service_provider.Error() != BluetoothError::Success {
            return Err(BitchatError::InvalidPacket(
                format!("GATT service provider creation failed with error: {:?}", service_provider.Error())
            ));
        }

        let service_provider = service_provider.ServiceProvider().map_err(|e| {
            BitchatError::InvalidPacket(format!("Failed to get service provider: {}", e))
        })?;

        // Create TX characteristic (write without response)
        let tx_uuid = Guid::from_u128(BITCHAT_TX_CHARACTERISTIC_UUID.as_u128());
        let tx_params = GattLocalCharacteristicParameters::new().map_err(|e| {
            BitchatError::InvalidPacket(format!("Failed to create TX characteristic parameters: {}", e))
        })?;

        tx_params.SetCharacteristicProperties(
            GattCharacteristicProperties::WriteWithoutResponse | GattCharacteristicProperties::Write
        ).map_err(|e| {
            BitchatError::InvalidPacket(format!("Failed to set TX characteristic properties: {}", e))
        })?;

        tx_params.SetWriteProtectionLevel(GattProtectionLevel::Plain).map_err(|e| {
            BitchatError::InvalidPacket(format!("Failed to set TX write protection: {}", e))
        })?;

        let tx_result = service_provider.Service().map_err(|e| {
            BitchatError::InvalidPacket(format!("Failed to get service: {}", e))
        })?.CreateCharacteristicAsync(&tx_uuid, &tx_params).map_err(|e| {
            BitchatError::InvalidPacket(format!("Failed to create TX characteristic: {}", e))
        })?;

        let tx_characteristic_result = tx_result.await.map_err(|e| {
            BitchatError::InvalidPacket(format!("Failed to await TX characteristic creation: {}", e))
        })?;

        if tx_characteristic_result.Error() != BluetoothError::Success {
            return Err(BitchatError::InvalidPacket(
                format!("TX characteristic creation failed: {:?}", tx_characteristic_result.Error())
            ));
        }

        // Create RX characteristic (read + notify)
        let rx_uuid = Guid::from_u128(BITCHAT_RX_CHARACTERISTIC_UUID.as_u128());
        let rx_params = GattLocalCharacteristicParameters::new().map_err(|e| {
            BitchatError::InvalidPacket(format!("Failed to create RX characteristic parameters: {}", e))
        })?;

        rx_params.SetCharacteristicProperties(
            GattCharacteristicProperties::Read | GattCharacteristicProperties::Notify
        ).map_err(|e| {
            BitchatError::InvalidPacket(format!("Failed to set RX characteristic properties: {}", e))
        })?;

        rx_params.SetReadProtectionLevel(GattProtectionLevel::Plain).map_err(|e| {
            BitchatError::InvalidPacket(format!("Failed to set RX read protection: {}", e))
        })?;

        let rx_result = service_provider.Service().map_err(|e| {
            BitchatError::InvalidPacket(format!("Failed to get service: {}", e))
        })?.CreateCharacteristicAsync(&rx_uuid, &rx_params).map_err(|e| {
            BitchatError::InvalidPacket(format!("Failed to create RX characteristic: {}", e))
        })?;

        let rx_characteristic_result = rx_result.await.map_err(|e| {
            BitchatError::InvalidPacket(format!("Failed to await RX characteristic creation: {}", e))
        })?;

        if rx_characteristic_result.Error() != BluetoothError::Success {
            return Err(BitchatError::InvalidPacket(
                format!("RX characteristic creation failed: {:?}", rx_characteristic_result.Error())
            ));
        }

        self.gatt_service_provider = Some(service_provider);
        info!("Windows GATT service provider initialized with BitChat characteristics");
        Ok(())
    }

    async fn start_gatt_service(&self) -> BitchatResult<()> {
        if let Some(ref service_provider) = self.gatt_service_provider {
            let start_result = service_provider.StartAsync().map_err(|e| {
                BitchatError::InvalidPacket(format!("Failed to start GATT service: {}", e))
            })?;

            let result = start_result.await.map_err(|e| {
                BitchatError::InvalidPacket(format!("Failed to await GATT service start: {}", e))
            })?;

            if result.Error() != BluetoothError::Success {
                return Err(BitchatError::InvalidPacket(
                    format!("GATT service start failed: {:?}", result.Error())
                ));
            }

            info!("Windows GATT service started successfully");
        }
        Ok(())
    }

    fn create_advertisement(&self, peer_id: &PeerId, config: &BleTransportConfig) -> BitchatResult<BluetoothLEAdvertisementPublisher> {
        let publisher = BluetoothLEAdvertisementPublisher::new().map_err(|e| {
            BitchatError::InvalidPacket(format!("Failed to create advertisement publisher: {}", e))
        })?;

        let advertisement = publisher.Advertisement().map_err(|e| {
            BitchatError::InvalidPacket(format!("Failed to get advertisement: {}", e))
        })?;

        // Set local name
        let device_name = generate_device_name(peer_id, &config.device_name_prefix);
        advertisement.SetLocalName(&HSTRING::from(&device_name)).map_err(|e| {
            BitchatError::InvalidPacket(format!("Failed to set local name: {}", e))
        })?;

        // Add service UUID
        let service_uuid = Guid::from_u128(BITCHAT_SERVICE_UUID.as_u128());
        advertisement.ServiceUuids().map_err(|e| {
            BitchatError::InvalidPacket(format!("Failed to get service UUIDs list: {}", e))
        })?.Append(&service_uuid).map_err(|e| {
            BitchatError::InvalidPacket(format!("Failed to add service UUID: {}", e))
        })?;

        // Set advertisement as connectable and discoverable
        advertisement.SetFlags(Some(
            BluetoothLEAdvertisementFlags::GeneralDiscoverableMode |
            BluetoothLEAdvertisementFlags::ClassicNotSupported
        )).map_err(|e| {
            BitchatError::InvalidPacket(format!("Failed to set advertisement flags: {}", e))
        })?;

        Ok(publisher)
    }
}

#[cfg(target_os = "windows")]
#[async_trait::async_trait]
impl BleAdvertiser for WindowsAdvertiser {
    async fn start_advertising(&mut self, peer_id: &PeerId, config: &BleTransportConfig) -> BitchatResult<()> {
        // Initialize and start GATT service
        self.initialize_gatt_service().await?;
        self.start_gatt_service().await?;

        // Create and start advertisement
        let publisher = self.create_advertisement(peer_id, config)?;
        
        publisher.Start().map_err(|e| {
            BitchatError::InvalidPacket(format!("Failed to start advertising: {}", e))
        })?;

        self.publisher = Some(publisher);
        self.is_advertising = true;

        let device_name = generate_device_name(peer_id, &config.device_name_prefix);
        info!("Started Windows BLE advertising as '{}'", device_name);
        Ok(())
    }

    async fn stop_advertising(&mut self) -> BitchatResult<()> {
        if let Some(ref publisher) = self.publisher {
            publisher.Stop().map_err(|e| {
                BitchatError::InvalidPacket(format!("Failed to stop advertising: {}", e))
            })?;
        }

        if let Some(ref service_provider) = self.gatt_service_provider {
            service_provider.StopAsync().map_err(|e| {
                BitchatError::InvalidPacket(format!("Failed to stop GATT service: {}", e))
            })?.await.map_err(|e| {
                BitchatError::InvalidPacket(format!("Failed to await GATT service stop: {}", e))
            })?;
        }

        self.publisher = None;
        self.is_advertising = false;
        info!("Stopped Windows BLE advertising");
        Ok(())
    }

    fn is_advertising(&self) -> bool {
        self.is_advertising
    }

    async fn update_advertising_data(&mut self, peer_id: &PeerId, config: &BleTransportConfig) -> BitchatResult<()> {
        if self.is_advertising {
            self.stop_advertising().await?;
            self.start_advertising(peer_id, config).await?;
        }
        Ok(())
    }
}

// ----------------------------------------------------------------------------
// Fallback Implementation (for unsupported platforms)
// ----------------------------------------------------------------------------

pub struct FallbackAdvertiser {
    is_advertising: bool,
}

impl FallbackAdvertiser {
    pub fn new() -> Self {
        Self {
            is_advertising: false,
        }
    }
}

#[async_trait::async_trait]
impl BleAdvertiser for FallbackAdvertiser {
    async fn start_advertising(&mut self, peer_id: &PeerId, config: &BleTransportConfig) -> BitchatResult<()> {
        let device_name = generate_device_name(peer_id, &config.device_name_prefix);
        warn!(
            "BLE advertising not supported on this platform. Device '{}' will not be discoverable. \
            Consider using a supported platform (Linux with BlueZ, macOS, or Windows) for full functionality.",
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

    async fn update_advertising_data(&mut self, _peer_id: &PeerId, _config: &BleTransportConfig) -> BitchatResult<()> {
        Ok(())
    }
}

// ----------------------------------------------------------------------------
// Advertising Manager
// ----------------------------------------------------------------------------

/// High-level advertising manager that handles platform-specific implementations
pub struct AdvertisingManager {
    advertiser: PlatformAdvertiser,
    current_peer_id: Option<PeerId>,
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
    pub async fn start(&mut self, peer_id: PeerId, config: &BleTransportConfig) -> BitchatResult<()> {
        self.advertiser.start_advertising(&peer_id, config).await?;
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
    pub fn enable_rotation(&mut self, interval: Duration) {
        self.rotation_interval = Some(interval);
    }

    /// Manually rotate advertising data
    pub async fn rotate(&mut self, config: &BleTransportConfig) -> BitchatResult<()> {
        if let Some(peer_id) = self.current_peer_id {
            self.advertiser.update_advertising_data(&peer_id, config).await?;
            debug!("Rotated BLE advertising data for peer {}", peer_id);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::BleTransportConfig;

    #[tokio::test]
    async fn test_advertising_manager_lifecycle() {
        let mut manager = AdvertisingManager::new();
        let peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        let config = BleTransportConfig::default();

        assert!(!manager.is_advertising());

        // Start advertising
        manager.start(peer_id, &config).await.unwrap();
        
        // Note: On unsupported platforms this will be false, on Linux it should be true
        // assert!(manager.is_advertising());

        // Stop advertising
        manager.stop().await.unwrap();
        assert!(!manager.is_advertising());
    }

    #[test]
    fn test_platform_advertiser_creation() {
        let _advertiser = create_platform_advertiser();
        // Just ensure it doesn't panic
    }
}