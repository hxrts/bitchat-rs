//! Channel utilities for BitChat harness
//!
//! Provides feature-gated channel implementations for std (Tokio) and wasm targets.

use bitchat_core::config::ChannelConfig;

use crate::messages::{AppEvent, Command, Effect, Event};

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

pub fn create_command_channel(config: &ChannelConfig) -> (CommandSender, CommandReceiver) {
    cfg_if::cfg_if! {
        if #[cfg(feature = "std")] {
            tokio::sync::mpsc::channel(config.command_buffer_size)
        } else if #[cfg(feature = "wasm")] {
            futures_channel::mpsc::channel(config.command_buffer_size)
        } else {
            let _ = config;
            ((), ())
        }
    }
}

pub fn create_event_channel(config: &ChannelConfig) -> (EventSender, EventReceiver) {
    cfg_if::cfg_if! {
        if #[cfg(feature = "std")] {
            tokio::sync::mpsc::channel(config.event_buffer_size)
        } else if #[cfg(feature = "wasm")] {
            futures_channel::mpsc::channel(config.event_buffer_size)
        } else {
            let _ = config;
            ((), ())
        }
    }
}

pub fn create_effect_channel(config: &ChannelConfig) -> (EffectSender, EffectReceiver) {
    cfg_if::cfg_if! {
        if #[cfg(feature = "std")] {
            tokio::sync::broadcast::channel(config.effect_buffer_size)
        } else if #[cfg(feature = "wasm")] {
            let (sender, receiver) = async_broadcast::channel(config.effect_buffer_size);
            sender.set_overflow(true);
            (sender, receiver)
        } else {
            let _ = config;
            ((), ())
        }
    }
}

pub fn create_effect_receiver(effect_sender: &EffectSender) -> EffectReceiver {
    cfg_if::cfg_if! {
        if #[cfg(feature = "std")] {
            effect_sender.subscribe()
        } else if #[cfg(feature = "wasm")] {
            effect_sender.new_receiver()
        } else {
            let _ = effect_sender;
            ()
        }
    }
}

pub fn create_app_event_channel(config: &ChannelConfig) -> (AppEventSender, AppEventReceiver) {
    cfg_if::cfg_if! {
        if #[cfg(feature = "std")] {
            tokio::sync::mpsc::channel(config.app_event_buffer_size)
        } else if #[cfg(feature = "wasm")] {
            futures_channel::mpsc::channel(config.app_event_buffer_size)
        } else {
            let _ = config;
            ((), ())
        }
    }
}

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
