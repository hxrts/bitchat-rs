//! TUI Manager - handles terminal and rendering

use std::sync::Arc;
use std::time::{Duration, Instant};

use bitchat_core::{transport::TransportType, BitchatMessage};
use tracing::info;

use crate::app::{PeerStatus, TransportStatus};
use crate::tui::types::{InputMode, LogLevel};

use crossterm::{
    event::{self, Event, KeyEventKind},
    terminal::{enable_raw_mode, EnterAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{
        Block, Borders, Clear, Gauge, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Tabs, Wrap,
    },
    Frame, Terminal,
};
use tokio::sync::{mpsc, Mutex};

use crate::app::{AppEvent, BitchatApp};
use crate::error::{CliError, Result};

use super::app::TuiApp;
use super::render::centered_rect;
use super::types::{Tab, TICK_RATE};

// ----------------------------------------------------------------------------
// TUI Manager
// ----------------------------------------------------------------------------

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
            if event::poll(timeout)
                .map_err(|e| CliError::UI(format!("Event poll failed: {}", e)))?
            {
                if let Event::Key(key) = event::read()
                    .map_err(|e| CliError::UI(format!("Failed to read event: {}", e)))?
                {
                    if key.kind == KeyEventKind::Press {
                        if let Some(message) = self.tui_app.handle_key(key) {
                            // Send message through BitChat app
                            let mut app = self.app.lock().await;
                            match app.send_message(None, message.clone()).await {
                                Ok(msg_id) => {
                                    // Add to local chat for immediate feedback
                                    let own_message = BitchatMessage::new(
                                        app.config().display_name.clone(),
                                        message,
                                    );
                                    self.tui_app.add_chat_message(
                                        app.peer_id(),
                                        &own_message,
                                        true,
                                    );
                                    self.tui_app.add_log(
                                        LogLevel::Info,
                                        format!("Sent message: {}", msg_id),
                                    );
                                }
                                Err(e) => {
                                    self.tui_app.add_log(
                                        LogLevel::Error,
                                        format!("Failed to send message: {}", e),
                                    );
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
                let tui_app = &self.tui_app;
                self.terminal
                    .draw(|f| {
                        Self::render_ui_static(f, tui_app);
                    })
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
    fn render_ui_static(frame: &mut Frame, tui_app: &TuiApp) {
        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header with tabs
                Constraint::Min(0),    // Main content
                Constraint::Length(if tui_app.active_tab == Tab::Chat {
                    3
                } else {
                    1
                }), // Input/status
            ])
            .split(frame.area());

        // Render header with tabs
        Self::render_header_static(frame, main_layout[0], tui_app);

        // Render main content based on active tab
        match tui_app.active_tab {
            Tab::Chat => {
                Self::render_chat_tab_static(frame, main_layout[1], main_layout[2], tui_app)
            }
            Tab::Peers => Self::render_peers_tab_static(frame, main_layout[1], tui_app),
            Tab::Transports => Self::render_transports_tab_static(frame, main_layout[1], tui_app),
            Tab::Logs => Self::render_logs_tab_static(frame, main_layout[1], tui_app),
            Tab::Stats => Self::render_stats_tab_static(frame, main_layout[1], tui_app),
        }

        // Render status bar for non-chat tabs
        if tui_app.active_tab != Tab::Chat {
            Self::render_status_bar_static(frame, main_layout[2], tui_app);
        }

        // Render help overlay if shown
        if tui_app.show_help {
            Self::render_help_overlay_static(frame);
        }
    }

    /// Render the chat tab (static version)
    fn render_chat_tab_static(
        frame: &mut Frame,
        main_area: Rect,
        input_area: Rect,
        tui_app: &TuiApp,
    ) {
        // Split main area for messages and peer list
        let chat_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(75), Constraint::Percentage(25)])
            .split(main_area);

        // Messages area
        Self::render_messages_static(frame, chat_layout[0], tui_app);

        // Peer list sidebar
        Self::render_peer_sidebar_static(frame, chat_layout[1], tui_app);

        // Input area
        Self::render_input_static(frame, input_area, tui_app);
    }

    /// Render chat messages (static version)
    fn render_messages_static(frame: &mut Frame, area: Rect, tui_app: &TuiApp) {
        let messages: Vec<ListItem> = tui_app
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
                        "[DELIVERED]"
                    } else {
                        "[PENDING]"
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

                let content = format!(
                    "[{}] {}: {} {}",
                    timestamp, msg.sender, msg.content, delivery_indicator
                );
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

        // Clone list state for rendering (since we can't mutate in static context)
        let mut chat_list_state = tui_app.chat_list_state.clone();
        frame.render_stateful_widget(messages_list, area, &mut chat_list_state);

        // Render scrollbar
        let scrollbar = Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));

        let mut scrollbar_state = ScrollbarState::default()
            .content_length(tui_app.chat_messages.len())
            .position(tui_app.chat_list_state.selected().unwrap_or(0));

        frame.render_stateful_widget(
            scrollbar,
            area.inner(Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut scrollbar_state,
        );
    }

    /// Render the header with tabs
    fn render_header_static(frame: &mut Frame, area: Rect, tui_app: &TuiApp) {
        let titles: Vec<Line> = Tab::ALL.iter().map(|tab| Line::from(tab.title())).collect();

        let tabs = Tabs::new(titles)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("BitChat TUI")
                    .title_style(Style::default().fg(Color::Cyan).bold()),
            )
            .select(
                Tab::ALL
                    .iter()
                    .position(|&t| t == tui_app.active_tab)
                    .unwrap(),
            )
            .style(Style::default().fg(Color::White))
            .highlight_style(Style::default().bg(Color::Blue).fg(Color::White).bold());

        frame.render_widget(tabs, area);
    }

    /// Render the chat tab (UNUSED - kept for reference)
    #[allow(dead_code)]
    fn render_chat_tab(&mut self, frame: &mut Frame, main_area: Rect, input_area: Rect) {
        // Split main area for messages and peer list
        let chat_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(75), Constraint::Percentage(25)])
            .split(main_area);

        // Messages area
        self.render_messages(frame, chat_layout[0]);

        // Peer list sidebar
        Self::render_peer_sidebar_static(frame, chat_layout[1], &self.tui_app);

        // Input area
        Self::render_input_static(frame, input_area, &self.tui_app);
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
                        "[DELIVERED]"
                    } else {
                        "[PENDING]"
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

                let content = format!(
                    "[{}] {}: {} {}",
                    timestamp, msg.sender, msg.content, delivery_indicator
                );
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
            area.inner(Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut scrollbar_state,
        );
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

                ListItem::new(Line::from(Span::styled(
                    content,
                    Style::default().fg(status_color),
                )))
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
                    TransportStatus::Failed(_err) => ("Failed", Color::Red),
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

                ListItem::new(Line::from(Span::styled(
                    content,
                    Style::default().fg(status_color),
                )))
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

        frame.render_stateful_widget(
            transports_list,
            area,
            &mut self.tui_app.transports_list_state,
        );
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
            area.inner(Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut scrollbar_state,
        );
    }

    /// Render peer sidebar
    fn render_peer_sidebar_static(frame: &mut Frame, area: Rect, tui_app: &TuiApp) {
        let peers: Vec<ListItem> = tui_app
            .peers
            .iter()
            .map(|peer| {
                let status_indicator = match peer.status {
                    PeerStatus::Online => "[ONLINE]",
                    PeerStatus::Offline => "[OFFLINE]",
                    PeerStatus::Connecting => "[CONNECTING]",
                };

                let transports = peer
                    .transport_types
                    .iter()
                    .map(|t| match t {
                        TransportType::Ble => "[BLE]",
                        TransportType::Nostr => "[NOSTR]",
                        _ => "[UNKNOWN]",
                    })
                    .collect::<String>();

                let peer_id_short = &peer.peer_id.to_string()[..8];
                let display_name = peer.display_name.as_deref().unwrap_or("Unknown");

                let content = format!(
                    "{} {} {} ({})",
                    status_indicator, transports, display_name, peer_id_short
                );
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
    fn render_input_static(frame: &mut Frame, area: Rect, tui_app: &TuiApp) {
        let input_style = match tui_app.input_mode {
            InputMode::Normal => Style::default().fg(Color::Gray),
            InputMode::Editing => Style::default().fg(Color::White),
        };

        let mode_indicator = match tui_app.input_mode {
            InputMode::Normal => "Normal (press 'i' to edit)",
            InputMode::Editing => "Editing (ESC to exit, Enter to send)",
        };

        let input_text = format!("> {}", tui_app.input_buffer);
        let input = Paragraph::new(input_text).style(input_style).block(
            Block::default()
                .borders(Borders::ALL)
                .title(mode_indicator)
                .title_style(Style::default().fg(Color::Yellow)),
        );

        frame.render_widget(input, area);

        // Render cursor
        if tui_app.input_mode == InputMode::Editing {
            frame.set_cursor_position((area.x + tui_app.input_cursor as u16 + 3, area.y + 1));
        }
    }

    /// Render peers tab
    fn render_peers_tab_static(frame: &mut Frame, area: Rect, tui_app: &TuiApp) {
        let peers: Vec<ListItem> = tui_app
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

                ListItem::new(Line::from(Span::styled(
                    content,
                    Style::default().fg(status_color),
                )))
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

        let mut peers_list_state = tui_app.peers_list_state.clone();
        frame.render_stateful_widget(peers_list, area, &mut peers_list_state);
    }

    /// Render transports tab
    fn render_transports_tab_static(frame: &mut Frame, area: Rect, tui_app: &TuiApp) {
        let transports: Vec<ListItem> = tui_app
            .transports
            .iter()
            .map(|transport| {
                let (status_text, status_color) = match &transport.status {
                    TransportStatus::Starting => ("Starting...", Color::Yellow),
                    TransportStatus::Active => ("Active", Color::Green),
                    TransportStatus::Failed(_err) => ("Failed", Color::Red),
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

                ListItem::new(Line::from(Span::styled(
                    content,
                    Style::default().fg(status_color),
                )))
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

        let mut transports_list_state = tui_app.transports_list_state.clone();
        frame.render_stateful_widget(transports_list, area, &mut transports_list_state);
    }

    /// Render logs tab
    fn render_logs_tab_static(frame: &mut Frame, area: Rect, tui_app: &TuiApp) {
        let logs: Vec<ListItem> = tui_app
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

        let mut logs_list_state = tui_app.logs_list_state.clone();
        frame.render_stateful_widget(logs_list, area, &mut logs_list_state);

        // Render scrollbar
        let scrollbar = Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));

        let mut scrollbar_state = ScrollbarState::default()
            .content_length(tui_app.logs.len())
            .position(tui_app.logs_list_state.selected().unwrap_or(0));

        frame.render_stateful_widget(
            scrollbar,
            area.inner(Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut scrollbar_state,
        );
    }

    /// Render stats tab
    fn render_stats_tab_static(frame: &mut Frame, area: Rect, tui_app: &TuiApp) {
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
            tui_app.stats.messages_sent,
            tui_app.stats.messages_received,
            tui_app.stats.peers_discovered,
            tui_app.stats.total_peers,
            tui_app.stats.active_transports,
            tui_app.stats.uptime.as_secs() / 3600,
            (tui_app.stats.uptime.as_secs() % 3600) / 60,
            tui_app.stats.uptime.as_secs() % 60,
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
        let delivery_rate = tui_app.stats.delivery_success_rate;
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
    fn render_status_bar_static(frame: &mut Frame, area: Rect, tui_app: &TuiApp) {
        let status_text = match tui_app.active_tab {
            Tab::Chat => "i: Insert mode | q: Quit | Tab: Switch tabs | F1: Help",
            _ => "q: Quit | Tab: Switch tabs | F1: Help | ↑/↓: Navigate",
        };

        let status = Paragraph::new(status_text)
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center);

        frame.render_widget(status, area);
    }

    /// Render help overlay
    fn render_help_overlay_static(frame: &mut Frame) {
        let area = centered_rect(80, 70, frame.area());

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
