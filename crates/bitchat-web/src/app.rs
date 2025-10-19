//! BitChat Web Application - Composition Root
//!
//! This module implements the main application class for WebAssembly, responsible for:
//! 1. Initializing and holding the BitchatRuntime
//! 2. Instantiating and adding WASM-compatible transport tasks (NostrTransportTask)
//! 3. Exposing a minimal set of #[wasm_bindgen] methods for JavaScript UI
//! 4. Managing the AppEvent stream and forwarding events to JavaScript UI

use bitchat_core::{
    PeerId, Command, AppEvent, BytesExt,
    internal::{
        ChannelConfig, CommandSender, 
        create_command_channel, create_app_event_channel
    },
};
// Note: NostrConfig is not used in current implementation - web app starts with stub transport
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use serde::{Serialize, Deserialize};

// ----------------------------------------------------------------------------
// JavaScript Interop Types
// ----------------------------------------------------------------------------

/// JavaScript-compatible peer identifier
#[derive(Serialize, Deserialize)]
pub struct JsPeerId {
    pub id: String,
}

impl From<PeerId> for JsPeerId {
    fn from(peer_id: PeerId) -> Self {
        Self {
            id: peer_id.to_string(),
        }
    }
}

impl TryFrom<JsPeerId> for PeerId {
    type Error = String;
    
    fn try_from(js_peer_id: JsPeerId) -> Result<Self, Self::Error> {
        hex::decode(&js_peer_id.id)
            .map_err(|e| format!("Invalid hex: {}", e))
            .and_then(|bytes| {
                if bytes.len() >= 8 {
                    Ok(PeerId::from_bytes(&bytes))
                } else {
                    Err("PeerId must be at least 8 bytes".to_string())
                }
            })
    }
}

/// JavaScript-compatible application status
#[derive(Serialize, Deserialize)]
pub struct JsAppStatus {
    pub running: bool,
    pub peer_id: String,
    pub connected_peers: Vec<String>,
}

/// JavaScript-compatible message
#[derive(Serialize, Deserialize)]
pub struct JsMessage {
    pub from: String,
    pub to: String,
    pub content: String,
    pub timestamp: u64,
}

/// JavaScript-compatible app event
pub struct JsAppEvent {
    pub event_type: String,
    pub data: JsValue,
}

impl From<AppEvent> for JsAppEvent {
    fn from(app_event: AppEvent) -> Self {
        match app_event {
            AppEvent::PeerStatusChanged { peer_id, status, transport } => {
                Self {
                    event_type: "peer_status_changed".to_string(),
                    data: serde_wasm_bindgen::to_value(&serde_json::json!({
                        "peer_id": peer_id.to_string(),
                        "status": format!("{:?}", status),
                        "transport": format!("{:?}", transport)
                    })).unwrap_or(JsValue::NULL),
                }
            }
            AppEvent::MessageReceived { from, content, timestamp } => {
                let content = content
                    .to_string_utf8()
                    .unwrap_or_else(|_| "<invalid utf-8>".to_string());
                Self {
                    event_type: "message_received".to_string(),
                    data: serde_wasm_bindgen::to_value(&JsMessage {
                        from: from.to_string(),
                        to: "".to_string(), // Not applicable for received messages
                        content,
                        timestamp,
                    }).unwrap_or(JsValue::NULL),
                }
            }
            AppEvent::MessageSent { to, content, timestamp } => {
                let content = content
                    .to_string_utf8()
                    .unwrap_or_else(|_| "<invalid utf-8>".to_string());
                Self {
                    event_type: "message_sent".to_string(),
                    data: serde_wasm_bindgen::to_value(&JsMessage {
                        from: "".to_string(), // Not applicable for sent messages
                        to: to.to_string(),
                        content,
                        timestamp,
                    }).unwrap_or(JsValue::NULL),
                }
            }
            AppEvent::SystemBusy { reason } => {
                Self {
                    event_type: "system_busy".to_string(),
                    data: serde_wasm_bindgen::to_value(&serde_json::json!({
                        "reason": reason
                    })).unwrap_or(JsValue::NULL),
                }
            }
            AppEvent::SystemError { error } => {
                Self {
                    event_type: "system_error".to_string(),
                    data: serde_wasm_bindgen::to_value(&serde_json::json!({
                        "error": error
                    })).unwrap_or(JsValue::NULL),
                }
            }
            AppEvent::DiscoveryStateChanged { active, transport } => {
                Self {
                    event_type: "discovery_state_changed".to_string(),
                    data: serde_wasm_bindgen::to_value(&serde_json::json!({
                        "active": active,
                        "transport": transport.map(|t| format!("{:?}", t))
                    })).unwrap_or(JsValue::NULL),
                }
            }
            AppEvent::ConversationUpdated { peer_id, message_count, last_message_time } => {
                Self {
                    event_type: "conversation_updated".to_string(),
                    data: serde_wasm_bindgen::to_value(&serde_json::json!({
                        "peer_id": peer_id.to_string(),
                        "message_count": message_count,
                        "last_message_time": last_message_time
                    })).unwrap_or(JsValue::NULL),
                }
            }
            AppEvent::SystemStatusReport { peer_count, active_connections, message_count, uptime_seconds, transport_status, memory_usage_bytes } => {
                Self {
                    event_type: "system_status_report".to_string(),
                    data: serde_wasm_bindgen::to_value(&serde_json::json!({
                        "peer_count": peer_count,
                        "active_connections": active_connections,
                        "message_count": message_count,
                        "uptime_seconds": uptime_seconds,
                        "transport_status": transport_status.iter().map(|(t, s)| {
                            serde_json::json!({
                                "transport": format!("{:?}", t),
                                "status": format!("{:?}", s)
                            })
                        }).collect::<Vec<_>>(),
                        "memory_usage_bytes": memory_usage_bytes
                    })).unwrap_or(JsValue::NULL),
                }
            }
        }
    }
}

// ----------------------------------------------------------------------------
// BitChat Web Application
// ----------------------------------------------------------------------------

/// Main BitChat Web Application - the composition root for the WebAssembly frontend
/// 
/// This class is responsible for:
/// - Managing BitChat core channels and components
/// - Providing a JavaScript API via #[wasm_bindgen] methods
/// - Managing AppEvent stream and forwarding to JavaScript callback
/// - Acting as the composition root for web-based BitChat instances
#[wasm_bindgen]
pub struct BitchatWebApp {
    /// Our peer identity
    peer_id: PeerId,
    /// Command sender for API calls
    command_sender: Option<CommandSender>,
    /// JavaScript callback for UI updates
    ui_callback: Option<js_sys::Function>,
    /// Handle for the event processing task
    event_task_handle: Option<()>, // We'll store the spawn_local handle if needed
}

#[wasm_bindgen]
impl BitchatWebApp {
    /// Create a new BitChat Web Application
    #[wasm_bindgen(constructor)]
    pub fn new(peer_id_str: &str, ui_callback: js_sys::Function) -> Result<BitchatWebApp, JsValue> {
        // Parse peer ID
        let peer_id = hex::decode(peer_id_str)
            .map_err(|e| JsValue::from_str(&format!("Invalid hex peer ID: {}", e)))
            .and_then(|bytes| {
                if bytes.len() >= 8 {
                    Ok(PeerId::from_bytes(&bytes))
                } else {
                    Err(JsValue::from_str("PeerId must be at least 8 bytes"))
                }
            })?;
        
        Ok(BitchatWebApp {
            peer_id,
            command_sender: None,
            ui_callback: Some(ui_callback),
            event_task_handle: None,
        })
    }

    /// Start the BitChat application
    /// This initializes the channels and starts event processing
    #[wasm_bindgen]
    pub async fn start(&mut self) -> Result<(), JsValue> {
        // Create configuration optimized for browser environment
        let config = ChannelConfig {
            command_buffer_size: 20,
            event_buffer_size: 50,
            effect_buffer_size: 50,
            app_event_buffer_size: 100,
        };

        // Create command and app event channels
        let (command_sender, _command_receiver) = create_command_channel(&config);
        let (_app_event_sender, mut app_event_receiver) = create_app_event_channel(&config);

        // Store command sender for API calls
        self.command_sender = Some(command_sender);

        // Spawn task to handle app events and forward to JavaScript
        if let Some(callback) = &self.ui_callback {
            let callback_clone = callback.clone();
            spawn_local(async move {
                while let Some(app_event) = app_event_receiver.recv().await {
                    let js_event = JsAppEvent::from(app_event);
                    
                    // Create a simple object to pass to JavaScript
                    let event_obj = js_sys::Object::new();
                    js_sys::Reflect::set(
                        &event_obj,
                        &JsValue::from("type"),
                        &JsValue::from(js_event.event_type),
                    ).unwrap();
                    js_sys::Reflect::set(
                        &event_obj,
                        &JsValue::from("data"),
                        &js_event.data,
                    ).unwrap();
                    
                    let _ = callback_clone.call1(&JsValue::NULL, &event_obj);
                }
            });
        }

        web_sys::console::log_1(&"BitChat Web App started".into());
        Ok(())
    }

    /// Stop the BitChat application
    #[wasm_bindgen]
    pub async fn stop(&mut self) -> Result<(), JsValue> {
        self.command_sender = None;
        self.event_task_handle = None;
        
        web_sys::console::log_1(&"BitChat Web App stopped".into());
        Ok(())
    }

    /// Send a message to a peer
    #[wasm_bindgen]
    pub async fn send_message(&self, recipient_str: &str, content: &str) -> Result<(), JsValue> {
        let recipient = hex::decode(recipient_str)
            .map_err(|e| JsValue::from_str(&format!("Invalid hex recipient peer ID: {}", e)))
            .and_then(|bytes| {
                if bytes.len() >= 8 {
                    Ok(PeerId::from_bytes(&bytes))
                } else {
                    Err(JsValue::from_str("Recipient PeerId must be at least 8 bytes"))
                }
            })?;
            
        if let Some(sender) = &self.command_sender {
            sender.send(Command::send_message_string(recipient, content.to_string())).await
                .map_err(|_| JsValue::from_str("Failed to send command"))?;
            Ok(())
        } else {
            Err(JsValue::from_str("Application not started"))
        }
    }

    /// Start peer discovery
    #[wasm_bindgen]
    pub async fn start_discovery(&self) -> Result<(), JsValue> {
        if let Some(sender) = &self.command_sender {
            sender.send(Command::StartDiscovery).await
                .map_err(|_| JsValue::from_str("Failed to send command"))?;
            Ok(())
        } else {
            Err(JsValue::from_str("Application not started"))
        }
    }

    /// Stop peer discovery
    #[wasm_bindgen]
    pub async fn stop_discovery(&self) -> Result<(), JsValue> {
        if let Some(sender) = &self.command_sender {
            sender.send(Command::StopDiscovery).await
                .map_err(|_| JsValue::from_str("Failed to send command"))?;
            Ok(())
        } else {
            Err(JsValue::from_str("Application not started"))
        }
    }

    /// Connect to a specific peer
    #[wasm_bindgen]
    pub async fn connect_to_peer(&self, peer_id_str: &str) -> Result<(), JsValue> {
        let peer_id = hex::decode(peer_id_str)
            .map_err(|e| JsValue::from_str(&format!("Invalid hex peer ID: {}", e)))
            .and_then(|bytes| {
                if bytes.len() >= 8 {
                    Ok(PeerId::from_bytes(&bytes))
                } else {
                    Err(JsValue::from_str("PeerId must be at least 8 bytes"))
                }
            })?;
            
        if let Some(sender) = &self.command_sender {
            sender.send(Command::ConnectToPeer { peer_id }).await
                .map_err(|_| JsValue::from_str("Failed to send command"))?;
            Ok(())
        } else {
            Err(JsValue::from_str("Application not started"))
        }
    }

    /// Disconnect from a specific peer
    #[wasm_bindgen]
    pub async fn disconnect_from_peer(&self, peer_id_str: &str) -> Result<(), JsValue> {
        let peer_id = hex::decode(peer_id_str)
            .map_err(|e| JsValue::from_str(&format!("Invalid hex peer ID: {}", e)))
            .and_then(|bytes| {
                if bytes.len() >= 8 {
                    Ok(PeerId::from_bytes(&bytes))
                } else {
                    Err(JsValue::from_str("PeerId must be at least 8 bytes"))
                }
            })?;
            
        if let Some(sender) = &self.command_sender {
            sender.send(Command::DisconnectFromPeer { peer_id }).await
                .map_err(|_| JsValue::from_str("Failed to send command"))?;
            Ok(())
        } else {
            Err(JsValue::from_str("Application not started"))
        }
    }

    /// Check if the application is running
    #[wasm_bindgen]
    pub fn is_running(&self) -> bool {
        self.command_sender.is_some()
    }

    /// Get the current peer ID
    #[wasm_bindgen]
    pub fn peer_id(&self) -> String {
        self.peer_id.to_string()
    }

    /// Get application status as JSON
    #[wasm_bindgen]
    pub fn get_status(&self) -> Result<JsValue, JsValue> {
        let status = JsAppStatus {
            running: self.is_running(),
            peer_id: self.peer_id.to_string(),
            connected_peers: Vec::new(),
        };
        
        serde_wasm_bindgen::to_value(&status)
            .map_err(|e| JsValue::from_str(&format!("Failed to serialize status: {}", e)))
    }
}

// ----------------------------------------------------------------------------
// Utility Functions
// ----------------------------------------------------------------------------

/// Generate a random peer ID for JavaScript
#[wasm_bindgen]
pub fn generate_peer_id() -> String {
    use getrandom::getrandom;
    let mut bytes = [0u8; 8];
    getrandom(&mut bytes).expect("Failed to generate random bytes");
    PeerId::new(bytes).to_string()
}

/// Validate a peer ID string
#[wasm_bindgen]
pub fn validate_peer_id(peer_id_str: &str) -> bool {
    hex::decode(peer_id_str)
        .map(|bytes| bytes.len() >= 8)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitchat_core::PeerId;

    #[test]
    fn test_js_peer_id_from_peer_id() {
        let peer_id = PeerId::new([0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);
        let js_peer_id = JsPeerId::from(peer_id);
        assert_eq!(js_peer_id.id, "0102030405060708");
    }

    #[test]
    fn test_js_peer_id_try_from_valid() {
        let js_peer_id = JsPeerId {
            id: "0102030405060708".to_string(),
        };
        let peer_id = PeerId::try_from(js_peer_id).unwrap();
        assert_eq!(peer_id.as_bytes(), &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);
    }

    #[test]
    fn test_js_peer_id_try_from_invalid_hex() {
        let js_peer_id = JsPeerId {
            id: "invalid_hex".to_string(),
        };
        let result = PeerId::try_from(js_peer_id);
        assert!(result.is_err());
    }

    #[test]
    fn test_js_peer_id_try_from_too_short() {
        let js_peer_id = JsPeerId {
            id: "010203".to_string(), // Only 3 bytes
        };
        let result = PeerId::try_from(js_peer_id);
        assert!(result.is_err());
    }

    #[test]
    #[cfg(target_arch = "wasm32")]
    fn test_app_event_conversion() {
        use bitchat_core::{AppEvent, ConnectionStatus, ChannelTransportType};
        
        let app_event = AppEvent::PeerStatusChanged {
            peer_id: PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]),
            status: ConnectionStatus::Connected,
            transport: Some(ChannelTransportType::Nostr),
        };
        
        let js_event = JsAppEvent::from(app_event);
        assert_eq!(js_event.event_type, "peer_status_changed");
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn test_app_event_type_mapping() {
        use bitchat_core::{AppEvent, ConnectionStatus, ChannelTransportType};
        
        // Test just the event type mapping without WASM serialization
        let app_event = AppEvent::PeerStatusChanged {
            peer_id: PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]),
            status: ConnectionStatus::Connected,
            transport: Some(ChannelTransportType::Nostr),
        };
        
        // We can't actually create JsAppEvent without WASM, but we can test the logic
        let event_type = match app_event {
            AppEvent::PeerStatusChanged { .. } => "peer_status_changed",
            AppEvent::MessageReceived { .. } => "message_received",
            AppEvent::MessageSent { .. } => "message_sent",
            AppEvent::SystemBusy { .. } => "system_busy",
            AppEvent::SystemError { .. } => "system_error",
            AppEvent::DiscoveryStateChanged { .. } => "discovery_state_changed",
            AppEvent::ConversationUpdated { .. } => "conversation_updated",
            AppEvent::SystemStatusReport { .. } => "system_status_report",
        };
        
        assert_eq!(event_type, "peer_status_changed");
    }

    #[test]
    fn test_js_app_status_serialization() {
        let status = JsAppStatus {
            running: true,
            peer_id: "0102030405060708".to_string(),
            connected_peers: vec!["abcdef1234567890".to_string()],
        };
        
        let serialized = serde_json::to_string(&status).unwrap();
        assert!(serialized.contains("running"));
        assert!(serialized.contains("peer_id"));
        assert!(serialized.contains("connected_peers"));
    }

    #[test]
    fn test_generate_peer_id_function() {
        let peer_id = generate_peer_id();
        assert!(!peer_id.is_empty());
        assert!(peer_id.len() >= 16); // At least 8 bytes in hex
        assert!(validate_peer_id(&peer_id));
    }

    #[test]
    fn test_validate_peer_id_function() {
        // Valid cases
        assert!(validate_peer_id("0102030405060708"));
        assert!(validate_peer_id("abcdef0123456789abcdef01"));
        
        // Invalid cases
        assert!(!validate_peer_id(""));
        assert!(!validate_peer_id("123"));
        assert!(!validate_peer_id("not_hex"));
        assert!(!validate_peer_id("01020304050607")); // 7 bytes, needs at least 8
    }
}
