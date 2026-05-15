use arboard::Clipboard;
use crossterm::{
    ExecutableCommand,
    event::{self, Event, KeyCode},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, ListState, Paragraph},
};
use std::{io::stdout, time::Duration};

use seer_protos_community_neoeinstein_prost::seer::sessions::v1::{
    run_session_stream_response::Payload,
    session_status::State,
    RunSessionStreamResponse,
};

// Stream events
enum StreamEvent {
    Pending(String),
    Running { url: String, expires_at: i64 },
    Completed { error: String },
    Log(String),
    Done,
    Error(String),
}

// Transaction model (populated by parsing log lines)
#[allow(unused)]
struct Transaction {
    /// The log line that triggered creation of this entry.
    header: String,
    /// Short timestamp extracted from the log line, e.g. "14:32:01".
    timestamp: String,
    /// Transaction signature extracted from the log line, or a short excerpt.
    signature: String,
    /// Subsequent log lines that belong to this transaction.
    detail_lines: Vec<String>,
    expanded: bool,
}

/// Flat cursor items for the Transactions list.
#[derive(Clone, Copy)]
#[allow(unused)]
enum FlatItem {
    TxHeader(usize),
    TxDetail(usize, usize), // (tx_idx, detail_idx)
}

// Application status
enum AppStatus {
    Connecting,
    Pending(String),
    Running,
    Completed { error: String },
    StreamError(String),
}

#[derive(PartialEq)]
enum Focus {
    Transactions,
    Logs,
}

// Application state
struct SessionApp {
    rpc_url: String,
    expires_at: Option<i64>,
    status: AppStatus,
    transactions: Vec<Transaction>,
    tx_state: ListState,
    logs: Vec<String>,
    log_offset: usize,
    log_follow: bool,
    log_panel_height: usize,
    log_panel_width: usize,
    focus: Focus,
    url_copied: bool,
    stream_done: bool,
}

#[allow(unused)]
impl SessionApp {
    fn new() -> Self {
        let mut tx_state = ListState::default();
        tx_state.select(Some(0));
        Self {
            rpc_url: String::new(),
            expires_at: None,
            status: AppStatus::Connecting,
            transactions: Vec::new(),
            tx_state,
            logs: Vec::new(),
            log_offset: 0,
            log_follow: true,
            log_panel_height: 0,
            log_panel_width: 0,
            focus: Focus::Transactions,
            url_copied: false,
            stream_done: false,
        }
    }

    fn apply(&mut self, event: StreamEvent) {
        match event {
            StreamEvent::Pending(reason) => {
                self.status = AppStatus::Pending(reason);
            }
            StreamEvent::Running { url, expires_at } => {
                self.rpc_url = url;
                self.expires_at = Some(expires_at);
                self.status = AppStatus::Running;
            }
            StreamEvent::Completed { error } => {
                self.status = AppStatus::Completed { error };
                self.stream_done = true;
            }
            StreamEvent::Log(line) => {
                // Route the line into the transactions tree and always into raw logs.
                self.ingest_log(line.clone());
                self.logs.push(line);
            }
            StreamEvent::Done => {
                self.stream_done = true;
            }
            StreamEvent::Error(e) => {
                self.status = AppStatus::StreamError(e);
                self.stream_done = true;
            }
        }
    }

    /// Parses a single log line and routes it into the transactions tree.
    ///
    /// Solana program logs start a new execution group with "Program <id> invoke [1]".
    /// We use that as the transaction header; all following lines (Program log:,
    /// Program success/failed, etc.) become expandable detail rows under it.
    /// Any line arriving before the first "invoke [1]" (e.g. validator startup
    /// messages) also gets its own entry so nothing is lost.
    fn ingest_log(&mut self, line: String) {
        // "invoke [1]" = top-level program invocation → new transaction group.
        // Also treat explicit sendTransaction / "Transaction signature" lines as headers.
        let is_tx_start = line.contains("invoke [1]")
            || line.contains("sendTransaction")
            || line.to_lowercase().contains("transaction signature");

        if is_tx_start || self.transactions.is_empty() {
            let timestamp = extract_timestamp(&line);
            let signature = extract_signature(&line);
            self.transactions.push(Transaction {
                header: line,
                timestamp,
                signature,
                detail_lines: Vec::new(),
                expanded: false,
            });
            // Keep the selection on the newest entry while Transactions is focused.
            if self.focus == Focus::Transactions {
                let last = self.transactions.len().saturating_sub(1);
                self.tx_state.select(Some(last));
            }
        } else if let Some(tx) = self.transactions.last_mut() {
            tx.detail_lines.push(line);
        }
    }

    // ── Flat item helpers ─────────────────────────────────────────────────
    fn flat_items(&self) -> Vec<FlatItem> {
        let mut items = Vec::new();
        for (ti, tx) in self.transactions.iter().enumerate() {
            items.push(FlatItem::TxHeader(ti));
            if tx.expanded {
                for di in 0..tx.detail_lines.len() {
                    items.push(FlatItem::TxDetail(ti, di));
                }
            }
        }
        items
    }

    fn toggle_expand(&mut self) {
        let items = self.flat_items();
        if let Some(sel) = self.tx_state.selected() {
            match items.get(sel) {
                Some(&FlatItem::TxHeader(ti)) => {
                    self.transactions[ti].expanded = !self.transactions[ti].expanded;
                    let new_len = self.flat_items().len();
                    if sel >= new_len {
                        self.tx_state.select(Some(new_len.saturating_sub(1)));
                    }
                }
                Some(&FlatItem::TxDetail(_, _)) | None => {}
            }
        }
    }

    // ── Transactions panel navigation ─────────────────────────────────────
    fn tx_next(&mut self) {
        let len = self.flat_items().len();
        if len == 0 { return; }
        let i = self.tx_state.selected().map_or(0, |i| (i + 1) % len);
        self.tx_state.select(Some(i));
    }

    fn tx_prev(&mut self) {
        let len = self.flat_items().len();
        if len == 0 { return; }
        let i = self.tx_state.selected()
            .map_or(0, |i| if i == 0 { len - 1 } else { i - 1 });
        self.tx_state.select(Some(i));
    }

    // ── Visual-row helpers (accounts for word-wrap) ───────────────────────
    fn line_visual_rows(line: &str, width: usize) -> usize {
        let w = width.max(1);
        let len = line.chars().count();
        if len == 0 { 1 } else { (len + w - 1) / w }
    }

    fn total_visual_rows(&self) -> usize {
        let w = self.log_panel_width.max(1);
        self.logs.iter().map(|l| Self::line_visual_rows(l, w)).sum()
    }

    fn visual_bottom(&self) -> usize {
        self.total_visual_rows()
            .saturating_sub(self.log_panel_height.max(1))
    }

    // ── RPC Logs panel navigation ─────────────────────────────────────────
    fn log_next(&mut self) {
        let bottom = self.visual_bottom();
        self.log_offset = (self.log_offset + 1).min(bottom);
        if self.log_offset >= bottom {
            self.log_follow = true;
        }
    }

    fn log_prev(&mut self) {
        // Anchor at the current rendered bottom before leaving follow mode.
        if self.log_follow {
            self.log_offset = self.visual_bottom();
            self.log_follow = false;
        }
        self.log_offset = self.log_offset.saturating_sub(1);
    }

    // ── Focus ─────────────────────────────────────────────────────────────
    fn focus_transactions(&mut self) {
        self.focus = Focus::Transactions;
        if self.tx_state.selected().is_none() {
            self.tx_state.select(Some(0));
        }
    }

    fn focus_logs(&mut self) {
        self.focus = Focus::Logs;
    }
}

// Log-line parsers
/// Extract a short timestamp like "14:32:01" from the beginning of a log line.
fn extract_timestamp(line: &str) -> String {
    // Matches patterns: "[14:32:01]", "14:32:01.123", "T14:32:01"
    for token in line.split_whitespace().take(3) {
        let stripped = token.trim_matches(|c| c == '[' || c == ']');
        // "HH:MM:SS" or "HH:MM:SS.mmm"
        let parts: Vec<&str> = stripped.splitn(3, ':').collect();
        if parts.len() == 3
            && parts[0].chars().rev().take(2).all(|c| c.is_ascii_digit())
            && parts[1].len() == 2 && parts[1].chars().all(|c| c.is_ascii_digit())
        {
            let sec = parts[2].split('.').next().unwrap_or("");
            if sec.len() == 2 && sec.chars().all(|c| c.is_ascii_digit()) {
                return format!("{}:{}:{}", parts[0].chars().rev().take(2).collect::<String>().chars().rev().collect::<String>(), parts[1], sec);
            }
        }
    }
    String::new()
}

/// Extract a transaction signature (base58, 43–88 chars) from a log line.
fn extract_signature(line: &str) -> String {
    for token in line.split_whitespace() {
        let t = token.trim_matches(|c| c == '(' || c == ')' || c == ',' || c == ':');
        if t.len() >= 32
            && t.chars().all(|c| {
                matches!(c, '1'..='9' | 'A'..='H' | 'J'..='N' | 'P'..='Z' | 'a'..='k' | 'm'..='z')
            })
        {
            // Truncate long signatures to keep the UI tidy
            if t.len() > 16 {
                return format!("{}...{}", &t[..6], &t[t.len() - 5..]);
            }
            return t.to_string();
        }
    }
    // Fallback: first 40 chars of the line
    line.chars().take(40).collect()
}

/// Split `line` into chunks of exactly `width` characters each.
/// An empty line produces a single empty string so blank rows are preserved.
fn split_line_at(line: &str, width: usize) -> Vec<&str> {
    if width == 0 || line.is_empty() {
        return vec![line];
    }
    let mut chunks = Vec::new();
    let mut char_count = 0usize;
    let mut byte_start = 0usize;
    for (byte_idx, _) in line.char_indices() {
        if char_count > 0 && char_count % width == 0 {
            chunks.push(&line[byte_start..byte_idx]);
            byte_start = byte_idx;
        }
        char_count += 1;
    }
    chunks.push(&line[byte_start..]);
    chunks
}

// Public entry point
pub async fn run_session_tui(
    stream: tonic::Streaming<RunSessionStreamResponse>,
) -> anyhow::Result<()> {
    let (tx, rx) = std::sync::mpsc::channel::<StreamEvent>();
    spawn_stream_reader(stream, tx);
    tokio::task::spawn_blocking(move || tui_loop(rx))
        .await
        .map_err(|e| anyhow::anyhow!("TUI task panicked: {}", e))?
}

// Async stream → std channel bridge
fn spawn_stream_reader(
    mut stream: tonic::Streaming<RunSessionStreamResponse>,
    tx: std::sync::mpsc::Sender<StreamEvent>,
) {
    tokio::spawn(async move {
        loop {
            match stream.message().await {
                Ok(Some(resp)) => {
                    let event = match resp.payload {
                        Some(Payload::Status(s)) => match s.state {
                            Some(State::Pending(p)) => Some(StreamEvent::Pending(p.reason)),
                            Some(State::Running(r)) => Some(StreamEvent::Running {
                                url: r.solana_validator_url,
                                expires_at: r.expires_at,
                            }),
                            Some(State::Completed(c)) => Some(StreamEvent::Completed {
                                error: c.error,
                            }),
                            None => None,
                        },
                        Some(Payload::Log(log)) => Some(StreamEvent::Log(log.line)),
                        None => None,
                    };
                    if let Some(e) = event {
                        if tx.send(e).is_err() {
                            break;
                        }
                    }
                }
                Ok(None) => {
                    let _ = tx.send(StreamEvent::Done);
                    break;
                }
                Err(e) => {
                    let _ = tx.send(StreamEvent::Error(e.to_string()));
                    break;
                }
            }
        }
    });
}

// Blocking TUI loop
fn tui_loop(rx: std::sync::mpsc::Receiver<StreamEvent>) -> anyhow::Result<()> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    let mut app = SessionApp::new();

    let result = run_loop(&mut terminal, &mut app, &rx);

    let _ = disable_raw_mode();
    let _ = stdout().execute(LeaveAlternateScreen);
    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &mut SessionApp,
    rx: &std::sync::mpsc::Receiver<StreamEvent>,
) -> anyhow::Result<()> {
    loop {
        while let Ok(ev) = rx.try_recv() {
            app.apply(ev);
        }

        terminal.draw(|f| render(f, app))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                app.url_copied = false;
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Char('c') => {
                        if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                            return Ok(());
                        }
                        if !app.rpc_url.is_empty() {
                            if let Ok(mut cb) = Clipboard::new() {
                                let _ = cb.set_text(app.rpc_url.clone());
                                app.url_copied = true;
                            }
                        }
                    }
                    KeyCode::Down => app.log_next(),
                    KeyCode::Up => app.log_prev(),
                    KeyCode::Char('j') => {
                        app.log_follow = true;
                    }
                    KeyCode::Char('k') => {
                        app.log_follow = false;
                        app.log_offset = 0;
                    }
                    _ => {}
                }
            }
        }
    }
}

// Rendering
fn format_expiry(expires_at: i64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let remaining = expires_at - now;
    if remaining <= 0 {
        "  (expired)".to_string()
    } else {
        let m = remaining / 60;
        let s = remaining % 60;
        format!("  ({m}m {s}s remaining)")
    }
}

fn render(f: &mut Frame, app: &mut SessionApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4), // header
            Constraint::Min(0),    // RPC Logs
            Constraint::Length(1), // help bar
        ])
        .split(f.area());

    #[allow(unused)]
    let focused_style = Style::default().fg(Color::Yellow);

    // ── Header ───────────────────────────────────────────────────────────────
    let (status_text, status_style) = match &app.status {
        AppStatus::Connecting => (
            "\u{25cb} Connecting...".to_string(),
            Style::default().fg(Color::DarkGray),
        ),
        AppStatus::Pending(reason) => (
            format!("\u{23f3} Pending  {}", reason),
            Style::default().fg(Color::Yellow),
        ),
        AppStatus::Running => (
            "\u{25cf} Running".to_string(),
            Style::default().fg(Color::Green).bold(),
        ),
        AppStatus::Completed { error } => {
            if error.is_empty() {
                ("\u{2714} Completed".to_string(), Style::default().fg(Color::Cyan).bold())
            } else {
                (
                    format!("\u{2718} Completed with error: {}", error),
                    Style::default().fg(Color::Red).bold(),
                )
            }
        }
        AppStatus::StreamError(e) => (
            format!("\u{2718} Error: {}", e),
            Style::default().fg(Color::Red).bold(),
        ),
    };

    let rpc_display = if app.rpc_url.is_empty() {
        "\u{2014}".to_string()
    } else {
        app.rpc_url.clone()
    };

    let expiry_str = app.expires_at.map(format_expiry).unwrap_or_default();
    let copy_indicator = if app.url_copied {
        Span::styled(" \u{2713} Copied!", Style::default().fg(Color::Green).bold())
    } else {
        Span::raw("")
    };

    let col = app.rpc_url.len().max(status_text.len());
    let id_gap  = " ".repeat(col.saturating_sub(status_text.len()) + 3);
    let url_gap = " ".repeat(col.saturating_sub(app.rpc_url.len()) + 3);

    let header = Paragraph::new(Text::from(vec![
        Line::from(vec![
            Span::raw("Status   "),
            Span::styled(status_text, status_style),
            Span::raw(id_gap),
        ]),
        Line::from(vec![
            Span::raw("RPC      "),
            Span::styled(rpc_display, Style::default().fg(Color::Yellow).bold()),
            Span::raw(url_gap),
            copy_indicator,
            Span::styled(expiry_str, Style::default().fg(Color::DarkGray)),
        ]),
    ]))
    .block(Block::default().borders(Borders::ALL).title(" Seer Session "));
    f.render_widget(header, chunks[0]);

    // ── RPC Logs ──────────────────────────────────────────────────────────────
    let logs_title = if app.stream_done {
        " RPC Logs (session ended \u{2014} press q to exit) "
    } else {
        " RPC Logs "
    };

    let logs_block = Block::default()
        .borders(Borders::ALL)
        .title(logs_title)
        .border_style(Style::default());

    // Capture panel dimensions for consistent offset arithmetic.
    let inner_w = chunks[1].width.saturating_sub(2) as usize;
    let visible  = chunks[1].height.saturating_sub(2) as usize;
    app.log_panel_height = visible;
    app.log_panel_width  = inner_w;

    // Pre-split every raw log entry into fixed-width visual lines.
    // We then pass ONLY the visible slice to the widget — no .wrap(), no
    // .scroll(). This makes the offset arithmetic exact and means ratatui
    // never sees off-screen content, eliminating all garbling artefacts.
    let w = inner_w.max(1);
    let visual_lines: Vec<&str> = app.logs.iter()
        .flat_map(|l| split_line_at(l, w))
        .collect();
    let total  = visual_lines.len();
    let bottom = total.saturating_sub(visible);

    if app.log_offset >= bottom {
        app.log_follow = true;
    }
    let scroll_offset = if app.log_follow { bottom } else { app.log_offset.min(bottom) };
    app.log_offset = scroll_offset;

    let start = scroll_offset.min(total);
    let end   = (start + visible).min(total);
    let log_lines: Vec<Line> = visual_lines[start..end]
        .iter()
        .map(|l| Line::from(Span::styled(*l, Style::default().fg(Color::DarkGray))))
        .collect();

    f.render_widget(
        Paragraph::new(log_lines).block(logs_block),
        chunks[1],
    );

    // ── Help bar ──────────────────────────────────────────────────────────────
    let mut help_spans = vec![
        Span::styled(" Esc ", Style::default().fg(Color::Black).bg(Color::DarkGray).bold()),
        Span::raw(" quit   "),
        Span::styled(" ↑↓ ", Style::default().fg(Color::Black).bg(Color::DarkGray).bold()),
        Span::raw(" scroll   "),
        Span::styled(" j/k ", Style::default().fg(Color::Black).bg(Color::DarkGray).bold()),
        Span::raw(" bottom/top   "),
    ];
    if !app.rpc_url.is_empty() {
        help_spans.extend([
            Span::styled(" c ", Style::default().fg(Color::Black).bg(Color::DarkGray).bold()),
            Span::raw(" copy rpc url"),
        ]);
    }

    f.render_widget(Paragraph::new(Line::from(help_spans)), chunks[2]);
}

