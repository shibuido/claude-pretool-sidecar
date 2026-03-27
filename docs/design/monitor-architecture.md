# Design: Real-Time Monitor — Shared Core + Multiple Frontends

*Date: 2026-03-27*

## Problem

Users want to watch tool approval activity in real time. Different contexts need different UIs: quick CLI tail, rich terminal dashboard, or browser-accessible web dashboard.

## Architecture

```
audit-YYYY-MM-DD.jsonl files
       │ (filesystem watch)
       ▼
┌─────────────────────────┐
│   src/monitor.rs        │  ← shared core library
│                         │
│   LogWatcher            │  watches directory, yields new entries
│   MonitorState          │  running stats, recent events, patterns
│   EventStream           │  push-based update channel
│   AuditEntry (reused)   │  from src/audit.rs
└────┬──────┬──────┬──────┘
     │      │      │
     ▼      ▼      ▼
    CLI    TUI    WebUI
```

## Shared Core: `src/monitor.rs`

### LogWatcher

Watches an audit log directory for changes. When new lines appear in any `audit-*.jsonl` file, parses them and yields `AuditEntry` values.

```rust
pub struct LogWatcher {
    dir: PathBuf,
    poll_interval: Duration,  // default 500ms
}

impl LogWatcher {
    pub fn new(dir: &Path) -> Self;
    /// Blocking iterator — yields entries as they appear in log files.
    pub fn watch(&self) -> impl Iterator<Item = AuditEntry>;
    /// Non-blocking — read all existing entries then return.
    pub fn read_history(&self) -> Vec<AuditEntry>;
}
```

Implementation: poll-based (check file sizes, read new bytes). No inotify dependency needed — simple and cross-platform.

### MonitorState

Accumulates statistics from a stream of audit entries.

```rust
pub struct MonitorState {
    pub total_requests: u64,
    pub decisions: HashMap<String, u64>,    // "allow" → count
    pub tools: HashMap<String, ToolStats>,  // "Bash" → {allow, deny, pass}
    pub providers: HashMap<String, ProviderStats>,  // "checker" → {avg_ms, max_ms, errors}
    pub recent: VecDeque<AuditEntry>,       // last N entries (configurable)
    pub patterns: Vec<PatternCandidate>,    // auto-approval candidates
    pub time_range: (String, String),       // first..last timestamp
}

impl MonitorState {
    pub fn new(recent_limit: usize) -> Self;
    pub fn ingest(&mut self, entry: &AuditEntry);
    pub fn auto_approval_candidates(&self) -> Vec<&PatternCandidate>;
}
```

### Supporting Types

```rust
pub struct ToolStats {
    pub allow: u64, pub deny: u64, pub passthrough: u64,
}

pub struct ProviderStats {
    pub total_calls: u64, pub total_ms: u64, pub max_ms: u64, pub errors: u64,
}

pub struct PatternCandidate {
    pub pattern: String,     // e.g., "Bash(ls *)"
    pub count: u64,
    pub allow_rate: f64,     // 0.0–1.0
}
```

## Binary: `claude-pretool-monitor`

Single binary with subcommands:

```
claude-pretool-monitor cli  /path/to/audit-dir    # Live tail with stats
claude-pretool-monitor tui  /path/to/audit-dir    # Terminal dashboard
claude-pretool-monitor web  /path/to/audit-dir    # HTTP dashboard (future)
claude-pretool-monitor history /path/to/audit-dir  # One-shot analysis (like analyzer)
```

## CLI Frontend

Live tail with periodic stats summary:

```
[14:30:01] PreToolUse  Bash(ls -la)     → allow  [security:45ms policy:120ms]
[14:30:05] PreToolUse  Write(/tmp/f)    → DENY   [security:30ms "sensitive path"]
[14:30:08] PostToolUse Bash(ls -la)     → logged

--- Stats (12 requests, last 60s) ---
  Allow: 9 (75%)  Deny: 2 (17%)  Pass: 1 (8%)
  Providers: security(avg 40ms) policy(avg 90ms, 1 error)
  Candidates: Bash(ls *) 100% allowed, Read(*) 100% allowed
```

Stats printed every N seconds (default 30) or on Ctrl+S.

## TUI Frontend (ratatui)

```
┌─ Live Events ───────────────────────────────────────────────┐
│ 14:30:01 PreToolUse  Bash(ls -la)      allow    165ms      │
│ 14:30:05 PreToolUse  Write(/tmp/f)     DENY     30ms       │
│ 14:30:08 PostToolUse Bash(ls -la)      logged   1ms        │
│ 14:30:12 PreToolUse  Bash(git status)  allow    88ms       │
│                                                             │
├─ Decisions ──────────────────┬─ Providers ──────────────────┤
│  Allow:       9 (75%)  ████ │  security    avg: 40ms  e:0  │
│  Deny:        2 (17%)  ██   │  policy      avg: 90ms  e:1  │
│  Passthrough: 1 (8%)   █    │                               │
├─ Top Tools ──────────────────┤─ Auto-Approve Candidates ────┤
│  Bash:  8  (6/1/1)          │  Bash(ls *):  8×  100%       │
│  Read:  5  (5/0/0)          │  Read(*):     5×  100%       │
│  Write: 2  (1/1/0)          │  Bash(git *): 3×  100%       │
└──────────────────────────────┴──────────────────────────────┘
  q:quit  ↑↓:scroll  r:reset stats  p:pause
```

Dependencies: `ratatui`, `crossterm`

## WebUI Frontend (Future)

Minimal embedded HTTP server serving a static HTML page that polls `/api/state` JSON endpoint. No JavaScript framework — plain HTML + fetch().

Dependencies: `tiny-http` or raw `std::net::TcpListener`

## Implementation Plan

1. Core module (`src/monitor.rs`) — LogWatcher, MonitorState, types
2. Binary skeleton (`src/bin/monitor.rs`) — clap subcommands
3. CLI frontend — live tail rendering
4. TUI frontend — ratatui dashboard
5. WebUI frontend — deferred to separate task

## Dependencies to Add

```toml
ratatui = { version = "0.29", optional = true }
crossterm = { version = "0.28", optional = true }

[features]
default = ["tui"]
tui = ["ratatui", "crossterm"]
```

Making TUI optional via feature flag keeps the base binary lean.
