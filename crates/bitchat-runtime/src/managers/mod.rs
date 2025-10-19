//! Stateful managers for the BitChat runtime
//!
//! This module contains manager structs that maintain state and orchestrate
//! protocol functionality across the BitChat system.

pub mod connection;
pub mod delivery;
pub mod session;

pub use connection::{ConnectionManager, ConnectionStats, StateDistribution};
pub use delivery::{DeliveryStatistics, DeliveryTracker};
pub use session::{NoiseSessionManager, SessionTimeouts};
