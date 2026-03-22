# Design: System Architecture

*Date: 2026-03-22*

## Overview

`claude-pretool-sidecar` integrates with Claude Code's hook system as a **command hook** for PreToolUse and PostToolUse events. It acts as a multiplexer: receiving hook payloads from Claude Code, distributing them to multiple external providers, aggregating votes, and returning a decision.

## Claude Code Hook Integration

### Hook Input (stdin from Claude Code)

Claude Code sends JSON on stdin to all command hooks:

```json
{
  "tool_name": "Bash",
  "tool_input": {"command": "rm -rf /tmp/foo"},
  "hook_event_name": "PreToolUse",
  "session_id": "abc123",
  "transcript_path": "/path/to/transcript.txt",
  "cwd": "/home/user/project",
  "permission_mode": "ask"
}
```

For PostToolUse, additional field `tool_result` contains the tool's output.

### Hook Output (stdout to Claude Code)

**PreToolUse — Allow:**
```json
{
  "hookSpecificOutput": {
    "permissionDecision": "allow"
  }
}
```

**PreToolUse — Deny:**
```json
{
  "hookSpecificOutput": {
    "permissionDecision": "deny"
  },
  "systemMessage": "Blocked by policy: dangerous command pattern detected"
}
```

**PreToolUse — Passthrough (no opinion):**
```json
{}
```

**PostToolUse — Logging only (no decision needed):**
Exit code 0, output ignored or used for logging.

### Exit Codes

* `0` — Success (stdout parsed as JSON response)
* `2` — Blocking error (stderr fed back to Claude as error message)
* Other — Non-blocking error (logged, operation continues)

## System Architecture

```
┌─────────────────────────────────────────────────────────┐
│                     Claude Code                         │
│                                                         │
│  PreToolUse hook ──stdin──►┌────────────────────────┐   │
│                            │ claude-pretool-sidecar │   │
│  ◄──stdout─────────────────│ (this tool)            │   │
│                            └────────┬───────────────┘   │
│  PostToolUse hook ──stdin──►┌───────┤────────────────┐  │
│                            │ claude-pretool-sidecar  │  │
│  ◄──stdout─────────────────│ (same binary, log mode) │  │
│                            └─────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
                              │
        ┌─────────────────────┼─────────────────────┐
        ▼                     ▼                     ▼
┌──────────────┐   ┌──────────────┐   ┌──────────────────┐
│ Provider 1   │   │ Provider 2   │   │ FYI: Logger      │
│ (vote)       │   │ (vote)       │   │ (output ignored) │
│ stdin→stdout │   │ stdin→stdout │   │ stdin→file       │
└──────────────┘   └──────────────┘   └──────────────────┘
```

## Dual-Hook Design: PreToolUse + PostToolUse

The sidecar is invoked as **two separate hooks** by Claude Code:

### PreToolUse Hook
* Receives tool request BEFORE execution
* Distributes to providers, aggregates votes
* Returns allow/deny/passthrough
* Writes audit log entry: "tool X requested, decision: Y, providers: [...]"

### PostToolUse Hook
* Receives tool result AFTER execution
* Confirms the tool actually ran (if it appears in PostToolUse, it was approved)
* Writes audit log entry: "tool X completed, result available"
* Enables correlation with PreToolUse log for pattern analysis

### Pattern Analysis Use Case

By pairing PreToolUse and PostToolUse logs, users can:
1. See which tools are always approved → candidates for auto-approval
2. See which tools are always denied → candidates for permanent block
3. Measure provider response times → identify slow providers
4. Audit the full lifecycle: request → decision → execution → result

## Audit Logging (Built-in)

The sidecar itself logs decision details — separate from FYI providers:

```json
{
  "timestamp": "2026-03-22T14:30:00Z",
  "hook_event": "PreToolUse",
  "tool_name": "Bash",
  "tool_input": {"command": "ls -la"},
  "session_id": "sess-123",
  "providers": [
    {
      "name": "security-checker",
      "vote": "allow",
      "response_time_ms": 45
    },
    {
      "name": "team-policy",
      "vote": "allow",
      "response_time_ms": 120
    },
    {
      "name": "audit-logger",
      "mode": "fyi",
      "response_time_ms": 12
    }
  ],
  "final_decision": "allow",
  "total_time_ms": 132
}
```

Configured via:
```toml
[audit]
enabled = true
output = "/var/log/claude-pretool-sidecar/audit.jsonl"
# or "stderr" for stderr output
```

## Installation Methods

### Method 1: Manual Hook Configuration

Add to `.claude/settings.json` (project-level) or `~/.claude/settings.json` (user-level):

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "claude-pretool-sidecar",
            "timeout": 30
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "claude-pretool-sidecar --post-tool",
            "timeout": 10
          }
        ]
      }
    ]
  }
}
```

### Method 2: Plugin Installation (Future)

A Claude Code plugin bundles hooks, skills, and resources:

```
claude-pretool-sidecar-plugin/
├── .claude-plugin/plugin.json
├── hooks/hooks.json          # PreToolUse + PostToolUse hooks
├── skills/
│   ├── configure/SKILL.md    # Help users configure the sidecar
│   ├── diagnose/SKILL.md     # Help users troubleshoot
│   └── report/SKILL.md       # Generate approval pattern reports
├── scripts/
│   └── install-hooks.sh      # Helper to install hooks in settings
└── resources/
    ├── config-schema.md       # Config file reference
    └── troubleshooting.md     # Common issues and fixes
```

## Configuration Locations

The sidecar searches for config in this order (first found wins):

1. `--config <path>` CLI flag
2. `$CLAUDE_PRETOOL_SIDECAR_CONFIG` environment variable
3. `.claude-pretool-sidecar.toml` in current directory
4. `~/.config/claude-pretool-sidecar/config.toml` (XDG)
5. `~/.claude-pretool-sidecar.toml` (home fallback)

Claude Code settings go in separate locations:

* Project: `.claude/settings.json` (checked into repo)
* Project-local: `.claude/settings.local.json` (gitignored)
* User: `~/.claude/settings.json` (global)

## Environment Variables Available in Hooks

Claude Code provides these to all command hooks:

* `$CLAUDE_PROJECT_DIR` — Project root
* `$CLAUDE_PLUGIN_ROOT` — Plugin directory (if installed as plugin)
* `$CLAUDE_ENV_FILE` — SessionStart only: persist env vars
