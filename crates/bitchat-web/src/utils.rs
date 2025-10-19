//! Utility functions for WASM module

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

/// Set up better panic messages in debug mode
pub fn set_panic_hook() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

/// Log to browser console (WASM only)
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    pub fn log(s: &str);

    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn log_u32(a: u32);

    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn log_many(a: &str, b: &str);
}

/// Fallback logging for non-WASM targets
#[cfg(not(target_arch = "wasm32"))]
pub fn log(s: &str) {
    println!("{}", s);
}

/// A macro to provide println!(..)-style syntax for console.log logging
/// Works in both WASM and native environments
#[allow(unused_macros)]
#[cfg(target_arch = "wasm32")]
macro_rules! console_log {
    ($($t:tt)*) => (crate::utils::log(&format_args!($($t)*).to_string()))
}

/// Fallback macro for non-WASM targets (just use println!)
#[allow(unused_macros)]
#[cfg(not(target_arch = "wasm32"))]
macro_rules! console_log {
    ($($t:tt)*) => (println!($($t)*))
}

#[allow(unused_imports)]
pub(crate) use console_log;
