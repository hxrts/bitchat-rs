//! Modern Terminal User Interface using ratatui
//!
//! A clean, responsive TUI for BitChat with multiple tabs and real-time updates.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, Clear, Gauge, List, ListItem, ListState, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Tabs, Wrap,
    },
    Frame, Terminal,
};
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, error, info};
use uuid::Uuid;

use bitchat_core::{BitchatMessage, PeerId, transport::TransportType};

use crate::app::{AppEvent, BitchatApp, PeerStatus, TransportStatus};
use crate::error::{CliError, Result};

/// Maximum number of messages to keep in memory
const MAX_CHAT_HISTORY: usize = 1000;
/// Maximum number of log entries to keep
const MAX_LOG_ENTRIES: usize = 500;
/// UI refresh rate
const TICK_RATE: Duration = Duration::from_millis(50);

/// Available tabs in the TUI
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Chat,
    Peers,
    Transports,
    Logs,
    Stats,
}

impl Tab {
    const ALL: &'static [Tab] = &[Tab::Chat, Tab::Peers, Tab::Transports, Tab::Logs, Tab::Stats];
    
    fn title(&self) -> &'static str {
        match self {
            Tab::Chat => "💬 Chat",
            Tab::Peers => "👥 Peers",
            Tab::Transports => "🚀 Transports",
            Tab::Logs => "📝 Logs",
            Tab::Stats => "📊 Stats",
        }
    }
    
    fn next(&self) -> Tab {
        let current_index = Self::ALL.iter().position(|&t| t == *self).unwrap();
        Self::ALL[(current_index + 1) % Self::ALL.len()]
    }
    
    fn previous(&self) -> Tab {
        let current_index = Self::ALL.iter().position(|&t| t == *self).unwrap();
        if current_index == 0 {
            Self::ALL[Self::ALL.len() - 1]
        } else {
            Self::ALL[current_index - 1]
        }
    }
}

/// A chat message for display
#[derive(Debug, Clone)]
struct ChatMessage {
    id: Uuid,
    timestamp: Instant,
    sender: String,
    sender_id: PeerId,
    content: String,
    is_private: bool,
    is_own: bool,
    is_delivered: bool,
}

/// Information about a discovered peer
#[derive(Debug, Clone)]
struct PeerInfo {
    peer_id: PeerId,
    display_name: Option<String>,
    transport_types: Vec<TransportType>,
    status: PeerStatus,
    last_seen: Instant,
    message_count: usize,
}

/// Information about a transport
#[derive(Debug, Clone)]
struct TransportInfo {
    transport_type: TransportType,
    status: TransportStatus,
    last_update: Instant,
    connection_count: usize,
    message_count: usize,
}

/// A log entry for the logs tab
#[derive(Debug, Clone)]
struct LogEntry {
    timestamp: Instant,
    level: LogLevel,
    message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
}

impl LogLevel {
    fn style(&self) -> Style {
        match self {
            LogLevel::Error => Style::default().fg(Color::Red).bold(),
            LogLevel::Warn => Style::default().fg(Color::Yellow),
            LogLevel::Info => Style::default().fg(Color::Green),
            LogLevel::Debug => Style::default().fg(Color::Gray),
        }
    }
    
    fn prefix(&self) -> &'static str {
        match self {
            LogLevel::Error => "ERROR",
            LogLevel::Warn => "WARN ",
            LogLevel::Info => "INFO ",
            LogLevel::Debug => "DEBUG",
        }
    }
}

/// Application statistics for display
#[derive(Debug, Clone, Default)]
struct AppStatistics {
    messages_sent: usize,
    messages_received: usize,
    peers_discovered: usize,
    uptime: Duration,
    delivery_success_rate: f64,
    active_transports: usize,
    total_peers: usize,
}

/// Input mode for the chat input
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputMode {
    Normal,
    Editing,
}

/// Main TUI application state
pub struct TuiApp {
    /// Current active tab
    active_tab: Tab,
    /// Chat messages history
    chat_messages: VecDeque<ChatMessage>,
    /// Current input buffer
    input_buffer: String,
    /// Input cursor position
    input_cursor: usize,
    /// Input mode
    input_mode: InputMode,
    /// Discovered peers
    peers: Vec<PeerInfo>,
    /// Transport status information
    transports: Vec<TransportInfo>,
    /// Log entries
    logs: VecDeque<LogEntry>,
    /// Application statistics
    stats: AppStatistics,
    /// Whether the application should quit
    should_quit: bool,
    /// Selected peer for private messaging
    selected_peer: Option<PeerId>,
    /// List states for scrolling
    chat_list_state: ListState,
    peers_list_state: ListState,
    transports_list_state: ListState,
    logs_list_state: ListState,
    /// Scrollbar states
    chat_scrollbar_state: ScrollbarState,
    logs_scrollbar_state: ScrollbarState,
    /// Application start time
    start_time: Instant,
    /// Show help overlay
    show_help: bool,
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
            AppEvent::DeliveryConfirmed { message_id, confirmed_by } => {
                self.mark_message_delivered(message_id);
                self.add_log(LogLevel::Debug, format!("Message {} delivered to {}", message_id, confirmed_by));
            }
            AppEvent::PeerDiscovered { peer_id, transport_type } => {
                self.add_or_update_peer(peer_id, transport_type);
                self.stats.peers_discovered += 1;
                self.add_log(LogLevel::Info, format!("Peer discovered: {} via {:?}", peer_id, transport_type));
            }
            AppEvent::PeerStatusChanged { peer_id, status } => {
                self.update_peer_status(peer_id, status.clone());
                self.add_log(LogLevel::Debug, format!("Peer {} status changed to {:?}", peer_id, status));
            }
            AppEvent::TransportStatusChanged { transport_type, status } => {
                self.update_transport_status(transport_type, status.clone());
                self.add_log(LogLevel::Info, format!("Transport {:?} status: {:?}", transport_type, status));
            }
            AppEvent::Error { message } => {
                self.add_log(LogLevel::Error, message);
            }
        }
        
        // Update general stats
        self.stats.uptime = self.start_time.elapsed();
        self.stats.total_peers = self.peers.len();
        self.stats.active_transports = self.transports.iter()
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
        self.chat_list_state.select(Some(self.chat_messages.len().saturating_sub(1)));
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
        if let Some(transport) = self.transports.iter_mut()
            .find(|t| t.transport_type == transport_type) {
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
    fn add_log(&mut self, level: LogLevel, message: String) {
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
            self.logs_list_state.select(Some(self.logs.len().saturating_sub(1)));
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
            InputMode::Normal => {
                match key.code {
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
                }
            }
            InputMode::Editing => {
                match key.code {
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
                }
            }
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
}

/// TUI Manager that handles the terminal and rendering
pub struct TuiManager {
    terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
    tui_app: TuiApp,
    app: Arc<Mutex<BitchatApp>>,
    event_receiver: Arc<Mutex<mpsc::UnboundedReceiver<AppEvent>>>,
}

impl TuiManager {
    /// Create a new TUI manager
    pub async fn new(app: BitchatApp) -> Result<Self> {
        // Initialize terminal
        enable_raw_mode().map_err(|e| CliError::UI(format!("Failed to enable raw mode: {}", e)))?;
        let mut stdout = std::io::stdout();
        stdout
            .execute(EnterAlternateScreen)
            .map_err(|e| CliError::UI(format!("Failed to enter alternate screen: {}", e)))?;
        
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)
            .map_err(|e| CliError::UI(format!("Failed to create terminal: {}", e)))?;

        let event_receiver = app.event_receiver();

        Ok(Self {
            terminal,
            tui_app: TuiApp::new(),
            app: Arc::new(Mutex::new(app)),
            event_receiver,
        })
    }

    /// Create a new TUI manager with an existing Arc<Mutex<BitchatApp>>
    pub async fn new_with_arc(app: Arc<Mutex<BitchatApp>>) -> Result<Self> {
        // Initialize terminal
        enable_raw_mode().map_err(|e| CliError::UI(format!("Failed to enable raw mode: {}", e)))?;
        let mut stdout = std::io::stdout();
        stdout
            .execute(EnterAlternateScreen)
            .map_err(|e| CliError::UI(format!("Failed to enter alternate screen: {}", e)))?;
        
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)
            .map_err(|e| CliError::UI(format!("Failed to create terminal: {}", e)))?;

        let event_receiver = {
            let app_lock = app.lock().await;
            app_lock.event_receiver()
        };

        Ok(Self {
            terminal,
            tui_app: TuiApp::new(),
            app,
            event_receiver,
        })
    }

    /// Run the TUI main loop
    pub async fn run(&mut self) -> Result<()> {
        let mut last_tick = Instant::now();

        info!("Starting TUI main loop");

        loop {
            // Handle terminal events
            let timeout = TICK_RATE.saturating_sub(last_tick.elapsed());
            if event::poll(timeout).map_err(|e| CliError::UI(format!("Event poll failed: {}", e)))? {
                if let Event::Key(key) = event::read().map_err(|e| CliError::UI(format!("Failed to read event: {}", e)))? {
                    if key.kind == KeyEventKind::Press {
                        if let Some(message) = self.tui_app.handle_key(key) {
                            // Send message through BitChat app
                            let mut app = self.app.lock().await;
                            match app.send_message(None, message.clone()).await {
                                Ok(msg_id) => {
                                    // Add to local chat for immediate feedback
                                    let own_message = BitchatMessage::new(app.config().display_name.clone(), message);
                                    self.tui_app.add_chat_message(app.peer_id(), &own_message, true);
                                    self.tui_app.add_log(LogLevel::Info, format!("Sent message: {}", msg_id));
                                }
                                Err(e) => {
                                    self.tui_app.add_log(LogLevel::Error, format!("Failed to send message: {}", e));
                                }
                            }
                        }
                    }
                }
            }

            // Handle application events
            {
                let mut receiver = self.event_receiver.lock().await;
                while let Ok(event) = receiver.try_recv() {
                    self.tui_app.handle_app_event(event);
                }
            }

            // Check if should quit
            if self.tui_app.should_quit() {
                break;
            }

            // Update statistics periodically
            if last_tick.elapsed() >= Duration::from_secs(1) {
                let app = self.app.lock().await;
                let (_, delivery_stats) = app.get_stats();
                let success_rate = if delivery_stats.total > 0 {
                    delivery_stats.confirmed as f64 / delivery_stats.total as f64 * 100.0
                } else {
                    0.0
                };
                self.tui_app.update_stats(success_rate);
            }

            // Render UI
            if last_tick.elapsed() >= TICK_RATE {
                self.terminal
                    .draw(|f| self.render_ui(f))
                    .map_err(|e| CliError::UI(format!("Failed to draw terminal: {}", e)))?;
                last_tick = Instant::now();
            }

            // Check if app is still running
            {
                let app = self.app.lock().await;
                if !app.is_running().await {
                    break;
                }
            }
        }

        Ok(())
    }

    /// Render the complete UI
    fn render_ui(&mut self, frame: &mut Frame) {
        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header with tabs
                Constraint::Min(0),    // Main content
                Constraint::Length(if self.tui_app.active_tab == Tab::Chat { 3 } else { 1 }), // Input/status
            ])
            .split(frame.size());

        // Render header with tabs
        self.render_header(frame, main_layout[0]);

        // Render main content based on active tab
        match self.tui_app.active_tab {
            Tab::Chat => self.render_chat_tab(frame, main_layout[1], main_layout[2]),
            Tab::Peers => self.render_peers_tab(frame, main_layout[1]),
            Tab::Transports => self.render_transports_tab(frame, main_layout[1]),
            Tab::Logs => self.render_logs_tab(frame, main_layout[1]),
            Tab::Stats => self.render_stats_tab(frame, main_layout[1]),
        }

        // Render status bar for non-chat tabs
        if self.tui_app.active_tab != Tab::Chat {
            self.render_status_bar(frame, main_layout[2]);
        }

        // Render help overlay if shown
        if self.tui_app.show_help {
            self.render_help_overlay(frame);
        }
    }

    /// Render the header with tabs
    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let titles: Vec<Line> = Tab::ALL
            .iter()
            .map(|tab| Line::from(tab.title()))
            .collect();

        let tabs = Tabs::new(titles)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("BitChat TUI")
                    .title_style(Style::default().fg(Color::Cyan).bold()),
            )
            .select(Tab::ALL.iter().position(|&t| t == self.tui_app.active_tab).unwrap())
            .style(Style::default().fg(Color::White))
            .highlight_style(Style::default().bg(Color::Blue).fg(Color::White).bold());

        frame.render_widget(tabs, area);
    }

    /// Render the chat tab
    fn render_chat_tab(&mut self, frame: &mut Frame, main_area: Rect, input_area: Rect) {
        // Split main area for messages and peer list
        let chat_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(75), Constraint::Percentage(25)])
            .split(main_area);

        // Messages area
        self.render_messages(frame, chat_layout[0]);

        // Peer list sidebar
        self.render_peer_sidebar(frame, chat_layout[1]);

        // Input area
        self.render_input(frame, input_area);
    }

    /// Render chat messages
    fn render_messages(&mut self, frame: &mut Frame, area: Rect) {
        let messages: Vec<ListItem> = self
            .tui_app
            .chat_messages
            .iter()
            .map(|msg| {
                let elapsed = msg.timestamp.elapsed();
                let timestamp = format!(
                    "{:02}:{:02}:{:02}",
                    elapsed.as_secs() / 3600,
                    (elapsed.as_secs() % 3600) / 60,
                    elapsed.as_secs() % 60
                );

                let delivery_indicator = if msg.is_own {
                    if msg.is_delivered {
                        "✓"
                    } else {
                        "○"
                    }
                } else {
                    ""
                };

                let sender_style = if msg.is_own {
                    Style::default().fg(Color::Green)
                } else if msg.is_private {
                    Style::default().fg(Color::Magenta)
                } else {
                    Style::default().fg(Color::Cyan)
                };

                let content = format!("[{}] {}: {} {}", timestamp, msg.sender, msg.content, delivery_indicator);
                ListItem::new(Line::from(Span::styled(content, sender_style)))
            })
            .collect();

        let messages_block = Block::default()
            .borders(Borders::ALL)
            .title("Messages")
            .title_style(Style::default().fg(Color::Yellow));

        let messages_list = List::new(messages)
            .block(messages_block)
            .highlight_style(Style::default().bg(Color::DarkGray));

        frame.render_stateful_widget(messages_list, area, &mut self.tui_app.chat_list_state);

        // Render scrollbar
        let scrollbar = Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));

        let mut scrollbar_state = ScrollbarState::default()
            .content_length(self.tui_app.chat_messages.len())
            .position(self.tui_app.chat_list_state.selected().unwrap_or(0));

        frame.render_stateful_widget(
            scrollbar,
            area.inner(Margin { vertical: 1, horizontal: 0 }),
            &mut scrollbar_state,
        );
    }

    /// Render peer sidebar
    fn render_peer_sidebar(&self, frame: &mut Frame, area: Rect) {
        let peers: Vec<ListItem> = self
            .tui_app
            .peers
            .iter()
            .map(|peer| {
                let status_indicator = match peer.status {
                    PeerStatus::Online => "🟢",
                    PeerStatus::Offline => "🔴",
                    PeerStatus::Connecting => "🟡",
                };

                let transports = peer
                    .transport_types
                    .iter()
                    .map(|t| match t {
                        TransportType::Ble => "📶",
                        TransportType::Nostr => "🌐",
                        _ => "❓",
                    })
                    .collect::<String>();

                let peer_id_short = &peer.peer_id.to_string()[..8];
                let display_name = peer.display_name.as_deref().unwrap_or("Unknown");
                
                let content = format!("{} {} {} ({})", status_indicator, transports, display_name, peer_id_short);
                ListItem::new(Line::from(content))
            })
            .collect();

        let peers_list = List::new(peers).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Online Peers")
                .title_style(Style::default().fg(Color::Green)),
        );

        frame.render_widget(peers_list, area);
    }

    /// Render input area
    fn render_input(&self, frame: &mut Frame, area: Rect) {
        let input_style = match self.tui_app.input_mode {
            InputMode::Normal => Style::default().fg(Color::Gray),
            InputMode::Editing => Style::default().fg(Color::White),
        };

        let mode_indicator = match self.tui_app.input_mode {
            InputMode::Normal => "Normal (press 'i' to edit)",
            InputMode::Editing => "Editing (ESC to exit, Enter to send)",
        };

        let input_text = format!("> {}", self.tui_app.input_buffer);
        let input = Paragraph::new(input_text)
            .style(input_style)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(mode_indicator)
                    .title_style(Style::default().fg(Color::Yellow)),
            );

        frame.render_widget(input, area);

        // Render cursor
        if self.tui_app.input_mode == InputMode::Editing {
            frame.set_cursor(
                area.x + self.tui_app.input_cursor as u16 + 3,
                area.y + 1,
            );
        }
    }

    /// Render peers tab
    fn render_peers_tab(&mut self, frame: &mut Frame, area: Rect) {
        let peers: Vec<ListItem> = self
            .tui_app
            .peers
            .iter()
            .map(|peer| {
                let status_color = match peer.status {
                    PeerStatus::Online => Color::Green,
                    PeerStatus::Offline => Color::Red,
                    PeerStatus::Connecting => Color::Yellow,
                };

                let transports = peer
                    .transport_types
                    .iter()
                    .map(|t| format!("{:?}", t))
                    .collect::<Vec<_>>()
                    .join(", ");

                let elapsed = peer.last_seen.elapsed();
                let last_seen = if elapsed.as_secs() < 60 {
                    "just now".to_string()
                } else if elapsed.as_secs() < 3600 {
                    format!("{}m ago", elapsed.as_secs() / 60)
                } else {
                    format!("{}h ago", elapsed.as_secs() / 3600)
                };

                let content = format!(
                    "{} | {} | {} | Last seen: {}",
                    peer.peer_id,
                    peer.display_name.as_deref().unwrap_or("Unknown"),
                    transports,
                    last_seen
                );

                ListItem::new(Line::from(Span::styled(content, Style::default().fg(status_color))))
            })
            .collect();

        let peers_list = List::new(peers)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Discovered Peers")
                    .title_style(Style::default().fg(Color::Green)),
            )
            .highlight_style(Style::default().bg(Color::DarkGray));

        frame.render_stateful_widget(peers_list, area, &mut self.tui_app.peers_list_state);
    }

    /// Render transports tab
    fn render_transports_tab(&mut self, frame: &mut Frame, area: Rect) {
        let transports: Vec<ListItem> = self
            .tui_app
            .transports
            .iter()
            .map(|transport| {
                let (status_text, status_color) = match &transport.status {
                    TransportStatus::Starting => ("Starting...", Color::Yellow),
                    TransportStatus::Active => ("Active", Color::Green),
                    TransportStatus::Failed(err) => ("Failed", Color::Red),
                    TransportStatus::Stopped => ("Stopped", Color::Gray),
                };

                let elapsed = transport.last_update.elapsed();
                let last_update = if elapsed.as_secs() < 60 {
                    "just now".to_string()
                } else {
                    format!("{}m ago", elapsed.as_secs() / 60)
                };

                let content = format!(
                    "{:?} | {} | Connections: {} | Messages: {} | Updated: {}",
                    transport.transport_type,
                    status_text,
                    transport.connection_count,
                    transport.message_count,
                    last_update
                );

                ListItem::new(Line::from(Span::styled(content, Style::default().fg(status_color))))
            })
            .collect();

        let transports_list = List::new(transports)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Transport Status")
                    .title_style(Style::default().fg(Color::Cyan)),
            )
            .highlight_style(Style::default().bg(Color::DarkGray));

        frame.render_stateful_widget(transports_list, area, &mut self.tui_app.transports_list_state);
    }

    /// Render logs tab
    fn render_logs_tab(&mut self, frame: &mut Frame, area: Rect) {
        let logs: Vec<ListItem> = self
            .tui_app
            .logs
            .iter()
            .map(|log| {
                let elapsed = log.timestamp.elapsed();
                let timestamp = format!(
                    "{:02}:{:02}:{:02}",
                    elapsed.as_secs() / 3600,
                    (elapsed.as_secs() % 3600) / 60,
                    elapsed.as_secs() % 60
                );

                let content = format!("[{}] {} {}", timestamp, log.level.prefix(), log.message);
                ListItem::new(Line::from(Span::styled(content, log.level.style())))
            })
            .collect();

        let logs_list = List::new(logs)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Application Logs")
                    .title_style(Style::default().fg(Color::Magenta)),
            )
            .highlight_style(Style::default().bg(Color::DarkGray));

        frame.render_stateful_widget(logs_list, area, &mut self.tui_app.logs_list_state);

        // Render scrollbar
        let scrollbar = Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));

        let mut scrollbar_state = ScrollbarState::default()
            .content_length(self.tui_app.logs.len())
            .position(self.tui_app.logs_list_state.selected().unwrap_or(0));

        frame.render_stateful_widget(
            scrollbar,
            area.inner(Margin { vertical: 1, horizontal: 0 }),
            &mut scrollbar_state,
        );
    }

    /// Render stats tab
    fn render_stats_tab(&self, frame: &mut Frame, area: Rect) {
        let stats_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        // Basic stats
        let basic_stats = format!(
            "Messages Sent: {}\n\
             Messages Received: {}\n\
             Peers Discovered: {}\n\
             Total Peers: {}\n\
             Active Transports: {}\n\
             Uptime: {}h {}m {}s",
            self.tui_app.stats.messages_sent,
            self.tui_app.stats.messages_received,
            self.tui_app.stats.peers_discovered,
            self.tui_app.stats.total_peers,
            self.tui_app.stats.active_transports,
            self.tui_app.stats.uptime.as_secs() / 3600,
            (self.tui_app.stats.uptime.as_secs() % 3600) / 60,
            self.tui_app.stats.uptime.as_secs() % 60,
        );

        let basic_stats_widget = Paragraph::new(basic_stats)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Basic Statistics")
                    .title_style(Style::default().fg(Color::Green)),
            )
            .wrap(Wrap { trim: true });

        frame.render_widget(basic_stats_widget, stats_layout[0]);

        // Performance stats
        let performance_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .split(stats_layout[1]);

        // Delivery success rate gauge
        let delivery_rate = self.tui_app.stats.delivery_success_rate;
        let gauge = Gauge::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Message Delivery Success Rate"),
            )
            .gauge_style(if delivery_rate > 80.0 {
                Style::default().fg(Color::Green)
            } else if delivery_rate > 50.0 {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::Red)
            })
            .ratio(delivery_rate / 100.0)
            .label(format!("{:.1}%", delivery_rate));

        frame.render_widget(gauge, performance_layout[0]);

        // Additional performance info
        let perf_info = "Performance Metrics:\n\n\
                         • Message throughput: Real-time\n\
                         • Peer discovery: Continuous\n\
                         • Transport health: Monitored\n\
                         • Memory usage: Bounded";

        let perf_widget = Paragraph::new(perf_info)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Performance")
                    .title_style(Style::default().fg(Color::Yellow)),
            )
            .wrap(Wrap { trim: true });

        frame.render_widget(perf_widget, performance_layout[1]);
    }

    /// Render status bar
    fn render_status_bar(&self, frame: &mut Frame, area: Rect) {
        let status_text = match self.tui_app.active_tab {
            Tab::Chat => "i: Insert mode | q: Quit | Tab: Switch tabs | F1: Help",
            _ => "q: Quit | Tab: Switch tabs | F1: Help | ↑/↓: Navigate",
        };

        let status = Paragraph::new(status_text)
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center);

        frame.render_widget(status, area);
    }

    /// Render help overlay
    fn render_help_overlay(&self, frame: &mut Frame) {
        let area = centered_rect(80, 70, frame.size());

        let help_text = "BitChat TUI Help\n\n\
            Global Shortcuts:\n\
            • F1: Toggle this help\n\
            • Tab / Shift+Tab: Switch between tabs\n\
            • Ctrl+C: Quit application\n\n\
            Chat Tab:\n\
            • i: Enter insert mode to type messages\n\
            • ESC: Exit insert mode\n\
            • Enter: Send message (in insert mode)\n\
            • ↑/↓: Scroll through message history\n\
            • q: Quit (in normal mode)\n\n\
            Other Tabs:\n\
            • ↑/↓: Navigate through lists\n\
            • q: Quit\n\n\
            Features:\n\
            • Real-time messaging via BLE and Nostr\n\
            • Peer discovery and status monitoring\n\
            • Transport health monitoring\n\
            • Application logs and statistics\n\
            • Message delivery confirmation";

        let help_paragraph = Paragraph::new(help_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Help")
                    .title_style(Style::default().fg(Color::Cyan).bold()),
            )
            .wrap(Wrap { trim: true })
            .style(Style::default().bg(Color::Black));

        frame.render_widget(Clear, area);
        frame.render_widget(help_paragraph, area);
    }
}

impl Drop for TuiManager {
    fn drop(&mut self) {
        // Restore terminal
        let _ = disable_raw_mode();
        let _ = self
            .terminal
            .backend_mut()
            .execute(LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

/// Helper function to create a centered rectangle
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}