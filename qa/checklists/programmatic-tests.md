# Programmatic QA Test Checklist

*Last updated: 2026-03-22*

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

## Running Tests

### All automated tests:
```bash
qa/scripts/run-all-qa.sh
```

### Individual test suites:
```bash
qa/scripts/test-config.sh
qa/scripts/test-providers.sh
qa/scripts/test-quorum.sh
qa/scripts/test-audit.sh
qa/scripts/test-hook-integration.sh
```

### In Docker:
```bash
qa/docker/qa-docker.sh test
```

## Adding New Tests

1. Add test to the appropriate `qa/scripts/test-*.sh` script
2. Update this checklist with the new test case
3. If new helper fixtures are needed, add to `qa/helpers/` or `qa/fixtures/`
4. Ensure the Docker environment includes any new dependencies
