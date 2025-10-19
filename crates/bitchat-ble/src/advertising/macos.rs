//! macOS BLE advertising implementation using Core Bluetooth

use bitchat_core::{BitchatError, PeerId, Result as BitchatResult};
use bitchat_core::internal::{IdentityKeyPair, TransportError};
use tracing::info;

use crate::config::BleTransportConfig;
use crate::protocol::{
    generate_advertising_data, generate_device_name, BITCHAT_RX_CHARACTERISTIC_UUID, BITCHAT_SERVICE_UUID,
    BITCHAT_TX_CHARACTERISTIC_UUID,
};

use super::BleAdvertiser;

#[cfg(target_os = "macos")]
use cocoa::base::{id, nil};
#[cfg(target_os = "macos")]
use objc::runtime::Class;
#[cfg(target_os = "macos")]
use objc::{msg_send, sel, sel_impl};
#[cfg(target_os = "macos")]
use objc_foundation::{INSString, NSString};

#[cfg(target_os = "macos")]
pub struct MacOSAdvertiser {
    peripheral_manager: Option<id>,
    is_advertising: bool,
    service: Option<id>,
}

#[cfg(target_os = "macos")]
unsafe impl Send for MacOSAdvertiser {}

#[cfg(target_os = "macos")]
unsafe impl Sync for MacOSAdvertiser {}

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
                .ok_or_else(|| BitchatError::Transport(
                    TransportError::TransportUnavailable {
                        transport_type: "Core Bluetooth framework missing".to_string(),
                    }
                ))?;

            // Create peripheral manager instance
            let peripheral_manager: id = msg_send![cb_peripheral_manager_class, alloc];
            let peripheral_manager: id = msg_send![peripheral_manager,
                initWithDelegate: nil
                queue: nil
                options: nil
            ];

            if peripheral_manager == nil {
                return Err(BitchatError::Transport(
                    TransportError::InvalidConfiguration {
                        reason: "Failed to create CBPeripheralManager instance".to_string(),
                    }
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
                .ok_or_else(|| BitchatError::Transport(
                    TransportError::TransportUnavailable {
                        transport_type: "CBUUID class not available".to_string(),
                    }
                ))?;

            let service_uuid_string = NSString::from_str(&BITCHAT_SERVICE_UUID.to_string());
            let service_uuid: id = msg_send![cbuuid_class, UUIDWithString: service_uuid_string];

            // Get CBMutableService class
            let cb_mutable_service_class = Class::get("CBMutableService").ok_or_else(|| {
                BitchatError::Transport(
                    TransportError::TransportUnavailable {
                        transport_type: "CBMutableService class not available".to_string(),
                    }
                )
            })?;

            // Create mutable service
            let service: id = msg_send![cb_mutable_service_class, alloc];
            let service: id = msg_send![service, initWithType: service_uuid primary: true];

            if service == nil {
                return Err(BitchatError::Transport(
                    TransportError::InvalidConfiguration {
                        reason: "Failed to create CBMutableService".to_string(),
                    }
                ));
            }

            // Create TX characteristic (write without response)
            let tx_uuid_string = NSString::from_str(&BITCHAT_TX_CHARACTERISTIC_UUID.to_string());
            let tx_uuid: id = msg_send![cbuuid_class, UUIDWithString: tx_uuid_string];

            let cb_mutable_characteristic_class = Class::get("CBMutableCharacteristic")
                .ok_or_else(|| {
                    BitchatError::Transport(
                        TransportError::TransportUnavailable {
                            transport_type: "CBMutableCharacteristic class not available".to_string(),
                        }
                    )
                })?;

            let tx_characteristic: id = msg_send![cb_mutable_characteristic_class, alloc];
            let tx_characteristic: id = msg_send![tx_characteristic,
                initWithType: tx_uuid
                properties: 4u32  // CBCharacteristicPropertyWriteWithoutResponse
                value: nil
                permissions: 16u32  // CBAttributePermissionsWriteable
            ];

            // Create RX characteristic (read + notify)
            let rx_uuid_string = NSString::from_str(&BITCHAT_RX_CHARACTERISTIC_UUID.to_string());
            let rx_uuid: id = msg_send![cbuuid_class, UUIDWithString: rx_uuid_string];

            let rx_characteristic: id = msg_send![cb_mutable_characteristic_class, alloc];
            let rx_characteristic: id = msg_send![rx_characteristic,
                initWithType: rx_uuid
                properties: 18u32  // CBCharacteristicPropertyRead | CBCharacteristicPropertyNotify
                value: nil
                permissions: 1u32   // CBAttributePermissionsReadable
            ];

            // Create NSArray with characteristics
            let nsarray_class = Class::get("NSArray")
                .ok_or_else(|| BitchatError::Transport(
                    TransportError::TransportUnavailable {
                        transport_type: "NSArray class not available".to_string(),
                    }
                ))?;

            let characteristics_vec = [tx_characteristic, rx_characteristic];
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
    async fn start_advertising(
        &mut self,
        peer_id: &PeerId,
        identity: &IdentityKeyPair,
        config: &BleTransportConfig,
    ) -> BitchatResult<()> {
        self.initialize_peripheral_manager()?;
        self.create_bitchat_service()?;

        let device_name = generate_device_name(peer_id, &config.device_name_prefix);
        
        // Generate secure advertising data
        let secure_advertising_data = generate_advertising_data(*peer_id, identity, &device_name)?;

        unsafe {
            let peripheral_manager = self.peripheral_manager.ok_or_else(|| {
                BitchatError::Transport(
                    TransportError::InvalidConfiguration {
                        reason: "Peripheral manager not initialized".to_string(),
                    }
                )
            })?;

            let service = self
                .service
                .ok_or_else(|| BitchatError::Transport(
                    TransportError::InvalidConfiguration {
                        reason: "Service not created".to_string(),
                    }
                ))?;

            // Add service to peripheral manager
            let _: () = msg_send![peripheral_manager, addService: service];

            // Create advertising data dictionary
            let nsstring_class = Class::get("NSString").ok_or_else(|| {
                BitchatError::Transport(
                    TransportError::TransportUnavailable {
                        transport_type: "NSString class not available".to_string(),
                    }
                )
            })?;
            let nsdictionary_class = Class::get("NSDictionary").ok_or_else(|| {
                BitchatError::Transport(
                    TransportError::TransportUnavailable {
                        transport_type: "NSDictionary class not available".to_string(),
                    }
                )
            })?;
            let nsarray_class = Class::get("NSArray")
                .ok_or_else(|| BitchatError::Transport(
                    TransportError::TransportUnavailable {
                        transport_type: "NSArray class not available".to_string(),
                    }
                ))?;
            let cbuuid_class = Class::get("CBUUID")
                .ok_or_else(|| BitchatError::Transport(
                    TransportError::TransportUnavailable {
                        transport_type: "CBUUID class not available".to_string(),
                    }
                ))?;
            let nsdata_class = Class::get("NSData")
                .ok_or_else(|| BitchatError::Transport(
                    TransportError::TransportUnavailable {
                        transport_type: "NSData class not available".to_string(),
                    }
                ))?;
            let nsnumber_class = Class::get("NSNumber")
                .ok_or_else(|| BitchatError::Transport(
                    TransportError::TransportUnavailable {
                        transport_type: "NSNumber class not available".to_string(),
                    }
                ))?;

            // Local name key and value
            let local_name_key: id =
                msg_send![nsstring_class, stringWithUTF8String: c"kCBAdvDataLocalName".as_ptr()];
            let local_name_value: id = msg_send![nsstring_class, stringWithUTF8String: format!("{}\0", device_name).as_ptr()];

            // Service UUIDs key and value
            let service_uuids_key: id = msg_send![nsstring_class, stringWithUTF8String: c"kCBAdvDataServiceUUIDs".as_ptr()];
            let service_uuid_string = NSString::from_str(&BITCHAT_SERVICE_UUID.to_string());
            let service_uuid: id = msg_send![cbuuid_class, UUIDWithString: service_uuid_string];
            let service_uuids_vec = [service_uuid];
            let service_uuids_array: id = msg_send![nsarray_class,
                arrayWithObjects: service_uuids_vec.as_ptr() count: 1
            ];

            // Manufacturer data key and value
            let manufacturer_data_key: id = msg_send![nsstring_class, stringWithUTF8String: c"kCBAdvDataManufacturerData".as_ptr()];
            let company_id: id = msg_send![nsnumber_class, numberWithInt: 0xFFFFi32]; // Use 0xFFFF for test/development
            let data_nsdata: id = msg_send![nsdata_class, dataWithBytes: secure_advertising_data.as_ptr() length: secure_advertising_data.len()];
            let manufacturer_dict: id = msg_send![nsdictionary_class,
                dictionaryWithObjects: &[data_nsdata] as *const id
                forKeys: &[company_id] as *const id
                count: 1u64
            ];

            // Create advertising data dictionary
            let advertising_data: id = msg_send![nsdictionary_class,
                dictionaryWithObjects: &[local_name_value, service_uuids_array, manufacturer_dict] as *const id
                forKeys: &[local_name_key, service_uuids_key, manufacturer_data_key] as *const id
                count: 3u64
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
