//! TUI Application State

use std::collections::VecDeque;
use std::time::Instant;

use crossterm::event::{self, KeyCode, KeyModifiers};
use ratatui::widgets::ListState;
use ratatui::widgets::ScrollbarState;
use tracing::info;
use uuid::Uuid;

use crate::app::{AppEvent, PeerStatus, TransportStatus};
use bitchat_core::{transport::TransportType, BitchatMessage, PeerId};

use super::types::{
    AppStatistics, ChatMessage, InputMode, LogEntry, LogLevel, PeerInfo, Tab, TransportInfo,
    MAX_CHAT_HISTORY, MAX_LOG_ENTRIES,
};

// ----------------------------------------------------------------------------
// TUI Application State
// ----------------------------------------------------------------------------

pub struct TuiApp {
    /// Current active tab
    pub(crate) active_tab: Tab,
    /// Chat messages history
    pub(crate) chat_messages: VecDeque<ChatMessage>,
    /// Current input buffer
    pub(crate) input_buffer: String,
    /// Input cursor position
    pub(crate) input_cursor: usize,
    /// Input mode
    pub(crate) input_mode: InputMode,
    /// Discovered peers
    pub(crate) peers: Vec<PeerInfo>,
    /// Transport status information
    pub(crate) transports: Vec<TransportInfo>,
    /// Log entries
    pub(crate) logs: VecDeque<LogEntry>,
    /// Application statistics
    pub(crate) stats: AppStatistics,
    /// Whether the application should quit
    should_quit: bool,
    /// Selected peer for private messaging
    selected_peer: Option<PeerId>,
    /// List states for scrolling
    pub(crate) chat_list_state: ListState,
    pub(crate) peers_list_state: ListState,
    pub(crate) transports_list_state: ListState,
    pub(crate) logs_list_state: ListState,
    /// Scrollbar states
    chat_scrollbar_state: ScrollbarState,
    logs_scrollbar_state: ScrollbarState,
    /// Application start time
    start_time: Instant,
    /// Show help overlay
    pub(crate) show_help: bool,
}

impl TuiApp {
    pub fn new() -> Self {
        Self {
            active_tab: Tab::Chat,
            chat_messages: VecDeque::new(),
            input_buffer: String::new(),
            input_cursor: 0,
            input_mode: InputMode::Normal,
            peers: Vec::new(),
            transports: Vec::new(),
            logs: VecDeque::new(),
            stats: AppStatistics::default(),
            should_quit: false,
            selected_peer: None,
            chat_list_state: ListState::default(),
            peers_list_state: ListState::default(),
            transports_list_state: ListState::default(),
            logs_list_state: ListState::default(),
            chat_scrollbar_state: ScrollbarState::default(),
            logs_scrollbar_state: ScrollbarState::default(),
            start_time: Instant::now(),
            show_help: false,
        }
    }

    /// Handle application events from BitChat
    pub fn handle_app_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::MessageReceived { from, message } => {
                self.add_chat_message(from, &message, false);
                self.stats.messages_received += 1;
                self.add_log(LogLevel::Info, format!("Message received from {}", from));
            }
            AppEvent::DeliveryConfirmed {
                message_id,
                confirmed_by,
            } => {
                self.mark_message_delivered(message_id);
                self.add_log(
                    LogLevel::Debug,
                    format!("Message {} delivered to {}", message_id, confirmed_by),
                );
            }
            AppEvent::PeerDiscovered {
                peer_id,
                transport_type,
            } => {
                self.add_or_update_peer(peer_id, transport_type.clone());
                self.stats.peers_discovered += 1;
                self.add_log(
                    LogLevel::Info,
                    format!("Peer discovered: {} via {:?}", peer_id, transport_type),
                );
            }
            AppEvent::PeerStatusChanged { peer_id, status } => {
                self.update_peer_status(peer_id, status.clone());
                self.add_log(
                    LogLevel::Debug,
                    format!("Peer {} status changed to {:?}", peer_id, status),
                );
            }
            AppEvent::TransportStatusChanged {
                transport_type,
                status,
            } => {
                self.update_transport_status(transport_type.clone(), status.clone());
                self.add_log(
                    LogLevel::Info,
                    format!("Transport {:?} status: {:?}", transport_type, status),
                );
            }
            AppEvent::Error { message } => {
                self.add_log(LogLevel::Error, message);
            }
        }

        // Update general stats
        self.stats.uptime = self.start_time.elapsed();
        self.stats.total_peers = self.peers.len();
        self.stats.active_transports = self
            .transports
            .iter()
            .filter(|t| matches!(t.status, TransportStatus::Active))
            .count();
    }

    /// Add a chat message to the display
    pub fn add_chat_message(&mut self, sender_id: PeerId, message: &BitchatMessage, is_own: bool) {
        let chat_msg = ChatMessage {
            id: message.id,
            timestamp: Instant::now(),
            sender: message.sender.clone(),
            sender_id,
            content: message.content.clone(),
            is_private: message.flags.is_private,
            is_own,
            is_delivered: false,
        };

        self.chat_messages.push_back(chat_msg);

        // Trim old messages
        while self.chat_messages.len() > MAX_CHAT_HISTORY {
            self.chat_messages.pop_front();
        }

        // Auto-scroll to bottom
        self.chat_list_state
            .select(Some(self.chat_messages.len().saturating_sub(1)));
    }

    /// Mark a message as delivered
    fn mark_message_delivered(&mut self, message_id: Uuid) {
        if let Some(msg) = self.chat_messages.iter_mut().find(|m| m.id == message_id) {
            msg.is_delivered = true;
        }
    }

    /// Add or update peer information
    fn add_or_update_peer(&mut self, peer_id: PeerId, transport_type: TransportType) {
        if let Some(peer) = self.peers.iter_mut().find(|p| p.peer_id == peer_id) {
            if !peer.transport_types.contains(&transport_type) {
                peer.transport_types.push(transport_type);
            }
            peer.last_seen = Instant::now();
            peer.status = PeerStatus::Online;
        } else {
            self.peers.push(PeerInfo {
                peer_id,
                display_name: None,
                transport_types: vec![transport_type],
                status: PeerStatus::Online,
                last_seen: Instant::now(),
                message_count: 0,
            });
        }
    }

    /// Update peer status
    fn update_peer_status(&mut self, peer_id: PeerId, status: PeerStatus) {
        if let Some(peer) = self.peers.iter_mut().find(|p| p.peer_id == peer_id) {
            peer.status = status;
            peer.last_seen = Instant::now();
        }
    }

    /// Update transport status
    fn update_transport_status(&mut self, transport_type: TransportType, status: TransportStatus) {
        if let Some(transport) = self
            .transports
            .iter_mut()
            .find(|t| t.transport_type == transport_type)
        {
            transport.status = status;
            transport.last_update = Instant::now();
        } else {
            self.transports.push(TransportInfo {
                transport_type,
                status,
                last_update: Instant::now(),
                connection_count: 0,
                message_count: 0,
            });
        }
    }

    /// Add a log entry
    pub(crate) fn add_log(&mut self, level: LogLevel, message: String) {
        self.logs.push_back(LogEntry {
            timestamp: Instant::now(),
            level,
            message,
        });

        // Trim old logs
        while self.logs.len() > MAX_LOG_ENTRIES {
            self.logs.pop_front();
        }

        // Auto-scroll to bottom if we're on the logs tab
        if self.active_tab == Tab::Logs {
            self.logs_list_state
                .select(Some(self.logs.len().saturating_sub(1)));
        }
    }

    /// Handle keyboard input
    pub fn handle_key(&mut self, key: event::KeyEvent) -> Option<String> {
        // Global keybindings
        match (key.code, key.modifiers) {
            // Quit
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                self.should_quit = true;
                return None;
            }
            // Help toggle
            (KeyCode::F(1), _) => {
                self.show_help = !self.show_help;
                return None;
            }
            // Tab navigation
            (KeyCode::Tab, KeyModifiers::NONE) => {
                self.active_tab = self.active_tab.next();
                return None;
            }
            (KeyCode::BackTab, _) => {
                self.active_tab = self.active_tab.previous();
                return None;
            }
            _ => {}
        }

        // Tab-specific keybindings
        match self.active_tab {
            Tab::Chat => self.handle_chat_input(key),
            _ => None,
        }
    }

    /// Handle input for the chat tab
    fn handle_chat_input(&mut self, key: event::KeyEvent) -> Option<String> {
        match self.input_mode {
            InputMode::Normal => match key.code {
                KeyCode::Char('i') => {
                    self.input_mode = InputMode::Editing;
                }
                KeyCode::Char('q') => {
                    self.should_quit = true;
                }
                KeyCode::Up => {
                    if let Some(selected) = self.chat_list_state.selected() {
                        if selected > 0 {
                            self.chat_list_state.select(Some(selected - 1));
                        }
                    }
                }
                KeyCode::Down => {
                    if let Some(selected) = self.chat_list_state.selected() {
                        if selected < self.chat_messages.len().saturating_sub(1) {
                            self.chat_list_state.select(Some(selected + 1));
                        }
                    } else if !self.chat_messages.is_empty() {
                        self.chat_list_state.select(Some(0));
                    }
                }
                _ => {}
            },
            InputMode::Editing => match key.code {
                KeyCode::Enter => {
                    if !self.input_buffer.is_empty() {
                        let message = self.input_buffer.clone();
                        self.input_buffer.clear();
                        self.input_cursor = 0;
                        self.stats.messages_sent += 1;
                        self.input_mode = InputMode::Normal;
                        return Some(message);
                    }
                }
                KeyCode::Char(c) => {
                    self.input_buffer.insert(self.input_cursor, c);
                    self.input_cursor += 1;
                }
                KeyCode::Backspace => {
                    if self.input_cursor > 0 {
                        self.input_cursor -= 1;
                        self.input_buffer.remove(self.input_cursor);
                    }
                }
                KeyCode::Delete => {
                    if self.input_cursor < self.input_buffer.len() {
                        self.input_buffer.remove(self.input_cursor);
                    }
                }
                KeyCode::Left => {
                    if self.input_cursor > 0 {
                        self.input_cursor -= 1;
                    }
                }
                KeyCode::Right => {
                    if self.input_cursor < self.input_buffer.len() {
                        self.input_cursor += 1;
                    }
                }
                KeyCode::Home => {
                    self.input_cursor = 0;
                }
                KeyCode::End => {
                    self.input_cursor = self.input_buffer.len();
                }
                KeyCode::Esc => {
                    self.input_mode = InputMode::Normal;
                }
                _ => {}
            },
        }
        None
    }

    /// Check if should quit
    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    /// Update statistics
    pub fn update_stats(&mut self, delivery_success_rate: f64) {
        self.stats.delivery_success_rate = delivery_success_rate;
        self.stats.uptime = self.start_time.elapsed();
    }

    // Getters for private fields
    pub fn active_tab(&self) -> Tab {
        self.active_tab
    }

    pub fn stats(&self) -> &AppStatistics {
        &self.stats
    }
}
