//! Advanced Transport Failover Logic
//!
//! This module implements the Advanced Transport Failover Logic feature from the roadmap,
//! building upon the Basic Transport Failover to provide intelligent multi-transport routing
//! with performance-based decisions and active health monitoring.
//!
//! Design is based on the canonical BLEService health monitoring and MessageRouter patterns
//! from the Swift/iOS BitChat implementation, adapted for the Rust CSP-based architecture.

use hashbrown::HashMap;
use alloc::vec::Vec;
use alloc::collections::BTreeMap;
use core::time::Duration;
use serde::{Deserialize, Serialize};

use crate::types::{PeerId, Timestamp};
use crate::transport::failover::{TransportType, MessageContext, TransportSelection, BasicTransportManager};

/// Transport health metrics for performance-based routing decisions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportHealth {
    /// Average latency in milliseconds over recent operations
    pub latency_ms: Option<u64>,
    /// Success rate over recent operations (0.0 to 1.0)
    pub success_rate: f64,
    /// Timestamp of last successful operation
    pub last_success: Option<Timestamp>,
    /// Timestamp of last failure
    pub last_failure: Option<Timestamp>,
    /// Number of consecutive failures
    pub consecutive_failures: u32,
    /// Connection quality score (0.0 to 1.0, higher is better)
    pub connection_quality: f64,
    /// Recent operation history (success = true, failure = false)
    pub recent_operations: Vec<(Timestamp, bool, Option<u64>)>, // (timestamp, success, latency_ms)
    /// Transport capacity utilization (0.0 to 1.0)
    pub utilization: f64,
}

impl TransportHealth {
    /// Create new transport health tracker
    pub fn new() -> Self {
        Self {
            latency_ms: None,
            success_rate: 1.0,
            last_success: None,
            last_failure: None,
            consecutive_failures: 0,
            connection_quality: 1.0,
            recent_operations: Vec::new(),
            utilization: 0.0,
        }
    }
    
    /// Record a successful operation with optional latency
    pub fn record_success(&mut self, latency_ms: Option<u64>) {
        let now = Timestamp::now();
        self.last_success = Some(now);
        self.consecutive_failures = 0;
        self.recent_operations.push((now, true, latency_ms));
        
        // Update running latency average
        if let Some(latency) = latency_ms {
            self.latency_ms = Some(match self.latency_ms {
                Some(current) => (current + latency) / 2, // Simple exponential moving average
                None => latency,
            });
        }
        
        self.update_metrics();
    }
    
    /// Record a failed operation
    pub fn record_failure(&mut self) {
        let now = Timestamp::now();
        self.last_failure = Some(now);
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
        self.recent_operations.push((now, false, None));
        
        self.update_metrics();
    }
    
    /// Update derived metrics based on recent operations
    fn update_metrics(&mut self) {
        // Prune old operations (keep last 100 or last hour)
        let cutoff_time = Timestamp::now().saturating_sub(Duration::from_secs(3600));
        self.recent_operations.retain(|(timestamp, _, _)| *timestamp > cutoff_time);
        
        if self.recent_operations.len() > 100 {
            self.recent_operations.drain(0..self.recent_operations.len() - 100);
        }
        
        // Calculate success rate
        if !self.recent_operations.is_empty() {
            let successes = self.recent_operations.iter()
                .filter(|(_, success, _)| *success)
                .count();
            self.success_rate = successes as f64 / self.recent_operations.len() as f64;
        }
        
        // Calculate connection quality based on success rate and consecutive failures
        self.connection_quality = self.success_rate * 
            (1.0 - (self.consecutive_failures as f64 * 0.1).min(0.9));
    }
    
    /// Check if transport is healthy based on canonical criteria
    pub fn is_healthy(&self) -> bool {
        self.consecutive_failures < 3 && self.success_rate > 0.5
    }
    
    /// Get transport score for routing decisions (0.0 to 1.0, higher is better)
    pub fn get_transport_score(&self) -> f64 {
        let mut score = self.connection_quality;
        
        // Adjust for latency (canonical: prefer lower latency)
        if let Some(latency) = self.latency_ms {
            let latency_factor = if latency < 100 {
                1.0
            } else if latency < 500 {
                0.8
            } else if latency < 1000 {
                0.6
            } else {
                0.4
            };
            score *= latency_factor;
        }
        
        // Adjust for utilization (prefer less utilized transports)
        score *= 1.0 - self.utilization * 0.5;
        
        score.clamp(0.0, 1.0)
    }
}

/// Routing rule for specific peer and message type combinations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingRule {
    /// Optional peer ID (None means applies to all peers)
    pub peer_id: Option<PeerId>,
    /// Optional message context filter
    pub message_context: Option<MessageContext>,
    /// Preferred transport type
    pub preferred_transport: TransportType,
    /// Fallback transport types in priority order
    pub fallback_transports: Vec<TransportType>,
    /// Rule priority (higher numbers take precedence)
    pub priority: u32,
    /// Whether this rule is active
    pub enabled: bool,
}

impl RoutingRule {
    /// Check if this rule applies to the given context
    pub fn matches(&self, peer_id: Option<PeerId>, context: &MessageContext) -> bool {
        if !self.enabled {
            return false;
        }
        
        // Check peer ID match
        if let Some(rule_peer) = self.peer_id {
            if peer_id != Some(rule_peer) {
                return false;
            }
        }
        
        // Check message context match
        if let Some(ref rule_context) = self.message_context {
            if core::mem::discriminant(rule_context) != core::mem::discriminant(context) {
                return false;
            }
        }
        
        true
    }
}

/// Advanced routing table with configurable rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingTable {
    /// Ordered list of routing rules (higher priority first)
    pub rules: Vec<RoutingRule>,
    /// Default routing strategy when no rules match
    pub default_strategy: crate::transport::failover::BasicRoutingStrategy,
}

impl RoutingTable {
    /// Create a new routing table with canonical defaults
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            default_strategy: crate::transport::failover::BasicRoutingStrategy::PreferPrimary,
        }
    }
    
    /// Add a routing rule
    pub fn add_rule(&mut self, rule: RoutingRule) {
        self.rules.push(rule);
        // Sort by priority descending
        self.rules.sort_by(|a, b| b.priority.cmp(&a.priority));
    }
    
    /// Find the best matching rule for the given context
    pub fn find_rule(&self, peer_id: Option<PeerId>, context: &MessageContext) -> Option<&RoutingRule> {
        self.rules.iter()
            .find(|rule| rule.matches(peer_id, context))
    }
}

/// Transport health monitor with active health checking
#[derive(Debug)]
pub struct TransportHealthMonitor {
    /// Transport health metrics
    transport_health: HashMap<TransportType, TransportHealth>,
    /// Health check configuration
    config: HealthMonitorConfig,
    /// Last health check timestamp
    last_health_check: Option<Timestamp>,
    /// Pending health check requests
    pending_checks: HashMap<TransportType, Timestamp>,
}

/// Configuration for transport health monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthMonitorConfig {
    /// Interval between active health checks
    pub health_check_interval: Duration,
    /// Timeout for health check operations
    pub health_check_timeout: Duration,
    /// Maximum number of recent operations to track
    pub max_recent_operations: usize,
    /// Minimum success rate to consider transport healthy
    pub min_success_rate: f64,
    /// Maximum consecutive failures before marking unhealthy
    pub max_consecutive_failures: u32,
}

impl Default for HealthMonitorConfig {
    fn default() -> Self {
        Self {
            health_check_interval: Duration::from_secs(30),
            health_check_timeout: Duration::from_secs(5),
            max_recent_operations: 100,
            min_success_rate: 0.5,
            max_consecutive_failures: 3,
        }
    }
}

impl TransportHealthMonitor {
    /// Create a new transport health monitor
    pub fn new(config: HealthMonitorConfig) -> Self {
        Self {
            transport_health: HashMap::new(),
            config,
            last_health_check: None,
            pending_checks: HashMap::new(),
        }
    }
    
    /// Record transport operation result
    pub fn record_operation(&mut self, transport: TransportType, success: bool, latency_ms: Option<u64>) {
        let health = self.transport_health
            .entry(transport)
            .or_insert_with(TransportHealth::new);
        
        if success {
            health.record_success(latency_ms);
        } else {
            health.record_failure();
        }
    }
    
    /// Get transport health metrics
    pub fn get_health(&self, transport: TransportType) -> Option<&TransportHealth> {
        self.transport_health.get(&transport)
    }
    
    /// Check if active health check is needed
    pub fn needs_health_check(&self) -> bool {
        match self.last_health_check {
            Some(last) => Timestamp::now().duration_since(last) > self.config.health_check_interval,
            None => true,
        }
    }
    
    /// Trigger active health check for all transports
    pub fn trigger_health_check(&mut self) -> Vec<TransportType> {
        self.last_health_check = Some(Timestamp::now());
        
        let mut transports_to_check = Vec::new();
        for transport in [TransportType::Ble, TransportType::Nostr] {
            if !self.pending_checks.contains_key(&transport) {
                self.pending_checks.insert(transport, Timestamp::now());
                transports_to_check.push(transport);
            }
        }
        
        transports_to_check
    }
    
    /// Complete health check for a transport
    pub fn complete_health_check(&mut self, transport: TransportType, success: bool, latency_ms: Option<u64>) {
        self.pending_checks.remove(&transport);
        self.record_operation(transport, success, latency_ms);
    }
}

/// Advanced Transport Manager with intelligent routing and health monitoring
pub struct AdvancedTransportManager {
    /// Basic transport manager for core functionality
    basic_manager: BasicTransportManager,
    /// Transport health monitor
    health_monitor: TransportHealthMonitor,
    /// Routing table with configurable rules
    routing_table: RoutingTable,
    /// Message queue for failed deliveries
    message_queue: BTreeMap<Timestamp, QueuedMessage>,
    /// Configuration
    config: AdvancedFailoverConfig,
}

/// Configuration for advanced transport failover
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedFailoverConfig {
    /// Enable performance-based routing
    pub enable_performance_routing: bool,
    /// Enable active health monitoring
    pub enable_health_monitoring: bool,
    /// Enable message queuing for failed deliveries
    pub enable_message_queuing: bool,
    /// Maximum queue size for failed messages
    pub max_queue_size: usize,
    /// Queue retention time for failed messages
    pub queue_retention_time: Duration,
    /// Health monitor configuration
    pub health_monitor: HealthMonitorConfig,
}

impl Default for AdvancedFailoverConfig {
    fn default() -> Self {
        Self {
            enable_performance_routing: true,
            enable_health_monitoring: true,
            enable_message_queuing: true,
            max_queue_size: 1000,
            queue_retention_time: Duration::from_secs(3600), // 1 hour
            health_monitor: HealthMonitorConfig::default(),
        }
    }
}

/// Queued message for retry delivery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedMessage {
    /// Message context
    pub context: MessageContext,
    /// Target peer ID (if applicable)
    pub peer_id: Option<PeerId>,
    /// Message payload (serialized)
    pub payload: Vec<u8>,
    /// Number of retry attempts
    pub retry_count: u32,
    /// Timestamp when message was first queued
    pub queued_at: Timestamp,
    /// Timestamp of last retry attempt
    pub last_retry: Option<Timestamp>,
}

impl AdvancedTransportManager {
    /// Create a new advanced transport manager
    pub fn new(config: AdvancedFailoverConfig) -> Self {
        let basic_config = crate::transport::failover::FailoverConfig::default();
        let health_monitor = TransportHealthMonitor::new(config.health_monitor.clone());
        
        Self {
            basic_manager: BasicTransportManager::new(basic_config),
            health_monitor,
            routing_table: RoutingTable::new(),
            message_queue: BTreeMap::new(),
            config,
        }
    }
    
    /// Select transport using advanced routing logic
    pub fn select_transport_advanced(&mut self, context: &MessageContext) -> TransportSelection {
        let peer_id = match context {
            MessageContext::Private { recipient } |
            MessageContext::ReadReceipt { recipient } |
            MessageContext::DeliveryAck { recipient } |
            MessageContext::FavoriteNotification { recipient } => Some(*recipient),
            _ => None,
        };
        
        // Check for routing rule override
        if let Some(rule) = self.routing_table.find_rule(peer_id, context) {
            return self.apply_routing_rule(rule, context);
        }
        
        // Use performance-based selection if enabled
        if self.config.enable_performance_routing {
            return self.select_by_performance(context);
        }
        
        // Fall back to basic selection
        self.basic_manager.select_transport(context)
    }
    
    /// Apply a specific routing rule
    fn apply_routing_rule(&self, rule: &RoutingRule, _context: &MessageContext) -> TransportSelection {
        // Check if preferred transport is healthy
        if let Some(health) = self.health_monitor.get_health(rule.preferred_transport) {
            if health.is_healthy() && self.basic_manager.is_transport_healthy(rule.preferred_transport) {
                return TransportSelection::UseTransport(rule.preferred_transport);
            }
        }
        
        // Try fallback transports
        for fallback in &rule.fallback_transports {
            if let Some(health) = self.health_monitor.get_health(*fallback) {
                if health.is_healthy() && self.basic_manager.is_transport_healthy(*fallback) {
                    return TransportSelection::UseTransport(*fallback);
                }
            }
        }
        
        // No healthy transports in rule, queue message
        TransportSelection::Queue
    }
    
    /// Select transport based on performance metrics
    fn select_by_performance(&self, context: &MessageContext) -> TransportSelection {
        let mut transport_scores = Vec::new();
        
        // Calculate scores for each transport
        for transport in [TransportType::Ble, TransportType::Nostr] {
            if !self.basic_manager.is_transport_healthy(transport) {
                continue;
            }
            
            let score = if let Some(health) = self.health_monitor.get_health(transport) {
                health.get_transport_score()
            } else {
                0.5 // Default score for unknown health
            };
            
            transport_scores.push((transport, score));
        }
        
        // Sort by score descending
        transport_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(core::cmp::Ordering::Equal));
        
        // Apply canonical routing preferences
        match context {
            MessageContext::PublicMesh => {
                // Mesh messages always prefer BLE (canonical behavior)
                if transport_scores.iter().any(|(t, _)| *t == TransportType::Ble) {
                    TransportSelection::UseTransport(TransportType::Ble)
                } else {
                    TransportSelection::Queue
                }
            }
            MessageContext::PublicLocation => {
                // Location messages always prefer Nostr (canonical behavior)
                if transport_scores.iter().any(|(t, _)| *t == TransportType::Nostr) {
                    TransportSelection::UseTransport(TransportType::Nostr)
                } else {
                    TransportSelection::Queue
                }
            }
            _ => {
                // For private messages, use best performing transport
                if let Some((best_transport, _)) = transport_scores.first() {
                    TransportSelection::UseTransport(*best_transport)
                } else {
                    TransportSelection::Queue
                }
            }
        }
    }
    
    /// Queue a message for later delivery
    pub fn queue_message(&mut self, context: MessageContext, peer_id: Option<PeerId>, payload: Vec<u8>) -> bool {
        if !self.config.enable_message_queuing {
            return false;
        }
        
        if self.message_queue.len() >= self.config.max_queue_size {
            // Remove oldest message to make space
            if let Some((oldest_key, _)) = self.message_queue.iter().next() {
                let oldest_key = *oldest_key;
                self.message_queue.remove(&oldest_key);
            }
        }
        
        let queued_message = QueuedMessage {
            context,
            peer_id,
            payload,
            retry_count: 0,
            queued_at: Timestamp::now(),
            last_retry: None,
        };
        
        self.message_queue.insert(Timestamp::now(), queued_message);
        true
    }
    
    /// Process queued messages for retry
    pub fn process_message_queue(&mut self) -> Vec<(MessageContext, Option<PeerId>, Vec<u8>)> {
        let mut ready_messages = Vec::new();
        let mut expired_keys = Vec::new();
        let now = Timestamp::now();
        
        // Collect messages that need to be checked, avoiding borrow conflicts
        let mut messages_to_check = Vec::new();
        for (timestamp, message) in &self.message_queue {
            // Check if message has expired
            if now.duration_since(*timestamp) > self.config.queue_retention_time {
                expired_keys.push(*timestamp);
                continue;
            }
            
            messages_to_check.push((*timestamp, message.context.clone(), message.peer_id, message.payload.clone()));
        }
        
        // Now check transport availability for each message
        for (timestamp, context, peer_id, payload) in messages_to_check {
            let selection = self.select_transport_advanced(&context);
            if let TransportSelection::UseTransport(_) = selection {
                ready_messages.push((context, peer_id, payload));
                expired_keys.push(timestamp);
            }
        }
        
        // Remove processed and expired messages
        for key in expired_keys {
            self.message_queue.remove(&key);
        }
        
        ready_messages
    }
    
    /// Add a routing rule
    pub fn add_routing_rule(&mut self, rule: RoutingRule) {
        self.routing_table.add_rule(rule);
    }
    
    /// Record transport operation for health monitoring
    pub fn record_transport_operation(&mut self, transport: TransportType, success: bool, latency_ms: Option<u64>) {
        self.health_monitor.record_operation(transport, success, latency_ms);
        
        // Also update basic manager
        match transport {
            TransportType::Ble => {
                self.basic_manager.update_transport_status(transport, success, latency_ms);
            }
            TransportType::Nostr => {
                self.basic_manager.update_transport_status(transport, success, latency_ms);
            }
        }
    }
    
    /// Check if health monitoring is needed
    pub fn needs_health_check(&self) -> bool {
        self.config.enable_health_monitoring && self.health_monitor.needs_health_check()
    }
    
    /// Trigger health check
    pub fn trigger_health_check(&mut self) -> Vec<TransportType> {
        if !self.config.enable_health_monitoring {
            return Vec::new();
        }
        
        self.health_monitor.trigger_health_check()
    }
    
    /// Complete health check
    pub fn complete_health_check(&mut self, transport: TransportType, success: bool, latency_ms: Option<u64>) {
        self.health_monitor.complete_health_check(transport, success, latency_ms);
    }
    
    /// Get transport health metrics
    pub fn get_transport_health(&self, transport: TransportType) -> Option<&TransportHealth> {
        self.health_monitor.get_health(transport)
    }
    
    /// Get all available transports with their scores
    pub fn get_transport_scores(&self) -> Vec<(TransportType, f64)> {
        let mut scores = Vec::new();
        
        for transport in [TransportType::Ble, TransportType::Nostr] {
            if self.basic_manager.is_transport_healthy(transport) {
                let score = if let Some(health) = self.health_monitor.get_health(transport) {
                    health.get_transport_score()
                } else {
                    0.5
                };
                scores.push((transport, score));
            }
        }
        
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(core::cmp::Ordering::Equal));
        scores
    }
    
    /// Get queue statistics
    pub fn get_queue_stats(&self) -> (usize, usize) {
        let now = Timestamp::now();
        let expired_count = self.message_queue.iter()
            .filter(|(timestamp, _)| now.duration_since(**timestamp) > self.config.queue_retention_time)
            .count();
        
        (self.message_queue.len(), expired_count)
    }
}

impl Default for AdvancedTransportManager {
    fn default() -> Self {
        Self::new(AdvancedFailoverConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::PeerId;
    
    fn create_test_peer_id(id: u8) -> PeerId {
        PeerId::new([id, 0, 0, 0, 0, 0, 0, 0])
    }
    
    #[test]
    fn test_transport_health_tracking() {
        let mut health = TransportHealth::new();
        
        // Initially healthy
        assert!(health.is_healthy());
        assert_eq!(health.success_rate, 1.0);
        
        // Record some operations
        health.record_success(Some(50));
        health.record_success(Some(75));
        health.record_failure();
        
        // Should still be healthy with good success rate
        assert!(health.is_healthy());
        assert!(health.success_rate > 0.5);
        assert_eq!(health.latency_ms, Some(62)); // Average of 50 and 75
    }
    
    #[test]
    fn test_routing_rule_matching() {
        let peer1 = create_test_peer_id(1);
        let rule = RoutingRule {
            peer_id: Some(peer1),
            message_context: Some(MessageContext::Private { recipient: peer1 }),
            preferred_transport: TransportType::Ble,
            fallback_transports: vec![TransportType::Nostr],
            priority: 100,
            enabled: true,
        };
        
        // Should match specific peer and context
        assert!(rule.matches(Some(peer1), &MessageContext::Private { recipient: peer1 }));
        
        // Should not match different peer
        let peer2 = create_test_peer_id(2);
        assert!(!rule.matches(Some(peer2), &MessageContext::Private { recipient: peer2 }));
        
        // Should not match different context
        assert!(!rule.matches(Some(peer1), &MessageContext::PublicMesh));
    }
    
    #[test]
    fn test_performance_based_routing() {
        let mut manager = AdvancedTransportManager::new(AdvancedFailoverConfig::default());
        
        // Set up transport health
        manager.record_transport_operation(TransportType::Ble, true, Some(25)); // Fast BLE
        manager.record_transport_operation(TransportType::Nostr, true, Some(200)); // Slower Nostr
        
        // Should prefer BLE for private messages due to better performance
        let context = MessageContext::Private { recipient: create_test_peer_id(1) };
        
        // Update basic manager to mark transports as healthy
        manager.basic_manager.update_transport_status(TransportType::Ble, true, Some(25));
        manager.basic_manager.update_transport_status(TransportType::Nostr, true, Some(200));
        manager.basic_manager.update_peer_reachability(create_test_peer_id(1), true, true);
        
        let selection = manager.select_transport_advanced(&context);
        match selection {
            TransportSelection::UseTransport(TransportType::Ble) => {}, // Expected
            _ => panic!("Expected BLE transport for better performance"),
        }
    }
    
    #[test]
    fn test_message_queuing() {
        let mut manager = AdvancedTransportManager::new(AdvancedFailoverConfig::default());
        let context = MessageContext::Private { recipient: create_test_peer_id(1) };
        let payload = vec![1, 2, 3, 4];
        
        // Queue a message
        assert!(manager.queue_message(context.clone(), Some(create_test_peer_id(1)), payload.clone()));
        
        // Should have one queued message
        let (queue_size, _) = manager.get_queue_stats();
        assert_eq!(queue_size, 1);
        
        // Process queue (should not be ready yet since no transports are healthy)
        let ready = manager.process_message_queue();
        assert!(ready.is_empty());
    }
    
    #[test]
    fn test_canonical_message_routing() {
        let mut manager = AdvancedTransportManager::new(AdvancedFailoverConfig::default());
        
        // Set up healthy transports
        manager.basic_manager.update_transport_status(TransportType::Ble, true, Some(25));
        manager.basic_manager.update_transport_status(TransportType::Nostr, true, Some(100));
        
        // Mesh messages should always use BLE (canonical behavior)
        let mesh_selection = manager.select_transport_advanced(&MessageContext::PublicMesh);
        match mesh_selection {
            TransportSelection::UseTransport(TransportType::Ble) => {}, // Expected
            _ => panic!("Expected BLE transport for mesh messages"),
        }
        
        // Location messages should always use Nostr (canonical behavior)
        let location_selection = manager.select_transport_advanced(&MessageContext::PublicLocation);
        match location_selection {
            TransportSelection::UseTransport(TransportType::Nostr) => {}, // Expected
            _ => panic!("Expected Nostr transport for location messages"),
        }
    }
}