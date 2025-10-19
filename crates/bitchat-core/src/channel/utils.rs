//! Channel Utilities for CSP Communication
//!
//! This module provides platform-abstracted channel implementations:
//! - tokio-channels: Uses tokio::sync::mpsc for std environments
//! - wasm-channels: Uses futures-channel::mpsc for WASM environments

use crate::channel::communication::{Command, Event, Effect, AppEvent};
use crate::config::ChannelConfig;

cfg_if::cfg_if! {
    if #[cfg(feature = "wasm")] {
        use futures_channel::mpsc;
    }
}

cfg_if::cfg_if! {
    if #[cfg(feature = "std")] {
        use std::fmt;
    } else {
        use core::fmt;
    }
}

#[derive(Debug)]
pub enum ChannelError {
    ChannelFull,
    ChannelClosed,
    ReceiverDropped,
}

impl fmt::Display for ChannelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChannelError::ChannelFull => write!(f, "Channel buffer is full"),
            ChannelError::ChannelClosed => write!(f, "Channel is closed"),
            ChannelError::ReceiverDropped => write!(f, "Channel receiver was dropped"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ChannelError {}

// Feature-gated channel type definitions

cfg_if::cfg_if! {
    if #[cfg(feature = "std")] {
        pub type CommandSender = tokio::sync::mpsc::Sender<Command>;
        pub type CommandReceiver = tokio::sync::mpsc::Receiver<Command>;
        pub type EventSender = tokio::sync::mpsc::Sender<Event>;
        pub type EventReceiver = tokio::sync::mpsc::Receiver<Event>;
        pub type EffectSender = tokio::sync::broadcast::Sender<Effect>;
        pub type EffectReceiver = tokio::sync::broadcast::Receiver<Effect>;
        pub type AppEventSender = tokio::sync::mpsc::Sender<AppEvent>;
        pub type AppEventReceiver = tokio::sync::mpsc::Receiver<AppEvent>;
    } else if #[cfg(feature = "wasm")] {
        pub type CommandSender = futures_channel::mpsc::Sender<Command>;
        pub type CommandReceiver = futures_channel::mpsc::Receiver<Command>;
        pub type EventSender = futures_channel::mpsc::Sender<Event>;
        pub type EventReceiver = futures_channel::mpsc::Receiver<Event>;
        pub type EffectSender = async_broadcast::Sender<Effect>;
        pub type EffectReceiver = async_broadcast::Receiver<Effect>;
        pub type AppEventSender = futures_channel::mpsc::Sender<AppEvent>;
        pub type AppEventReceiver = futures_channel::mpsc::Receiver<AppEvent>;
    } else {
        // Fallback for alloc-only builds (no channels available)
        pub type CommandSender = ();
        pub type CommandReceiver = ();
        pub type EventSender = ();
        pub type EventReceiver = ();
        pub type EffectSender = ();
        pub type EffectReceiver = ();
        pub type AppEventSender = ();
        pub type AppEventReceiver = ();
    }
}

// ----------------------------------------------------------------------------
// Channel Creation Utilities
// ----------------------------------------------------------------------------

/// Create bounded command channel (UI → Core Logic)
pub fn create_command_channel(config: &ChannelConfig) -> (CommandSender, CommandReceiver) {
    cfg_if::cfg_if! {
        if #[cfg(feature = "std")] {
            tokio::sync::mpsc::channel(config.command_buffer_size)
        } else if #[cfg(feature = "wasm")] {
            futures_channel::mpsc::channel(config.command_buffer_size)
        } else {
            let _ = config; // Suppress unused parameter warning
            ((), ()) // Fallback for alloc-only builds
        }
    }
}

/// Create bounded event channel (Transport → Core Logic)
pub fn create_event_channel(config: &ChannelConfig) -> (EventSender, EventReceiver) {
    cfg_if::cfg_if! {
        if #[cfg(feature = "std")] {
            tokio::sync::mpsc::channel(config.event_buffer_size)
        } else if #[cfg(feature = "wasm")] {
            futures_channel::mpsc::channel(config.event_buffer_size)
        } else {
            let _ = config; // Suppress unused parameter warning
            ((), ()) // Fallback for alloc-only builds
        }
    }
}

/// Create broadcast effect channel (One-to-Many: Core Logic → Transports)
/// Returns a sender and a _receiver. Actual receivers should be created by calling sender.subscribe()
pub fn create_effect_channel(config: &ChannelConfig) -> (EffectSender, EffectReceiver) {
    cfg_if::cfg_if! {
        if #[cfg(feature = "std")] {
            tokio::sync::broadcast::channel(config.effect_buffer_size)
        } else if #[cfg(feature = "wasm")] {
            let (sender, receiver) = async_broadcast::channel(config.effect_buffer_size);
            // Match tokio broadcast behaviour by dropping oldest messages on overflow
            sender.set_overflow(true);
            (sender, receiver)
        } else {
            let _ = config; // Suppress unused parameter warning
            ((), ()) // Fallback for alloc-only builds
        }
    }
}

/// Create an effect receiver by subscribing to the broadcast channel
/// This is how transports should get their effect receivers
pub fn create_effect_receiver(effect_sender: &EffectSender) -> EffectReceiver {
    cfg_if::cfg_if! {
        if #[cfg(feature = "std")] {
            effect_sender.subscribe()
        } else if #[cfg(feature = "wasm")] {
            effect_sender.new_receiver()
        } else {
            let _ = effect_sender;
            () // Fallback for alloc-only builds
        }
    }
}

/// Create bounded app event channel (Core Logic → UI)
pub fn create_app_event_channel(config: &ChannelConfig) -> (AppEventSender, AppEventReceiver) {
    cfg_if::cfg_if! {
        if #[cfg(feature = "std")] {
            tokio::sync::mpsc::channel(config.app_event_buffer_size)
        } else if #[cfg(feature = "wasm")] {
            futures_channel::mpsc::channel(config.app_event_buffer_size)
        } else {
            let _ = config; // Suppress unused parameter warning
            ((), ()) // Fallback for alloc-only builds
        }
    }
}

// ----------------------------------------------------------------------------
// Non-blocking Send Utilities
// ----------------------------------------------------------------------------

/// Non-blocking send for UI tasks to prevent freezing
pub trait NonBlockingSend<T> {
    fn try_send_non_blocking(&mut self, message: T) -> Result<(), ChannelError>;
}

cfg_if::cfg_if! {
    if #[cfg(feature = "std")] {
        impl NonBlockingSend<Command> for CommandSender {
            fn try_send_non_blocking(&mut self, command: Command) -> Result<(), ChannelError> {
                self.try_send(command).map_err(|e| match e {
                    tokio::sync::mpsc::error::TrySendError::Full(_) => ChannelError::ChannelFull,
                    tokio::sync::mpsc::error::TrySendError::Closed(_) => ChannelError::ChannelClosed,
                })
            }
        }

        impl NonBlockingSend<AppEvent> for AppEventSender {
            fn try_send_non_blocking(&mut self, event: AppEvent) -> Result<(), ChannelError> {
                self.try_send(event).map_err(|e| match e {
                    tokio::sync::mpsc::error::TrySendError::Full(_) => ChannelError::ChannelFull,
                    tokio::sync::mpsc::error::TrySendError::Closed(_) => ChannelError::ChannelClosed,
                })
            }
        }
    } else if #[cfg(feature = "wasm")] {
        impl NonBlockingSend<Command> for CommandSender {
            fn try_send_non_blocking(&mut self, command: Command) -> Result<(), ChannelError> {
                self.try_send(command).map_err(|e| {
                    if e.is_full() {
                        ChannelError::ChannelFull
                    } else {
                        ChannelError::ChannelClosed
                    }
                })
            }
        }

        impl NonBlockingSend<AppEvent> for AppEventSender {
            fn try_send_non_blocking(&mut self, event: AppEvent) -> Result<(), ChannelError> {
                self.try_send(event).map_err(|e| {
                    if e.is_full() {
                        ChannelError::ChannelFull
                    } else {
                        ChannelError::ChannelClosed
                    }
                })
            }
        }
    }
}

// ----------------------------------------------------------------------------
// Channel Health Monitoring
// ----------------------------------------------------------------------------

use core::sync::atomic::{AtomicU64, AtomicU32, Ordering};

/// Channel utilization statistics for monitoring
/// Uses atomic counters to prevent race conditions in concurrent environments
#[derive(Debug)]
pub struct ChannelStats {
    pub channel_type: &'static str,
    pub buffer_size: usize,
    messages_sent: AtomicU64,
    messages_dropped: AtomicU64,
    // Store utilization as u32 representing percentage * 100 for precision
    current_utilization: AtomicU32,
}

impl ChannelStats {
    pub fn new(channel_type: &'static str, buffer_size: usize) -> Self {
        Self {
            channel_type,
            buffer_size,
            messages_sent: AtomicU64::new(0),
            messages_dropped: AtomicU64::new(0),
            current_utilization: AtomicU32::new(0),
        }
    }

    /// Record successful message send (thread-safe)
    pub fn record_send_success(&self) {
        self.messages_sent.fetch_add(1, Ordering::Relaxed);
    }

    /// Record dropped message (thread-safe)
    pub fn record_send_dropped(&self) {
        self.messages_dropped.fetch_add(1, Ordering::Relaxed);
    }

    /// Update current utilization (thread-safe)
    /// current_size: number of items currently in the channel buffer
    pub fn update_utilization(&self, current_size: usize) {
        let utilization_percent = if self.buffer_size == 0 {
            0
        } else {
            (current_size * 10000 / self.buffer_size) as u32 // Store as percentage * 100
        };
        self.current_utilization.store(utilization_percent, Ordering::Relaxed);
    }

    /// Get current drop rate (thread-safe)
    pub fn drop_rate(&self) -> f32 {
        let sent = self.messages_sent.load(Ordering::Relaxed);
        let dropped = self.messages_dropped.load(Ordering::Relaxed);
        
        if sent + dropped == 0 {
            0.0
        } else {
            dropped as f32 / (sent + dropped) as f32
        }
    }

    /// Get current utilization as a percentage (thread-safe)
    pub fn current_utilization(&self) -> f32 {
        self.current_utilization.load(Ordering::Relaxed) as f32 / 10000.0
    }

    /// Get total messages sent (thread-safe)
    pub fn messages_sent(&self) -> u64 {
        self.messages_sent.load(Ordering::Relaxed)
    }

    /// Get total messages dropped (thread-safe)
    pub fn messages_dropped(&self) -> u64 {
        self.messages_dropped.load(Ordering::Relaxed)
    }
}

// Implement Clone manually since atomic types don't implement Clone
impl Clone for ChannelStats {
    fn clone(&self) -> Self {
        Self {
            channel_type: self.channel_type,
            buffer_size: self.buffer_size,
            messages_sent: AtomicU64::new(self.messages_sent.load(Ordering::Relaxed)),
            messages_dropped: AtomicU64::new(self.messages_dropped.load(Ordering::Relaxed)),
            current_utilization: AtomicU32::new(self.current_utilization.load(Ordering::Relaxed)),
        }
    }
}

// ----------------------------------------------------------------------------
// Task Spawning Abstractions
// ----------------------------------------------------------------------------

/// Task spawning trait for platform compatibility
pub trait TaskSpawner {
    fn spawn<F>(&self, future: F)
    where
        F: core::future::Future<Output = ()> + Send + 'static;
    
    fn spawn_local<F>(&self, future: F)
    where
        F: core::future::Future<Output = ()> + 'static;
}

cfg_if::cfg_if! {
    if #[cfg(feature = "std")] {
        /// Native tokio task spawner
        pub struct NativeTaskSpawner;

        impl TaskSpawner for NativeTaskSpawner {
            fn spawn<F>(&self, future: F)
            where
                F: core::future::Future<Output = ()> + Send + 'static,
            {
                tokio::spawn(future);
            }
            
            fn spawn_local<F>(&self, future: F)
            where
                F: core::future::Future<Output = ()> + 'static,
            {
                tokio::task::spawn_local(future);
            }
        }
    } else if #[cfg(feature = "wasm")] {
        /// WASM task spawner
        pub struct WasmTaskSpawner;

        impl TaskSpawner for WasmTaskSpawner {
            fn spawn<F>(&self, future: F)
            where
                F: core::future::Future<Output = ()> + Send + 'static,
            {
                wasm_bindgen_futures::spawn_local(future);
            }
            
            fn spawn_local<F>(&self, future: F)
            where
                F: core::future::Future<Output = ()> + 'static,
            {
                wasm_bindgen_futures::spawn_local(future);
            }
        }
    }
}

// ----------------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_config_defaults() {
        let config = ChannelConfig::default();
        assert_eq!(config.command_buffer_size, 32);
        assert_eq!(config.event_buffer_size, 128);
        assert_eq!(config.effect_buffer_size, 64);
        assert_eq!(config.app_event_buffer_size, 64);
    }

    #[test]
    fn test_channel_stats() {
        let stats = ChannelStats::new("test", 100);
        assert_eq!(stats.drop_rate(), 0.0);
        
        stats.record_send_success();
        stats.record_send_success();
        stats.record_send_dropped();
        
        assert_eq!(stats.messages_sent(), 2);
        assert_eq!(stats.messages_dropped(), 1);
        assert!((stats.drop_rate() - 0.333).abs() < 0.01);
        
        stats.update_utilization(50);
        assert!((stats.current_utilization() - 0.5).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_command_channel_creation() {
        let config = ChannelConfig::default();
        let (sender, mut receiver) = create_command_channel(&config);
        
        let test_command = Command::StartDiscovery;
        sender.send(test_command).await.unwrap();
        
        let received = receiver.recv().await.unwrap();
        match received {
            Command::StartDiscovery => (),
            _ => panic!("Unexpected command type"),
        }
    }
}
#[cfg(all(feature = "std", feature = "wasm"))]
compile_error!("`std` and `wasm` features are mutually exclusive. Enable only one channel backend at a time.");
