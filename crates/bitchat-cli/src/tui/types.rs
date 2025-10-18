//! Type definitions for the TUI

use std::time::{Duration, Instant};

use ratatui::prelude::Stylize;
use ratatui::style::{Color, Style};
use uuid::Uuid;

use crate::app::{PeerStatus, TransportStatus};
use bitchat_core::{transport::TransportType, PeerId};

// ----------------------------------------------------------------------------
// Constants
// ----------------------------------------------------------------------------

/// Maximum number of messages to keep in memory
pub const MAX_CHAT_HISTORY: usize = 1000;
/// Maximum number of log entries to keep
pub const MAX_LOG_ENTRIES: usize = 500;
/// UI refresh rate
pub const TICK_RATE: Duration = Duration::from_millis(50);

// ----------------------------------------------------------------------------
// Tab Navigation
// ----------------------------------------------------------------------------

/// Available tabs in the TUI
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Chat,
    Peers,
    Transports,
    Logs,
    Stats,
}

impl Tab {
    pub const ALL: &'static [Tab] = &[
        Tab::Chat,
        Tab::Peers,
        Tab::Transports,
        Tab::Logs,
        Tab::Stats,
    ];

    pub fn title(&self) -> &'static str {
        match self {
            Tab::Chat => "Chat",
            Tab::Peers => "Peers",
            Tab::Transports => "Transports",
            Tab::Logs => "Logs",
            Tab::Stats => "Stats",
        }
    }

    pub fn next(&self) -> Tab {
        let current_index = Self::ALL.iter().position(|&t| t == *self).unwrap();
        Self::ALL[(current_index + 1) % Self::ALL.len()]
    }

    pub fn previous(&self) -> Tab {
        let current_index = Self::ALL.iter().position(|&t| t == *self).unwrap();
        if current_index == 0 {
            Self::ALL[Self::ALL.len() - 1]
        } else {
            Self::ALL[current_index - 1]
        }
    }
}

// ----------------------------------------------------------------------------
// Chat Types
// ----------------------------------------------------------------------------

/// A chat message for display
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub id: Uuid,
    pub timestamp: Instant,
    pub sender: String,
    pub sender_id: PeerId,
    pub content: String,
    pub is_private: bool,
    pub is_own: bool,
    pub is_delivered: bool,
}

/// Input mode for the chat input
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Editing,
}

// ----------------------------------------------------------------------------
// Peer Types
// ----------------------------------------------------------------------------

/// Information about a discovered peer
#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub peer_id: PeerId,
    pub display_name: Option<String>,
    pub transport_types: Vec<TransportType>,
    pub status: PeerStatus,
    pub last_seen: Instant,
    pub message_count: usize,
}

// ----------------------------------------------------------------------------
// Transport Types
// ----------------------------------------------------------------------------

/// Information about a transport
#[derive(Debug, Clone)]
pub struct TransportInfo {
    pub transport_type: TransportType,
    pub status: TransportStatus,
    pub last_update: Instant,
    pub connection_count: usize,
    pub message_count: usize,
}

// ----------------------------------------------------------------------------
// Logging Types
// ----------------------------------------------------------------------------

/// A log entry for the logs tab
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: Instant,
    pub level: LogLevel,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
}

impl LogLevel {
    pub fn style(&self) -> Style {
        match self {
            LogLevel::Error => Style::default().fg(Color::Red).bold(),
            LogLevel::Warn => Style::default().fg(Color::Yellow),
            LogLevel::Info => Style::default().fg(Color::Green),
            LogLevel::Debug => Style::default().fg(Color::Gray),
        }
    }

    pub fn prefix(&self) -> &'static str {
        match self {
            LogLevel::Error => "ERROR",
            LogLevel::Warn => "WARN ",
            LogLevel::Info => "INFO ",
            LogLevel::Debug => "DEBUG",
        }
    }
}

// ----------------------------------------------------------------------------
// Statistics Types
// ----------------------------------------------------------------------------

/// Application statistics for display
#[derive(Debug, Clone, Default)]
pub struct AppStatistics {
    pub messages_sent: usize,
    pub messages_received: usize,
    pub peers_discovered: usize,
    pub uptime: Duration,
    pub delivery_success_rate: f64,
    pub active_transports: usize,
    pub total_peers: usize,
}
