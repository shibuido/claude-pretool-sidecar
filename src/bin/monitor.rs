//! # claude-pretool-monitor
//!
//! Real-time audit log monitor with CLI and TUI frontends.
//!
//! ## Usage
//!
//! ```sh
//! claude-pretool-monitor cli /path/to/audit-dir/
//! claude-pretool-monitor cli /path/to/audit-dir/ --interval 10
//! claude-pretool-monitor tui /path/to/audit-dir/
//! claude-pretool-monitor history /path/to/audit-dir/
//! ```

// The monitor module lives alongside the sidecar crate but we reference it
// directly since this binary is standalone.
#[path = "../monitor.rs"]
mod monitor;

use monitor::{
    format_entry_line, format_stats_block, LogWatcher, MonitorEntry, MonitorState,
};

use clap::{Parser, Subcommand};
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap},
    Frame, Terminal,
};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// CLI argument parsing
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(name = "claude-pretool-monitor")]
#[command(about = "Real-time audit log monitor for claude-pretool-sidecar")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Stream audit events to the terminal with periodic stats
    Cli {
        /// Path to audit log directory
        audit_dir: PathBuf,
        /// Stats summary interval in seconds
        #[arg(long, default_value = "30")]
        interval: u64,
    },
    /// Full-screen TUI dashboard
    Tui {
        /// Path to audit log directory
        audit_dir: PathBuf,
    },
    /// Print history summary and exit
    History {
        /// Path to audit log directory
        audit_dir: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Cli {
            audit_dir,
            interval,
        } => cmd_cli(&audit_dir, interval),
        Commands::Tui { audit_dir } => cmd_tui(&audit_dir),
        Commands::History { audit_dir } => cmd_history(&audit_dir),
    }
}

// ---------------------------------------------------------------------------
// CLI frontend
// ---------------------------------------------------------------------------

fn cmd_cli(audit_dir: &Path, interval_secs: u64) {
    let watcher = LogWatcher::new(audit_dir, Duration::from_millis(500));
    let mut state = MonitorState::new(200);

    // Load and display history
    let history = watcher.read_history();
    if !history.is_empty() {
        for entry in &history {
            state.ingest(entry);
        }
        println!("{}", format_stats_block(&state));
        println!();
    } else {
        println!("No history found. Watching for new entries...");
    }

    // Set up Ctrl+C handler
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc_handler(r);

    let mut last_stats = Instant::now();
    let stats_interval = Duration::from_secs(interval_secs);

    // Watch loop
    for entry in watcher.watch() {
        if !running.load(Ordering::Relaxed) {
            break;
        }

        println!("{}", format_entry_line(&entry));
        state.ingest(&entry);

        if last_stats.elapsed() >= stats_interval {
            println!();
            println!("{}", format_stats_block(&state));
            println!();
            last_stats = Instant::now();
        }
    }

    println!();
    println!("{}", format_stats_block(&state));
}

fn ctrlc_handler(running: Arc<AtomicBool>) {
    let _ = ctrlc::set_handler(move || {
        running.store(false, Ordering::Relaxed);
    });
}

// ---------------------------------------------------------------------------
// History command
// ---------------------------------------------------------------------------

fn cmd_history(audit_dir: &Path) {
    let watcher = LogWatcher::new(audit_dir, Duration::from_secs(1));
    let mut state = MonitorState::new(50);

    let history = watcher.read_history();
    if history.is_empty() {
        eprintln!("No audit entries found in: {}", audit_dir.display());
        std::process::exit(1);
    }

    for entry in &history {
        state.ingest(entry);
    }

    println!("{}", format_stats_block(&state));

    // Print last 20 entries
    println!();
    println!("Recent events:");
    let start = if history.len() > 20 {
        history.len() - 20
    } else {
        0
    };
    for entry in &history[start..] {
        if entry.hook_event == "PreToolUse" {
            println!("  {}", format_entry_line(entry));
        }
    }
}

// ---------------------------------------------------------------------------
// TUI frontend
// ---------------------------------------------------------------------------

/// Application state for the TUI.
struct App {
    state: MonitorState,
    all_events: Vec<MonitorEntry>,
    scroll_offset: usize,
    paused: bool,
    should_quit: bool,
}

impl App {
    fn new() -> Self {
        Self {
            state: MonitorState::new(500),
            all_events: Vec::new(),
            scroll_offset: 0,
            paused: false,
            should_quit: false,
        }
    }

    fn ingest(&mut self, entry: MonitorEntry) {
        self.state.ingest(&entry);
        if entry.hook_event == "PreToolUse" {
            self.all_events.push(entry);
        }
    }

    fn reset(&mut self) {
        self.state = MonitorState::new(500);
        self.all_events.clear();
        self.scroll_offset = 0;
    }

    fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }

    fn scroll_down(&mut self) {
        let max = self.all_events.len().saturating_sub(1);
        if self.scroll_offset < max {
            self.scroll_offset += 1;
        }
    }

    fn scroll_to_bottom(&mut self) {
        self.scroll_offset = self.all_events.len().saturating_sub(1);
    }
}

fn cmd_tui(audit_dir: &Path) {
    // Load history
    let watcher = LogWatcher::new(audit_dir, Duration::from_millis(500));
    let history = watcher.read_history();

    let mut app = App::new();
    for entry in history {
        app.ingest(entry);
    }
    app.scroll_to_bottom();

    // Set up terminal
    enable_raw_mode().expect("Failed to enable raw mode");
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).expect("Failed to enter alternate screen");
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).expect("Failed to create terminal");

    // Spawn watcher thread
    let (tx, rx) = std::sync::mpsc::channel::<MonitorEntry>();
    let watch_dir = audit_dir.clone();
    std::thread::spawn(move || {
        let watcher = LogWatcher::new(&watch_dir, Duration::from_millis(500));
        for entry in watcher.watch() {
            if tx.send(entry).is_err() {
                break;
            }
        }
    });

    // Main loop
    let tick_rate = Duration::from_millis(500);
    let mut last_tick = Instant::now();

    loop {
        // Draw
        terminal
            .draw(|f| draw_ui(f, &app))
            .expect("Failed to draw");

        // Handle events
        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if event::poll(timeout).unwrap_or(false) {
            if let Ok(Event::Key(key)) = event::read() {
                match key.code {
                    KeyCode::Char('q') => app.should_quit = true,
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app.should_quit = true
                    }
                    KeyCode::Up => app.scroll_up(),
                    KeyCode::Down => app.scroll_down(),
                    KeyCode::Char('r') => app.reset(),
                    KeyCode::Char('p') => app.paused = !app.paused,
                    KeyCode::End | KeyCode::Char('G') => app.scroll_to_bottom(),
                    KeyCode::Home | KeyCode::Char('g') => app.scroll_offset = 0,
                    _ => {}
                }
            }
        }

        if app.should_quit {
            break;
        }

        // Ingest new entries from watcher
        if !app.paused {
            while let Ok(entry) = rx.try_recv() {
                app.ingest(entry);
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }

    // Restore terminal
    disable_raw_mode().expect("Failed to disable raw mode");
    execute!(terminal.backend_mut(), LeaveAlternateScreen)
        .expect("Failed to leave alternate screen");
    terminal.show_cursor().expect("Failed to show cursor");
}

fn draw_ui(f: &mut Frame, app: &App) {
    let size = f.area();

    // Top/bottom split: 60% events, 40% stats panels
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(size);

    draw_events_panel(f, app, main_chunks[0]);

    // Bottom: 3 columns
    let bottom_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(34),
            Constraint::Percentage(33),
        ])
        .split(main_chunks[1]);

    draw_decisions_panel(f, app, bottom_chunks[0]);
    draw_providers_panel(f, app, bottom_chunks[1]);
    draw_candidates_panel(f, app, bottom_chunks[2]);
}

fn draw_events_panel(f: &mut Frame, app: &App, area: Rect) {
    let title = format!(
        " Events ({}) {} ",
        app.all_events.len(),
        if app.paused { "[PAUSED]" } else { "" }
    );
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.all_events.is_empty() {
        let msg = Paragraph::new("Waiting for audit events...")
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(msg, inner);
        return;
    }

    let visible_height = inner.height as usize;
    let total = app.all_events.len();

    // Calculate visible window
    let start = if app.scroll_offset + visible_height > total {
        total.saturating_sub(visible_height)
    } else {
        app.scroll_offset
    };
    let end = (start + visible_height).min(total);

    let lines: Vec<Line> = app.all_events[start..end]
        .iter()
        .map(|entry| {
            let color = decision_color(&entry.final_decision);
            let time = if entry.timestamp.len() >= 19 {
                &entry.timestamp[11..19]
            } else {
                &entry.timestamp
            };
            let input_summary = summarize_input_brief(&entry.tool_name, &entry.tool_input);

            Line::from(vec![
                Span::styled(
                    format!("[{time}] "),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("{:<12}", entry.tool_name),
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{:<30} ", input_summary),
                    Style::default().fg(Color::Gray),
                ),
                Span::styled(
                    format!("{:<12}", entry.final_decision),
                    Style::default().fg(color),
                ),
                Span::styled(
                    format!("{}ms", entry.total_time_ms),
                    Style::default().fg(Color::DarkGray),
                ),
            ])
        })
        .collect();

    let paragraph = Paragraph::new(lines);
    f.render_widget(paragraph, inner);
}

fn draw_decisions_panel(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Decisions ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let total = app.state.total_requests;
    let mut lines = vec![
        Line::from(Span::styled(
            format!("Total: {total}"),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    for (decision, color) in &[
        ("allow", Color::Green),
        ("deny", Color::Red),
        ("passthrough", Color::Yellow),
    ] {
        let count = app.state.decisions.get(*decision).copied().unwrap_or(0);
        let pct = if total > 0 {
            (count as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        // Simple bar
        let bar_width = if total > 0 && area.width > 20 {
            ((count as f64 / total as f64) * (area.width as f64 - 20.0)) as usize
        } else {
            0
        };
        let bar = "\u{2588}".repeat(bar_width);

        lines.push(Line::from(vec![
            Span::styled(format!("{:<12}", decision), Style::default().fg(*color)),
            Span::styled(
                format!("{:>4} ({:>3.0}%) ", count, pct),
                Style::default().fg(Color::White),
            ),
            Span::styled(bar, Style::default().fg(*color)),
        ]));
    }

    // Top tools
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Top Tools:",
        Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
    )));

    let mut tools: Vec<_> = app.state.tools.iter().collect();
    tools.sort_by(|a, b| b.1.total.cmp(&a.1.total));
    for (name, ts) in tools.iter().take(5) {
        lines.push(Line::from(vec![
            Span::styled(format!("  {:<10}", name), Style::default().fg(Color::Cyan)),
            Span::styled(format!("{}", ts.total), Style::default().fg(Color::White)),
        ]));
    }

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    f.render_widget(paragraph, inner);
}

fn draw_providers_panel(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Provider Performance ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));

    if app.state.providers.is_empty() {
        let msg = Paragraph::new("No provider data yet")
            .style(Style::default().fg(Color::DarkGray))
            .block(block);
        f.render_widget(msg, area);
        return;
    }

    let header = Row::new(vec![
        Cell::from("Provider").style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Cell::from("Calls").style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Cell::from("Avg ms").style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Cell::from("Max ms").style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Cell::from("Errs").style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
    ]);

    let mut providers: Vec<_> = app.state.providers.iter().collect();
    providers.sort_by_key(|(name, _)| (*name).clone());

    let rows: Vec<Row> = providers
        .iter()
        .map(|(name, ps)| {
            let err_color = if ps.errors > 0 { Color::Red } else { Color::Green };
            Row::new(vec![
                Cell::from(name.as_str()).style(Style::default().fg(Color::Cyan)),
                Cell::from(format!("{}", ps.invocations)),
                Cell::from(format!("{}", ps.avg_time_ms())),
                Cell::from(format!("{}", ps.max_time_ms)),
                Cell::from(format!("{}", ps.errors)).style(Style::default().fg(err_color)),
            ])
        })
        .collect();

    let widths = [
        Constraint::Percentage(30),
        Constraint::Percentage(18),
        Constraint::Percentage(18),
        Constraint::Percentage(18),
        Constraint::Percentage(16),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(block);

    f.render_widget(table, area);
}

fn draw_candidates_panel(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Auto-Approval Candidates ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));

    let candidates = app.state.auto_approval_candidates();

    if candidates.is_empty() {
        let msg = Paragraph::new("No candidates yet\n(need 3+ requests,\n100% allow rate)")
            .style(Style::default().fg(Color::DarkGray))
            .block(block);
        f.render_widget(msg, area);
        return;
    }

    let mut lines = Vec::new();
    for (pattern, ps) in candidates.iter().take(10) {
        lines.push(Line::from(vec![
            Span::styled(
                format!("{pattern}"),
                Style::default().fg(Color::Green),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled(
                format!("  {} requests, {:.0}% allowed", ps.total, ps.allow_rate() * 100.0),
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    }

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(paragraph, area);
}

fn decision_color(decision: &str) -> Color {
    match decision {
        "allow" => Color::Green,
        "deny" => Color::Red,
        "passthrough" => Color::Yellow,
        _ => Color::Gray,
    }
}

fn summarize_input_brief(tool_name: &str, input: &serde_json::Value) -> String {
    match tool_name {
        "Bash" => input
            .get("command")
            .and_then(|v| v.as_str())
            .map(|cmd| {
                if cmd.len() > 28 {
                    format!("{}...", &cmd[..25])
                } else {
                    cmd.to_string()
                }
            })
            .unwrap_or_else(|| "?".to_string()),
        "Read" | "Write" | "Edit" => input
            .get("file_path")
            .and_then(|v| v.as_str())
            .map(|p| {
                std::path::Path::new(p)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(p)
                    .to_string()
            })
            .unwrap_or_else(|| "?".to_string()),
        _ => "...".to_string(),
    }
}
