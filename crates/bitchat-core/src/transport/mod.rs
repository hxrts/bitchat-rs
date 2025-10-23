//! Transport layer implementations and failover logic
//!
//! This module provides transport abstractions and failover mechanisms for the BitChat protocol.
//! It implements the canonical transport selection strategy from the Swift/iOS implementation,
//! adapted for the Rust CSP-based architecture.

pub mod failover;
pub mod advanced_failover;
pub mod integration;

pub use failover::{
    BasicTransportManager, FailoverConfig, TransportType, TransportStatus,
    BasicRoutingStrategy, PeerReachability, MessageContext, TransportSelection,
};

pub use advanced_failover::{
    AdvancedTransportManager, AdvancedFailoverConfig, TransportHealth,
    TransportHealthMonitor, RoutingRule, RoutingTable, QueuedMessage,
};

pub use integration::{
    TransportFailoverCoordinator, FailoverIntegrationConfig, TransportRoutingDecision,
    QueuedMessageReady, TransportSwitchingRecommendation, FailoverEffect, FailoverEvent,
};