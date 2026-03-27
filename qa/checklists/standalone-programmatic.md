# Programmatic QA Test Checklist

*Last updated: 2026-03-27*

This document maps automated test scripts to the features they cover. Each script in `qa/scripts/` is listed with its test cases.

## Test Coverage Map

### `cargo test` — Unit + Integration Tests (Rust)

| Module | Tests | Coverage |
|--------|-------|----------|
| `hook.rs` | 6 | Input parsing, response serialization, passthrough format |
| `config.rs` | 4 | TOML parsing, defaults, provider modes |
| `quorum.rs` | 12 | All quorum scenarios: single/multi/error/edge cases |
| `provider.rs` | 7 | Response parsing: allow/deny/passthrough/error/whitespace |
| `audit.rs` | 6 | Entry serialization, date chunking, truncation, rotation |
| `integration_test.rs` | 6 | End-to-end binary with mock providers |

### `qa/scripts/test-config.sh` — Config Loading Tests

| # | Test | Checks |
|---|------|--------|
| 1 | Env var config loading | `CLAUDE_PRETOOL_SIDECAR_CONFIG` picks up file |
| 2 | CWD config loading | `.claude-pretool-sidecar.toml` in current dir |
| 3 | Missing config error | Clear error message when no config found |
| 4 | Invalid TOML error | Parse errors produce helpful messages |
| 5 | Minimal config defaults | Missing fields get correct defaults |
| 6 | Full config parsing | All fields parsed without error |
| 7 | Example configs valid | All `examples/*.toml` parse successfully |

### `qa/scripts/test-providers.sh` — Provider Execution Tests

| # | Test | Checks |
|---|------|--------|
| 1 | Allow provider | Returns allow via hookSpecificOutput |
| 2 | Deny provider | Returns deny with permissionDecision |
| 3 | Passthrough provider | Returns empty JSON `{}` |
| 4 | Crash provider | Non-zero exit → error handling |
| 5 | Bad JSON provider | Invalid stdout → error |
| 6 | FYI provider ignored | FYI deny doesn't affect decision |
| 7 | Provider env vars | Custom env vars passed to provider |
| 8 | Provider args | CLI args passed correctly |
| 9 | Provider stdin content | Provider receives full hook payload |

### `qa/scripts/test-quorum.sh` — Quorum Logic Tests

| # | Test | Checks |
|---|------|--------|
| 1 | Single allow quorum | min_allow=1, 1 allow → allow |
| 2 | Single deny blocks | max_deny=0, 1 deny → deny |
| 3 | Deny priority | Enough allows but deny exceeds → deny |
| 4 | Tolerated deny | max_deny=1, deny tolerated → allow |
| 5 | Quorum not met | Not enough allows → default_decision |
| 6 | Error as deny | error_policy=deny, crash → deny |
| 7 | Error as passthrough | error_policy=passthrough, crash → no effect |
| 8 | Zero providers | min_allow=0 → allow |
| 9 | Mixed providers | 2 allow + 1 deny + 1 FYI → correct result |

### `qa/scripts/test-audit.sh` — Audit Logging Tests

| # | Test | Checks |
|---|------|--------|
| 1 | Audit disabled | No log file created when disabled |
| 2 | Audit to stderr | Entries appear on stderr |
| 3 | Audit to directory | Date-chunked file created |
| 4 | Entry format | JSON with all required fields |
| 5 | Provider timing | response_time_ms is positive |
| 6 | Log rotation truncate | File truncated when exceeding max_file_bytes |
| 7 | Log rotation delete | Oldest files deleted when total exceeds max_total_bytes |
| 8 | Sentinel line | Truncated file starts with _truncated sentinel |
| 9 | Recent lines kept | Most recent entries preserved after truncation |

### `qa/scripts/test-hook-integration.sh` — Claude Code Compliance

| # | Test | Checks |
|---|------|--------|
| 1 | Allow format | Output matches Claude Code hookSpecificOutput schema |
| 2 | Deny format | permissionDecision="deny" + optional systemMessage |
| 3 | Passthrough format | Empty object `{}` |
| 4 | Input parsing | All Claude Code fields parsed (tool_name, tool_input, etc.) |
| 5 | Unknown fields | Extra input fields don't cause errors |
| 6 | Realistic payloads | Bash, Write, Edit, Read payloads all work |
| 7 | Exit code | Always exits 0 on successful processing |

### `qa/scripts/standalone-plugin-structure.sh` — Plugin Structure Validation

| # | Test | Checks |
|---|------|--------|
| 1 | plugin.json valid | Valid JSON with name and description fields |
| 2 | hooks.json valid | Valid JSON with top-level `hooks` key |
| 3 | SKILL.md frontmatter | All SKILL.md files have YAML frontmatter with name and description |
| 4 | Expected skills exist | configure-sidecar, diagnose-sidecar, file-issue directories present |
| 5 | Scripts executable | All plugin/scripts/*.sh are executable |
| 6 | Resources non-empty | config-schema.md, hook-setup.md, troubleshooting.md exist and non-empty |
| 7 | No broken refs | Resource path references in SKILL.md files resolve correctly |
| 8 | README exists | plugin/README.md present and non-empty |

### `qa/scripts/standalone-plugin-hooks.sh` — Plugin Hook Configuration

| # | Test | Checks |
|---|------|--------|
| 1 | PreToolUse matcher | Matcher is `*` (matches all tools) |
| 2 | PostToolUse matcher | Matcher is `*` (matches all tools) |
| 3 | PreToolUse command | References `claude-pretool-sidecar` |
| 4 | PostToolUse command | References `claude-pretool-sidecar` |
| 5 | PostToolUse flag | Command includes `--post-tool` flag |
| 6 | SessionStart exists | SessionStart hook present, references check-sidecar.sh |
| 7 | PreToolUse timeout | Timeout within 5-30s range |
| 8 | PostToolUse timeout | Timeout within 5-30s range |
| 9 | SessionStart timeout | Timeout within 1-10s range |
| 10 | Hook types | All hook types are `command` |
| 11 | Event count | Exactly 3 hook events (SessionStart, PreToolUse, PostToolUse) |

### `qa/scripts/standalone-plugin-scripts.sh` — Plugin Scripts Validation

| # | Test | Checks |
|---|------|--------|
| 1 | check-sidecar runs | check-sidecar.sh exits 0 (or skipped if binary not in PATH) |
| 2 | check-sidecar quiet | `--quiet` suppresses non-error output |
| 3 | check-sidecar help | `--help` exits 0 and documents flags |
| 4 | install creates JSON | install-hooks.sh --scope project creates valid settings with hooks |
| 5 | install idempotent | Running install twice does not duplicate hooks |
| 6 | install preserves | Existing hooks preserved when installing sidecar hooks |
| 7 | install help | `--help` exits 0 and documents --scope |
| 8 | uninstall removes | uninstall-hooks.sh removes sidecar hooks from settings |
| 9 | uninstall preserves | Non-sidecar hooks preserved after uninstall |
| 10 | uninstall missing file | Exits 0 gracefully when settings file does not exist |
| 11 | uninstall help | `--help` exits 0 and documents --scope |

### `qa/scripts/live-claude-code-hook-install.sh` — Hook Installation (requires CC CLI)

| # | Test | Checks |
|---|------|--------|
| 1 | Valid hook settings | Generated settings.local.json is valid JSON |
| 2 | CC accepts settings | Claude Code starts without errors |
| 3 | Dual-hook config | Pre + Post hook settings valid |
| 4 | Audit-enabled hook | Config with audit logging generates correctly |
| 5 | Binary accessible | Sidecar binary is executable from hook context |

### `qa/scripts/live-claude-code-hook-execution.sh` — Hook Execution (requires CC CLI + API key)

| # | Test | Checks |
|---|------|--------|
| 1 | Hook invocation | Sidecar invoked when Claude uses tools |
| 2 | Tool name in audit | Audit log captures correct tool_name |
| 3 | Provider vote | Provider vote recorded in audit |
| 4 | Provider timing | response_time_ms is recorded |
| 5 | Passthrough works | Sidecar doesn't block Claude |
| 6 | Audit format | Audit log passes format validation |

## Running Tests

### Standalone tests (no Claude Code needed):
```bash
qa/scripts/run-all-standalone.sh
```

### Live Claude Code tests (requires API key):
```bash
export ANTHROPIC_API_KEY="sk-ant-..."
qa/scripts/run-all-live-claude-code.sh
```

### Individual standalone suites:
```bash
qa/scripts/standalone-config.sh
qa/scripts/standalone-providers.sh
qa/scripts/standalone-quorum.sh
qa/scripts/standalone-audit.sh
qa/scripts/standalone-hook-format.sh
qa/scripts/standalone-plugin-structure.sh
qa/scripts/standalone-plugin-hooks.sh
qa/scripts/standalone-plugin-scripts.sh
```

### In Docker:
```bash
qa/docker/cpts-standalone.sh test                # Standalone
qa/docker/cpts-claude-code.sh test               # Live CC (needs API key)
qa/docker/cpts-claude-code.sh test-standalone    # Standalone in CC image
```

## Adding New Tests

1. Add test to the appropriate `qa/scripts/` script:
   - `standalone-*.sh` for tests without Claude Code
   - `live-claude-code-*.sh` for tests requiring Claude Code CLI
2. Update this checklist with the new test case
3. If new helper fixtures are needed, add to `qa/helpers/` or `qa/fixtures/`
4. Ensure the Docker environment includes any new dependencies
