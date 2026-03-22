# Design: Configuration Format

*Date: 2026-03-22*

## Problem

Users need to configure: which providers to run, quorum rules, timeouts, and logging. The config must be simple enough for scripters to edit by hand.

## Format Choice: TOML

TOML is chosen because:

* Human-readable and writable (unlike JSON — no trailing comma issues, has comments)
* Well-supported in Rust ecosystem (`toml` crate)
* Natural fit for the hierarchical-but-flat config structure
* Comments allow inline documentation

## Config File Location

Searched in order (first found wins):

1. `--config <path>` CLI flag (explicit)
2. `$CLAUDE_PRETOOL_SIDECAR_CONFIG` environment variable
3. `.claude-pretool-sidecar.toml` in current directory (project-level)
4. `~/.config/claude-pretool-sidecar/config.toml` (user-level, XDG)
5. `~/.claude-pretool-sidecar.toml` (user home fallback)

## Config Schema

```toml
# Global settings
[quorum]
min_allow = 1          # Minimum "allow" votes required
max_deny = 0           # Maximum "deny" votes tolerated
error_policy = "passthrough"  # How to treat errors: "passthrough" | "deny" | "allow"
default_decision = "passthrough"  # When quorum not met and no deny threshold exceeded

[timeout]
provider_default = 5000  # Default timeout per provider in milliseconds
total = 30000            # Total timeout for all providers combined

# Provider definitions
[[providers]]
name = "security-checker"
command = "/usr/local/bin/my-security-checker"
args = ["--strict"]
mode = "vote"           # "vote" (default) or "fyi"
timeout = 10000         # Override default timeout (ms)
env = { MY_VAR = "value" }  # Additional environment variables

[[providers]]
name = "audit-logger"
command = "claude-pretool-logger"
args = ["--output", "/var/log/claude-tools.jsonl"]
mode = "fyi"            # Output ignored, just logging

[[providers]]
name = "custom-policy"
command = "./scripts/check-policy.sh"
mode = "vote"
```

## Minimal Config (Logging Only)

```toml
[quorum]
min_allow = 0
default_decision = "passthrough"

[[providers]]
name = "logger"
command = "claude-pretool-logger"
mode = "fyi"
```

## Environment Variable Overrides

Key settings can be overridden via environment variables (prefix `CPTS_`):

* `CPTS_MIN_ALLOW` — overrides `quorum.min_allow`
* `CPTS_MAX_DENY` — overrides `quorum.max_deny`
* `CPTS_ERROR_POLICY` — overrides `quorum.error_policy`
* `CPTS_TIMEOUT` — overrides `timeout.provider_default`

## Validation

On startup, the sidecar validates the config and exits with a clear error if:

* No config file found and no `--passthrough` flag
* Provider command not found / not executable
* Quorum values are contradictory (e.g., `min_allow > number of vote providers`)
* Invalid enum values

A `--validate` CLI flag allows users to check config without running.
