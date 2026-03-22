# Current Work — claude-pretool-sidecar

*Last updated: 2026-03-22*

## Phase 1: Foundation (NOW)

### 1.1 Research & Architecture

* [x] Save initial voice transcript
* [ ] Research Claude Code Hooks (PreToolUse, PostToolUse lifecycle, input/output JSON format)
* [ ] Research Claude Code Plugins (structure, manifest, skills, resources, hooks)
* [ ] Research Claude Code Permissions (MCP permission tool flag, settings locations)
* [ ] Synthesize research into architecture document (`docs/design/architecture.md`)

### 1.2 Core Design Documents

* [ ] Voting/quorum design (`docs/design/voting-quorum.md`)
* [ ] Configuration format design (`docs/design/configuration.md`)
* [ ] stdio protocol design (`docs/design/stdio-protocol.md`)
* [ ] Design guidelines summary (`docs/guidelines/README.md`)

### 1.3 Rust Project Setup

* [ ] Initialize Cargo workspace
* [ ] Define crate structure (core sidecar binary + companion tool crates)
* [ ] Update `.gitignore` for Rust artifacts

### 1.4 Core Sidecar Binary (`claude-pretool-sidecar`)

* [ ] Config file parsing (TOML or JSON — TBD)
* [ ] stdin JSON reader (receives hook payload from Claude Code)
* [ ] Provider executor (spawns configured decision-maker scripts via stdio)
* [ ] Vote aggregator (quorum logic: min-allow, max-deny, error handling)
* [ ] stdout JSON writer (returns allow/deny/passthrough to Claude Code)
* [ ] Unit tests for all core logic
* [ ] Integration tests with mock providers

### 1.5 Companion: FYI Logger (`claude-pretool-logger`)

* [ ] Simple stdin-to-file/stdout logger
* [ ] Configurable output format (JSON lines, human-readable)
* [ ] Designed to be used as an FYI provider (output ignored by sidecar)

## Phase 1 Definition of Done

* Core sidecar can be installed as a Claude Code PreToolUse hook
* Reads hook JSON from stdin, fans out to configured providers
* Aggregates votes per quorum rules, returns decision on stdout
* FYI logger companion can be composed alongside decision providers
* All core logic covered by tests (unit + integration)
* Design documents capture key decisions with rationale
