//! Thin compatibility re-export.
//!
//! For the initial refactor pass we simply re-export the existing channel
//! message types from `bitchat-core`. This allows downstream crates to depend
//! on `bitchat-harness` without immediately breaking the rest of the tree. Once
//! transports and runtime are fully migrated we can move the definitions here
//! and remove the legacy module from `bitchat-core`.

pub use bitchat_core::channel::{
    AppEvent,
    ChannelTransportType,
    Command,
    ConnectionStatus,
    Effect,
    Event,
    TransportStatus,
};
