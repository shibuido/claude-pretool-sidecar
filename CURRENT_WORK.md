# Current Work — claude-pretool-sidecar

*Last updated: 2026-03-27*

## Status: All phases through Phase 6 complete

All planned features have been implemented, tested, and pushed to
https://github.com/shibuido/claude-pretool-sidecar

### Binaries (4)

* `claude-pretool-sidecar` — core sidecar (hook → providers → quorum → decision)
* `claude-pretool-logger` — FYI logger companion
* `claude-pretool-analyzer` — session summary analytics
* `claude-pretool-notifier` — desktop notification companion

### Test Totals

* 157 Rust tests (113 unit + 16 analyzer + 8 notifier + 20 integration)
* 70 QA script tests across 8 standalone suites
* **227 total automated tests, all passing**

### Features Implemented

* CLI: --config, --validate, --post-tool, --passthrough, --version (clap)
* Config: TOML parsing, env var overrides (CPTS_*), validation, file discovery
* Providers: stdio protocol, timeout enforcement (wait-timeout), weighted voting
* Quorum: min_allow, max_deny, error_policy, weighted votes
* Audit: date-chunked JSONL, log rotation, PostToolUse correlation (tool_use_id)
* Cache: file-based decision caching per session with TTL
* Health: provider error tracking, auto-disable, session-scoped persistence
* Rules: built-in regex rule engine (fast-path, skips providers)
* Plugin: Claude Code plugin with 3 skills, 3 resources, install/uninstall scripts
* QA: standalone + live-claude-code test suites, 2 Docker environments
* Packaging: LICENSE, AUR PKGBUILD, Homebrew formula, release script
* Examples: 5 provider scripts (bash + python), 5 config templates
* Docs: README, 8 design docs, QA checklists
