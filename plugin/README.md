# claude-pretool-sidecar Plugin

Claude Code plugin for `claude-pretool-sidecar` -- a composable tool-approval sidecar with multi-provider voting.

## Installation

### Via Plugin Directory

```bash
claude --plugin-dir /path/to/claude-pretool-sidecar/plugin
```

Or add to your Claude Code configuration to load automatically.

### Prerequisites

The `claude-pretool-sidecar` binary must be installed and available in your PATH. Install from source:

```bash
cd /path/to/claude-pretool-sidecar
cargo install --path .
```

Verify the installation:

```bash
bash plugin/scripts/check-sidecar.sh
```

## What the Plugin Provides

### Hooks

The plugin registers two hooks that route all tool calls through the sidecar:

* **PreToolUse** -- Before any tool executes, the sidecar distributes the request to configured providers, aggregates their votes, and returns an allow/deny/passthrough decision.
* **PostToolUse** -- After tool execution, the sidecar logs the result for audit and pattern analysis.

### Skills

Three interactive skills are available via slash commands:

* **/configure-sidecar** -- Create or edit the sidecar's TOML configuration file. Add providers, set quorum rules, configure timeouts and audit logging.

* **/diagnose-sidecar** -- Troubleshoot issues with the sidecar. Systematically checks binary installation, configuration, hook registration, provider health, and audit logs.

* **/file-issue** -- File a GitHub issue against the claude-pretool-sidecar repository. Automatically gathers environment context, config, and audit logs.

### Resources

Reference documents available to skills and users:

* `resources/config-schema.md` -- Complete TOML configuration reference with all fields, types, defaults, and examples.
* `resources/troubleshooting.md` -- Common issues and their solutions.
* `resources/hook-setup.md` -- Step-by-step guide for manual hook setup without the plugin.

### Scripts

* `scripts/check-sidecar.sh` -- Health check script that verifies binary installation, config file presence, config validation, and hook registration.

## Configuration

The sidecar itself is configured via a TOML file (separate from the plugin). See `resources/config-schema.md` for the full schema.

Config file is searched in this order:

1. `--config <path>` CLI flag
2. `$CLAUDE_PRETOOL_SIDECAR_CONFIG` environment variable
3. `.claude-pretool-sidecar.toml` in current directory
4. `~/.config/claude-pretool-sidecar/config.toml`
5. `~/.claude-pretool-sidecar.toml`

## Repository

* Source: <https://github.com/shibuido/claude-pretool-sidecar>
* License: MIT
