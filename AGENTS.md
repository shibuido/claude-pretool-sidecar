# AGENTS.md — claude-pretool-sidecar

## Project Overview

A composable sidecar for Claude Code's PreToolUse and PostToolUse hooks that aggregates tool-approval votes from multiple external providers. Includes a built-in rules engine, decision caching, provider health monitoring, session analytics, and desktop notifications. Built in Rust, following KISS/Unix philosophy.

## Architecture

```
Claude Code (PreToolUse / PostToolUse hook)
    │ JSON on stdin
    ▼
claude-pretool-sidecar
    │
    ├── CLI parsing (--config, --validate, --post-tool, --passthrough)
    ├── Config loading + CPTS_* env overrides
    ├── Rules engine (fast-path regex matching)
    ├── Decision cache lookup (file-based, per-session)
    ├── Health check (skip disabled providers)
    ├── Fan out to healthy providers via stdio
    │   ├── provider-1 (vote: allow/deny/passthrough, weight: N)
    │   ├── provider-2 (vote)
    │   └── logger (fyi: output ignored)
    ├── Weighted quorum aggregation
    ├── Cache store
    ├── Health state update
    ├── Audit log append
    │
    ▼ JSON on stdout
Claude Code (receives decision)
```

## Binaries (4 total)

* `claude-pretool-sidecar` — Main sidecar: stdin → rules → cache → providers → quorum → stdout
* `claude-pretool-logger` — FYI provider that logs hook payloads to file or stderr
* `claude-pretool-analyzer` — Reads audit JSONL files, prints session analytics (decisions, tools, providers, correlation, auto-approval candidates)
* `claude-pretool-notifier` — FYI provider that sends desktop notifications (notify-send / osascript)

## Key Files

* `src/main.rs` — Entry point: cli → config → rules → cache → health → providers → quorum → audit
* `src/cli.rs` — CLI argument parsing (clap): --config, --validate, --post-tool, --passthrough, --version
* `src/config.rs` — TOML config parsing, CPTS_* env overrides, validation
* `src/hook.rs` — Hook event/response types (PreToolUse, PostToolUse, tool_use_id, tool_result)
* `src/quorum.rs` — Vote aggregation algorithm (weighted)
* `src/provider.rs` — External process execution, weighted vote extraction
* `src/audit.rs` — Audit logging with rotation, tool_result summarization
* `src/rules.rs` — Built-in regex rule engine (fast-path shortcut before providers)
* `src/cache.rs` — File-based decision caching (per-session, TTL-based)
* `src/health.rs` — Provider health monitoring (error rate tracking, auto-disable)
* `src/bin/logger.rs` — FYI logger binary
* `src/bin/analyzer.rs` — Session analytics binary
* `src/bin/notifier.rs` — Desktop notification binary

## Key Directories

* `plugin/` — Claude Code plugin (hooks, skills, scripts, resources)
* `packaging/` — Distribution packaging (AUR PKGBUILD, Homebrew formula, release script)
* `examples/` — Example TOML configs + provider scripts (bash, python)
* `tests/` — Integration tests
* `qa/` — QA test suites (standalone + live Claude Code) and Docker environments

## Design Documents

All design decisions are in `docs/design/`:

* `voting-quorum.md` — Vote aggregation rules (including weighted voting)
* `stdio-protocol.md` — Provider communication protocol
* `configuration.md` — Config file format and locations
* `architecture.md` — System architecture
* `log-rotation.md` — Audit log rotation strategy
* `claude-code-hooks-reference.md` — Claude Code hooks API reference
* `mcp-permission-tool-analysis.md` — MCP permission tool analysis

Guidelines in `docs/guidelines/`:

* `README.md` — Core principles summary
* `testing.md` — Testing philosophy

## Tracking

* `CURRENT_WORK.md` — What's being worked on now
* `FUTURE_WORK.md` — Deferred scope and ideas
* `unstructured/` — Raw notes, voice transcripts

## Development

```bash
cargo test                              # Run all 157 Rust tests
cargo build                             # Build all 4 binaries
cargo run -- --passthrough < payload.json   # Test with a hook payload
cargo run -- --validate --config path.toml  # Validate a config file
qa/scripts/run-all-standalone.sh        # Run ~70 QA shell tests
```

### CLI Flags

| Flag | Description |
|------|-------------|
| `--config <PATH>` | Explicit config file path |
| `--validate` | Validate config and exit |
| `--post-tool` | PostToolUse mode (audit-log only, output `{}`) |
| `--passthrough` | Return passthrough when no config found |
| `--version` | Show version |

## Configuration

Config file: `.claude-pretool-sidecar.toml` (TOML format)
See `docs/design/configuration.md` for full schema.

Config sections: `[quorum]`, `[timeout]`, `[audit]`, `[cache]`, `[health]`, `[[providers]]`, `[[rules]]`

### Environment Variable Overrides (CPTS_* prefix)

* `CPTS_MIN_ALLOW` — overrides `quorum.min_allow`
* `CPTS_MAX_DENY` — overrides `quorum.max_deny`
* `CPTS_ERROR_POLICY` — overrides `quorum.error_policy`
* `CPTS_TIMEOUT` — overrides `timeout.provider_default`
* `CPTS_MAX_LOG_BYTES` — overrides `audit.max_total_bytes`
