# Configuration Schema Reference

`claude-pretool-sidecar` uses TOML configuration files.

## Config File Locations

Searched in order (first found wins):

1. `--config <path>` CLI flag (explicit)
2. `$CLAUDE_PRETOOL_SIDECAR_CONFIG` environment variable
3. `.claude-pretool-sidecar.toml` in current directory (project-level)
4. `~/.config/claude-pretool-sidecar/config.toml` (user-level, XDG)
5. `~/.claude-pretool-sidecar.toml` (home directory fallback)

## Full Schema

### `[quorum]` -- Vote Aggregation Rules

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `min_allow` | integer | `1` | Minimum "allow" votes required to approve |
| `max_deny` | integer | `0` | Maximum "deny" votes tolerated before blocking |
| `error_policy` | string | `"passthrough"` | How to treat provider errors: `"passthrough"`, `"deny"`, `"allow"` |
| `default_decision` | string | `"passthrough"` | Decision when quorum is not met and no deny threshold exceeded: `"passthrough"`, `"deny"`, `"allow"` |

### `[timeout]` -- Timeout Settings

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `provider_default` | integer | `5000` | Default timeout per provider in milliseconds |
| `total` | integer | `30000` | Total timeout for all providers combined in milliseconds |

### `[[providers]]` -- Provider Definitions

Each provider is an entry in the `[[providers]]` array:

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `name` | string | yes | -- | Unique identifier for this provider |
| `command` | string | yes | -- | Path to executable (absolute or in PATH) |
| `args` | array of strings | no | `[]` | Command-line arguments |
| `mode` | string | no | `"vote"` | `"vote"` (participates in quorum) or `"fyi"` (logging only, output ignored) |
| `timeout` | integer | no | `provider_default` | Override default timeout for this provider (ms) |
| `env` | table | no | `{}` | Additional environment variables passed to the provider |

### `[audit]` -- Audit Logging

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | boolean | `true` | Enable or disable audit logging |
| `output` | string | `"stderr"` | Output destination: file path for JSONL or `"stderr"` |

## Environment Variable Overrides

Key settings can be overridden via environment variables (prefix `CPTS_`):

| Variable | Overrides |
|----------|-----------|
| `CPTS_MIN_ALLOW` | `quorum.min_allow` |
| `CPTS_MAX_DENY` | `quorum.max_deny` |
| `CPTS_ERROR_POLICY` | `quorum.error_policy` |
| `CPTS_TIMEOUT` | `timeout.provider_default` |

## Example Configurations

### Minimal: Logging Only

```toml
[quorum]
min_allow = 0
default_decision = "passthrough"

[[providers]]
name = "logger"
command = "claude-pretool-logger"
mode = "fyi"
```

### Standard: One Voter + Logger

```toml
[quorum]
min_allow = 1
max_deny = 0
error_policy = "passthrough"
default_decision = "passthrough"

[timeout]
provider_default = 5000
total = 30000

[[providers]]
name = "security-checker"
command = "/usr/local/bin/my-security-checker"
args = ["--strict"]
mode = "vote"
timeout = 10000

[[providers]]
name = "audit-logger"
command = "claude-pretool-logger"
args = ["--output", "/var/log/claude-tools.jsonl"]
mode = "fyi"

[audit]
enabled = true
output = "/var/log/claude-pretool-sidecar/audit.jsonl"
```

### Multi-Voter: Team Policy

```toml
[quorum]
min_allow = 2
max_deny = 0
error_policy = "deny"
default_decision = "deny"

[timeout]
provider_default = 5000
total = 15000

[[providers]]
name = "security-checker"
command = "/usr/local/bin/security-checker"
mode = "vote"

[[providers]]
name = "team-policy"
command = "/usr/local/bin/team-policy-checker"
mode = "vote"

[[providers]]
name = "audit-logger"
command = "claude-pretool-logger"
mode = "fyi"
```

## Validation

Run `claude-pretool-sidecar --validate` to check your config file without executing.

Validation checks:

* Config file found and parseable as TOML
* Provider commands exist and are executable
* Quorum values are consistent (e.g., `min_allow` does not exceed number of vote-mode providers)
* Enum fields contain valid values
