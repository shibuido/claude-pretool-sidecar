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

## Hook Installation

The plugin provides hooks that route tool calls through the sidecar. There are two ways to set them up:

### Automatic (SessionStart Check)

When the plugin loads, a **SessionStart** hook automatically runs `check-sidecar.sh --quiet` to verify the binary is available. If the binary is missing, a failure message is shown at the start of the session.

### Install Hooks to Settings (Recommended)

Use `install-hooks.sh` to merge the sidecar hooks into your Claude Code settings. This works alongside any existing hooks you have configured.

**Project-level** (recommended -- applies to current project only):

```bash
bash plugin/scripts/install-hooks.sh --scope project
```

This writes to `.claude/settings.local.json`, which is typically gitignored and personal to you.

**User-level** (applies to all projects):

```bash
bash plugin/scripts/install-hooks.sh --scope user
```

This writes to `~/.claude/settings.json`.

#### Key behaviors

* **Non-destructive**: Existing hooks are preserved. The sidecar hooks are appended alongside them.
* **Idempotent**: Running the install script multiple times does not create duplicate entries.
* **Creates files if needed**: If the settings file or directory does not exist, they are created.
* **Validates JSON**: The script validates JSON before and after writing.

### Uninstall Hooks

To remove sidecar hooks while preserving all other hooks:

```bash
bash plugin/scripts/uninstall-hooks.sh --scope project
bash plugin/scripts/uninstall-hooks.sh --scope user
```

### How Hooks Integrate with Existing Hooks

Claude Code settings support multiple hooks per event. When you install sidecar hooks via `install-hooks.sh`, they are appended to the existing `PreToolUse` and `PostToolUse` arrays. Multiple hooks for the same event run in sequence, so existing hooks continue to work as before.

For example, if you already have a custom PreToolUse hook:

```json
{
  "hooks": {
    "PreToolUse": [
      { "matcher": "Bash", "hooks": [{ "type": "command", "command": "my-custom-check" }] }
    ]
  }
}
```

After running `install-hooks.sh`, the file will contain both:

```json
{
  "hooks": {
    "PreToolUse": [
      { "matcher": "Bash", "hooks": [{ "type": "command", "command": "my-custom-check" }] },
      { "matcher": "*", "hooks": [{ "type": "command", "command": "claude-pretool-sidecar", "timeout": 30 }] }
    ],
    "PostToolUse": [
      { "matcher": "*", "hooks": [{ "type": "command", "command": "claude-pretool-sidecar --post-tool", "timeout": 10 }] }
    ]
  }
}
```

## What the Plugin Provides

### Hooks

The plugin registers hooks that route all tool calls through the sidecar:

* **SessionStart** -- Verifies the sidecar binary is available when a session begins.
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

* `scripts/check-sidecar.sh` -- Health check script that verifies binary installation, config file presence, config validation, and hook registration. Supports `--quiet` (errors only) and `--install-hint` (suggests install script) flags.
* `scripts/install-hooks.sh` -- Installs sidecar hooks into Claude Code settings (project or user scope). Non-destructive, idempotent.
* `scripts/uninstall-hooks.sh` -- Removes sidecar hooks from Claude Code settings. Preserves other hooks.

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
