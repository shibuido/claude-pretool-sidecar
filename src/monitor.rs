//! # Real-Time Audit Log Monitor
//!
//! Shared core for watching audit JSONL files and accumulating statistics.
//! Used by both the CLI and TUI frontends in `src/bin/monitor.rs`.
//!
//! ## Design
//!
//! - **LogWatcher**: polls audit log directory for new data (cross-platform, no inotify)
//! - **MonitorEntry**: standalone deserialization struct (same JSONL format as audit.rs)
//! - **MonitorState**: accumulates live statistics from ingested entries

use serde::Deserialize;
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::io::{BufRead, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::Duration;

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// A single audit log entry, deserialized from JSONL.
/// Standalone struct — does not import from audit.rs.
#[derive(Deserialize, Clone, Debug)]
pub struct MonitorEntry {
    pub timestamp: String,
    pub hook_event: String,
    pub tool_name: String,
    pub tool_input: serde_json::Value,
    #[serde(default)]
    #[allow(dead_code)]
    pub session_id: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    pub tool_use_id: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    pub tool_result_summary: Option<String>,
    #[serde(default)]
    pub providers: Vec<ProviderInfo>,
    pub final_decision: String,
    #[serde(default)]
    pub total_time_ms: u64,
}

/// Provider vote info within a monitor entry.
#[derive(Deserialize, Clone, Debug)]
pub struct ProviderInfo {
    pub name: String,
    #[serde(default)]
    pub vote: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub mode: String,
    #[serde(default)]
    pub response_time_ms: u64,
}

/// Per-tool accumulated statistics.
#[derive(Debug, Clone, Default)]
pub struct ToolStats {
    pub total: u64,
    pub decisions: HashMap<String, u64>,
}

/// Per-provider accumulated statistics.
#[derive(Debug, Clone, Default)]
pub struct ProviderStats {
    pub invocations: u64,
    pub total_time_ms: u64,
    pub max_time_ms: u64,
    pub errors: u64,
}

impl ProviderStats {
    pub fn avg_time_ms(&self) -> u64 {
        if self.invocations == 0 {
            0
        } else {
            self.total_time_ms / self.invocations
        }
    }
}

/// Pattern statistics for auto-approval candidate detection.
#[derive(Debug, Clone, Default)]
pub struct PatternStats {
    pub total: u64,
    pub allowed: u64,
}

impl PatternStats {
    pub fn allow_rate(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.allowed as f64 / self.total as f64
        }
    }
}

// ---------------------------------------------------------------------------
// MonitorState
// ---------------------------------------------------------------------------

/// Accumulated statistics from ingested audit entries.
pub struct MonitorState {
    pub total_requests: u64,
    pub decisions: HashMap<String, u64>,
    pub tools: HashMap<String, ToolStats>,
    pub providers: HashMap<String, ProviderStats>,
    pub recent: VecDeque<MonitorEntry>,
    pub patterns: HashMap<String, PatternStats>,
    pub time_range: Option<(String, String)>,
    recent_limit: usize,
}

impl MonitorState {
    pub fn new(recent_limit: usize) -> Self {
        Self {
            total_requests: 0,
            decisions: HashMap::new(),
            tools: HashMap::new(),
            providers: HashMap::new(),
            recent: VecDeque::new(),
            patterns: HashMap::new(),
            time_range: None,
            recent_limit,
        }
    }

    /// Ingest a single entry, updating all statistics.
    pub fn ingest(&mut self, entry: &MonitorEntry) {
        // Only count PreToolUse events for stats (same logic as analyzer)
        if entry.hook_event != "PreToolUse" {
            return;
        }

        self.total_requests += 1;

        // Decision counts
        *self.decisions.entry(entry.final_decision.clone()).or_insert(0) += 1;

        // Per-tool stats
        let tool = self.tools.entry(entry.tool_name.clone()).or_default();
        tool.total += 1;
        *tool.decisions.entry(entry.final_decision.clone()).or_insert(0) += 1;

        // Provider stats
        for p in &entry.providers {
            let ps = self.providers.entry(p.name.clone()).or_default();
            ps.invocations += 1;
            ps.total_time_ms += p.response_time_ms;
            if p.response_time_ms > ps.max_time_ms {
                ps.max_time_ms = p.response_time_ms;
            }
            // Count votes that indicate errors
            if p.vote == "error" || p.vote.is_empty() {
                ps.errors += 1;
            }
        }

        // Recent entries (ring buffer)
        self.recent.push_back(entry.clone());
        while self.recent.len() > self.recent_limit {
            self.recent.pop_front();
        }

        // Pattern detection
        let pattern = normalize_tool_input(&entry.tool_name, &entry.tool_input);
        let ps = self.patterns.entry(pattern).or_default();
        ps.total += 1;
        if entry.final_decision == "allow" {
            ps.allowed += 1;
        }

        // Time range
        match &self.time_range {
            None => {
                self.time_range = Some((entry.timestamp.clone(), entry.timestamp.clone()));
            }
            Some((start, end)) => {
                let new_start = if entry.timestamp < *start {
                    entry.timestamp.clone()
                } else {
                    start.clone()
                };
                let new_end = if entry.timestamp > *end {
                    entry.timestamp.clone()
                } else {
                    end.clone()
                };
                self.time_range = Some((new_start, new_end));
            }
        }
    }

    /// Return patterns that are auto-approval candidates:
    /// >= 3 occurrences, 100% allow rate.
    pub fn auto_approval_candidates(&self) -> Vec<(&str, &PatternStats)> {
        let mut candidates: Vec<_> = self
            .patterns
            .iter()
            .filter(|(_, ps)| ps.total >= 3 && ps.total == ps.allowed)
            .map(|(name, ps)| (name.as_str(), ps))
            .collect();
        candidates.sort_by(|a, b| b.1.total.cmp(&a.1.total));
        candidates
    }
}

// ---------------------------------------------------------------------------
// Pattern normalization (mirrors analyzer.rs logic)
// ---------------------------------------------------------------------------

/// Normalize tool_input to a simplified pattern string for grouping.
pub fn normalize_tool_input(tool_name: &str, input: &serde_json::Value) -> String {
    match tool_name {
        "Bash" => {
            if let Some(cmd) = input.get("command").and_then(|v| v.as_str()) {
                let first_word = cmd.split_whitespace().next().unwrap_or("?");
                format!("{first_word} *")
            } else {
                "?".to_string()
            }
        }
        "Read" | "Write" | "Edit" => {
            if let Some(path) = input.get("file_path").and_then(|v| v.as_str()) {
                if path.starts_with("/etc/") || path.starts_with("/root") {
                    format!("{tool_name}({path})")
                } else {
                    format!("{tool_name}(*)")
                }
            } else {
                format!("{tool_name}(?)")
            }
        }
        _ => format!("{tool_name}(*)"),
    }
}

// ---------------------------------------------------------------------------
// LogWatcher
// ---------------------------------------------------------------------------

/// Watches an audit log directory for new JSONL entries.
///
/// Uses polling (tracks file sizes) for cross-platform compatibility.
pub struct LogWatcher {
    dir: PathBuf,
    poll_interval: Duration,
}

impl LogWatcher {
    pub fn new(dir: &Path, poll_interval: Duration) -> Self {
        Self {
            dir: dir.to_path_buf(),
            poll_interval,
        }
    }

    /// Read all existing audit entries from the directory (history).
    pub fn read_history(&self) -> Vec<MonitorEntry> {
        let files = match collect_audit_files(&self.dir) {
            Ok(f) => f,
            Err(_) => return Vec::new(),
        };

        let mut entries = Vec::new();
        for file in &files {
            if let Ok(f) = fs::File::open(file) {
                let reader = std::io::BufReader::new(f);
                for line in reader.lines().map_while(Result::ok) {
                    let trimmed = line.trim();
                    if trimmed.is_empty() || trimmed.contains("\"_truncated\"") {
                        continue;
                    }
                        if let Ok(entry) = serde_json::from_str::<MonitorEntry>(trimmed) {
                            entries.push(entry);
                        }
                    }
                }
            }
        }
        entries
    }

    /// Blocking iterator: yields new entries as they appear (tail -f style).
    ///
    /// Polls the directory at the configured interval, tracking file sizes
    /// to detect new data.
    pub fn watch(&self) -> WatchIter {
        // Record current file sizes as the baseline
        let mut file_positions: HashMap<PathBuf, u64> = HashMap::new();
        if let Ok(files) = collect_audit_files(&self.dir) {
            for file in files {
                if let Ok(meta) = fs::metadata(&file) {
                    file_positions.insert(file, meta.len());
                }
            }
        }

        WatchIter {
            dir: self.dir.clone(),
            poll_interval: self.poll_interval,
            file_positions,
            pending: VecDeque::new(),
        }
    }
}

/// Iterator that yields new MonitorEntry values as files grow.
pub struct WatchIter {
    dir: PathBuf,
    poll_interval: Duration,
    file_positions: HashMap<PathBuf, u64>,
    pending: VecDeque<MonitorEntry>,
}

impl Iterator for WatchIter {
    type Item = MonitorEntry;

    fn next(&mut self) -> Option<MonitorEntry> {
        loop {
            // Drain pending entries first
            if let Some(entry) = self.pending.pop_front() {
                return Some(entry);
            }

            // Sleep then poll
            std::thread::sleep(self.poll_interval);

            // Check for new/grown files
            let files = match collect_audit_files(&self.dir) {
                Ok(f) => f,
                Err(_) => continue,
            };

            for file in files {
                let current_size = match fs::metadata(&file) {
                    Ok(m) => m.len(),
                    Err(_) => continue,
                };

                let prev_size = self.file_positions.get(&file).copied().unwrap_or(0);
                if current_size <= prev_size {
                    continue;
                }

                // Read new bytes from prev_size to current_size
                if let Ok(mut f) = fs::File::open(&file) {
                    if f.seek(SeekFrom::Start(prev_size)).is_ok() {
                        let reader = std::io::BufReader::new(f);
                        for line in reader.lines() {
                            if let Ok(line) = line {
                                let trimmed = line.trim();
                                if trimmed.is_empty() || trimmed.contains("\"_truncated\"") {
                                    continue;
                                }
                                if let Ok(entry) =
                                    serde_json::from_str::<MonitorEntry>(trimmed)
                                {
                                    self.pending.push_back(entry);
                                }
                            }
                        }
                    }
                }

                self.file_positions.insert(file, current_size);
            }
        }
    }
}

/// Collect sorted audit-*.jsonl files from a directory.
fn collect_audit_files(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with("audit-") && name_str.ends_with(".jsonl") {
            files.push(entry.path());
        }
    }
    files.sort();
    Ok(files)
}

// ---------------------------------------------------------------------------
// Formatting helpers (used by CLI frontend)
// ---------------------------------------------------------------------------

/// Format a MonitorEntry as a single line for CLI output.
pub fn format_entry_line(entry: &MonitorEntry) -> String {
    // Extract time portion from timestamp (HH:MM:SS)
    let time = if entry.timestamp.len() >= 19 {
        &entry.timestamp[11..19]
    } else {
        &entry.timestamp
    };

    let input_summary = summarize_input(&entry.tool_name, &entry.tool_input);

    let providers_str: Vec<String> = entry
        .providers
        .iter()
        .map(|p| format!("{}:{}({}ms)", p.name, p.vote, p.response_time_ms))
        .collect();

    format!(
        "[{}] {}  {}({})  \u{2192} {}  [{}]",
        time,
        entry.hook_event,
        entry.tool_name,
        input_summary,
        entry.final_decision,
        providers_str.join(" "),
    )
}

/// Brief summary of tool_input for display.
fn summarize_input(tool_name: &str, input: &serde_json::Value) -> String {
    match tool_name {
        "Bash" => input
            .get("command")
            .and_then(|v| v.as_str())
            .map(|cmd| {
                if cmd.len() > 40 {
                    format!("{}...", &cmd[..37])
                } else {
                    cmd.to_string()
                }
            })
            .unwrap_or_else(|| "?".to_string()),
        "Read" | "Write" | "Edit" => input
            .get("file_path")
            .and_then(|v| v.as_str())
            .map(|p| {
                // Show just the filename
                Path::new(p)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(p)
                    .to_string()
            })
            .unwrap_or_else(|| "?".to_string()),
        _ => {
            let json = serde_json::to_string(input).unwrap_or_default();
            if json.len() > 30 {
                format!("{}...", &json[..27])
            } else {
                json
            }
        }
    }
}

/// Format a stats summary block for CLI periodic output.
pub fn format_stats_block(state: &MonitorState) -> String {
    let mut lines = Vec::new();
    lines.push(format!("--- Stats ({} requests) ---", state.total_requests));

    // Decisions
    for dec in &["allow", "deny", "passthrough"] {
        let count = state.decisions.get(*dec).copied().unwrap_or(0);
        let pct = if state.total_requests > 0 {
            (count as f64 / state.total_requests as f64) * 100.0
        } else {
            0.0
        };
        lines.push(format!("  {:<14} {:>4} ({:.0}%)", format!("{dec}:"), count, pct));
    }

    // Top tools
    let mut tools: Vec<_> = state.tools.iter().collect();
    tools.sort_by(|a, b| b.1.total.cmp(&a.1.total));
    if !tools.is_empty() {
        lines.push("  Tools:".to_string());
        for (name, ts) in tools.iter().take(5) {
            lines.push(format!("    {}: {}", name, ts.total));
        }
    }

    // Auto-approval candidates
    let candidates = state.auto_approval_candidates();
    if !candidates.is_empty() {
        lines.push("  Auto-approval candidates:".to_string());
        for (pattern, ps) in candidates.iter().take(3) {
            lines.push(format!("    {} ({} requests)", pattern, ps.total));
        }
    }

    lines.push("---".to_string());
    lines.join("\n")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn make_jsonl_entry(
        tool_name: &str,
        decision: &str,
        command: &str,
        providers: &[(&str, &str, u64)],
        timestamp: &str,
    ) -> String {
        let providers_json: Vec<serde_json::Value> = providers
            .iter()
            .map(|(name, vote, ms)| {
                serde_json::json!({
                    "name": name,
                    "vote": vote,
                    "mode": "vote",
                    "response_time_ms": ms,
                })
            })
            .collect();

        let entry = serde_json::json!({
            "timestamp": timestamp,
            "hook_event": "PreToolUse",
            "tool_name": tool_name,
            "tool_input": {"command": command},
            "providers": providers_json,
            "final_decision": decision,
            "total_time_ms": 50,
        });
        serde_json::to_string(&entry).unwrap()
    }

    fn write_audit_file(dir: &Path, filename: &str, lines: &[String]) {
        let path = dir.join(filename);
        let mut f = fs::File::create(&path).unwrap();
        for line in lines {
            writeln!(f, "{line}").unwrap();
        }
    }

    #[test]
    fn watcher_reads_existing_files() {
        let dir = TempDir::new().unwrap();
        let lines = vec![
            make_jsonl_entry("Bash", "allow", "ls", &[("checker", "allow", 30)], "2026-03-27T10:00:00Z"),
            make_jsonl_entry("Bash", "deny", "rm -rf /", &[("checker", "deny", 20)], "2026-03-27T10:01:00Z"),
        ];
        write_audit_file(dir.path(), "audit-2026-03-27.jsonl", &lines);

        let watcher = LogWatcher::new(dir.path(), Duration::from_secs(1));
        let history = watcher.read_history();

        assert_eq!(history.len(), 2);
        assert_eq!(history[0].tool_name, "Bash");
        assert_eq!(history[0].final_decision, "allow");
        assert_eq!(history[1].final_decision, "deny");
    }

    #[test]
    fn state_ingests_entries() {
        let mut state = MonitorState::new(100);
        let entry = MonitorEntry {
            timestamp: "2026-03-27T10:00:00Z".to_string(),
            hook_event: "PreToolUse".to_string(),
            tool_name: "Bash".to_string(),
            tool_input: serde_json::json!({"command": "ls"}),
            session_id: None,
            tool_use_id: None,
            tool_result_summary: None,
            providers: vec![ProviderInfo {
                name: "checker".to_string(),
                vote: "allow".to_string(),
                mode: "vote".to_string(),
                response_time_ms: 30,
            }],
            final_decision: "allow".to_string(),
            total_time_ms: 35,
        };

        state.ingest(&entry);

        assert_eq!(state.total_requests, 1);
        assert_eq!(state.recent.len(), 1);
        assert_eq!(state.tools["Bash"].total, 1);
        assert_eq!(state.providers["checker"].invocations, 1);
        assert_eq!(state.providers["checker"].total_time_ms, 30);
    }

    #[test]
    fn state_tracks_decisions() {
        let mut state = MonitorState::new(100);

        for decision in &["allow", "allow", "deny", "passthrough"] {
            let entry = MonitorEntry {
                timestamp: "2026-03-27T10:00:00Z".to_string(),
                hook_event: "PreToolUse".to_string(),
                tool_name: "Bash".to_string(),
                tool_input: serde_json::json!({"command": "ls"}),
                session_id: None,
                tool_use_id: None,
                tool_result_summary: None,
                providers: vec![],
                final_decision: decision.to_string(),
                total_time_ms: 10,
            };
            state.ingest(&entry);
        }

        assert_eq!(state.total_requests, 4);
        assert_eq!(state.decisions["allow"], 2);
        assert_eq!(state.decisions["deny"], 1);
        assert_eq!(state.decisions["passthrough"], 1);
    }

    #[test]
    fn state_tracks_providers() {
        let mut state = MonitorState::new(100);

        let entry = MonitorEntry {
            timestamp: "2026-03-27T10:00:00Z".to_string(),
            hook_event: "PreToolUse".to_string(),
            tool_name: "Bash".to_string(),
            tool_input: serde_json::json!({"command": "ls"}),
            session_id: None,
            tool_use_id: None,
            tool_result_summary: None,
            providers: vec![
                ProviderInfo {
                    name: "checker".to_string(),
                    vote: "allow".to_string(),
                    mode: "vote".to_string(),
                    response_time_ms: 30,
                },
                ProviderInfo {
                    name: "policy".to_string(),
                    vote: "allow".to_string(),
                    mode: "vote".to_string(),
                    response_time_ms: 100,
                },
            ],
            final_decision: "allow".to_string(),
            total_time_ms: 105,
        };
        state.ingest(&entry);

        // Second entry with higher time for checker
        let entry2 = MonitorEntry {
            timestamp: "2026-03-27T10:01:00Z".to_string(),
            hook_event: "PreToolUse".to_string(),
            tool_name: "Bash".to_string(),
            tool_input: serde_json::json!({"command": "pwd"}),
            session_id: None,
            tool_use_id: None,
            tool_result_summary: None,
            providers: vec![ProviderInfo {
                name: "checker".to_string(),
                vote: "allow".to_string(),
                mode: "vote".to_string(),
                response_time_ms: 50,
            }],
            final_decision: "allow".to_string(),
            total_time_ms: 55,
        };
        state.ingest(&entry2);

        let checker = &state.providers["checker"];
        assert_eq!(checker.invocations, 2);
        assert_eq!(checker.total_time_ms, 80); // 30 + 50
        assert_eq!(checker.max_time_ms, 50);
        assert_eq!(checker.avg_time_ms(), 40); // 80 / 2

        let policy = &state.providers["policy"];
        assert_eq!(policy.invocations, 1);
        assert_eq!(policy.max_time_ms, 100);
    }

    #[test]
    fn state_identifies_patterns() {
        let mut state = MonitorState::new(100);

        // 5 "ls *" commands, all allowed => candidate
        for i in 0..5 {
            let entry = MonitorEntry {
                timestamp: format!("2026-03-27T10:{:02}:00Z", i),
                hook_event: "PreToolUse".to_string(),
                tool_name: "Bash".to_string(),
                tool_input: serde_json::json!({"command": format!("ls -la {}", i)}),
                session_id: None,
                tool_use_id: None,
                tool_result_summary: None,
                providers: vec![],
                final_decision: "allow".to_string(),
                total_time_ms: 10,
            };
            state.ingest(&entry);
        }

        // 2 "rm *" commands, all allowed => NOT candidate (< 3)
        for _ in 0..2 {
            let entry = MonitorEntry {
                timestamp: "2026-03-27T10:10:00Z".to_string(),
                hook_event: "PreToolUse".to_string(),
                tool_name: "Bash".to_string(),
                tool_input: serde_json::json!({"command": "rm file.txt"}),
                session_id: None,
                tool_use_id: None,
                tool_result_summary: None,
                providers: vec![],
                final_decision: "allow".to_string(),
                total_time_ms: 10,
            };
            state.ingest(&entry);
        }

        let candidates = state.auto_approval_candidates();
        // Only "ls *" should qualify (5 >= 3)
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].0, "ls *");
        assert_eq!(candidates[0].1.total, 5);
    }

    #[test]
    fn normalize_tool_input_bash_extracts_first_word() {
        let input = serde_json::json!({"command": "git status --short"});
        assert_eq!(normalize_tool_input("Bash", &input), "git *");
    }

    #[test]
    fn normalize_tool_input_read_normal_path() {
        let input = serde_json::json!({"file_path": "/home/user/foo.rs"});
        assert_eq!(normalize_tool_input("Read", &input), "Read(*)");
    }

    #[test]
    fn normalize_tool_input_write_sensitive_path() {
        let input = serde_json::json!({"file_path": "/etc/passwd"});
        assert_eq!(normalize_tool_input("Write", &input), "Write(/etc/passwd)");
    }

    #[test]
    fn normalize_tool_input_edit_normal_path() {
        let input = serde_json::json!({"file_path": "/home/user/main.rs"});
        assert_eq!(normalize_tool_input("Edit", &input), "Edit(*)");
    }

    #[test]
    fn normalize_tool_input_unknown_tool() {
        let input = serde_json::json!({"something": "else"});
        assert_eq!(normalize_tool_input("CustomTool", &input), "CustomTool(*)");
    }

    #[test]
    fn normalize_tool_input_bash_empty_command() {
        let input = serde_json::json!({});
        assert_eq!(normalize_tool_input("Bash", &input), "?");
    }

    #[test]
    fn normalize_tool_input_read_no_file_path() {
        let input = serde_json::json!({"other": "field"});
        assert_eq!(normalize_tool_input("Read", &input), "Read(?)");
    }

    #[test]
    fn state_skips_post_tool_use_events() {
        let mut state = MonitorState::new(100);
        let entry = MonitorEntry {
            timestamp: "2026-03-27T10:00:00Z".to_string(),
            hook_event: "PostToolUse".to_string(),
            tool_name: "Bash".to_string(),
            tool_input: serde_json::json!({}),
            session_id: None,
            tool_use_id: None,
            tool_result_summary: None,
            providers: vec![],
            final_decision: "allow".to_string(),
            total_time_ms: 10,
        };
        state.ingest(&entry);

        assert_eq!(state.total_requests, 0);
        assert!(state.recent.is_empty());
    }

    #[test]
    fn state_recent_buffer_respects_limit() {
        let mut state = MonitorState::new(3);

        for i in 0..5 {
            let entry = MonitorEntry {
                timestamp: format!("2026-03-27T10:{:02}:00Z", i),
                hook_event: "PreToolUse".to_string(),
                tool_name: "Bash".to_string(),
                tool_input: serde_json::json!({"command": "ls"}),
                session_id: None,
                tool_use_id: None,
                tool_result_summary: None,
                providers: vec![],
                final_decision: "allow".to_string(),
                total_time_ms: 10,
            };
            state.ingest(&entry);
        }

        assert_eq!(state.recent.len(), 3);
        // Should have the last 3 entries
        assert_eq!(state.recent[0].timestamp, "2026-03-27T10:02:00Z");
        assert_eq!(state.recent[2].timestamp, "2026-03-27T10:04:00Z");
    }

    #[test]
    fn state_tracks_time_range() {
        let mut state = MonitorState::new(100);

        for ts in &["2026-03-27T10:05:00Z", "2026-03-27T09:00:00Z", "2026-03-27T11:30:00Z"] {
            let entry = MonitorEntry {
                timestamp: ts.to_string(),
                hook_event: "PreToolUse".to_string(),
                tool_name: "Bash".to_string(),
                tool_input: serde_json::json!({"command": "ls"}),
                session_id: None,
                tool_use_id: None,
                tool_result_summary: None,
                providers: vec![],
                final_decision: "allow".to_string(),
                total_time_ms: 10,
            };
            state.ingest(&entry);
        }

        let (start, end) = state.time_range.as_ref().unwrap();
        assert_eq!(start, "2026-03-27T09:00:00Z");
        assert_eq!(end, "2026-03-27T11:30:00Z");
    }

    #[test]
    fn watcher_reads_multiple_files_sorted() {
        let dir = TempDir::new().unwrap();

        let day1 = vec![
            make_jsonl_entry("Bash", "allow", "ls", &[], "2026-03-26T10:00:00Z"),
        ];
        let day2 = vec![
            make_jsonl_entry("Read", "allow", "cat", &[], "2026-03-27T10:00:00Z"),
        ];
        write_audit_file(dir.path(), "audit-2026-03-27.jsonl", &day2);
        write_audit_file(dir.path(), "audit-2026-03-26.jsonl", &day1);

        let watcher = LogWatcher::new(dir.path(), Duration::from_secs(1));
        let history = watcher.read_history();

        assert_eq!(history.len(), 2);
        // Should be sorted by file name (date order)
        assert_eq!(history[0].tool_name, "Bash");
        assert_eq!(history[1].tool_name, "Read");
    }

    #[test]
    fn watcher_skips_truncation_sentinels() {
        let dir = TempDir::new().unwrap();
        let lines = vec![
            r#"{"_truncated":true,"timestamp":"2026-03-27T10:00:00Z","lines_removed":50}"#.to_string(),
            make_jsonl_entry("Bash", "allow", "ls", &[], "2026-03-27T10:01:00Z"),
        ];
        write_audit_file(dir.path(), "audit-2026-03-27.jsonl", &lines);

        let watcher = LogWatcher::new(dir.path(), Duration::from_secs(1));
        let history = watcher.read_history();

        assert_eq!(history.len(), 1);
        assert_eq!(history[0].final_decision, "allow");
    }

    #[test]
    fn watcher_skips_malformed_lines() {
        let dir = TempDir::new().unwrap();
        let lines = vec![
            "not json".to_string(),
            r#"{"partial": true}"#.to_string(),
            make_jsonl_entry("Bash", "allow", "ls", &[], "2026-03-27T10:00:00Z"),
        ];
        write_audit_file(dir.path(), "audit-2026-03-27.jsonl", &lines);

        let watcher = LogWatcher::new(dir.path(), Duration::from_secs(1));
        let history = watcher.read_history();

        assert_eq!(history.len(), 1);
    }

    #[test]
    fn format_entry_line_output() {
        let entry = MonitorEntry {
            timestamp: "2026-03-27T10:05:30Z".to_string(),
            hook_event: "PreToolUse".to_string(),
            tool_name: "Bash".to_string(),
            tool_input: serde_json::json!({"command": "ls -la"}),
            session_id: None,
            tool_use_id: None,
            tool_result_summary: None,
            providers: vec![ProviderInfo {
                name: "checker".to_string(),
                vote: "allow".to_string(),
                mode: "vote".to_string(),
                response_time_ms: 30,
            }],
            final_decision: "allow".to_string(),
            total_time_ms: 35,
        };

        let line = format_entry_line(&entry);
        assert!(line.contains("[10:05:30]"));
        assert!(line.contains("Bash(ls -la)"));
        assert!(line.contains("allow"));
        assert!(line.contains("checker:allow(30ms)"));
    }

    #[test]
    fn provider_stats_avg_time() {
        let ps = ProviderStats {
            invocations: 4,
            total_time_ms: 200,
            max_time_ms: 80,
            errors: 0,
        };
        assert_eq!(ps.avg_time_ms(), 50);
    }

    #[test]
    fn provider_stats_avg_time_zero_invocations() {
        let ps = ProviderStats::default();
        assert_eq!(ps.avg_time_ms(), 0);
    }

    #[test]
    fn pattern_stats_allow_rate() {
        let ps = PatternStats {
            total: 10,
            allowed: 8,
        };
        assert!((ps.allow_rate() - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn pattern_stats_allow_rate_zero() {
        let ps = PatternStats::default();
        assert!((ps.allow_rate() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn auto_approval_candidates_excludes_mixed_decisions() {
        let mut state = MonitorState::new(100);

        // 5 "ls" commands: 4 allow, 1 deny => NOT a candidate
        for i in 0..5 {
            let decision = if i == 4 { "deny" } else { "allow" };
            let entry = MonitorEntry {
                timestamp: format!("2026-03-27T10:{:02}:00Z", i),
                hook_event: "PreToolUse".to_string(),
                tool_name: "Bash".to_string(),
                tool_input: serde_json::json!({"command": format!("ls {}", i)}),
                session_id: None,
                tool_use_id: None,
                tool_result_summary: None,
                providers: vec![],
                final_decision: decision.to_string(),
                total_time_ms: 10,
            };
            state.ingest(&entry);
        }

        let candidates = state.auto_approval_candidates();
        assert!(candidates.is_empty());
    }
}
