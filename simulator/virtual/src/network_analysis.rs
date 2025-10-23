//! Network Analysis Module for BitChat Protocol Validation
//!
//! This module provides network analysis capabilities including:
//! - Real-time packet capture and inspection
//! - Protocol compliance checking
//! - Performance metrics and benchmarking
//! - Network behavior analysis and reporting

#![allow(dead_code)] // Module provides API for future network analysis features

use std::collections::{HashMap, VecDeque};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};
use tracing::{info, debug};
use bitchat_core::{PeerId, protocol::MessageType};

// ----------------------------------------------------------------------------
// Packet Capture and Analysis
// ----------------------------------------------------------------------------

/// Captured network packet with analysis metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapturedPacket {
    /// Unique capture ID
    pub capture_id: u64,
    /// Source peer ID
    pub source: PeerId,
    /// Destination peer ID (None for broadcasts)
    pub destination: Option<PeerId>,
    /// Packet type (MESSAGE, ANNOUNCE, HANDSHAKE, etc.)
    pub packet_type: MessageType,
    /// Raw packet data
    pub data: Vec<u8>,
    /// Packet size in bytes
    pub size: usize,
    /// Timestamp when packet was captured
    pub captured_at: SystemTime,
    /// Network latency if measurable
    pub latency_ms: Option<u64>,
    /// Whether packet was successfully delivered
    pub delivered: bool,
    /// Protocol compliance status
    pub compliance_status: ComplianceStatus,
    /// Analysis flags
    pub analysis_flags: Vec<AnalysisFlag>,
}

/// Protocol compliance status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComplianceStatus {
    /// Packet follows protocol specification
    Compliant,
    /// Packet has warnings but is acceptable
    Warning { issues: Vec<String> },
    /// Packet violates protocol specification
    NonCompliant { violations: Vec<String> },
    /// Analysis could not be completed
    Unknown { reason: String },
}

/// Analysis flags for special packet characteristics
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AnalysisFlag {
    /// Packet was retransmitted
    Retransmission,
    /// Packet was reordered
    OutOfOrder,
    /// Packet is a duplicate
    Duplicate,
    /// Packet is fragmented
    Fragmented,
    /// Packet has security implications
    SecurityRelevant,
    /// Packet contains announce information
    PeerDiscovery,
    /// Packet is part of session establishment
    Handshake,
    /// Packet size is unusually large
    LargePayload,
    /// Packet has invalid structure
    Malformed,
}

// ----------------------------------------------------------------------------
// Network Performance Metrics
// ----------------------------------------------------------------------------

/// Comprehensive network performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkMetrics {
    /// Time window for these metrics
    pub window_start: SystemTime,
    pub window_end: SystemTime,
    
    /// Packet statistics
    pub total_packets: u64,
    pub successful_deliveries: u64,
    pub failed_deliveries: u64,
    pub duplicate_packets: u64,
    pub out_of_order_packets: u64,
    
    /// Latency statistics
    pub min_latency_ms: Option<u64>,
    pub max_latency_ms: Option<u64>,
    pub avg_latency_ms: Option<u64>,
    pub latency_p50_ms: Option<u64>,
    pub latency_p95_ms: Option<u64>,
    pub latency_p99_ms: Option<u64>,
    
    /// Throughput statistics
    pub bytes_transmitted: u64,
    pub bytes_received: u64,
    pub packets_per_second: f64,
    pub bytes_per_second: f64,
    
    /// Protocol-specific metrics
    pub announce_packets: u64,
    pub message_packets: u64,
    pub handshake_packets: u64,
    pub error_packets: u64,
    
    /// Peer statistics
    pub active_peers: u64,
    pub peer_connections: u64,
    pub peer_discoveries: u64,
    pub peer_timeouts: u64,
    
    /// Network quality indicators
    pub packet_loss_rate: f64,
    pub jitter_ms: f64,
    pub delivery_ratio: f64,
    pub protocol_compliance_rate: f64,
}

impl Default for NetworkMetrics {
    fn default() -> Self {
        let now = SystemTime::now();
        Self {
            window_start: now,
            window_end: now,
            total_packets: 0,
            successful_deliveries: 0,
            failed_deliveries: 0,
            duplicate_packets: 0,
            out_of_order_packets: 0,
            min_latency_ms: None,
            max_latency_ms: None,
            avg_latency_ms: None,
            latency_p50_ms: None,
            latency_p95_ms: None,
            latency_p99_ms: None,
            bytes_transmitted: 0,
            bytes_received: 0,
            packets_per_second: 0.0,
            bytes_per_second: 0.0,
            announce_packets: 0,
            message_packets: 0,
            handshake_packets: 0,
            error_packets: 0,
            active_peers: 0,
            peer_connections: 0,
            peer_discoveries: 0,
            peer_timeouts: 0,
            packet_loss_rate: 0.0,
            jitter_ms: 0.0,
            delivery_ratio: 1.0,
            protocol_compliance_rate: 1.0,
        }
    }
}

// ----------------------------------------------------------------------------
// Protocol Compliance Checker
// ----------------------------------------------------------------------------

/// Protocol compliance checker for BitChat packets
pub struct ProtocolChecker {
    /// Known protocol versions
    supported_versions: Vec<u8>,
    /// Maximum allowed packet size
    max_packet_size: usize,
    /// Minimum required fields for each packet type
    required_fields: HashMap<MessageType, Vec<String>>,
}

impl Default for ProtocolChecker {
    fn default() -> Self {
        let mut required_fields = HashMap::new();
        
        // Define required fields for each message type
        required_fields.insert(MessageType::Message, vec![
            "sender_id".to_string(),
            "payload".to_string(),
            "timestamp".to_string(),
        ]);
        
        required_fields.insert(MessageType::Announce, vec![
            "sender_id".to_string(),
            "payload".to_string(),
            "signature".to_string(),
        ]);
        
        required_fields.insert(MessageType::NoiseHandshake, vec![
            "sender_id".to_string(),
            "payload".to_string(),
        ]);

        Self {
            supported_versions: vec![1], // Protocol version 1
            max_packet_size: 64 * 1024, // 64KB max packet size
            required_fields,
        }
    }
}

impl ProtocolChecker {
    /// Check if a packet complies with the BitChat protocol
    pub fn check_compliance(&self, packet: &CapturedPacket) -> ComplianceStatus {
        let mut warnings = Vec::new();
        let mut violations = Vec::new();
        
        // Check packet size
        if packet.size > self.max_packet_size {
            violations.push(format!(
                "Packet size {} exceeds maximum allowed size {}",
                packet.size, self.max_packet_size
            ));
        }
        
        // Check if packet is too small
        if packet.size < 8 { // Minimum header size
            violations.push("Packet too small - missing required header".to_string());
        }
        
        // Try to parse as BitChat packet for validation
        match self.validate_packet_structure(&packet.data) {
            Ok(validation_warnings) => {
                warnings.extend(validation_warnings);
            }
            Err(validation_errors) => {
                violations.extend(validation_errors);
            }
        }
        
        // Check message type specific requirements
        if let Some(required) = self.required_fields.get(&packet.packet_type) {
            for field in required {
                if !self.packet_has_field(&packet.data, field) {
                    violations.push(format!("Missing required field: {}", field));
                }
            }
        }
        
        // Return compliance status
        if !violations.is_empty() {
            ComplianceStatus::NonCompliant { violations }
        } else if !warnings.is_empty() {
            ComplianceStatus::Warning { issues: warnings }
        } else {
            ComplianceStatus::Compliant
        }
    }
    
    /// Validate packet structure
    fn validate_packet_structure(&self, data: &[u8]) -> Result<Vec<String>, Vec<String>> {
        let mut warnings = Vec::new();
        let mut errors = Vec::new();
        
        // Basic structure validation
        if data.len() < 8 {
            errors.push("Packet too short for valid header".to_string());
            return Err(errors);
        }
        
        // Try to parse the packet
        match self.parse_packet_header(data) {
            Ok(header_info) => {
                // Validate header fields
                if header_info.version == 0 {
                    warnings.push("Protocol version 0 is deprecated".to_string());
                }
                
                if !self.supported_versions.contains(&header_info.version) {
                    errors.push(format!("Unsupported protocol version: {}", header_info.version));
                }
                
                // Check payload size consistency
                if header_info.payload_size > data.len() - 8 {
                    errors.push("Payload size field exceeds actual packet size".to_string());
                }
            }
            Err(e) => {
                errors.push(format!("Failed to parse packet header: {}", e));
            }
        }
        
        if errors.is_empty() {
            Ok(warnings)
        } else {
            Err(errors)
        }
    }
    
    /// Parse packet header information
    fn parse_packet_header(&self, data: &[u8]) -> Result<PacketHeaderInfo, String> {
        if data.len() < 8 {
            return Err("Insufficient data for header".to_string());
        }
        
        // Simple header parsing (this would be more complex in reality)
        let version = data[0];
        let message_type = data[1];
        let flags = data[2];
        let payload_size = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;
        
        Ok(PacketHeaderInfo {
            version,
            message_type,
            flags,
            payload_size,
        })
    }
    
    /// Check if packet contains a specific field
    fn packet_has_field(&self, _data: &[u8], _field: &str) -> bool {
        // Simplified field checking - in reality this would parse the packet structure
        true
    }
}

/// Packet header information
#[derive(Debug)]
struct PacketHeaderInfo {
    version: u8,
    message_type: u8,
    flags: u8,
    payload_size: usize,
}

// ----------------------------------------------------------------------------
// Network Analyzer
// ----------------------------------------------------------------------------

/// Main network analyzer that coordinates all analysis activities
pub struct NetworkAnalyzer {
    /// Unique analyzer ID
    id: String,
    /// Packet capture buffer
    captured_packets: VecDeque<CapturedPacket>,
    /// Maximum packets to keep in memory
    max_capture_buffer: usize,
    /// Protocol compliance checker
    protocol_checker: ProtocolChecker,
    /// Current metrics
    current_metrics: NetworkMetrics,
    /// Metrics history
    metrics_history: VecDeque<NetworkMetrics>,
    /// Analysis configuration
    config: AnalyzerConfig,
    /// Next capture ID
    next_capture_id: u64,
    /// Latency measurements
    latency_samples: VecDeque<u64>,
    /// Active analysis tasks
    analysis_tasks: Vec<AnalysisTask>,
}

/// Network analyzer configuration
#[derive(Debug, Clone)]
pub struct AnalyzerConfig {
    /// Enable real-time packet capture
    pub enable_capture: bool,
    /// Enable protocol compliance checking
    pub enable_compliance_checking: bool,
    /// Enable performance metrics
    pub enable_metrics: bool,
    /// Metrics calculation window
    pub metrics_window_seconds: u64,
    /// Maximum latency samples to keep
    pub max_latency_samples: usize,
    /// Enable detailed logging
    pub enable_detailed_logging: bool,
    /// Export analysis results
    pub export_results: bool,
    /// Export file path
    pub export_path: Option<String>,
}

impl Default for AnalyzerConfig {
    fn default() -> Self {
        Self {
            enable_capture: true,
            enable_compliance_checking: true,
            enable_metrics: true,
            metrics_window_seconds: 30,
            max_latency_samples: 1000,
            enable_detailed_logging: false,
            export_results: false,
            export_path: None,
        }
    }
}

/// Analysis task for background processing
#[derive(Debug)]
pub struct AnalysisTask {
    pub id: String,
    pub task_type: AnalysisTaskType,
    pub started_at: Instant,
    pub progress: f32,
}

/// Types of analysis tasks
#[derive(Debug)]
pub enum AnalysisTaskType {
    /// Real-time packet analysis
    RealTimeAnalysis,
    /// Historical data analysis
    HistoricalAnalysis,
    /// Protocol compliance audit
    ComplianceAudit,
    /// Performance benchmarking
    PerformanceBenchmark,
    /// Network behavior profiling
    BehaviorProfiling,
}

impl NetworkAnalyzer {
    /// Create a new network analyzer
    pub fn new(config: AnalyzerConfig) -> Self {
        let analyzer_id = format!("analyzer_{}", SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs());
            
        Self {
            id: analyzer_id,
            captured_packets: VecDeque::new(),
            max_capture_buffer: 10000,
            protocol_checker: ProtocolChecker::default(),
            current_metrics: NetworkMetrics::default(),
            metrics_history: VecDeque::new(),
            config,
            next_capture_id: 1,
            latency_samples: VecDeque::new(),
            analysis_tasks: Vec::new(),
        }
    }
    
    /// Start the network analyzer
    pub async fn start(&mut self) -> Result<(), String> {
        info!("Starting network analyzer: {}", self.id);
        
        if self.config.enable_capture {
            self.start_packet_capture().await?;
        }
        
        if self.config.enable_metrics {
            self.start_metrics_collection().await?;
        }
        
        info!("Network analyzer started successfully");
        Ok(())
    }
    
    /// Capture and analyze a network packet
    pub async fn capture_packet(
        &mut self, 
        source: PeerId,
        destination: Option<PeerId>,
        packet_type: MessageType,
        data: Vec<u8>,
        latency_ms: Option<u64>,
        delivered: bool,
    ) -> Result<u64, String> {
        if !self.config.enable_capture {
            return Err("Packet capture is disabled".to_string());
        }
        
        let capture_id = self.next_capture_id;
        self.next_capture_id += 1;
        
        // Analyze packet compliance
        let captured_packet = CapturedPacket {
            capture_id,
            source,
            destination,
            packet_type,
            size: data.len(),
            data: data.clone(),
            captured_at: SystemTime::now(),
            latency_ms,
            delivered,
            compliance_status: ComplianceStatus::Unknown { reason: "Analysis pending".to_string() },
            analysis_flags: Vec::new(),
        };
        
        // Perform compliance checking
        let mut analyzed_packet = captured_packet;
        if self.config.enable_compliance_checking {
            analyzed_packet.compliance_status = self.protocol_checker.check_compliance(&analyzed_packet);
        }
        
        // Add analysis flags
        analyzed_packet.analysis_flags = self.analyze_packet_flags(&analyzed_packet);
        
        // Update metrics
        self.update_metrics(&analyzed_packet);
        
        // Store packet (with buffer management)
        self.captured_packets.push_back(analyzed_packet.clone());
        if self.captured_packets.len() > self.max_capture_buffer {
            self.captured_packets.pop_front();
        }
        
        // Log if detailed logging is enabled
        if self.config.enable_detailed_logging {
            debug!("Captured packet {}: {:?}", capture_id, analyzed_packet);
        }
        
        Ok(capture_id)
    }
    
    /// Generate analysis flags for a packet
    fn analyze_packet_flags(&self, packet: &CapturedPacket) -> Vec<AnalysisFlag> {
        let mut flags = Vec::new();
        
        // Check for large payload
        if packet.size > 1024 {
            flags.push(AnalysisFlag::LargePayload);
        }
        
        // Check packet type
        match packet.packet_type {
            MessageType::Announce => {
                flags.push(AnalysisFlag::PeerDiscovery);
            }
            MessageType::NoiseHandshake => {
                flags.push(AnalysisFlag::Handshake);
                flags.push(AnalysisFlag::SecurityRelevant);
            }
            _ => {}
        }
        
        // Check compliance status for malformed packets
        if matches!(packet.compliance_status, ComplianceStatus::NonCompliant { .. }) {
            flags.push(AnalysisFlag::Malformed);
        }
        
        // Check for duplicates (simplified check)
        if self.captured_packets.iter().any(|p| 
            p.source == packet.source && 
            p.destination == packet.destination &&
            p.data == packet.data &&
            p.capture_id != packet.capture_id
        ) {
            flags.push(AnalysisFlag::Duplicate);
        }
        
        flags
    }
    
    /// Update network metrics with new packet data
    fn update_metrics(&mut self, packet: &CapturedPacket) {
        self.current_metrics.total_packets += 1;
        
        if packet.delivered {
            self.current_metrics.successful_deliveries += 1;
        } else {
            self.current_metrics.failed_deliveries += 1;
        }
        
        // Update packet type counters
        match packet.packet_type {
            MessageType::Announce => self.current_metrics.announce_packets += 1,
            MessageType::Message => self.current_metrics.message_packets += 1,
            MessageType::NoiseHandshake => self.current_metrics.handshake_packets += 1,
            _ => {}
        }
        
        // Update byte counters
        self.current_metrics.bytes_transmitted += packet.size as u64;
        if packet.delivered {
            self.current_metrics.bytes_received += packet.size as u64;
        }
        
        // Update latency metrics
        if let Some(latency) = packet.latency_ms {
            self.latency_samples.push_back(latency);
            if self.latency_samples.len() > self.config.max_latency_samples {
                self.latency_samples.pop_front();
            }
            
            // Recalculate latency statistics
            self.calculate_latency_stats();
        }
        
        // Update compliance rate
        let compliant_packets = self.captured_packets.iter()
            .filter(|p| matches!(p.compliance_status, ComplianceStatus::Compliant))
            .count();
        
        if self.captured_packets.len() > 0 {
            self.current_metrics.protocol_compliance_rate = 
                compliant_packets as f64 / self.captured_packets.len() as f64;
        }
        
        // Update delivery ratio
        if self.current_metrics.total_packets > 0 {
            self.current_metrics.delivery_ratio = 
                self.current_metrics.successful_deliveries as f64 / 
                self.current_metrics.total_packets as f64;
        }
        
        // Update packet loss rate
        self.current_metrics.packet_loss_rate = 1.0 - self.current_metrics.delivery_ratio;
    }
    
    /// Calculate latency statistics from samples
    fn calculate_latency_stats(&mut self) {
        if self.latency_samples.is_empty() {
            return;
        }
        
        let mut sorted_samples: Vec<u64> = self.latency_samples.iter().cloned().collect();
        sorted_samples.sort_unstable();
        
        self.current_metrics.min_latency_ms = Some(sorted_samples[0]);
        self.current_metrics.max_latency_ms = Some(*sorted_samples.last().unwrap());
        
        // Calculate average
        let sum: u64 = sorted_samples.iter().sum();
        self.current_metrics.avg_latency_ms = Some(sum / sorted_samples.len() as u64);
        
        // Calculate percentiles
        let len = sorted_samples.len();
        self.current_metrics.latency_p50_ms = Some(sorted_samples[len / 2]);
        self.current_metrics.latency_p95_ms = Some(sorted_samples[(len * 95) / 100]);
        self.current_metrics.latency_p99_ms = Some(sorted_samples[(len * 99) / 100]);
        
        // Calculate jitter (standard deviation)
        let avg = self.current_metrics.avg_latency_ms.unwrap() as f64;
        let variance: f64 = sorted_samples.iter()
            .map(|&x| {
                let diff = x as f64 - avg;
                diff * diff
            })
            .sum::<f64>() / sorted_samples.len() as f64;
        
        self.current_metrics.jitter_ms = variance.sqrt();
    }
    
    /// Start packet capture task
    async fn start_packet_capture(&mut self) -> Result<(), String> {
        let task = AnalysisTask {
            id: "packet_capture".to_string(),
            task_type: AnalysisTaskType::RealTimeAnalysis,
            started_at: Instant::now(),
            progress: 0.0,
        };
        
        self.analysis_tasks.push(task);
        info!("Started packet capture task");
        Ok(())
    }
    
    /// Start metrics collection task
    async fn start_metrics_collection(&mut self) -> Result<(), String> {
        let task = AnalysisTask {
            id: "metrics_collection".to_string(),
            task_type: AnalysisTaskType::PerformanceBenchmark,
            started_at: Instant::now(),
            progress: 0.0,
        };
        
        self.analysis_tasks.push(task);
        info!("Started metrics collection task");
        Ok(())
    }
    
    /// Get current network metrics
    pub fn get_current_metrics(&self) -> &NetworkMetrics {
        &self.current_metrics
    }
    
    /// Get captured packets summary
    pub fn get_capture_summary(&self) -> CaptureSummary {
        let total_captures = self.captured_packets.len();
        let compliant_packets = self.captured_packets.iter()
            .filter(|p| matches!(p.compliance_status, ComplianceStatus::Compliant))
            .count();
        let warning_packets = self.captured_packets.iter()
            .filter(|p| matches!(p.compliance_status, ComplianceStatus::Warning { .. }))
            .count();
        let non_compliant_packets = self.captured_packets.iter()
            .filter(|p| matches!(p.compliance_status, ComplianceStatus::NonCompliant { .. }))
            .count();
            
        CaptureSummary {
            total_packets: total_captures,
            compliant_packets,
            warning_packets,
            non_compliant_packets,
            analyzer_id: self.id.clone(),
            analysis_window_start: self.current_metrics.window_start,
            analysis_window_end: SystemTime::now(),
        }
    }
    
    /// Generate comprehensive analysis report
    pub fn generate_analysis_report(&self) -> AnalysisReport {
        AnalysisReport {
            analyzer_id: self.id.clone(),
            generated_at: SystemTime::now(),
            metrics: self.current_metrics.clone(),
            capture_summary: self.get_capture_summary(),
            compliance_summary: self.generate_compliance_summary(),
            performance_summary: self.generate_performance_summary(),
            recommendations: self.generate_recommendations(),
        }
    }
    
    /// Generate protocol compliance summary
    fn generate_compliance_summary(&self) -> ComplianceSummary {
        let mut violations = HashMap::new();
        let mut warnings = HashMap::new();
        
        for packet in &self.captured_packets {
            match &packet.compliance_status {
                ComplianceStatus::NonCompliant { violations: v } => {
                    for violation in v {
                        *violations.entry(violation.clone()).or_insert(0) += 1;
                    }
                }
                ComplianceStatus::Warning { issues } => {
                    for issue in issues {
                        *warnings.entry(issue.clone()).or_insert(0) += 1;
                    }
                }
                _ => {}
            }
        }
        
        ComplianceSummary {
            compliance_rate: self.current_metrics.protocol_compliance_rate,
            total_violations: violations.values().sum(),
            total_warnings: warnings.values().sum(),
            common_violations: violations,
            common_warnings: warnings,
        }
    }
    
    /// Generate performance summary
    fn generate_performance_summary(&self) -> PerformanceSummary {
        PerformanceSummary {
            average_latency_ms: self.current_metrics.avg_latency_ms.unwrap_or(0),
            packet_loss_rate: self.current_metrics.packet_loss_rate,
            throughput_bps: self.current_metrics.bytes_per_second,
            jitter_ms: self.current_metrics.jitter_ms,
            delivery_ratio: self.current_metrics.delivery_ratio,
        }
    }
    
    /// Generate analysis recommendations
    fn generate_recommendations(&self) -> Vec<String> {
        let mut recommendations = Vec::new();
        
        // Performance recommendations
        if self.current_metrics.packet_loss_rate > 0.05 {
            recommendations.push("High packet loss detected. Consider investigating network reliability.".to_string());
        }
        
        if let Some(avg_latency) = self.current_metrics.avg_latency_ms {
            if avg_latency > 500 {
                recommendations.push("High latency detected. Network performance may need optimization.".to_string());
            }
        }
        
        // Compliance recommendations
        if self.current_metrics.protocol_compliance_rate < 0.95 {
            recommendations.push("Low protocol compliance rate. Review packet structure validation.".to_string());
        }
        
        // Security recommendations
        let security_packets = self.captured_packets.iter()
            .filter(|p| p.analysis_flags.contains(&AnalysisFlag::SecurityRelevant))
            .count();
        
        if security_packets as f64 / self.captured_packets.len() as f64 > 0.3 {
            recommendations.push("High volume of security-relevant packets. Monitor for potential security events.".to_string());
        }
        
        recommendations
    }
}

// ----------------------------------------------------------------------------
// Analysis Report Structures
// ----------------------------------------------------------------------------

/// Summary of packet capture
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureSummary {
    pub total_packets: usize,
    pub compliant_packets: usize,
    pub warning_packets: usize,
    pub non_compliant_packets: usize,
    pub analyzer_id: String,
    pub analysis_window_start: SystemTime,
    pub analysis_window_end: SystemTime,
}

/// Protocol compliance summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceSummary {
    pub compliance_rate: f64,
    pub common_violations: HashMap<String, u32>,
    pub common_warnings: HashMap<String, u32>,
    pub total_violations: u32,
    pub total_warnings: u32,
}

/// Performance analysis summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSummary {
    pub average_latency_ms: u64,
    pub packet_loss_rate: f64,
    pub throughput_bps: f64,
    pub jitter_ms: f64,
    pub delivery_ratio: f64,
}

/// Comprehensive analysis report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisReport {
    pub analyzer_id: String,
    pub generated_at: SystemTime,
    pub metrics: NetworkMetrics,
    pub capture_summary: CaptureSummary,
    pub compliance_summary: ComplianceSummary,
    pub performance_summary: PerformanceSummary,
    pub recommendations: Vec<String>,
}

// ----------------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use bitchat_core::protocol::MessageType;

    #[tokio::test]
    async fn test_network_analyzer_creation() {
        let config = AnalyzerConfig::default();
        let analyzer = NetworkAnalyzer::new(config);
        
        assert!(analyzer.captured_packets.is_empty());
        assert_eq!(analyzer.next_capture_id, 1);
    }

    #[tokio::test]
    async fn test_packet_capture() {
        let config = AnalyzerConfig::default();
        let mut analyzer = NetworkAnalyzer::new(config);
        
        let source = PeerId::new([1; 8]);
        let destination = Some(PeerId::new([2; 8]));
        let packet_data = b"test packet".to_vec();
        
        let capture_id = analyzer.capture_packet(
            source,
            destination,
            MessageType::Message,
            packet_data,
            Some(50),
            true,
        ).await.unwrap();
        
        assert_eq!(capture_id, 1);
        assert_eq!(analyzer.captured_packets.len(), 1);
        assert_eq!(analyzer.current_metrics.total_packets, 1);
        assert_eq!(analyzer.current_metrics.successful_deliveries, 1);
    }

    #[tokio::test]
    async fn test_protocol_compliance_checking() {
        let checker = ProtocolChecker::default();
        
        let packet = CapturedPacket {
            capture_id: 1,
            source: PeerId::new([1; 8]),
            destination: Some(PeerId::new([2; 8])),
            packet_type: MessageType::Message,
            data: vec![1, 0, 0, 0, 0, 0, 0, 0, 42], // Valid minimal packet
            size: 9,
            captured_at: SystemTime::now(),
            latency_ms: Some(10),
            delivered: true,
            compliance_status: ComplianceStatus::Unknown { reason: "test".to_string() },
            analysis_flags: Vec::new(),
        };
        
        let status = checker.check_compliance(&packet);
        
        // Should be compliant for a properly structured packet
        match status {
            ComplianceStatus::Compliant => {
                // Expected
            }
            ComplianceStatus::Warning { .. } => {
                // Also acceptable
            }
            ComplianceStatus::NonCompliant { violations } => {
                panic!("Unexpected non-compliance: {:?}", violations);
            }
            ComplianceStatus::Unknown { .. } => {
                panic!("Analysis should complete");
            }
        }
    }
}