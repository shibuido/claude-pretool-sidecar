# AGENTS.md — claude-pretool-sidecar

## Project Overview

A composable sidecar for Claude Code's PreToolUse hook that aggregates tool-approval votes from multiple external providers. Built in Rust, following KISS/Unix philosophy.

## Architecture

```
Claude Code (PreToolUse hook)
    │ JSON on stdin
    ▼
claude-pretool-sidecar
    │ fans out to providers via stdio
    ├── provider-1 (vote: allow/deny/passthrough)
    ├── provider-2 (vote)
    └── logger (fyi: output ignored)
    │
    ▼ quorum aggregation
    │ JSON on stdout
Claude Code (receives decision)
```

## Key Files

* `src/main.rs` — Entry point: stdin → providers → quorum → stdout
* `src/hook.rs` — Hook event/response types
* `src/config.rs` — TOML config parsing
* `src/quorum.rs` — Vote aggregation algorithm
* `src/provider.rs` — External process execution
* `src/bin/logger.rs` — Companion FYI logger binary

## Design Documents

All design decisions are in `docs/design/`:

* `voting-quorum.md` — Vote aggregation rules
* `stdio-protocol.md` — Provider communication protocol
* `configuration.md` — Config file format and locations
* `architecture.md` — System architecture (pending)

Guidelines in `docs/guidelines/`:

* `README.md` — Core principles summary
* `testing.md` — Testing philosophy

## Tracking

* `CURRENT_WORK.md` — What's being worked on now
* `FUTURE_WORK.md` — Deferred scope and ideas
* `unstructured/` — Raw notes, voice transcripts

## Development

```bash
cargo test          # Run all tests
cargo build         # Build binaries
cargo run -- < payload.json  # Test with a hook payload
```

## Configuration

Config file: `.claude-pretool-sidecar.toml` (TOML format)
See `docs/design/configuration.md` for full schema.
