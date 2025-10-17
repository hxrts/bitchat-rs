//! Terminal User Interface using ratatui

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, Clear, Gauge, List, ListItem, Paragraph, Tabs, Wrap,
    },
    Frame, Terminal,
};
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, error, info};

use bitchat_core::{BitchatMessage, PeerId};
use bitchat_core::transport::TransportType;

use crate::app::{AppEvent, BitchatApp, PeerStatus, TransportStatus};
use crate::error::{CliError, Result};

/// Maximum number of messages to keep in chat history
const MAX_CHAT_HISTORY: usize = 1000;

/// TUI application state
pub struct TuiApp {
    /// Current active tab
    active_tab: usize,
    /// Chat messages
    chat_messages: VecDeque<ChatMessage>,
    /// Current input buffer
    input_buffer: String,
    /// Input cursor position
    input_cursor: usize,
    /// Discovered peers
    peers: Vec<PeerInfo>,
    /// Transport status
    transport_status: Vec<TransportInfo>,
    /// Error messages
    errors: VecDeque<String>,
    /// Whether to show help
    show_help: bool,
    /// Application statistics
    stats: AppStatistics,
    /// Whether the application should quit
    should_quit: bool,
    /// Selected peer for private messaging
    selected_peer: Option<PeerId>,
    /// Scroll offset for chat messages
    chat_scroll: usize,
}

#[derive(Debug, Clone)]
struct ChatMessage {
    timestamp: Instant,
    sender: String,
    sender_id: PeerId,
    content: String,
    is_private: bool,
    is_own: bool,
}

#[derive(Debug, Clone)]
struct PeerInfo {
    peer_id: PeerId,
    display_name: Option<String>,
    transport_types: Vec<TransportType>,
    status: PeerStatus,
    last_seen: Instant,
}

#[derive(Debug, Clone)]
struct TransportInfo {
    transport_type: TransportType,
    status: TransportStatus,
    last_update: Instant,
}

#[derive(Debug, Clone, Default)]
struct AppStatistics {
    messages_sent: usize,
    messages_received: usize,
    peers_discovered: usize,
    uptime: Duration,
}

impl TuiApp {
    pub fn new() -> Self {
        Self {
            active_tab: 0,
            chat_messages: VecDeque::new(),
            input_buffer: String::new(),
            input_cursor: 0,
            peers: Vec::new(),
            transport_status: Vec::new(),
            errors: VecDeque::new(),
            show_help: false,
            stats: AppStatistics::default(),
            should_quit: false,
            selected_peer: None,
            chat_scroll: 0,
        }
    }

    /// Process an application event
    pub fn handle_app_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::MessageReceived { from, message } => {
                self.add_chat_message(from, &message, false);
                self.stats.messages_received += 1;
            }
            AppEvent::PeerDiscovered { peer_id, transport_type } => {
                self.add_or_update_peer(peer_id, transport_type);
                self.stats.peers_discovered += 1;
            }
            AppEvent::PeerStatusChanged { peer_id, status } => {
                self.update_peer_status(peer_id, status);
            }
            AppEvent::TransportStatusChanged { transport_type, status } => {
                self.update_transport_status(transport_type, status);
            }
            AppEvent::Error { message } => {
                self.add_error(message);
            }
            _ => {} // Handle other events as needed
        }
    }

    /// Add a chat message
    pub fn add_chat_message(&mut self, sender_id: PeerId, message: &BitchatMessage, is_own: bool) {
        let chat_msg = ChatMessage {
            timestamp: Instant::now(),
            sender: message.sender.clone(),
            sender_id,
            content: message.content.clone(),
            is_private: message.flags.is_private,
            is_own,
        };

        self.chat_messages.push_back(chat_msg);

        // Keep only the last MAX_CHAT_HISTORY messages
        while self.chat_messages.len() > MAX_CHAT_HISTORY {
            self.chat_messages.pop_front();
        }

        // Auto-scroll to bottom
        self.chat_scroll = 0;
    }

    /// Add or update a peer
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
        if let Some(transport) = self.transport_status.iter_mut()
            .find(|t| t.transport_type == transport_type) {
            transport.status = status;
            transport.last_update = Instant::now();
        } else {
            self.transport_status.push(TransportInfo {
                transport_type,
                status,
                last_update: Instant::now(),
            });
        }
    }

    /// Add an error message
    fn add_error(&mut self, message: String) {
        self.errors.push_back(message);
        while self.errors.len() > 10 {
            self.errors.pop_front();
        }
    }

    /// Handle keyboard input
    pub fn handle_key_input(&mut self, key: KeyCode, modifiers: KeyModifiers) -> Option<String> {
        match (key, modifiers) {
            // Quit
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                self.should_quit = true;
                None
            }
            (KeyCode::Char('q'), KeyModifiers::NONE) if self.active_tab != 0 => {
                self.should_quit = true;
                None
            }

            // Tab navigation
            (KeyCode::Tab, KeyModifiers::NONE) => {
                self.active_tab = (self.active_tab + 1) % 4;
                None
            }
            (KeyCode::BackTab, KeyModifiers::SHIFT) => {
                self.active_tab = if self.active_tab == 0 { 3 } else { self.active_tab - 1 };
                None
            }

            // Help toggle
            (KeyCode::F(1), KeyModifiers::NONE) => {
                self.show_help = !self.show_help;
                None
            }

            // Chat input handling
            (KeyCode::Enter, KeyModifiers::NONE) if self.active_tab == 0 && !self.input_buffer.is_empty() => {
                let message = self.input_buffer.clone();
                self.input_buffer.clear();
                self.input_cursor = 0;
                self.stats.messages_sent += 1;
                Some(message)
            }

            // Text input
            (KeyCode::Char(c), KeyModifiers::NONE) if self.active_tab == 0 => {
                self.input_buffer.insert(self.input_cursor, c);
                self.input_cursor += 1;
                None
            }

            // Backspace
            (KeyCode::Backspace, KeyModifiers::NONE) if self.active_tab == 0 && self.input_cursor > 0 => {
                self.input_cursor -= 1;
                self.input_buffer.remove(self.input_cursor);
                None
            }

            // Cursor movement
            (KeyCode::Left, KeyModifiers::NONE) if self.active_tab == 0 && self.input_cursor > 0 => {
                self.input_cursor -= 1;
                None
            }
            (KeyCode::Right, KeyModifiers::NONE) if self.active_tab == 0 && self.input_cursor < self.input_buffer.len() => {
                self.input_cursor += 1;
                None
            }

            // Chat scrolling
            (KeyCode::Up, KeyModifiers::NONE) if self.active_tab == 0 => {
                if self.chat_scroll < self.chat_messages.len().saturating_sub(1) {
                    self.chat_scroll += 1;
                }
                None
            }
            (KeyCode::Down, KeyModifiers::NONE) if self.active_tab == 0 => {
                if self.chat_scroll > 0 {
                    self.chat_scroll -= 1;
                }
                None
            }

            // Page up/down
            (KeyCode::PageUp, KeyModifiers::NONE) if self.active_tab == 0 => {
                self.chat_scroll = (self.chat_scroll + 10).min(self.chat_messages.len().saturating_sub(1));
                None
            }
            (KeyCode::PageDown, KeyModifiers::NONE) if self.active_tab == 0 => {
                self.chat_scroll = self.chat_scroll.saturating_sub(10);
                None
            }

            _ => None,
        }
    }

    /// Check if should quit
    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    /// Get current input buffer
    pub fn input_buffer(&self) -> &str {
        &self.input_buffer
    }
}

/// TUI manager for the BitChat application
pub struct TuiManager {
    terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
    tui_app: TuiApp,
    app: Arc<Mutex<BitchatApp>>,
    event_receiver: Arc<Mutex<mpsc::UnboundedReceiver<AppEvent>>>,
}

impl TuiManager {
    /// Create a new TUI manager
    pub async fn new(mut app: BitchatApp) -> Result<Self> {
        // Setup terminal
        enable_raw_mode().map_err(|e| CliError::UI(format!("Failed to enable raw mode: {}", e)))?;
        let mut stdout = std::io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
            .map_err(|e| CliError::UI(format!("Failed to setup terminal: {}", e)))?;
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

    /// Run the TUI event loop
    pub async fn run(&mut self) -> Result<()> {
        let tick_rate = Duration::from_millis(50);
        let mut last_tick = Instant::now();

        info!("Starting TUI event loop");

        loop {
            // Handle crossterm events
            let timeout = tick_rate.saturating_sub(last_tick.elapsed());
            if event::poll(timeout).map_err(|e| CliError::UI(format!("Event poll failed: {}", e)))? {
                if let Ok(Event::Key(key)) = event::read() {
                    if let Some(message) = self.tui_app.handle_key_input(key.code, key.modifiers) {
                        // Send message through BitChat app
                        let mut app = self.app.lock().await;
                        if let Err(e) = app.send_message(None, message.clone()).await {
                            self.tui_app.add_error(format!("Failed to send message: {}", e));
                        } else {
                            // Add to local chat for immediate feedback
                            let own_message = BitchatMessage::new(app.config().display_name.clone(), message);
                            self.tui_app.add_chat_message(app.peer_id(), &own_message, true);
                        }
                    }
                }
            }

            // Handle app events
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

            // Render UI
            if last_tick.elapsed() >= tick_rate {
                self.terminal.draw(|f| self.render_ui(f))
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

    /// Render the UI
    fn render_ui(&mut self, frame: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Tab bar
                Constraint::Min(0),    // Main content
                Constraint::Length(3), // Input bar
                Constraint::Length(1), // Status bar
            ])
            .split(frame.size());

        // Render tabs
        self.render_tabs(frame, chunks[0]);

        // Render main content based on active tab
        match self.tui_app.active_tab {
            0 => self.render_chat(frame, chunks[1]),
            1 => self.render_peers(frame, chunks[1]),
            2 => self.render_transports(frame, chunks[1]),
            3 => self.render_stats(frame, chunks[1]),
            _ => {}
        }

        // Render input (only for chat tab)
        if self.tui_app.active_tab == 0 {
            self.render_input(frame, chunks[2]);
        }

        // Render status bar
        self.render_status_bar(frame, chunks[3]);

        // Render help if shown
        if self.tui_app.show_help {
            self.render_help(frame);
        }
    }

    /// Render tab bar
    fn render_tabs(&self, frame: &mut Frame, area: Rect) {
        let titles = vec!["Chat", "Peers", "Transports", "Stats"];
        let tabs = Tabs::new(titles)
            .block(Block::default().borders(Borders::ALL).title("BitChat CLI"))
            .select(self.tui_app.active_tab)
            .style(Style::default().fg(Color::Cyan))
            .highlight_style(Style::default().add_modifier(Modifier::BOLD));
        
        frame.render_widget(tabs, area);
    }

    /// Render chat tab
    fn render_chat(&self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(3)])
            .split(area);

        // Chat messages
        let messages: Vec<ListItem> = self.tui_app.chat_messages
            .iter()
            .rev()
            .skip(self.tui_app.chat_scroll)
            .take(chunks[0].height as usize)
            .map(|msg| {
                let timestamp = format!("{:02}:{:02}:{:02}",
                    msg.timestamp.elapsed().as_secs() / 3600,
                    (msg.timestamp.elapsed().as_secs() % 3600) / 60,
                    msg.timestamp.elapsed().as_secs() % 60
                );

                let sender_style = if msg.is_own {
                    Style::default().fg(Color::Green)
                } else if msg.is_private {
                    Style::default().fg(Color::Magenta)
                } else {
                    Style::default().fg(Color::Blue)
                };

                let content = format!("[{}] {}: {}", timestamp, msg.sender, msg.content);
                ListItem::new(Line::from(Span::styled(content, sender_style)))
            })
            .collect();

        let messages_list = List::new(messages)
            .block(Block::default().borders(Borders::ALL).title("Messages"));

        frame.render_widget(messages_list, chunks[0]);
    }

    /// Render peers tab
    fn render_peers(&self, frame: &mut Frame, area: Rect) {
        let peers: Vec<ListItem> = self.tui_app.peers
            .iter()
            .map(|peer| {
                let status_color = match peer.status {
                    PeerStatus::Online => Color::Green,
                    PeerStatus::Offline => Color::Red,
                    PeerStatus::Connecting => Color::Yellow,
                };

                let transports = peer.transport_types.iter()
                    .map(|t| format!("{:?}", t))
                    .collect::<Vec<_>>()
                    .join(", ");

                let content = format!("{} [{}] ({})", 
                    peer.peer_id, 
                    peer.display_name.as_deref().unwrap_or("Unknown"), 
                    transports
                );

                ListItem::new(Line::from(Span::styled(content, Style::default().fg(status_color))))
            })
            .collect();

        let peers_list = List::new(peers)
            .block(Block::default().borders(Borders::ALL).title("Discovered Peers"));

        frame.render_widget(peers_list, area);
    }

    /// Render transports tab
    fn render_transports(&self, frame: &mut Frame, area: Rect) {
        let transports: Vec<ListItem> = self.tui_app.transport_status
            .iter()
            .map(|transport| {
                let status_text = match &transport.status {
                    TransportStatus::Starting => "Starting...",
                    TransportStatus::Active => "Active",
                    TransportStatus::Failed(err) => "Failed",
                    TransportStatus::Stopped => "Stopped",
                };

                let status_color = match &transport.status {
                    TransportStatus::Active => Color::Green,
                    TransportStatus::Starting => Color::Yellow,
                    TransportStatus::Failed(_) => Color::Red,
                    TransportStatus::Stopped => Color::Gray,
                };

                let content = format!("{:?}: {}", transport.transport_type, status_text);
                ListItem::new(Line::from(Span::styled(content, Style::default().fg(status_color))))
            })
            .collect();

        let transports_list = List::new(transports)
            .block(Block::default().borders(Borders::ALL).title("Transport Status"));

        frame.render_widget(transports_list, area);
    }

    /// Render stats tab
    fn render_stats(&self, frame: &mut Frame, area: Rect) {
        let stats_text = format!(
            "Messages Sent: {}\nMessages Received: {}\nPeers Discovered: {}\nUptime: {}s",
            self.tui_app.stats.messages_sent,
            self.tui_app.stats.messages_received,
            self.tui_app.stats.peers_discovered,
            self.tui_app.stats.uptime.as_secs()
        );

        let stats_paragraph = Paragraph::new(stats_text)
            .block(Block::default().borders(Borders::ALL).title("Statistics"))
            .wrap(Wrap { trim: true });

        frame.render_widget(stats_paragraph, area);
    }

    /// Render input bar
    fn render_input(&self, frame: &mut Frame, area: Rect) {
        let input_text = format!("> {}", self.tui_app.input_buffer);
        let input_paragraph = Paragraph::new(input_text)
            .block(Block::default().borders(Borders::ALL).title("Type message..."));

        frame.render_widget(input_paragraph, area);
    }

    /// Render status bar
    fn render_status_bar(&self, frame: &mut Frame, area: Rect) {
        let status_text = "F1: Help | Tab: Switch tabs | Ctrl+C: Quit | Enter: Send message";
        let status = Paragraph::new(status_text)
            .style(Style::default().fg(Color::Yellow))
            .alignment(Alignment::Center);

        frame.render_widget(status, area);
    }

    /// Render help overlay
    fn render_help(&self, frame: &mut Frame) {
        let area = centered_rect(80, 70, frame.size());
        
        let help_text = "BitChat CLI Help\n\n\
            Navigation:\n\
            • Tab / Shift+Tab: Switch between tabs\n\
            • F1: Toggle this help\n\
            • Ctrl+C: Quit application\n\n\
            Chat Tab:\n\
            • Type and press Enter to send messages\n\
            • Up/Down: Scroll chat history\n\
            • Page Up/Down: Fast scroll\n\n\
            Tabs:\n\
            • Chat: Send and receive messages\n\
            • Peers: View discovered peers\n\
            • Transports: View transport status\n\
            • Stats: View application statistics";

        let help_paragraph = Paragraph::new(help_text)
            .block(Block::default().borders(Borders::ALL).title("Help"))
            .wrap(Wrap { trim: true });

        frame.render_widget(Clear, area);
        frame.render_widget(help_paragraph, area);
    }
}

impl Drop for TuiManager {
    fn drop(&mut self) {
        // Restore terminal
        let _ = disable_raw_mode();
        let _ = execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        );
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