//! Modern Terminal User Interface using ratatui
//!
//! A clean, responsive TUI for BitChat with multiple tabs and real-time updates.
//!
//! ## Architecture
//!
//! The TUI is organized into several modules:
//!
//! - [`types`] - Type definitions and constants
//! - [`app`] - TUI application state and logic
//! - [`manager`] - Terminal manager and event loop
//! - [`render`] - Rendering helper functions
//!
//! ## Usage
//!
//! ```rust,no_run
//! use bitchat_cli::tui::TuiManager;
//! use bitchat_cli::app::BitchatApp;
//! use bitchat_cli::config::AppConfig;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = AppConfig::default();
//! let app = BitchatApp::new(config).await?;
//! let mut tui = TuiManager::new(app).await?;
//! tui.run().await?;
//! # Ok(())
//! # }
//! ```

pub mod app;
pub mod manager;
pub mod render;
pub mod types;

// Re-export main types
pub use app::TuiApp;
pub use manager::TuiManager;
