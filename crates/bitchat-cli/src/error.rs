//! Error handling for the BitChat CLI

use thiserror::Error;

/// CLI-specific error types
#[derive(Error, Debug)]
pub enum CliError {
    #[error("BitChat core error: {0}")]
    BitchatCore(#[from] bitchat_core::BitchatError),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Transport initialization failed: {0}")]
    TransportInit(String),

    #[error("State persistence error: {0}")]
    StatePersistence(String),

    #[error("UI error: {0}")]
    UI(String),

    #[error("Message processing error: {0}")]
    MessageProcessing(String),

    #[error("Peer discovery error: {0}")]
    PeerDiscovery(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("TOML parsing error: {0}")]
    TomlParsing(#[from] toml::de::Error),

    #[error("Hex decoding error: {0}")]
    HexDecoding(#[from] hex::FromHexError),

    #[error("Feature not available: {0}")]
    FeatureNotAvailable(String),
}

/// Result type for CLI operations
pub type Result<T> = std::result::Result<T, CliError>;

impl From<anyhow::Error> for CliError {
    fn from(err: anyhow::Error) -> Self {
        CliError::Config(err.to_string())
    }
}
