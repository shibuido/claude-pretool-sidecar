# Reference: Claude Code Hooks Specification

*Compiled: 2026-03-22 from official Claude Code documentation and research*

## Hook Input (stdin JSON)

### Common Fields (all events)

```json
{
  "session_id": "abc123",
  "transcript_path": "/path/to/transcript.jsonl",
  "cwd": "/current/working/directory",
  "hook_event_name": "PreToolUse",
  "permission_mode": "default"
}
```

### PreToolUse Additional Fields

```json
{
  "tool_name": "Bash",
  "tool_use_id": "toolu_01ABC123...",
  "tool_input": {
    "command": "npm test",
    "description": "Run test suite"
  }
}
```

### PostToolUse Additional Fields

Same as PreToolUse plus:
```json
{
  "tool_response": { "filePath": "...", "success": true },
  "tool_result": { "type": "text", "content": "..." }
}
```

### tool_input by Tool Type

| Tool | Fields |
|------|--------|
| Bash | `command`, `description`, `timeout`, `run_in_background` |
| Write | `file_path`, `content` |
| Edit | `file_path`, `old_string`, `new_string`, `replace_all` |
| Read | `file_path`, `offset`, `limit` |
| Glob | `pattern`, `path` |
| Grep | `pattern`, `path`, `glob`, `output_mode` |
| WebFetch | `url`, `prompt` |
| WebSearch | `query` |
| Agent | `prompt`, `description`, `subagent_type`, `model` |

## Hook Output (stdout JSON)

### PreToolUse Response

```json
{
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "permissionDecision": "allow|deny|ask",
    "permissionDecisionReason": "Explanation",
    "updatedInput": { "command": "modified" },
    "additionalContext": "Extra context for Claude"
  },
  "systemMessage": "Message shown to Claude"
}
```

### PostToolUse Response

```json
{
  "decision": "block",
  "reason": "Explanation",
  "hookSpecificOutput": {
    "hookEventName": "PostToolUse",
    "additionalContext": "Extra context"
  }
}
```

### Passthrough (no opinion)

Empty object `{}` or exit code 0 with no stdout.

## Exit Codes

| Code | Effect |
|------|--------|
| 0 | Success — stdout parsed as JSON |
| 2 | Blocking error — stderr sent to Claude, action blocked |
| Other | Non-blocking error — logged, action proceeds |

## Hook Configuration in settings.json

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "/path/to/script.sh",
            "timeout": 30,
            "async": false,
            "statusMessage": "Checking..."
          }
        ]
      }
    ]
  }
}
```

### Hook Types

| Type | Purpose | Key Fields |
|------|---------|------------|
| `command` | Execute shell command | `command`, `timeout`, `async` |
| `http` | HTTP POST | `url`, `headers`, `allowedEnvVars` |
| `prompt` | Single-turn LLM | `prompt`, `model` |
| `agent` | Multi-turn LLM agent | `prompt`, `model` |

### Matcher Patterns

* `"Bash"` — exact tool name
* `"Write|Edit"` — regex alternation
* `"mcp__.*"` — all MCP tools
* `"*"` or `""` — all tools
* Case-sensitive

## Plugin Hook Configuration

In `hooks/hooks.json`:
```json
{
  "description": "Plugin hooks",
  "hooks": {
    "PreToolUse": [...]
  }
}
```

Note the extra `"hooks"` wrapper vs settings.json format.

## Environment Variables

* `$CLAUDE_PROJECT_DIR` — project root
* `$CLAUDE_PLUGIN_ROOT` — plugin installation directory
* `$CLAUDE_PLUGIN_DATA` — persistent plugin data directory
* `$CLAUDE_ENV_FILE` — SessionStart only: write `export VAR=val` here

## Alternative: --permission-prompt-tool

For non-interactive mode (`claude -p`):
```bash
claude -p --permission-prompt-tool mcp_tool_name "query"
```
Specifies an MCP tool to handle permission prompts. The MCP tool receives the request and returns the decision.

## Key Hook Events (25+)

| Event | Can Block | Use Case |
|-------|-----------|----------|
| PreToolUse | Yes | Validate/modify/block tool calls |
| PostToolUse | Block result | React to results, logging |
| PermissionRequest | Yes | Auto-approve/deny permissions |
| Stop | Yes | Enforce completion standards |
| SubagentStop | Yes | Validate subagent work |
| SessionStart | Yes | Load context, set env vars |
| SessionEnd | No | Cleanup (1.5s timeout) |
| UserPromptSubmit | Yes | Validate user prompts |
| Notification | No | React to notifications |
| PreCompact / PostCompact | No | Context compaction hooks |
| ConfigChange | Yes | React to config changes |
