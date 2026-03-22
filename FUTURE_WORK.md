# Future Work — claude-pretool-sidecar

*Last updated: 2026-03-22*

## Phase 2: Plugin & Skills

* [ ] Design and build Claude Code Plugin (`plugin.json`, hooks, skills, resources)
* [ ] Plugin auto-installs PreToolUse/PostToolUse hooks when activated
* [ ] Skills for config management (find, validate, modify sidecar config)
* [ ] Skills for troubleshooting (diagnose broken config, suggest fixes)
* [ ] Skills for issue filing (`gh issue create` against this repo)
* [ ] Bundled resources (config schema reference, examples, troubleshooting guide)
* [ ] Plugin validation and testing

## Phase 3: PostToolUse Integration

* [ ] PostToolUse hook companion — correlates pre-tool requests with actual executions
* [ ] Audit trail: which tools were requested, which were approved, which actually ran
* [ ] Analytics/summary output (session summary of tool usage)

## Phase 4: Advanced Voting & Providers

* [ ] Async provider support (providers that need more time)
* [ ] Provider timeout configuration (per-provider)
* [ ] Provider priority/weight system
* [ ] Caching layer (same tool+input = cached decision within TTL)
* [ ] Provider health monitoring

## Phase 5: MCP Permission Tool Integration

* [ ] Research if `--permission-tool` MCP flag exists or is planned
* [ ] If available: build MCP server wrapper around sidecar
* [ ] Alternative integration path for users who prefer MCP over hooks

## Phase 6: Ecosystem & Distribution

* [ ] Companion tools collection (additional composable scripts/binaries)
* [ ] Example provider scripts (bash, python, node examples)
* [ ] Packaging (cargo install, AUR, brew, nix)
* [ ] Integration with shibuido super-repository (git submodule)
* [ ] Documentation site or comprehensive README

## Ideas / Parking Lot

* Web dashboard for real-time tool approval monitoring
* Notification integration (desktop notifications for pending approvals)
* Rule-based auto-approval engine (regex patterns for tool+input matching)
* Multi-session coordination (shared state across Claude Code sessions)
* Rate limiting per tool type
