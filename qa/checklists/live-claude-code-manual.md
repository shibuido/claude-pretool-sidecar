# Live Claude Code — Manual QA Checklist

*Last updated: 2026-03-23*

These tests require a working Claude Code CLI installation with a valid `ANTHROPIC_API_KEY`. They verify the sidecar works as an actual Claude Code hook.

**Prerequisites:**

* Claude Code CLI installed (`curl -fsSL https://claude.ai/install.sh | bash`)
* `ANTHROPIC_API_KEY` environment variable set
* `claude-pretool-sidecar` binary in PATH
* These tests make real API calls and cost money

---

## 1. Hook Installation

- [ ] Hook settings JSON is valid (`.claude/settings.local.json`)
- [ ] Claude Code starts without errors with hook configured
- [ ] `claude --version` works with hook settings present
- [ ] PreToolUse hook configured with wildcard matcher `*`
- [ ] PostToolUse hook configured alongside PreToolUse
- [ ] Hook `timeout` setting respected (set to 10-15s)

## 2. Hook Invocation (PreToolUse)

- [ ] Sidecar invoked when Claude uses `Read` tool
- [ ] Sidecar invoked when Claude uses `Bash` tool
- [ ] Sidecar invoked when Claude uses `Write` tool
- [ ] Sidecar invoked when Claude uses `Edit` tool
- [ ] Sidecar invoked when Claude uses `Glob` tool
- [ ] Sidecar invoked when Claude uses `Grep` tool
- [ ] Audit log entry created for each tool invocation
- [ ] Audit log contains correct `tool_name`
- [ ] Audit log contains `tool_input` matching the actual tool call

## 3. Decision Enforcement

- [ ] Passthrough config: Claude operates normally (no blocking)
- [ ] Allow config: tool calls proceed
- [ ] Deny config: tool calls blocked, Claude gets feedback
- [ ] Claude adjusts behavior after deny (tries different approach)

## 4. Audit Trail

- [ ] Audit log file created with date in name
- [ ] Per-provider vote recorded in log
- [ ] Provider response_time_ms is realistic (> 0)
- [ ] Session ID from Claude Code appears in audit
- [ ] Multiple tool calls produce multiple audit entries
- [ ] Final decision matches what Claude Code experienced

## 5. PostToolUse Integration

- [ ] PostToolUse hook fires after tool execution
- [ ] Audit log captures PostToolUse events (if configured)
- [ ] PreToolUse + PostToolUse pairs correlate via session_id and tool_name

## 6. Error Recovery

- [ ] Sidecar crash doesn't break Claude Code (non-zero exit handled)
- [ ] Sidecar timeout doesn't hang Claude Code
- [ ] Invalid sidecar output doesn't crash Claude Code
- [ ] Claude Code continues working after sidecar error

## 7. Multi-Session

- [ ] Different Claude Code sessions get different session_id in audit
- [ ] Audit log accumulates across sessions
- [ ] Hook settings changes require Claude Code restart

## 8. Docker Environment

- [ ] `cpts-claude-code.sh build` builds successfully
- [ ] `cpts-claude-code.sh test` runs live tests with API key
- [ ] `cpts-claude-code.sh test-standalone` runs without API key
- [ ] `cpts-claude-code.sh shell` provides interactive environment
- [ ] `cpts-claude-code.sh exec claude --version` shows version
- [ ] `cpts-claude-code.sh destroy` cleans up artifacts

---

## Sign-off

| Tester | Date | CC Version | Sidecar Version | Result |
|--------|------|------------|-----------------|--------|
|        |      |            |                 |        |
