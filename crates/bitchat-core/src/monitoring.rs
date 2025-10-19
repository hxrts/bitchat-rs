//! Comprehensive Monitoring and Logging
//!
//! Advanced monitoring for task communication debugging, channel utilization,
//! and operational health monitoring

use crate::{
    task_logging::{TaskId, LogLevel, CommEvent},
};
use serde::{Serialize, Deserialize};

cfg_if::cfg_if! {
    if #[cfg(feature = "std")] {
        use std::collections::{HashMap, VecDeque};
        use std::sync::{Arc, Mutex};
        use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
    } else {
        use alloc::collections::{BTreeMap as HashMap, VecDeque};
        use core::time::Duration;
        // Note: no_std version would need alternative sync and time implementations
    }
}

// ----------------------------------------------------------------------------
// Monitoring Configuration
// ----------------------------------------------------------------------------

/// Configuration for monitoring and logging
#[derive(Debug, Clone)]
pub struct MonitoringConfig {
    /// Maximum number of communication events to retain
    pub max_comm_events: usize,
    /// Maximum number of performance samples to retain
    pub max_performance_samples: usize,
    /// Interval for collecting performance metrics
    pub metrics_interval: Duration,
    /// Enable detailed channel utilization tracking
    pub track_channel_utilization: bool,
    /// Enable task health monitoring
    pub enable_health_monitoring: bool,
    /// Log level for monitoring output
    pub log_level: LogLevel,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            max_comm_events: 1000,
            max_performance_samples: 500,
            metrics_interval: Duration::from_secs(1),
            track_channel_utilization: true,
            enable_health_monitoring: true,
            log_level: LogLevel::Info,
        }
    }
}

// ----------------------------------------------------------------------------
// Channel Utilization Monitoring
// ----------------------------------------------------------------------------

/// Channel utilization statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelUtilization {
    /// Channel identifier
    pub channel_name: String,
    /// Current buffer usage (0.0 - 1.0)
    pub buffer_usage: f32,
    /// Messages sent per second
    pub send_rate: f64,
    /// Messages received per second
    pub receive_rate: f64,
    /// Number of times channel was full (backpressure events)
    pub backpressure_events: u64,
    /// Average message processing latency
    pub avg_latency_ms: f64,
    /// Peak buffer usage in last monitoring period
    pub peak_usage: f32,
    /// Time of last measurement
    pub timestamp: u64,
}

impl ChannelUtilization {
    pub fn new(channel_name: String) -> Self {
        Self {
            channel_name,
            buffer_usage: 0.0,
            send_rate: 0.0,
            receive_rate: 0.0,
            backpressure_events: 0,
            avg_latency_ms: 0.0,
            peak_usage: 0.0,
            timestamp: current_timestamp(),
        }
    }

    /// Update utilization metrics
    pub fn update(&mut self, buffer_usage: f32, send_count: u64, receive_count: u64, 
                  backpressure_count: u64, latency_ms: f64) {
        let now = current_timestamp();
        let elapsed_secs = (now - self.timestamp) as f64 / 1000.0;
        
        if elapsed_secs > 0.0 {
            self.send_rate = send_count as f64 / elapsed_secs;
            self.receive_rate = receive_count as f64 / elapsed_secs;
        }
        
        self.buffer_usage = buffer_usage;
        self.backpressure_events = backpressure_count;
        self.avg_latency_ms = latency_ms;
        self.peak_usage = self.peak_usage.max(buffer_usage);
        self.timestamp = now;
    }
}

// ----------------------------------------------------------------------------
// Task Health Monitoring
// ----------------------------------------------------------------------------

/// Health status of a task
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskHealth {
    Healthy,
    Warning,
    Critical,
    Unresponsive,
}

/// Task health metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskHealthMetrics {
    /// Task identifier
    pub task_id: TaskId,
    /// Current health status
    pub health: TaskHealth,
    /// Last heartbeat timestamp
    pub last_heartbeat: u64,
    /// CPU time used (approximation)
    pub cpu_time_ms: u64,
    /// Memory usage estimate
    pub memory_usage_bytes: u64,
    /// Number of messages processed
    pub messages_processed: u64,
    /// Error count in last monitoring period
    pub error_count: u64,
    /// Average response time
    pub avg_response_time_ms: f64,
    /// Task uptime
    pub uptime_seconds: u64,
}

impl TaskHealthMetrics {
    pub fn new(task_id: TaskId) -> Self {
        Self {
            task_id,
            health: TaskHealth::Healthy,
            last_heartbeat: current_timestamp(),
            cpu_time_ms: 0,
            memory_usage_bytes: 0,
            messages_processed: 0,
            error_count: 0,
            avg_response_time_ms: 0.0,
            uptime_seconds: 0,
        }
    }

    /// Update health metrics and calculate health status
    pub fn update(&mut self, messages_delta: u64, errors_delta: u64, response_time_ms: f64) {
        let now = current_timestamp();
        let elapsed = (now - self.last_heartbeat) / 1000; // seconds
        
        self.messages_processed += messages_delta;
        self.error_count += errors_delta;
        self.avg_response_time_ms = (self.avg_response_time_ms + response_time_ms) / 2.0;
        self.uptime_seconds += elapsed;
        self.last_heartbeat = now;

        // Calculate health status
        self.health = self.calculate_health_status(now);
    }

    fn calculate_health_status(&self, current_time: u64) -> TaskHealth {
        let heartbeat_age = current_time - self.last_heartbeat;
        
        // Check if unresponsive (no heartbeat for 30 seconds)
        if heartbeat_age > 30000 {
            return TaskHealth::Unresponsive;
        }
        
        // Check error rate
        let error_rate = if self.messages_processed > 0 {
            self.error_count as f64 / self.messages_processed as f64
        } else {
            0.0
        };
        
        // Check response time
        let response_time_warning = 1000.0; // 1 second
        let response_time_critical = 5000.0; // 5 seconds
        
        if error_rate > 0.1 || self.avg_response_time_ms > response_time_critical {
            TaskHealth::Critical
        } else if error_rate > 0.05 || self.avg_response_time_ms > response_time_warning {
            TaskHealth::Warning
        } else {
            TaskHealth::Healthy
        }
    }
}

// ----------------------------------------------------------------------------
// Performance Metrics
// ----------------------------------------------------------------------------

/// System-wide performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// Timestamp of measurement
    pub timestamp: u64,
    /// Overall system throughput (messages/second)
    pub throughput: f64,
    /// Average end-to-end latency
    pub avg_latency_ms: f64,
    /// Peak latency in measurement period
    pub peak_latency_ms: f64,
    /// Memory usage estimate
    pub memory_usage_mb: f64,
    /// Active peer count
    pub active_peers: u32,
    /// Active transport count
    pub active_transports: u32,
    /// Total channel utilization
    pub total_channel_utilization: f32,
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl PerformanceMetrics {
    pub fn new() -> Self {
        Self {
            timestamp: current_timestamp(),
            throughput: 0.0,
            avg_latency_ms: 0.0,
            peak_latency_ms: 0.0,
            memory_usage_mb: 0.0,
            active_peers: 0,
            active_transports: 0,
            total_channel_utilization: 0.0,
        }
    }
}

// ----------------------------------------------------------------------------
// Communication Event Tracking
// ----------------------------------------------------------------------------

/// Enhanced communication event for detailed tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedCommEvent {
    /// Base communication event
    pub base_event: CommEvent,
    /// Processing latency in milliseconds
    pub processing_latency_ms: f64,
    /// Queue depth when message was processed
    pub queue_depth: usize,
    /// Whether message was dropped due to backpressure
    pub dropped: bool,
    /// Retry count for this message
    pub retry_count: u32,
}

// ----------------------------------------------------------------------------
// Monitoring System
// ----------------------------------------------------------------------------

/// Comprehensive monitoring system
pub struct MonitoringSystem {
    /// Configuration
    config: MonitoringConfig,
    /// Communication event history
    comm_events: Arc<Mutex<VecDeque<EnhancedCommEvent>>>,
    /// Channel utilization tracking
    channel_utilization: Arc<Mutex<HashMap<String, ChannelUtilization>>>,
    /// Task health metrics
    task_health: Arc<Mutex<HashMap<TaskId, TaskHealthMetrics>>>,
    /// Performance metrics history
    performance_history: Arc<Mutex<VecDeque<PerformanceMetrics>>>,
    /// Start time for uptime calculation
    start_time: Instant,
}

impl MonitoringSystem {
    /// Create new monitoring system
    pub fn new(config: MonitoringConfig) -> Self {
        Self {
            config,
            comm_events: Arc::new(Mutex::new(VecDeque::new())),
            channel_utilization: Arc::new(Mutex::new(HashMap::default())),
            task_health: Arc::new(Mutex::new(HashMap::default())),
            performance_history: Arc::new(Mutex::new(VecDeque::new())),
            start_time: Instant::now(),
        }
    }

    /// Record a communication event
    pub fn record_comm_event(&self, event: EnhancedCommEvent) {
        if let Ok(mut events) = self.comm_events.lock() {
            // Trim old events if necessary
            while events.len() >= self.config.max_comm_events {
                events.pop_front();
            }
            
            events.push_back(event);
        }
    }

    /// Update channel utilization
    pub fn update_channel_utilization(&self, channel_name: String, buffer_usage: f32, 
                                     send_count: u64, receive_count: u64, 
                                     backpressure_count: u64, latency_ms: f64) {
        if !self.config.track_channel_utilization {
            return;
        }

        if let Ok(mut utilization) = self.channel_utilization.lock() {
            let metrics = utilization.entry(channel_name.clone())
                .or_insert_with(|| ChannelUtilization::new(channel_name));
            
            metrics.update(buffer_usage, send_count, receive_count, backpressure_count, latency_ms);
        }
    }

    /// Update task health
    pub fn update_task_health(&self, task_id: TaskId, messages_delta: u64, 
                             errors_delta: u64, response_time_ms: f64) {
        if !self.config.enable_health_monitoring {
            return;
        }

        if let Ok(mut health) = self.task_health.lock() {
            let metrics = health.entry(task_id)
                .or_insert_with(|| TaskHealthMetrics::new(task_id));
            
            metrics.update(messages_delta, errors_delta, response_time_ms);
        }
    }

    /// Record performance metrics
    pub fn record_performance_metrics(&self, metrics: PerformanceMetrics) {
        if let Ok(mut history) = self.performance_history.lock() {
            // Trim old metrics if necessary
            while history.len() >= self.config.max_performance_samples {
                history.pop_front();
            }
            
            history.push_back(metrics);
        }
    }

    /// Get current channel utilization summary
    pub fn get_channel_utilization_summary(&self) -> HashMap<String, ChannelUtilization> {
        self.channel_utilization.lock()
            .map(|util| util.clone())
            .unwrap_or_default()
    }

    /// Get current task health summary
    pub fn get_task_health_summary(&self) -> HashMap<TaskId, TaskHealthMetrics> {
        self.task_health.lock()
            .map(|health| health.clone())
            .unwrap_or_default()
    }

    /// Get recent performance metrics
    pub fn get_recent_performance_metrics(&self, count: usize) -> Vec<PerformanceMetrics> {
        self.performance_history.lock()
            .map(|history| {
                history.iter()
                    .rev()
                    .take(count)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get communication events for analysis
    pub fn get_recent_comm_events(&self, count: usize) -> Vec<EnhancedCommEvent> {
        self.comm_events.lock()
            .map(|events| {
                events.iter()
                    .rev()
                    .take(count)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Generate monitoring report
    pub fn generate_report(&self) -> MonitoringReport {
        let channel_utilization = self.get_channel_utilization_summary();
        let task_health = self.get_task_health_summary();
        let recent_performance = self.get_recent_performance_metrics(10);
        let uptime = self.start_time.elapsed();

        MonitoringReport {
            timestamp: current_timestamp(),
            uptime_seconds: uptime.as_secs(),
            channel_utilization,
            task_health,
            recent_performance,
            overall_health: self.calculate_overall_health(),
        }
    }

    /// Calculate overall system health
    fn calculate_overall_health(&self) -> TaskHealth {
        let task_health = self.get_task_health_summary();
        
        if task_health.is_empty() {
            return TaskHealth::Warning;
        }

        let mut critical_count = 0;
        let mut warning_count = 0;
        let mut unresponsive_count = 0;

        for (_, health) in task_health {
            match health.health {
                TaskHealth::Critical => critical_count += 1,
                TaskHealth::Warning => warning_count += 1,
                TaskHealth::Unresponsive => unresponsive_count += 1,
                TaskHealth::Healthy => {}
            }
        }

        if unresponsive_count > 0 || critical_count > 0 {
            TaskHealth::Critical
        } else if warning_count > 0 {
            TaskHealth::Warning
        } else {
            TaskHealth::Healthy
        }
    }

    /// Check for potential deadlocks
    pub fn detect_potential_deadlocks(&self) -> Vec<DeadlockWarning> {
        let mut warnings = Vec::new();
        let channel_util = self.get_channel_utilization_summary();
        let task_health = self.get_task_health_summary();

        // Check for high channel utilization combined with slow task response
        for (channel_name, util) in channel_util {
            if util.buffer_usage > 0.9 && util.backpressure_events > 10 {
                // Check if related tasks are slow
                for (task_id, health) in &task_health {
                    if health.avg_response_time_ms > 5000.0 {
                        warnings.push(DeadlockWarning {
                            warning_type: DeadlockWarningType::HighChannelUtilization,
                            channel_name: Some(channel_name.clone()),
                            task_id: Some(*task_id),
                            description: format!(
                                "High utilization ({:.1}%) in {} with slow task response ({:.1}ms)",
                                util.buffer_usage * 100.0,
                                channel_name,
                                health.avg_response_time_ms
                            ),
                        });
                    }
                }
            }
        }

        // Check for circular wait conditions (simplified detection)
        let unresponsive_tasks: Vec<_> = task_health.iter()
            .filter(|(_, health)| health.health == TaskHealth::Unresponsive)
            .map(|(task_id, _)| *task_id)
            .collect();

        if unresponsive_tasks.len() >= 2 {
            warnings.push(DeadlockWarning {
                warning_type: DeadlockWarningType::MultipleUnresponsiveTasks,
                channel_name: None,
                task_id: None,
                description: format!("Multiple tasks unresponsive: {:?}", unresponsive_tasks),
            });
        }

        warnings
    }
}

// ----------------------------------------------------------------------------
// Monitoring Report
// ----------------------------------------------------------------------------

/// Comprehensive monitoring report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringReport {
    /// Report timestamp
    pub timestamp: u64,
    /// System uptime in seconds
    pub uptime_seconds: u64,
    /// Channel utilization by channel name
    pub channel_utilization: HashMap<String, ChannelUtilization>,
    /// Task health by task ID
    pub task_health: HashMap<TaskId, TaskHealthMetrics>,
    /// Recent performance metrics
    pub recent_performance: Vec<PerformanceMetrics>,
    /// Overall system health
    pub overall_health: TaskHealth,
}

// ----------------------------------------------------------------------------
// Deadlock Detection
// ----------------------------------------------------------------------------

/// Deadlock warning types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeadlockWarningType {
    HighChannelUtilization,
    MultipleUnresponsiveTasks,
    CircularWaitDetected,
    ResourceStarvation,
}

/// Deadlock warning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadlockWarning {
    pub warning_type: DeadlockWarningType,
    pub channel_name: Option<String>,
    pub task_id: Option<TaskId>,
    pub description: String,
}

// ----------------------------------------------------------------------------
// Utility Functions
// ----------------------------------------------------------------------------

/// Get current timestamp in milliseconds
fn current_timestamp() -> u64 {
    cfg_if::cfg_if! {
        if #[cfg(feature = "std")] {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64
        } else {
            // no_std fallback - would need alternative time source
            0
        }
    }
}

// ----------------------------------------------------------------------------
// Integration Helpers
// ----------------------------------------------------------------------------

/// Helper trait for integrating monitoring into tasks
pub trait Monitorable {
    /// Record a heartbeat for health monitoring
    fn record_heartbeat(&self, monitoring: &MonitoringSystem);
    
    /// Record message processing metrics
    fn record_message_processed(&self, monitoring: &MonitoringSystem, processing_time_ms: f64);
    
    /// Record an error for health monitoring
    fn record_error(&self, monitoring: &MonitoringSystem, error_description: &str);
}

/// Default implementation of Monitorable for task IDs
impl Monitorable for TaskId {
    fn record_heartbeat(&self, monitoring: &MonitoringSystem) {
        monitoring.update_task_health(*self, 0, 0, 0.0);
    }
    
    fn record_message_processed(&self, monitoring: &MonitoringSystem, processing_time_ms: f64) {
        monitoring.update_task_health(*self, 1, 0, processing_time_ms);
    }
    
    fn record_error(&self, monitoring: &MonitoringSystem, _error_description: &str) {
        monitoring.update_task_health(*self, 0, 1, 0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monitoring_config_default() {
        let config = MonitoringConfig::default();
        assert_eq!(config.max_comm_events, 1000);
        assert_eq!(config.max_performance_samples, 500);
        assert!(config.track_channel_utilization);
        assert!(config.enable_health_monitoring);
    }

    #[test]
    fn test_channel_utilization_creation() {
        let util = ChannelUtilization::new("test_channel".to_string());
        assert_eq!(util.channel_name, "test_channel");
        assert_eq!(util.buffer_usage, 0.0);
        assert_eq!(util.send_rate, 0.0);
        assert_eq!(util.receive_rate, 0.0);
    }

    #[test]
    fn test_task_health_metrics_creation() {
        let health = TaskHealthMetrics::new(TaskId::CoreLogic);
        assert_eq!(health.task_id, TaskId::CoreLogic);
        assert_eq!(health.health, TaskHealth::Healthy);
        assert_eq!(health.messages_processed, 0);
        assert_eq!(health.error_count, 0);
    }

    #[test]
    fn test_task_health_status_calculation() {
        let mut health = TaskHealthMetrics::new(TaskId::CoreLogic);
        
        // Healthy by default
        assert_eq!(health.health, TaskHealth::Healthy);
        
        // Update with high error rate
        health.messages_processed = 100;
        health.error_count = 20; // 20% error rate
        health.health = health.calculate_health_status(current_timestamp());
        assert_eq!(health.health, TaskHealth::Critical);
    }

    #[test]
    fn test_monitoring_system_creation() {
        let config = MonitoringConfig::default();
        let monitoring = MonitoringSystem::new(config);
        
        let utilization = monitoring.get_channel_utilization_summary();
        assert!(utilization.is_empty());
        
        let health = monitoring.get_task_health_summary();
        assert!(health.is_empty());
    }

    #[test]
    fn test_monitoring_system_channel_tracking() {
        let config = MonitoringConfig::default();
        let monitoring = MonitoringSystem::new(config);
        
        monitoring.update_channel_utilization(
            "test_channel".to_string(),
            0.5,
            100,
            95,
            5,
            10.0
        );
        
        let utilization = monitoring.get_channel_utilization_summary();
        assert_eq!(utilization.len(), 1);
        assert!(utilization.contains_key("test_channel"));
    }

    #[test]
    fn test_monitoring_system_task_health_tracking() {
        let config = MonitoringConfig::default();
        let monitoring = MonitoringSystem::new(config);
        
        monitoring.update_task_health(TaskId::CoreLogic, 10, 1, 50.0);
        
        let health = monitoring.get_task_health_summary();
        assert_eq!(health.len(), 1);
        assert!(health.contains_key(&TaskId::CoreLogic));
    }

    #[test]
    fn test_monitoring_report_generation() {
        let config = MonitoringConfig::default();
        let monitoring = MonitoringSystem::new(config);
        
        // Add some data
        monitoring.update_channel_utilization("test".to_string(), 0.3, 50, 45, 2, 5.0);
        monitoring.update_task_health(TaskId::CoreLogic, 5, 0, 25.0);
        
        let report = monitoring.generate_report();
        assert!(!report.channel_utilization.is_empty());
        assert!(!report.task_health.is_empty());
        assert_eq!(report.overall_health, TaskHealth::Healthy);
    }
}