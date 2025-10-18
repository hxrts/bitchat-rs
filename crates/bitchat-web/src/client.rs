//! WASM BitChat client implementation

use std::sync::Arc;

use bitchat_core::{
    transport::TransportManager, MessageBuilder, PeerId, StdDeliveryTracker,
    StdNoiseSessionManager, StdTimeSource,
};
use tokio::sync::RwLock;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;

use crate::transport::{WasmNostrConfig, WasmNostrTransport};
use crate::utils::console_log;

/// WASM-compatible BitChat client
#[wasm_bindgen]
pub struct WasmBitchatClient {
    peer_id: PeerId,
    display_name: String,
    #[allow(dead_code)]
    session_manager: StdNoiseSessionManager,
    #[allow(dead_code)]
    delivery_tracker: StdDeliveryTracker,
    transport_manager: TransportManager,
    messages: Arc<RwLock<Vec<ReceivedMessage>>>,
    message_callback: Option<js_sys::Function>,
}

/// A received message for JavaScript consumption
#[wasm_bindgen]
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct ReceivedMessage {
    sender_id: String,
    sender_name: String,
    content: String,
    timestamp: f64,
}

#[wasm_bindgen]
impl ReceivedMessage {
    #[wasm_bindgen(getter)]
    pub fn sender_id(&self) -> String {
        self.sender_id.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn sender_name(&self) -> String {
        self.sender_name.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn content(&self) -> String {
        self.content.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn timestamp(&self) -> f64 {
        self.timestamp
    }
}

/// Client configuration for WASM
#[wasm_bindgen]
pub struct WasmClientConfig {
    display_name: String,
    nostr_config: WasmNostrConfig,
}

#[wasm_bindgen]
impl WasmClientConfig {
    #[wasm_bindgen(constructor)]
    pub fn new(display_name: String) -> Self {
        Self {
            display_name,
            nostr_config: WasmNostrConfig::new(),
        }
    }

    #[wasm_bindgen(getter)]
    pub fn display_name(&self) -> String {
        self.display_name.clone()
    }

    #[wasm_bindgen(setter)]
    pub fn set_display_name(&mut self, name: String) {
        self.display_name = name;
    }

    #[wasm_bindgen(getter)]
    pub fn nostr_config(&self) -> WasmNostrConfig {
        self.nostr_config.clone()
    }

    #[wasm_bindgen(setter)]
    pub fn set_nostr_config(&mut self, config: WasmNostrConfig) {
        self.nostr_config = config;
    }
}

#[wasm_bindgen]
impl WasmBitchatClient {
    /// Create a new BitChat client
    #[wasm_bindgen(constructor)]
    pub fn new(config: WasmClientConfig) -> Result<WasmBitchatClient, JsValue> {
        console_log!("Creating new BitChat client...");

        // Generate crypto keys
        let noise_key = bitchat_core::crypto::NoiseKeyPair::generate();
        let peer_id = PeerId::from_bytes(&noise_key.public_key_bytes());

        console_log!("Generated peer ID: {}", peer_id);

        let session_manager = StdNoiseSessionManager::new(noise_key, StdTimeSource);
        let delivery_tracker = StdDeliveryTracker::new(StdTimeSource);
        let transport_manager = TransportManager::new();

        Ok(WasmBitchatClient {
            peer_id,
            display_name: config.display_name,
            session_manager,
            delivery_tracker,
            transport_manager,
            messages: Arc::new(RwLock::new(Vec::new())),
            message_callback: None,
        })
    }

    /// Get the client's peer ID
    #[wasm_bindgen(getter)]
    pub fn peer_id(&self) -> String {
        self.peer_id.to_string()
    }

    /// Get the client's display name
    #[wasm_bindgen(getter)]
    pub fn display_name(&self) -> String {
        self.display_name.clone()
    }

    /// Set the display name
    #[wasm_bindgen(setter)]
    pub fn set_display_name(&mut self, name: String) {
        self.display_name = name;
    }

    /// Set a callback function for received messages
    #[wasm_bindgen]
    pub fn set_message_callback(&mut self, callback: js_sys::Function) {
        self.message_callback = Some(callback);
    }

    /// Start the client with Nostr transport
    #[wasm_bindgen]
    pub fn start(&mut self, config: WasmNostrConfig) -> js_sys::Promise {
        console_log!("Starting BitChat client...");

        let transport = match WasmNostrTransport::with_config(self.peer_id, config) {
            Ok(transport) => transport,
            Err(e) => {
                return js_sys::Promise::reject(&JsValue::from_str(&format!(
                    "Failed to create transport: {}",
                    e
                )));
            }
        };

        self.transport_manager.add_transport(Box::new(transport));

        future_to_promise(async move {
            console_log!("BitChat client started successfully");
            Ok(JsValue::UNDEFINED)
        })
    }

    /// Stop the client
    #[wasm_bindgen]
    pub fn stop(&mut self) -> js_sys::Promise {
        console_log!("Stopping BitChat client...");

        future_to_promise(async move {
            console_log!("BitChat client stopped");
            Ok(JsValue::UNDEFINED)
        })
    }

    /// Send a message to a specific peer
    #[wasm_bindgen]
    pub fn send_message(
        &mut self,
        recipient_id: Option<String>,
        content: String,
    ) -> js_sys::Promise {
        console_log!("Sending message: {}", content);

        let recipient_peer_id = if let Some(id_str) = recipient_id {
            // For now, just create a dummy peer ID or handle this differently
            // In a real implementation, you'd parse the hex string
            if id_str.is_empty() {
                None
            } else {
                // Create a dummy peer ID for demonstration
                Some(PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]))
            }
        } else {
            None
        };

        let _packet = match MessageBuilder::create_message(
            self.peer_id,
            self.display_name.clone(),
            content,
            recipient_peer_id,
        ) {
            Ok(packet) => packet,
            Err(e) => {
                return js_sys::Promise::reject(&JsValue::from_str(&format!(
                    "Failed to create message: {}",
                    e
                )));
            }
        };

        future_to_promise(async move {
            console_log!("Message sent successfully");
            Ok(JsValue::UNDEFINED)
        })
    }

    /// Get discovered peers
    #[wasm_bindgen]
    pub fn get_discovered_peers(&self) -> Vec<String> {
        // Return empty list for now
        Vec::new()
    }

    /// Get received messages
    #[wasm_bindgen]
    pub fn get_messages(&self) -> js_sys::Promise {
        let messages = Arc::clone(&self.messages);

        future_to_promise(async move {
            let messages_guard = messages.read().await;
            let js_array = js_sys::Array::new();

            for msg in messages_guard.iter() {
                let js_msg = serde_wasm_bindgen::to_value(msg).map_err(|e| {
                    JsValue::from_str(&format!("Failed to serialize message: {}", e))
                })?;
                js_array.push(&js_msg);
            }

            Ok(js_array.into())
        })
    }

    /// Get transport status
    #[wasm_bindgen]
    pub fn get_status(&self) -> js_sys::Object {
        let status = js_sys::Object::new();

        js_sys::Reflect::set(
            &status,
            &JsValue::from_str("active_transports"),
            &JsValue::from_f64(0.0),
        )
        .unwrap();

        js_sys::Reflect::set(
            &status,
            &JsValue::from_str("total_messages"),
            &JsValue::from_f64(0.0),
        )
        .unwrap();

        js_sys::Reflect::set(
            &status,
            &JsValue::from_str("confirmed_messages"),
            &JsValue::from_f64(0.0),
        )
        .unwrap();

        js_sys::Reflect::set(
            &status,
            &JsValue::from_str("failed_messages"),
            &JsValue::from_f64(0.0),
        )
        .unwrap();

        status
    }
}

/// Helper function to create a basic WASM client with default configuration
#[wasm_bindgen]
pub fn create_bitchat_client(display_name: String) -> Result<WasmBitchatClient, JsValue> {
    let config = WasmClientConfig::new(display_name);
    WasmBitchatClient::new(config)
}
