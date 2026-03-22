# Manual QA Test Checklist

*Last updated: 2026-03-22*

Use this checklist when performing manual QA on `claude-pretool-sidecar`. Each section covers a feature area. Check off items as you verify them.

---

## 1. Build & Installation

- [ ] `cargo build --release` completes without errors
- [ ] `cargo test` passes all unit and integration tests
- [ ] Binary `claude-pretool-sidecar` is produced in `target/release/`
- [ ] Binary `claude-pretool-logger` is produced in `target/release/`
- [ ] Both binaries show help/usage when run without config (exits non-zero with meaningful error)
- [ ] `.gitignore` excludes `target/` directory

## 2. Configuration Loading

### 2.1 Config File Discovery

- [ ] Config loaded from `$CLAUDE_PRETOOL_SIDECAR_CONFIG` env var when set
- [ ] Config loaded from `.claude-pretool-sidecar.toml` in current directory
- [ ] Config loaded from `~/.config/claude-pretool-sidecar/config.toml` (XDG)
- [ ] Config loaded from `~/.claude-pretool-sidecar.toml` (home fallback)
- [ ] Priority order respected: env var > cwd > XDG > home
- [ ] Clear error message when no config found

### 2.2 Config Parsing

- [ ] Minimal config (just one provider) parses correctly
- [ ] Full config with all fields parses correctly
- [ ] Missing optional fields use correct defaults:
  - [ ] `min_allow` defaults to 1
  - [ ] `max_deny` defaults to 0
  - [ ] `error_policy` defaults to "passthrough"
  - [ ] `default_decision` defaults to "passthrough"
  - [ ] `provider_default` timeout defaults to 5000ms
  - [ ] Provider `mode` defaults to "vote"
- [ ] Invalid TOML produces clear error message
- [ ] Unknown fields in config are silently ignored (forward compat)

### 2.3 Example Configs

- [ ] `examples/config-logging-only.toml` loads and works
- [ ] `examples/config-single-gatekeeper.toml` loads (with valid command)
- [ ] `examples/config-multi-provider.toml` loads (with valid commands)

## 3. Provider Execution

### 3.1 Basic Communication

- [ ] Provider receives JSON on stdin
- [ ] Provider's stdout JSON is parsed correctly
- [ ] Provider's stderr is captured but doesn't affect decision
- [ ] Provider's stdin is closed after writing (EOF signal)

### 3.2 Vote Responses

- [ ] `{"decision": "allow"}` → Vote::Allow
- [ ] `{"decision": "deny"}` → Vote::Deny
- [ ] `{"decision": "deny", "reason": "..."}` → Vote::Deny with reason logged
- [ ] `{"decision": "passthrough"}` → Vote::Passthrough
- [ ] Unknown decision value → Vote::Error
- [ ] Invalid JSON → Vote::Error
- [ ] Empty stdout → Vote::Error
- [ ] Whitespace-padded JSON still parses

### 3.3 Provider Modes

- [ ] `mode = "vote"` — provider's vote is included in quorum aggregation
- [ ] `mode = "fyi"` — provider runs but vote is ignored
- [ ] FYI provider crash does not affect final decision
- [ ] FYI provider's deny does not block the tool call

### 3.4 Error Conditions

- [ ] Provider not found (bad command) → Vote::Error with spawn error
- [ ] Provider exits non-zero → Vote::Error
- [ ] Provider returns non-UTF-8 → Vote::Error
- [ ] Provider arguments are passed correctly
- [ ] Provider environment variables from config are set

## 4. Quorum Logic

### 4.1 Basic Scenarios

- [ ] Single allow with `min_allow=1` → Allow
- [ ] Single deny with `max_deny=0` → Deny
- [ ] Single passthrough with `min_allow=1` → default_decision
- [ ] Zero providers with `min_allow=0` → Allow (0 >= 0)
- [ ] Zero providers with `min_allow=1` → default_decision

### 4.2 Multi-Provider Scenarios

- [ ] 2/3 allow, 0 deny, `min_allow=2` → Allow
- [ ] 2/3 allow, 1 deny, `max_deny=0` → Deny (deny priority)
- [ ] 2/3 allow, 1 deny, `max_deny=1` → Allow (deny tolerated)
- [ ] 1/3 allow, 0 deny, `min_allow=2` → default_decision (quorum not met)

### 4.3 Error Policy

- [ ] `error_policy = "passthrough"` — errors don't count as votes
- [ ] `error_policy = "deny"` — errors count as deny
- [ ] `error_policy = "allow"` — errors count as allow
- [ ] All providers error with `error_policy = "passthrough"` → default_decision

### 4.4 Deny Priority

- [ ] Deny threshold exceeded always results in Deny, regardless of allow count
- [ ] Even with `min_allow` met, `max_deny` exceeded → Deny

## 5. Hook Response Format (Claude Code Compatibility)

### 5.1 Allow Response

- [ ] Output JSON: `{"hookSpecificOutput":{"permissionDecision":"allow"}}`
- [ ] No `systemMessage` field when not needed
- [ ] Exit code 0

### 5.2 Deny Response

- [ ] Output JSON includes `hookSpecificOutput.permissionDecision = "deny"`
- [ ] `systemMessage` included when reason available
- [ ] Exit code 0 (not 2 — we use structured JSON, not stderr)

### 5.3 Passthrough Response

- [ ] Output JSON: `{}` (empty object)
- [ ] Exit code 0

### 5.4 Input Parsing

- [ ] Parses `tool_name` field correctly
- [ ] Parses `tool_input` (arbitrary JSON object)
- [ ] Parses `hook_event_name` with default "PreToolUse"
- [ ] Parses optional `session_id`, `transcript_path`, `cwd`, `permission_mode`
- [ ] Unknown fields in input are silently ignored

## 6. Audit Logging

### 6.1 Basic Logging

- [ ] Audit disabled by default (`enabled = false`)
- [ ] When `output = "stderr"`, audit entries appear on stderr
- [ ] When `output = "/path/to/dir"`, files created as `audit-YYYY-MM-DD.jsonl`
- [ ] Each log entry is valid JSON on a single line
- [ ] Multiple calls same day append to same file
- [ ] Different days create different files
- [ ] Audit directory created automatically if missing

### 6.2 Entry Content

- [ ] `timestamp` is ISO 8601 format
- [ ] `hook_event` matches input hook_event_name
- [ ] `tool_name` matches input
- [ ] `tool_input` matches input
- [ ] `session_id` present when provided in input, absent when not
- [ ] `providers` array contains one entry per provider
- [ ] Each provider entry has: `name`, `vote`, `mode`, `response_time_ms`
- [ ] `final_decision` matches the actual returned decision
- [ ] `total_time_ms` is reasonable (> 0, < timeout)
- [ ] FYI providers appear in `providers` array (with mode="fyi")
- [ ] Provider `error` field populated on failures, null on success
- [ ] Provider `reason` field captured when provider supplies one

### 6.3 Log Rotation — Per-File Limit

- [ ] File exceeding `max_file_bytes` is truncated
- [ ] Truncated file starts with `{"_truncated": true, ...}` sentinel
- [ ] Sentinel includes `lines_removed` count
- [ ] Most recent lines are preserved (not oldest)
- [ ] File size after truncation is within limit

### 6.4 Log Rotation — Total Size Limit

- [ ] When total exceeds `max_total_bytes`, oldest files deleted first
- [ ] Most recent file is never deleted
- [ ] If only one file remains and it's too large, it's truncated
- [ ] Non-audit files in the directory are not touched

## 7. Companion: claude-pretool-logger

- [ ] Reads JSON from stdin without errors
- [ ] Default output: writes to stderr
- [ ] `--output /path/to/file` writes to specified file (append mode)
- [ ] `-o /path/to/file` short flag works
- [ ] Log entry wraps input in `{"timestamp": ..., "event": ...}` envelope
- [ ] Invalid JSON input is handled gracefully (logged as raw)
- [ ] Always outputs `{"decision": "passthrough"}` on stdout (safe default)

## 8. End-to-End Integration

### 8.1 With Mock Providers

- [ ] Single allow provider → sidecar returns allow
- [ ] Single deny provider → sidecar returns deny
- [ ] Mixed providers with quorum rules → correct aggregation
- [ ] Crashing provider → handled per error_policy
- [ ] FYI logger provider → runs but doesn't affect decision
- [ ] FYI provider still logged in audit entry (visible for auditing)

### 8.4 Fail-Mode Configurations

- [ ] Fail-secure: `error_policy="deny"`, `max_deny=0` — any error blocks
- [ ] Fail-open: `error_policy="allow"`, `default_decision="allow"` — errors pass through
- [ ] Democratic: 3+ providers, `min_allow=2`, majority decides

### 8.2 With Claude Code Hook Format

- [ ] Input matches what Claude Code sends to PreToolUse hooks
- [ ] Output matches what Claude Code expects from hooks
- [ ] Passthrough doesn't interfere with Claude Code's normal flow
- [ ] Deny prevents tool execution in Claude Code (requires live test)

### 8.3 Config Combinations

- [ ] Logging-only setup (all FYI, min_allow=0) → always passthrough/allow
- [ ] Single gatekeeper (one vote provider) → strict enforcement
- [ ] Multi-provider with tolerances → nuanced policy

## 9. Error Handling & Edge Cases

- [ ] Empty stdin → meaningful error message
- [ ] Malformed JSON stdin → meaningful error message
- [ ] Config file read permission denied → meaningful error
- [ ] Provider binary not executable → meaningful error logged
- [ ] Very large tool_input (>1MB) handled without crash
- [ ] Unicode in tool_input handled correctly
- [ ] Concurrent invocations don't interfere (no shared state)
- [ ] Audit log directory creation when it doesn't exist
- [ ] Config path with spaces in path
- [ ] Provider command with spaces in path
- [ ] Provider args with special characters (quotes, spaces)
- [ ] Extremely long provider name in audit log
- [ ] Config with zero providers but audit enabled works
- [ ] Empty TOML config file (all defaults applied)

---

## Sign-off

| Tester | Date | Version | Result |
|--------|------|---------|--------|
|        |      |         |        |
