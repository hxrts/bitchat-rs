//! Transport Failover Integration with CSP Architecture
//!
//! This module provides integration between the transport failover systems
//! and the CSP-based channel architecture, bridging the gap between 
//! standalone failover decision making and the runtime transport management.

use alloc::vec::Vec;
use alloc::string::{String, ToString};
use alloc::format;
use core::time::Duration;
use serde::{Deserialize, Serialize};

use crate::types::{PeerId, Timestamp};
use crate::ChannelTransportType;
use crate::channel::communication::TransportStatus as ChannelTransportStatus;
use crate::channel::Effect;
use crate::transport::failover::{TransportType, MessageContext, TransportSelection};
use crate::transport::advanced_failover::{AdvancedTransportManager, AdvancedFailoverConfig, TransportHealth, RoutingRule};

/// Transport failover integration configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailoverIntegrationConfig {
    /// Enable advanced failover logic
    pub enable_advanced_failover: bool,
    /// Enable integration with channel transport status
    pub enable_status_integration: bool,
    /// Interval for health check integration
    pub health_check_integration_interval: Duration,
    /// Enable automatic transport switching
    pub enable_auto_switching: bool,
}

impl Default for FailoverIntegrationConfig {
    fn default() -> Self {
        Self {
            enable_advanced_failover: true,
            enable_status_integration: true,
            health_check_integration_interval: Duration::from_secs(30),
            enable_auto_switching: true,
        }
    }
}

/// Convert between transport type representations
impl From<ChannelTransportType> for TransportType {
    fn from(channel_type: ChannelTransportType) -> Self {
        match channel_type {
            ChannelTransportType::Ble => TransportType::Ble,
            ChannelTransportType::Nostr => TransportType::Nostr,
        }
    }
}

impl From<TransportType> for ChannelTransportType {
    fn from(failover_type: TransportType) -> Self {
        match failover_type {
            TransportType::Ble => ChannelTransportType::Ble,
            TransportType::Nostr => ChannelTransportType::Nostr,
        }
    }
}

/// Transport failover coordinator that integrates with CSP channels
pub struct TransportFailoverCoordinator {
    /// Advanced transport manager
    advanced_manager: AdvancedTransportManager,
    /// Integration configuration
    config: FailoverIntegrationConfig,
    /// Last status sync timestamp
    last_status_sync: Option<Timestamp>,
    /// Last health check timestamp
    last_health_check: Option<Timestamp>,
    /// Cached transport statuses from channels
    cached_transport_status: Vec<(ChannelTransportType, ChannelTransportStatus)>,
}

impl TransportFailoverCoordinator {
    /// Create a new transport failover coordinator
    pub fn new(config: FailoverIntegrationConfig) -> Self {
        let advanced_config = AdvancedFailoverConfig {
            enable_performance_routing: config.enable_advanced_failover,
            enable_health_monitoring: config.enable_advanced_failover,
            enable_message_queuing: true,
            ..AdvancedFailoverConfig::default()
        };
        
        Self {
            advanced_manager: AdvancedTransportManager::new(advanced_config),
            config,
            last_status_sync: None,
            last_health_check: None,
            cached_transport_status: Vec::new(),
        }
    }
    
    /// Process incoming transport status updates from channels
    pub fn process_transport_status(&mut self, transport_type: ChannelTransportType, status: ChannelTransportStatus) {
        // Update cached status
        if let Some(existing) = self.cached_transport_status.iter_mut()
            .find(|(t, _)| *t == transport_type) {
            existing.1 = status.clone();
        } else {
            self.cached_transport_status.push((transport_type, status.clone()));
        }
        
        // Convert to failover transport type
        let failover_transport = TransportType::from(transport_type);
        
        // Update transport manager based on status
        let is_healthy = matches!(status, ChannelTransportStatus::Active);
        self.advanced_manager.record_transport_operation(
            failover_transport,
            is_healthy,
            None // No latency info from status updates
        );
        
        self.last_status_sync = Some(Timestamp::now());
    }
    
    /// Generate transport routing decision for a message
    pub fn route_message(&mut self, context: MessageContext) -> TransportRoutingDecision {
        let selection = if self.config.enable_advanced_failover {
            self.advanced_manager.select_transport_advanced(&context)
        } else {
            // Fall back to basic manager logic
            // Note: We need access to the basic manager, but it's wrapped in advanced_manager
            // For now, use the advanced manager which internally uses basic logic
            self.advanced_manager.select_transport_advanced(&context)
        };
        
        match selection {
            TransportSelection::UseTransport(transport) => {
                TransportRoutingDecision::UseTransport {
                    transport: ChannelTransportType::from(transport),
                    reason: "Selected by failover logic".to_string(),
                }
            }
            TransportSelection::UseAll(transports) => {
                let channel_transports = transports.into_iter()
                    .map(ChannelTransportType::from)
                    .collect();
                TransportRoutingDecision::UseMultiple {
                    transports: channel_transports,
                    reason: "Broadcast strategy".to_string(),
                }
            }
            TransportSelection::Queue => {
                TransportRoutingDecision::Queue {
                    reason: "No healthy transports available".to_string(),
                }
            }
            TransportSelection::CannotSend { reason } => {
                TransportRoutingDecision::CannotSend { reason }
            }
        }
    }
    
    /// Queue a message for later delivery
    pub fn queue_message(&mut self, context: MessageContext, peer_id: Option<PeerId>, payload: Vec<u8>) -> bool {
        self.advanced_manager.queue_message(context, peer_id, payload)
    }
    
    /// Process queued messages and return ready-to-send messages
    pub fn process_message_queue(&mut self) -> Vec<QueuedMessageReady> {
        self.advanced_manager.process_message_queue()
            .into_iter()
            .map(|(context, peer_id, payload)| {
                let recommended_transport = self.route_message(context.clone());
                QueuedMessageReady {
                    context,
                    peer_id,
                    payload,
                    recommended_transport,
                }
            })
            .collect()
    }
    
    /// Check if health monitoring is needed and generate effects
    pub fn check_health_monitoring(&mut self) -> Vec<Effect> {
        if !self.config.enable_advanced_failover {
            return Vec::new();
        }
        
        let now = Timestamp::now();
        let needs_check = match self.last_health_check {
            Some(last) => now.duration_since(last) > self.config.health_check_integration_interval,
            None => true,
        };
        
        if !needs_check {
            return Vec::new();
        }
        
        self.last_health_check = Some(now);
        
        let transports_to_check = self.advanced_manager.trigger_health_check();
        
        // Generate health check effects for each transport
        transports_to_check.into_iter()
            .map(|transport| {
                let channel_transport = ChannelTransportType::from(transport);
                Effect::RequestTransportHealthCheck {
                    transport_type: channel_transport,
                    timeout: Duration::from_secs(5),
                }
            })
            .collect()
    }
    
    /// Process health check result
    pub fn process_health_check_result(&mut self, transport: ChannelTransportType, success: bool, latency_ms: Option<u64>) {
        let failover_transport = TransportType::from(transport);
        self.advanced_manager.complete_health_check(failover_transport, success, latency_ms);
    }
    
    /// Update peer reachability information
    pub fn update_peer_reachability(&mut self, _peer_id: PeerId, _ble_reachable: bool, _nostr_available: bool) {
        // Delegate to advanced manager's basic manager
        // Note: This requires exposing the basic manager or adding this method to advanced manager
        // For now, we'll track this internally
    }
    
    /// Add a custom routing rule
    pub fn add_routing_rule(&mut self, rule: RoutingRule) {
        self.advanced_manager.add_routing_rule(rule);
    }
    
    /// Get current transport health metrics
    pub fn get_transport_health(&self, transport: ChannelTransportType) -> Option<&TransportHealth> {
        let failover_transport = TransportType::from(transport);
        self.advanced_manager.get_transport_health(failover_transport)
    }
    
    /// Get transport performance scores
    pub fn get_transport_scores(&self) -> Vec<(ChannelTransportType, f64)> {
        self.advanced_manager.get_transport_scores()
            .into_iter()
            .map(|(transport, score)| (ChannelTransportType::from(transport), score))
            .collect()
    }
    
    /// Get queue statistics
    pub fn get_queue_stats(&self) -> (usize, usize) {
        self.advanced_manager.get_queue_stats()
    }
    
    /// Generate automatic transport switching recommendations
    pub fn check_auto_switching(&self) -> Vec<TransportSwitchingRecommendation> {
        if !self.config.enable_auto_switching {
            return Vec::new();
        }
        
        let mut recommendations = Vec::new();
        let transport_scores = self.get_transport_scores();
        
        // Check for significant performance differences
        if transport_scores.len() >= 2 {
            let best_score = transport_scores[0].1;
            let worst_score = transport_scores[transport_scores.len() - 1].1;
            
            // If there's a significant performance gap, recommend switching
            if best_score - worst_score > 0.3 {
                recommendations.push(TransportSwitchingRecommendation {
                    from_transport: transport_scores[transport_scores.len() - 1].0,
                    to_transport: transport_scores[0].0,
                    reason: format!("Performance difference: {:.2} vs {:.2}", worst_score, best_score),
                    confidence: ((best_score - worst_score) * 100.0) as u32,
                });
            }
        }
        
        recommendations
    }
}

/// Transport routing decision result
#[derive(Debug, Clone)]
pub enum TransportRoutingDecision {
    /// Use specific transport
    UseTransport {
        transport: ChannelTransportType,
        reason: String,
    },
    /// Use multiple transports (broadcast)
    UseMultiple {
        transports: Vec<ChannelTransportType>,
        reason: String,
    },
    /// Queue message for later
    Queue {
        reason: String,
    },
    /// Cannot send message
    CannotSend {
        reason: String,
    },
}

/// Queued message ready for sending
#[derive(Debug, Clone)]
pub struct QueuedMessageReady {
    /// Message context
    pub context: MessageContext,
    /// Target peer ID
    pub peer_id: Option<PeerId>,
    /// Message payload
    pub payload: Vec<u8>,
    /// Recommended transport routing
    pub recommended_transport: TransportRoutingDecision,
}

/// Transport switching recommendation
#[derive(Debug, Clone)]
pub struct TransportSwitchingRecommendation {
    /// Current transport to switch from
    pub from_transport: ChannelTransportType,
    /// Target transport to switch to
    pub to_transport: ChannelTransportType,
    /// Reason for recommendation
    pub reason: String,
    /// Confidence level (0-100)
    pub confidence: u32,
}

/// Extension effects for health checking
impl Effect {
    /// Request transport health check
    pub fn request_transport_health_check(transport_type: ChannelTransportType, timeout: Duration) -> Self {
        Effect::RequestTransportHealthCheck {
            transport_type,
            timeout,
        }
    }
}

/// Additional effect types needed for transport failover integration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FailoverEffect {
    /// Request health check for a transport
    RequestHealthCheck {
        transport_type: ChannelTransportType,
        timeout: Duration,
    },
    /// Switch primary transport
    SwitchPrimaryTransport {
        from: ChannelTransportType,
        to: ChannelTransportType,
        reason: String,
    },
    /// Update transport performance metrics
    UpdatePerformanceMetrics {
        transport_type: ChannelTransportType,
        latency_ms: Option<u64>,
        success_rate: f64,
    },
    /// Flush message queue for specific peer
    FlushMessageQueue {
        peer_id: Option<PeerId>,
    },
}

/// Additional event types for transport failover
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FailoverEvent {
    /// Health check completed
    HealthCheckCompleted {
        transport_type: ChannelTransportType,
        success: bool,
        latency_ms: Option<u64>,
    },
    /// Transport performance degraded
    TransportPerformanceDegraded {
        transport_type: ChannelTransportType,
        previous_score: f64,
        current_score: f64,
    },
    /// Transport failover occurred
    TransportFailover {
        from_transport: ChannelTransportType,
        to_transport: ChannelTransportType,
        reason: String,
    },
    /// Message queue flushed
    MessageQueueFlushed {
        peer_id: Option<PeerId>,
        messages_sent: u32,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::PeerId;
    
    fn create_test_peer_id(id: u8) -> PeerId {
        PeerId::new([id, 0, 0, 0, 0, 0, 0, 0])
    }
    
    #[test]
    fn test_transport_type_conversion() {
        // Test ChannelTransportType to TransportType conversion
        assert_eq!(TransportType::from(ChannelTransportType::Ble), TransportType::Ble);
        assert_eq!(TransportType::from(ChannelTransportType::Nostr), TransportType::Nostr);
        
        // Test TransportType to ChannelTransportType conversion
        assert_eq!(ChannelTransportType::from(TransportType::Ble), ChannelTransportType::Ble);
        assert_eq!(ChannelTransportType::from(TransportType::Nostr), ChannelTransportType::Nostr);
    }
    
    #[test]
    fn test_failover_coordinator_creation() {
        let config = FailoverIntegrationConfig::default();
        let coordinator = TransportFailoverCoordinator::new(config);
        
        // Should start with empty cached status
        assert!(coordinator.cached_transport_status.is_empty());
        assert!(coordinator.last_status_sync.is_none());
    }
    
    #[test]
    fn test_transport_status_processing() {
        let config = FailoverIntegrationConfig::default();
        let mut coordinator = TransportFailoverCoordinator::new(config);
        
        let status = ChannelTransportStatus::Active;
        
        coordinator.process_transport_status(ChannelTransportType::Ble, status.clone());
        
        // Should cache the status
        assert_eq!(coordinator.cached_transport_status.len(), 1);
        assert_eq!(coordinator.cached_transport_status[0].0, ChannelTransportType::Ble);
        assert_eq!(coordinator.cached_transport_status[0].1, ChannelTransportStatus::Active);
    }
    
    #[test]
    fn test_message_routing() {
        let config = FailoverIntegrationConfig::default();
        let mut coordinator = TransportFailoverCoordinator::new(config);
        
        // Process healthy transport status
        let healthy_status = ChannelTransportStatus::Active;
        
        coordinator.process_transport_status(ChannelTransportType::Ble, healthy_status);
        
        // Route a mesh message (should prefer BLE)
        let context = MessageContext::PublicMesh;
        let decision = coordinator.route_message(context);
        
        match decision {
            TransportRoutingDecision::UseTransport { transport, .. } => {
                assert_eq!(transport, ChannelTransportType::Ble);
            }
            _ => panic!("Expected UseTransport decision"),
        }
    }
    
    #[test]
    fn test_health_check_generation() {
        let config = FailoverIntegrationConfig {
            enable_advanced_failover: true,
            health_check_integration_interval: Duration::from_secs(1),
            ..Default::default()
        };
        let mut coordinator = TransportFailoverCoordinator::new(config);
        
        // Should generate health check effects initially
        let effects = coordinator.check_health_monitoring();
        assert!(!effects.is_empty());
        
        // Shouldn't generate effects immediately after
        let effects2 = coordinator.check_health_monitoring();
        assert!(effects2.is_empty());
    }
    
    #[test]
    fn test_message_queuing_integration() {
        let config = FailoverIntegrationConfig::default();
        let mut coordinator = TransportFailoverCoordinator::new(config);
        
        let context = MessageContext::Private { recipient: create_test_peer_id(1) };
        let payload = vec![1, 2, 3, 4];
        
        // Queue a message
        assert!(coordinator.queue_message(context.clone(), Some(create_test_peer_id(1)), payload.clone()));
        
        // Check queue stats
        let (queue_size, _) = coordinator.get_queue_stats();
        assert_eq!(queue_size, 1);
        
        // Process queue (should be empty since no transports are healthy)
        let ready_messages = coordinator.process_message_queue();
        assert!(ready_messages.is_empty());
    }
}