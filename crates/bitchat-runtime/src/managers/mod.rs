//! Stateful managers for the BitChat runtime
//!
//! This module contains manager structs that maintain state and orchestrate
//! protocol functionality across the BitChat system.

pub mod delivery;
pub mod session;
pub mod connection;

pub use delivery::{DeliveryTracker, DeliveryStatistics};
pub use session::{NoiseSessionManager, SessionTimeouts};
pub use connection::{ConnectionManager, ConnectionStats, StateDistribution};