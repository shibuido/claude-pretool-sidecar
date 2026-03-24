# Future Work — claude-pretool-sidecar

*Last updated: 2026-03-24*

## Phase 2: Claude Code Plugin & Skills

* [ ] Design and create plugin structure (.claude-plugin/plugin.json, hooks/hooks.json)
* [ ] Plugin auto-installs PreToolUse/PostToolUse hooks when activated
* [ ] Skill: configure-sidecar — find, create, validate, modify config files
* [ ] Skill: diagnose-sidecar — troubleshoot config/provider/hook issues
* [ ] Skill: file-issue — formulate and file GitHub issues via `gh` CLI
* [ ] Bundle resources: config schema, example configs, troubleshooting guide
* [ ] Plugin validation and testing with --plugin-dir

## Phase 3: PostToolUse Integration

* [ ] --post-tool CLI flag for PostToolUse audit-only mode
* [ ] Correlate PostToolUse with PreToolUse logs (session_id + tool_use_id)
* [ ] Audit trail: which tools requested → approved → actually ran
* [ ] Session summary analytics output (tool usage report)

## Phase 4: Advanced Voting & Providers

* [ ] Provider caching layer (same tool+input = cached decision within TTL)
* [ ] Provider priority/weight system
* [ ] Provider health monitoring (error rate tracking, auto-disable)
* [ ] Async provider support (providers that need more time)

## Phase 5: MCP Permission Tool Integration

* [ ] Investigate --permission-prompt-tool as alternative integration path
* [ ] Build MCP server wrapper around sidecar (if viable)

## Phase 6: Ecosystem & Distribution

* [ ] Example provider scripts (bash, python, node)
* [ ] Packaging: cargo install (crates.io), AUR, brew formula
* [ ] Integration with shibuido super-repository (git submodule + symlinks)
* [ ] Comprehensive README.md with quick start, examples, architecture

## Ideas / Parking Lot

* Rule-based auto-approval engine (regex patterns for tool+input matching)
* Desktop notifications for pending approvals (notify-send/osascript)
* Web dashboard for real-time tool approval monitoring
* Multi-session coordination (shared state across Claude Code sessions)
* Rate limiting per tool type
