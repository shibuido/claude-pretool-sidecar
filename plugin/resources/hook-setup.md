# Manual Hook Setup Guide

How to set up `claude-pretool-sidecar` as a Claude Code hook without the plugin system.

## Prerequisites

1. `claude-pretool-sidecar` binary installed and in PATH (or use absolute path)
2. A configuration file created (see `config-schema.md`)

## Option A: Project-Level Hooks (Recommended)

Project-level hooks apply only when working in a specific project directory.

### Shared with Team: `.claude/settings.json`

This file is checked into version control so all team members use the same hooks.

Create or edit `.claude/settings.json` in your project root:

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

### Personal Only: `.claude/settings.local.json`

This file is typically gitignored and applies only to you.

Same format as above. Useful when you want to test the sidecar on a project without affecting teammates.

## Option B: User-Level Hooks (Global)

User-level hooks apply to all projects for the current user.

Create or edit `~/.claude/settings.json`:

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

## Hook Configuration Details

### Matcher Patterns

The `matcher` field controls which tools trigger the hook:

| Pattern | Matches |
|---------|---------|
| `"*"` or `""` | All tools |
| `"Bash"` | Only the Bash tool |
| `"Write\|Edit"` | Write or Edit tools (regex) |
| `"mcp__.*"` | All MCP tools |

For the sidecar, `"*"` is recommended so all tool calls go through provider voting.

### Timeout

The `timeout` field (in seconds) controls how long Claude Code waits for the hook to respond.

* PreToolUse: `30` seconds recommended (providers may need time to evaluate)
* PostToolUse: `10` seconds recommended (logging is simpler)

If the hook times out, Claude Code treats it as a non-blocking error and proceeds.

### Async Hooks

Setting `"async": true` runs the hook without waiting for a response. This is NOT recommended for PreToolUse (you want to block on the decision) but can be used for PostToolUse if you only need fire-and-forget logging.

## Verifying the Setup

After configuring hooks, verify they work:

1. Start a new Claude Code session
2. Ask Claude to run a simple command (e.g., "list files in the current directory")
3. Check the audit log for entries
4. If no entries appear, check the troubleshooting guide

## Environment Variables Available in Hooks

Claude Code provides these environment variables to all command hooks:

| Variable | Description |
|----------|-------------|
| `$CLAUDE_PROJECT_DIR` | Project root directory |
| `$CLAUDE_PLUGIN_ROOT` | Plugin installation directory (plugin mode only) |
| `$CLAUDE_PLUGIN_DATA` | Persistent plugin data directory (plugin mode only) |

## Combining with Existing Hooks

If you already have other hooks configured, add the sidecar entries alongside them in the same arrays. Multiple hooks for the same event run in sequence.
