use std::io::Stdout;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyModifiers};
use futures::StreamExt as _;
use kanata_types::AgentEvent;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tokio::sync::mpsc;

use crate::commands::{self, CommandResult};
use crate::session::SessionManager;
use crate::ui::confirm::ConfirmDialog;
use crate::ui::input::{InputAction, InputBox};
use crate::ui::layout::AppLayout;
use crate::ui::output::{OutputBlock, OutputView, ToolStatusKind};
use crate::ui::spinner::Spinner;
use crate::ui::status::StatusBar;

/// Application mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    /// Normal input mode.
    Input,
    /// Scrolling through output.
    Scroll,
    /// Waiting for agent response.
    Waiting,
    /// Confirm dialog is active.
    Confirm,
}

/// Messages flowing through the application event loop.
#[derive(Debug)]
pub enum AppMessage {
    /// An agent event received from the session stream.
    Agent(AgentEvent),
}

/// The main application state machine.
pub struct App {
    session: SessionManager,
    mode: AppMode,
    should_quit: bool,
    input: InputBox,
    output: OutputView,
    status: StatusBar,
    spinner: Spinner,
    confirm: ConfirmDialog,
    agent_tx: mpsc::UnboundedSender<AppMessage>,
    agent_rx: mpsc::UnboundedReceiver<AppMessage>,
}

impl App {
    /// Create a new `App` backed by the given session.
    pub fn new(session: SessionManager) -> Self {
        let (agent_tx, agent_rx) = mpsc::unbounded_channel();
        let model_name = session.config_model().to_string();
        Self {
            session,
            mode: AppMode::Input,
            should_quit: false,
            input: InputBox::new(),
            output: OutputView::new(),
            status: StatusBar::new(model_name),
            spinner: Spinner::new(),
            confirm: ConfirmDialog::new(),
            agent_tx,
            agent_rx,
        }
    }

    /// Run the interactive TUI event loop.
    pub async fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
        self.output.push_block(OutputBlock::System(format!(
            "Kanata v{} — interactive mode. Type /help for commands, Ctrl+C to quit.",
            env!("CARGO_PKG_VERSION"),
        )));

        let mut event_stream = EventStream::new();
        let mut tick_interval = tokio::time::interval(Duration::from_millis(100));

        loop {
            // Draw
            terminal.draw(|frame| self.draw(frame))?;

            // Event loop
            tokio::select! {
                ev = event_stream.next() => {
                    if let Some(Ok(event)) = ev {
                        self.handle_crossterm_event(&event);
                    }
                }
                msg = self.agent_rx.recv() => {
                    if let Some(AppMessage::Agent(event)) = msg {
                        self.handle_agent_event(event);
                    }
                }
                _ = tick_interval.tick() => {
                    self.handle_tick();
                }
            }

            if self.should_quit {
                break;
            }
        }

        Ok(())
    }

    /// Draw the full UI frame.
    fn draw(&mut self, frame: &mut ratatui::Frame<'_>) {
        let layout = AppLayout::new(frame.area());

        // Output area
        self.output.render_widget(layout.output, frame.buffer_mut());

        // Status bar
        self.status.render_widget(layout.status, frame.buffer_mut());

        // Input box
        self.input.render_widget(layout.input, frame.buffer_mut());

        // Confirm dialog overlay
        if self.confirm.is_visible() {
            self.confirm.render_widget(frame.area(), frame.buffer_mut());
        }

        // Set cursor position if in input mode
        if self.mode == AppMode::Input {
            let (cx, cy) = self.input.cursor_position(layout.input);
            frame.set_cursor_position((cx, cy));
        }
    }

    /// Handle a crossterm terminal event.
    fn handle_crossterm_event(&mut self, event: &Event) {
        if let Event::Key(key) = event {
            self.handle_key_event(*key);
        }
    }

    /// Handle a key event depending on the current mode.
    fn handle_key_event(&mut self, key: KeyEvent) {
        // Confirm dialog has priority
        if self.confirm.is_visible() {
            if let Some(_confirmed) = self.confirm.handle_key(key) {
                self.mode = AppMode::Input;
            }
            return;
        }

        match self.mode {
            AppMode::Input => self.handle_input_key(key),
            AppMode::Scroll => self.handle_scroll_key(key),
            AppMode::Waiting => self.handle_waiting_key(key),
            AppMode::Confirm => {} // handled above
        }
    }

    /// Handle keys in normal input mode.
    fn handle_input_key(&mut self, key: KeyEvent) {
        match self.input.handle_key(key) {
            InputAction::Submit => {
                let text = self.input.submit();
                if !text.is_empty() {
                    self.submit_input(&text);
                }
            }
            InputAction::Quit => {
                self.should_quit = true;
            }
            InputAction::Consumed | InputAction::Ignored => {}
        }

        // PageUp/PageDown for scroll mode
        if key.code == KeyCode::PageUp {
            self.mode = AppMode::Scroll;
            self.output.scroll_up(10);
        }
    }

    /// Handle keys in scroll mode.
    fn handle_scroll_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.output.scroll_up(1),
            KeyCode::Down | KeyCode::Char('j') => self.output.scroll_down(1),
            KeyCode::PageUp => self.output.scroll_up(10),
            KeyCode::PageDown => self.output.scroll_down(10),
            KeyCode::Esc | KeyCode::Char('q') => {
                self.output.scroll_to_bottom();
                self.mode = AppMode::Input;
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            _ => {}
        }
    }

    /// Handle keys while waiting for agent response.
    fn handle_waiting_key(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.should_quit = true;
        }
    }

    /// Handle a timer tick (spinner animation).
    fn handle_tick(&mut self) {
        if let Some(frame) = self.spinner.tick() {
            self.status.set_spinner_char(frame);
        }
    }

    /// Submit user input — either a command or a message to the agent.
    fn submit_input(&mut self, text: &str) {
        if let Some(stripped) = text.strip_prefix('/') {
            let (cmd, args) = stripped.split_once(' ').map_or((stripped, ""), |(c, a)| (c, a));
            self.handle_command(cmd, args);
        } else {
            self.send_to_agent(text.to_string());
        }
    }

    /// Handle a slash command.
    fn handle_command(&mut self, cmd: &str, args: &str) {
        match commands::handle_command(cmd, args) {
            CommandResult::Display(text) => {
                self.output.push_block(OutputBlock::System(text));
            }
            CommandResult::Exit => {
                self.should_quit = true;
            }
            CommandResult::Clear => {
                self.output.clear();
            }
            CommandResult::SwitchModel(name) => {
                self.output.push_block(OutputBlock::System(format!("Switched model to: {name}")));
            }
            CommandResult::ShowStats => {
                let stats = self.session.stats();
                self.output.push_block(OutputBlock::System(format!(
                    "Model: {}\nTurns: {}\nInput tokens: {}\nOutput tokens: {}\nCost: ${:.4}",
                    stats.model,
                    stats.turns,
                    stats.total_input_tokens,
                    stats.total_output_tokens,
                    stats.total_cost_usd,
                )));
            }
            CommandResult::Unknown(cmd) => {
                self.output.push_block(OutputBlock::Error(format!(
                    "Unknown command: /{cmd}. Type /help for available commands."
                )));
            }
        }
    }

    /// Send a user message to the agent session.
    fn send_to_agent(&mut self, content: String) {
        self.output.push_block(OutputBlock::UserMessage(content.clone()));
        self.mode = AppMode::Waiting;
        self.spinner.start("Thinking...");
        self.status.set_status("Thinking...");

        let tx = self.agent_tx.clone();
        let session = self.session.session_handle();

        tokio::spawn(async move {
            match session.send_message(&content).await {
                Ok(stream) => {
                    futures::pin_mut!(stream);
                    while let Some(event) = stream.next().await {
                        if tx.send(AppMessage::Agent(event)).is_err() {
                            break;
                        }
                    }
                }
                Err(err) => {
                    let _ = tx.send(AppMessage::Agent(AgentEvent::Error(err.to_string())));
                }
            }
        });
    }

    /// Handle an agent event received from the session stream.
    fn handle_agent_event(&mut self, event: AgentEvent) {
        match event {
            AgentEvent::Thinking => {
                self.spinner.start("Thinking...");
                self.status.set_status("Thinking...");
            }
            AgentEvent::TextDelta(text) => {
                self.output.append_streaming_text(&text);
                if self.output.is_at_bottom() {
                    self.output.scroll_to_bottom();
                }
            }
            AgentEvent::ToolStart { name, input_preview } => {
                self.output.flush_streaming();
                self.spinner.start(format!("{name}..."));
                self.status.set_status(format!("Running {name}..."));
                self.output.push_block(OutputBlock::ToolStatus {
                    name,
                    status: ToolStatusKind::Running,
                    detail: input_preview,
                });
            }
            AgentEvent::ToolEnd { name, result_preview } => {
                self.output.push_block(OutputBlock::ToolStatus {
                    name,
                    status: ToolStatusKind::Done,
                    detail: result_preview,
                });
            }
            AgentEvent::Done { usage } => {
                self.output.flush_streaming();
                self.spinner.stop();
                self.status.set_spinner_char("");
                self.status.set_status("Ready");
                self.status.update_stats(usage);
                self.mode = AppMode::Input;
            }
            AgentEvent::Error(err) => {
                self.output.flush_streaming();
                self.output.append_error(&err);
                self.spinner.stop();
                self.status.set_spinner_char("");
                self.status.set_status("Ready");
                self.mode = AppMode::Input;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use kanata_types::KanataConfig;

    use super::*;

    fn make_test_app() -> App {
        let cfg = KanataConfig::default();
        let session = SessionManager::new(cfg).unwrap();
        App::new(session)
    }

    #[test]
    fn test_app_initial_mode() {
        let app = make_test_app();
        assert_eq!(app.mode, AppMode::Input);
        assert!(!app.should_quit);
    }

    #[test]
    fn test_handle_command_exit() {
        let mut app = make_test_app();
        app.handle_command("exit", "");
        assert!(app.should_quit);
    }

    #[test]
    fn test_handle_command_clear() {
        let mut app = make_test_app();
        app.output.push_block(OutputBlock::System("test".to_string()));
        assert_eq!(app.output.block_count(), 1);
        app.handle_command("clear", "");
        assert_eq!(app.output.block_count(), 0);
    }

    #[test]
    fn test_handle_command_help() {
        let mut app = make_test_app();
        app.handle_command("help", "");
        assert!(app.output.block_count() > 0);
    }

    #[test]
    fn test_handle_command_unknown() {
        let mut app = make_test_app();
        app.handle_command("nonexistent", "");
        assert!(app.output.block_count() > 0);
    }

    #[test]
    fn test_handle_agent_event_thinking() {
        let mut app = make_test_app();
        app.handle_agent_event(AgentEvent::Thinking);
        assert!(app.spinner.is_active());
    }

    #[test]
    fn test_handle_agent_event_text_delta() {
        let mut app = make_test_app();
        app.handle_agent_event(AgentEvent::TextDelta("hello".to_string()));
        // Should be in streaming buffer, not yet flushed
        assert_eq!(app.output.block_count(), 0);
    }

    #[test]
    fn test_handle_agent_event_done() {
        let mut app = make_test_app();
        app.mode = AppMode::Waiting;
        app.spinner.start("test");

        app.handle_agent_event(AgentEvent::Done {
            usage: kanata_types::SessionTokenStats {
                total_input_tokens: 10,
                total_output_tokens: 20,
                total_cost_usd: 0.001,
                turns: 1,
                model: "test".to_string(),
            },
        });

        assert_eq!(app.mode, AppMode::Input);
        assert!(!app.spinner.is_active());
    }

    #[test]
    fn test_handle_agent_event_error() {
        let mut app = make_test_app();
        app.mode = AppMode::Waiting;
        app.handle_agent_event(AgentEvent::Error("test error".to_string()));
        assert_eq!(app.mode, AppMode::Input);
        assert!(app.output.block_count() > 0);
    }

    #[test]
    fn test_submit_command() {
        let mut app = make_test_app();
        app.submit_input("/help");
        assert!(app.output.block_count() > 0);
    }
}
