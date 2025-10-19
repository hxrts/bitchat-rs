//! BitChat WebAssembly Frontend - Web Composition Root
//!
//! This crate provides the WebAssembly frontend for the BitChat protocol,
//! serving as the composition root for browser-based communication.
//!
//! It is responsible for:
//! - Initializing and holding the BitchatRuntime
//! - Adding WASM-compatible transport tasks (NostrTransportTask) 
//! - Exposing JavaScript API via #[wasm_bindgen] methods
//! - Managing AppEvent stream and forwarding to JavaScript UI

use wasm_bindgen::prelude::*;

mod utils;
mod app;

pub use utils::*;
pub use app::*;

// Initialize WASM module
#[wasm_bindgen(start)]
pub fn main() {
    utils::set_panic_hook();
    
    // Set up tracing for WASM
    tracing_wasm::set_as_global_default();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_peer_id() {
        let peer_id = generate_peer_id();
        assert!(!peer_id.is_empty());
        assert!(peer_id.len() >= 16); // At least 8 bytes in hex
    }

    #[test]
    fn test_validate_peer_id() {
        // Valid peer ID (16 hex chars = 8 bytes)
        let valid_id = "0102030405060708";
        assert!(validate_peer_id(valid_id));
        
        // Invalid peer ID (too short)
        let invalid_id = "010203";
        assert!(!validate_peer_id(invalid_id));
        
        // Invalid peer ID (not hex)
        let invalid_hex = "not_hex_string";
        assert!(!validate_peer_id(invalid_hex));
    }

    #[test]
    fn test_js_peer_id_conversion() {
        use crate::app::{JsPeerId};
        use bitchat_core::PeerId;
        
        let original_peer_id = PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]);
        let js_peer_id = JsPeerId::from(original_peer_id);
        let converted_back = PeerId::try_from(js_peer_id).unwrap();
        
        assert_eq!(original_peer_id, converted_back);
    }
}