# Current Work — claude-pretool-sidecar

*Last updated: 2026-03-24*

## Phase 1: Foundation — MOSTLY DONE

### 1.1 Research & Architecture — DONE

* [x] Save initial voice transcript
* [x] Research Claude Code Hooks (PreToolUse, PostToolUse, 25+ events, input/output JSON)
* [x] Research Claude Code Plugins (structure, manifest, skills, resources, hooks)
* [x] Research Claude Code Permissions (--permission-prompt-tool, settings locations)
* [x] Synthesize into architecture document (`docs/design/architecture.md`)

### 1.2 Core Design Documents — DONE

* [x] Voting/quorum design (`docs/design/voting-quorum.md`)
* [x] Configuration format design (`docs/design/configuration.md`)
* [x] stdio protocol design (`docs/design/stdio-protocol.md`)
* [x] Claude Code hooks reference (`docs/design/claude-code-hooks-reference.md`)
* [x] Log rotation design (`docs/design/log-rotation.md`)
* [x] Design guidelines summary (`docs/guidelines/README.md`)
* [x] Testing philosophy (`docs/guidelines/testing.md`)

### 1.3 Rust Project — DONE

* [x] Initialize Cargo project with two binary targets
* [x] Core modules: hook.rs, config.rs, quorum.rs, provider.rs, audit.rs
* [x] FYI logger companion: src/bin/logger.rs
* [x] 37 unit tests + 6 integration tests (43 total)
* [x] .gitignore for Rust artifacts

### 1.4 Audit & Logging — DONE

* [x] Built-in audit logging with per-provider timing
* [x] Date-based log file chunking (audit-YYYY-MM-DD.jsonl)
* [x] Log rotation: per-file truncation + total size enforcement
* [x] ISO 8601 timestamps without external dependency

### 1.5 QA Suite — DONE

* [x] 40 automated QA tests across 5 suites (standalone-*)
* [x] Manual test checklist (95+ items)
* [x] Live Claude Code test scripts (live-claude-code-*)
* [x] Docker: Dockerfile.standalone + Dockerfile.claude-code
* [x] Docker management scripts: cpts-standalone.sh, cpts-claude-code.sh
* [x] Helper scripts: gen-payload, gen-config, provider-echo, check-audit-log

### 1.6 Remaining Phase 1 Items

* [ ] Add CLI arg parsing with clap (--config, --validate, --post-tool, --passthrough)
* [ ] Implement proper timeout enforcement for providers (TODO in provider.rs)
* [ ] Implement CPTS_* env var overrides for config
* [ ] Config validation: provider commands exist, quorum sanity checks
* [ ] Create GitHub repo and push
* [ ] Write comprehensive README.md
