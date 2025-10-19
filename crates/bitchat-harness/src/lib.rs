#![cfg_attr(not(feature = "std"), no_std)]
#![doc = "BitChat Harness\n\nProvides the canonical channel/message schema and shared runtime utilities\nthat higher-level components (transports, runtime, applications) depend on."]

extern crate alloc;

pub mod messages;
pub mod channels;

pub use messages::*;
pub use channels::*;
