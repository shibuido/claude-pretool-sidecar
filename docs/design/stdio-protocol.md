# Design: stdio Protocol for Provider Communication

*Date: 2026-03-22*

## Problem

The sidecar needs to communicate with external decision-making scripts/binaries. We need a simple, universal protocol that works with any language.

## Design: JSON over stdio (inspired by MCP stdio transport)

### Input (sidecar → provider via stdin)

The sidecar writes a single JSON object to the provider's stdin, followed by a newline, then closes stdin. The JSON contains the same payload that Claude Code passes to the PreToolUse hook:

```json
{
  "tool_name": "Bash",
  "tool_input": {
    "command": "rm -rf /tmp/foo",
    "description": "Delete temporary files"
  },
  "session_id": "abc123",
  "hook_event": "PreToolUse"
}
```

The exact fields depend on Claude Code's hook specification (to be confirmed by research).

### Output (provider → sidecar via stdout)

The provider writes a single JSON object to stdout:

```json
{
  "decision": "allow"
}
```

Valid `decision` values: `"allow"`, `"deny"`, `"passthrough"`

Optional fields:

```json
{
  "decision": "deny",
  "reason": "Command matches dangerous pattern: rm -rf"
}
```

The `reason` field is logged but does not affect the decision logic.

### Error Handling

* **Non-zero exit code**: Treated as error (provider crashed/failed)
* **Invalid JSON on stdout**: Treated as error
* **Missing `decision` field**: Treated as error
* **Timeout exceeded**: Process is killed, treated as error
* **Empty stdout**: Treated as error

All errors are subject to the `error_policy` quorum configuration.

### FYI Providers

FYI providers receive the same stdin input but their stdout is ignored. They are still subject to timeout (to prevent hanging the sidecar). Their exit code is logged but does not affect decisions.

## Why stdio?

* **Universal**: Works with any language (bash, python, rust, node, etc.)
* **Simple**: No networking, no sockets, no dependencies
* **Composable**: Unix philosophy — pipe-friendly
* **Familiar**: Same pattern as MCP stdio transport, git hooks, pre-commit hooks
* **Isolated**: Each provider is a separate process — crash isolation for free

## Alternatives Considered

* **HTTP/REST**: More complex, requires networking, overkill for local decision-making
* **Unix sockets**: Platform-dependent, harder to debug
* **Shared memory**: Complex, error-prone, not worth the latency savings

## Performance Note

Spawning a process per provider per hook invocation has overhead. For the typical use case (human-speed interactions, <10 providers), this is negligible. If performance becomes critical, we can add optional persistent provider processes (long-running with line-delimited JSON) in a future phase.
