# Example Providers

Ready-to-use provider scripts for `claude-pretool-sidecar`. Copy, customize, and point your config at them.

## Available Providers

### dangerous-command-blocker.sh

Denies Bash commands matching dangerous patterns (`rm -rf`, `dd if=`, `mkfs`, `chmod 777`, etc.). Allows all other commands. Passes through non-Bash tools.

* **Decision style:** allow / deny
* **Customize:** Edit the `DANGEROUS_PATTERNS` array to add/remove regex patterns

### file-path-policy.py

Denies file writes to sensitive paths (`.env`, `/etc/`, credentials, SSH keys, etc.). Always allows read operations. Passes through non-file tools.

* **Decision style:** allow / deny / passthrough
* **Customize:** Edit `SENSITIVE_PATTERNS` list, or set `CPTS_SENSITIVE_PATHS` env var (colon-separated patterns)
* **Dependencies:** Python 3 stdlib only (no pip packages)

### approval-logger.sh

FYI provider that logs every tool invocation with timestamp to a file. Always returns passthrough. Useful as a session audit trail.

* **Decision style:** passthrough (FYI mode)
* **Customize:** Set `CPTS_AUDIT_LOG` env var for output path (default: `/tmp/claude-pretool-audit.log`)

### rate-limiter.sh

Tracks tool call frequency using temp files. Denies if calls exceed a configurable threshold per minute. Passes through when under the limit.

* **Decision style:** deny / passthrough
* **Customize:** Set `CPTS_RATE_LIMIT` (default: 30 calls/min) and `CPTS_RATE_STATE_DIR` (default: `/tmp/claude-pretool-rate`)

### ask-user.sh

Returns the "ask" decision for specific dangerous patterns, triggering Claude Code's interactive approval prompt. Passes through everything else. Demonstrates the `ask` permissionDecision value.

* **Decision style:** ask / passthrough
* **Customize:** Edit the pattern-matching sections for Bash commands, file writes, etc.

## Configuration

Add providers to your `.claude-pretool-sidecar.toml`:

```toml
[[providers]]
name = "command-blocker"
command = "/path/to/examples/providers/dangerous-command-blocker.sh"
timeout_ms = 5000

[[providers]]
name = "path-policy"
command = "python3 /path/to/examples/providers/file-path-policy.py"
timeout_ms = 5000

[[providers]]
name = "audit-logger"
mode = "fyi"
command = "/path/to/examples/providers/approval-logger.sh"

[[providers]]
name = "rate-limiter"
command = "/path/to/examples/providers/rate-limiter.sh"
timeout_ms = 5000

[[providers]]
name = "ask-user"
command = "/path/to/examples/providers/ask-user.sh"
timeout_ms = 5000
```

You can pass environment variables to providers:

```toml
[[providers]]
name = "path-policy"
command = "python3 /path/to/file-path-policy.py"
env = { CPTS_SENSITIVE_PATHS = ".secret:/opt/prod" }
```

## Writing Your Own Provider

A provider is any executable that:

1. **Reads JSON from stdin** -- a single JSON object with `tool_name`, `tool_input`, `hook_event_name`, and other fields
2. **Writes JSON to stdout** -- a single JSON object with a `decision` field and optional `reason`

### Minimal provider (bash)

```bash
#!/bin/bash
cat > /dev/null  # consume stdin (required)
echo '{"decision": "passthrough"}'
```

### Decision values

| Value | Effect |
|---|---|
| `allow` | Vote to approve the tool call |
| `deny` | Vote to reject the tool call |
| `passthrough` | Abstain (no opinion) |
| `ask` | Trigger Claude Code's interactive user approval |

### Protocol rules

* You **must** consume all of stdin, even if you don't need the data
* You **must** write valid JSON to stdout with at least a `decision` field
* The optional `reason` field is logged but does not affect quorum logic
* Non-zero exit codes are treated as errors (subject to `error_policy`)
* FYI providers (`mode = "fyi"`) have their stdout ignored, but should still output valid JSON

### Tips

* Use `set -euo pipefail` in bash scripts for robustness
* Extract fields with `python3 -c "import sys,json; ..."` or `jq` for JSON parsing
* Keep providers fast -- they run synchronously before every tool call
* Test with: `echo '{"tool_name":"Bash","tool_input":{"command":"ls"}}' | ./your-provider.sh`
