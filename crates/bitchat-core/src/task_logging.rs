//! Task Communication Logging Infrastructure
//!
//! Provides structured logging for CSP channel communication debugging
//! Compatible with both native and WASM environments

use crate::channel::{Command, Event, Effect, AppEvent, ChannelTransportType};
use serde::{Serialize, Deserialize};

cfg_if::cfg_if! {
    if #[cfg(feature = "std")] {
        use std::fmt;
    } else {
        use core::fmt;
        use alloc::string::{String, ToString};
    }
}

// ----------------------------------------------------------------------------
// Log Event Types
// ----------------------------------------------------------------------------

/// Log levels for task communication
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogLevel::Trace => write!(f, "TRACE"),
            LogLevel::Debug => write!(f, "DEBUG"),
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Warn => write!(f, "WARN"),
            LogLevel::Error => write!(f, "ERROR"),
        }
    }
}

/// Task identifiers for communication logging
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TaskId {
    CoreLogic,
    Transport(ChannelTransportType),
    UI,
    TestOrchestrator,
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskId::CoreLogic => write!(f, "CoreLogic"),
            TaskId::Transport(transport) => write!(f, "Transport({})", transport),
            TaskId::UI => write!(f, "UI"),
            TaskId::TestOrchestrator => write!(f, "TestOrchestrator"),
        }
    }
}

/// Communication direction for channel messages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    Send,
    Receive,
    Drop,
}

impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Direction::Send => write!(f, "→"),
            Direction::Receive => write!(f, "←"),
            Direction::Drop => write!(f, "✗"),
        }
    }
}

/// Communication event for logging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommEvent {
    pub timestamp: u64,
    pub level: LogLevel,
    pub from_task: TaskId,
    pub to_task: TaskId,
    pub direction: Direction,
    pub message_type: MessageType,
    pub message_summary: String,
    pub channel_utilization: Option<f32>,
}

/// Message type classification for logging
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageType {
    Command(String),
    Event(String),
    Effect(String),
    AppEvent(String),
}

impl fmt::Display for MessageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MessageType::Command(cmd) => write!(f, "Command::{}", cmd),
            MessageType::Event(event) => write!(f, "Event::{}", event),
            MessageType::Effect(effect) => write!(f, "Effect::{}", effect),
            MessageType::AppEvent(app_event) => write!(f, "AppEvent::{}", app_event),
        }
    }
}

// ----------------------------------------------------------------------------
// Message Type Extraction
// ----------------------------------------------------------------------------

impl From<&Command> for MessageType {
    fn from(command: &Command) -> Self {
        let variant = match command {
            Command::SendMessage { .. } => "SendMessage",
            Command::ConnectToPeer { .. } => "ConnectToPeer",
            Command::StartDiscovery => "StartDiscovery",
            Command::StopDiscovery => "StopDiscovery",
            Command::DisconnectFromPeer { .. } => "DisconnectFromPeer",
            Command::Shutdown => "Shutdown",
            Command::PauseTransport { .. } => "PauseTransport",
            Command::ResumeTransport { .. } => "ResumeTransport",
            Command::GetSystemStatus => "GetSystemStatus",
        };
        MessageType::Command(variant.to_string())
    }
}

impl From<&Event> for MessageType {
    fn from(event: &Event) -> Self {
        let variant = match event {
            Event::PeerDiscovered { .. } => "PeerDiscovered",
            Event::MessageReceived { .. } => "MessageReceived",
            Event::ConnectionEstablished { .. } => "ConnectionEstablished",
            Event::ConnectionLost { .. } => "ConnectionLost",
            Event::TransportError { .. } => "TransportError",
        };
        MessageType::Event(variant.to_string())
    }
}

impl From<&Effect> for MessageType {
    fn from(effect: &Effect) -> Self {
        let variant = match effect {
            Effect::SendPacket { .. } => "SendPacket",
            Effect::InitiateConnection { .. } => "InitiateConnection",
            Effect::StartListening { .. } => "StartListening",
            Effect::StopListening { .. } => "StopListening",
            Effect::WriteToStorage { .. } => "WriteToStorage",
            Effect::ScheduleRetry { .. } => "ScheduleRetry",
            Effect::StartTransportDiscovery { .. } => "StartTransportDiscovery",
            Effect::StopTransportDiscovery { .. } => "StopTransportDiscovery",
            Effect::PauseTransport { .. } => "PauseTransport",
            Effect::ResumeTransport { .. } => "ResumeTransport",
        };
        MessageType::Effect(variant.to_string())
    }
}

impl From<&AppEvent> for MessageType {
    fn from(app_event: &AppEvent) -> Self {
        let variant = match app_event {
            AppEvent::MessageReceived { .. } => "MessageReceived",
            AppEvent::MessageSent { .. } => "MessageSent",
            AppEvent::PeerStatusChanged { .. } => "PeerStatusChanged",
            AppEvent::DiscoveryStateChanged { .. } => "DiscoveryStateChanged",
            AppEvent::ConversationUpdated { .. } => "ConversationUpdated",
            AppEvent::SystemBusy { .. } => "SystemBusy",
            AppEvent::SystemError { .. } => "SystemError",
            AppEvent::SystemStatusReport { .. } => "SystemStatusReport",
        };
        MessageType::AppEvent(variant.to_string())
    }
}

// Add Into implementations for owned types
impl From<Command> for MessageType {
    fn from(val: Command) -> MessageType {
        (&val).into()
    }
}

impl From<Event> for MessageType {
    fn from(val: Event) -> MessageType {
        (&val).into()
    }
}

impl From<Effect> for MessageType {
    fn from(val: Effect) -> MessageType {
        (&val).into()
    }
}

impl From<AppEvent> for MessageType {
    fn from(val: AppEvent) -> MessageType {
        (&val).into()
    }
}

// ----------------------------------------------------------------------------
// Message Summary Generation
// ----------------------------------------------------------------------------

pub trait MessageSummary {
    fn summary(&self) -> String;
}

impl MessageSummary for Command {
    fn summary(&self) -> String {
        match self {
            Command::SendMessage { recipient, content } => {
                format!("to:{} content:{:.20}...", recipient, content)
            }
            Command::ConnectToPeer { peer_id } => format!("peer:{}", peer_id),
            Command::StartDiscovery => "starting discovery".to_string(),
            Command::StopDiscovery => "stopping discovery".to_string(),
            Command::DisconnectFromPeer { peer_id } => format!("peer:{}", peer_id),
            Command::Shutdown => "shutting down".to_string(),
            Command::PauseTransport { transport } => format!("pausing transport:{}", transport),
            Command::ResumeTransport { transport } => format!("resuming transport:{}", transport),
            Command::GetSystemStatus => "requesting system status".to_string(),
        }
    }
}

impl MessageSummary for Event {
    fn summary(&self) -> String {
        match self {
            Event::PeerDiscovered { peer_id, transport, signal_strength } => {
                format!("peer:{} via:{} signal:{:?}", peer_id, transport, signal_strength)
            }
            Event::MessageReceived { from, content, transport, .. } => {
                format!("from:{} via:{} content:{:.20}...", from, transport, content)
            }
            Event::ConnectionEstablished { peer_id, transport } => {
                format!("peer:{} via:{}", peer_id, transport)
            }
            Event::ConnectionLost { peer_id, transport, reason } => {
                format!("peer:{} via:{} reason:{}", peer_id, transport, reason)
            }
            Event::TransportError { transport, error } => {
                format!("transport:{} error:{}", transport, error)
            }
        }
    }
}

impl MessageSummary for Effect {
    fn summary(&self) -> String {
        match self {
            Effect::SendPacket { peer_id, data, transport } => {
                format!("to:{} via:{} bytes:{}", peer_id, transport, data.len())
            }
            Effect::InitiateConnection { peer_id, transport } => {
                format!("peer:{} via:{}", peer_id, transport)
            }
            Effect::StartListening { transport } => format!("transport:{}", transport),
            Effect::StopListening { transport } => format!("transport:{}", transport),
            Effect::WriteToStorage { key, data } => {
                format!("key:{} bytes:{}", key, data.len())
            }
            Effect::ScheduleRetry { delay, command } => {
                format!("delay:{:?} cmd:{}", delay, MessageType::from(command))
            }
            Effect::StartTransportDiscovery { transport } => format!("transport:{}", transport),
            Effect::StopTransportDiscovery { transport } => format!("transport:{}", transport),
            Effect::PauseTransport { transport } => format!("pausing transport:{}", transport),
            Effect::ResumeTransport { transport } => format!("resuming transport:{}", transport),
        }
    }
}

impl MessageSummary for AppEvent {
    fn summary(&self) -> String {
        match self {
            AppEvent::MessageReceived { from, content, .. } => {
                format!("from:{} content:{:.20}...", from, content)
            }
            AppEvent::MessageSent { to, content, .. } => {
                format!("to:{} content:{:.20}...", to, content)
            }
            AppEvent::PeerStatusChanged { peer_id, status, transport } => {
                format!("peer:{} status:{} via:{:?}", peer_id, status, transport)
            }
            AppEvent::DiscoveryStateChanged { active, transport } => {
                format!("active:{} transport:{:?}", active, transport)
            }
            AppEvent::ConversationUpdated { peer_id, message_count, .. } => {
                format!("peer:{} messages:{}", peer_id, message_count)
            }
            AppEvent::SystemBusy { reason } => format!("reason:{}", reason),
            AppEvent::SystemError { error } => format!("error:{}", error),
            AppEvent::SystemStatusReport { peer_count, active_connections, message_count, uptime_seconds, .. } => {
                format!("peers:{} connected:{} messages:{} uptime:{}s", peer_count, active_connections, message_count, uptime_seconds)
            },
        }
    }
}

// ----------------------------------------------------------------------------
// Logger Implementation
// ----------------------------------------------------------------------------

/// Task communication logger
pub trait TaskLogger {
    fn log_send<T>(
        &self,
        from: TaskId,
        to: TaskId,
        message: &T,
        channel_utilization: Option<f32>,
    ) where
        for<'a> &'a T: Into<MessageType>,
        T: MessageSummary;

    fn log_receive<T>(
        &self,
        from: TaskId,
        to: TaskId,
        message: &T,
        channel_utilization: Option<f32>,
    ) where
        for<'a> &'a T: Into<MessageType>,
        T: MessageSummary;

    fn log_drop<T>(
        &self,
        from: TaskId,
        to: TaskId,
        message: &T,
        reason: &str,
    ) where
        for<'a> &'a T: Into<MessageType>,
        T: MessageSummary;

    fn log_task_event(&self, task: TaskId, level: LogLevel, message: &str);
}

/// Console logger implementation
#[derive(Debug, Clone)]
pub struct ConsoleLogger {
    min_level: LogLevel,
    include_timestamps: bool,
}

impl ConsoleLogger {
    pub fn new(min_level: LogLevel) -> Self {
        Self {
            min_level,
            include_timestamps: true,
        }
    }

    pub fn with_timestamps(mut self, include: bool) -> Self {
        self.include_timestamps = include;
        self
    }

    fn should_log(&self, level: LogLevel) -> bool {
        use LogLevel::*;
        let level_order = [Trace, Debug, Info, Warn, Error];
        let min_index = level_order.iter().position(|&l| l == self.min_level).unwrap_or(0);
        let current_index = level_order.iter().position(|&l| l == level).unwrap_or(0);
        current_index >= min_index
    }

    fn format_timestamp(&self) -> String {
        if self.include_timestamps {
            cfg_if::cfg_if! {
                if #[cfg(feature = "std")] {
                    use std::time::{SystemTime, UNIX_EPOCH};
                    let timestamp = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis();
                    format!("[{}] ", timestamp)
                } else {
                    // no_std fallback - would need alternative time source
                    "[0] ".to_string()
                }
            }
        } else {
            String::new()
        }
    }
}

impl TaskLogger for ConsoleLogger {
    fn log_send<T>(
        &self,
        from: TaskId,
        to: TaskId,
        message: &T,
        channel_utilization: Option<f32>,
    ) where
        for<'a> &'a T: Into<MessageType>,
        T: MessageSummary
    {
        if !self.should_log(LogLevel::Debug) {
            return;
        }

        let utilization = channel_utilization
            .map(|u| format!(" util:{:.1}%", u * 100.0))
            .unwrap_or_default();

        println!(
            "{}[DEBUG] {} {} {} {} {}{}",
            self.format_timestamp(),
            from,
            Direction::Send,
            to,
            (message as &T).into(),
            message.summary(),
            utilization
        );
    }

    fn log_receive<T>(
        &self,
        from: TaskId,
        to: TaskId,
        message: &T,
        channel_utilization: Option<f32>,
    ) where
        for<'a> &'a T: Into<MessageType>,
        T: MessageSummary
    {
        if !self.should_log(LogLevel::Debug) {
            return;
        }

        let utilization = channel_utilization
            .map(|u| format!(" util:{:.1}%", u * 100.0))
            .unwrap_or_default();

        println!(
            "{}[DEBUG] {} {} {} {} {}{}",
            self.format_timestamp(),
            to,
            Direction::Receive,
            from,
            (message as &T).into(),
            message.summary(),
            utilization
        );
    }

    fn log_drop<T>(
        &self,
        from: TaskId,
        to: TaskId,
        message: &T,
        reason: &str,
    ) where
        for<'a> &'a T: Into<MessageType>,
        T: MessageSummary
    {
        if !self.should_log(LogLevel::Warn) {
            return;
        }

        println!(
            "{}[WARN] {} {} {} {} {} reason:{}",
            self.format_timestamp(),
            from,
            Direction::Drop,
            to,
            (message as &T).into(),
            message.summary(),
            reason
        );
    }

    fn log_task_event(&self, task: TaskId, level: LogLevel, message: &str) {
        if !self.should_log(level) {
            return;
        }

        println!(
            "{}[{}] {} {}",
            self.format_timestamp(),
            level,
            task,
            message
        );
    }
}

/// No-op logger for production or when logging is disabled
#[derive(Debug, Clone)]
pub struct NoOpLogger;

impl TaskLogger for NoOpLogger {
    fn log_send<T>(
        &self,
        _from: TaskId,
        _to: TaskId,
        _message: &T,
        _channel_utilization: Option<f32>,
    ) {
    }

    fn log_receive<T>(
        &self,
        _from: TaskId,
        _to: TaskId,
        _message: &T,
        _channel_utilization: Option<f32>,
    ) {
    }

    fn log_drop<T>(
        &self,
        _from: TaskId,
        _to: TaskId,
        _message: &T,
        _reason: &str,
    ) {
    }

    fn log_task_event(&self, _task: TaskId, _level: LogLevel, _message: &str) {
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PeerId;

    fn create_test_peer_id(id: u8) -> PeerId {
        PeerId::new([id, 0, 0, 0, 0, 0, 0, 0])
    }

    #[test]
    fn test_message_type_conversion() {
        let command = Command::StartDiscovery;
        let msg_type = MessageType::from(&command);
        assert_eq!(msg_type, MessageType::Command("StartDiscovery".to_string()));
    }

    #[test]
    fn test_message_summary() {
        let peer_id = create_test_peer_id(1);
        let command = Command::ConnectToPeer { peer_id };
        let summary = command.summary();
        assert!(summary.contains(&peer_id.to_string()));
    }

    #[test]
    fn test_task_id_display() {
        assert_eq!(format!("{}", TaskId::CoreLogic), "CoreLogic");
        assert_eq!(format!("{}", TaskId::Transport(ChannelTransportType::Ble)), "Transport(BLE)");
        assert_eq!(format!("{}", TaskId::UI), "UI");
    }

    #[test]
    fn test_console_logger_level_filtering() {
        let logger = ConsoleLogger::new(LogLevel::Warn);
        assert!(!logger.should_log(LogLevel::Debug));
        assert!(!logger.should_log(LogLevel::Info));
        assert!(logger.should_log(LogLevel::Warn));
        assert!(logger.should_log(LogLevel::Error));
    }
}
