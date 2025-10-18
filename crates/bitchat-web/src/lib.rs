//! BitChat WebAssembly bindings
//!
//! This crate provides WebAssembly bindings for the BitChat protocol,
//! enabling browser-based communication via Nostr relays.

use wasm_bindgen::prelude::*;

mod client;
mod transport;
mod utils;

pub use client::*;
pub use transport::*;
pub use utils::*;

// Initialize WASM module
#[wasm_bindgen(start)]
pub fn main() {
    utils::set_panic_hook();
    tracing_wasm::set_as_global_default();
}
