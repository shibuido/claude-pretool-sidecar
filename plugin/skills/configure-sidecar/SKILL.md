---
name: configure-sidecar
description: Help users configure the claude-pretool-sidecar. Triggers on "configure sidecar", "set up pretool", "create sidecar config", "edit provider config".
---

# Configure claude-pretool-sidecar

You are helping the user configure `claude-pretool-sidecar`. Follow these steps:

## 1. Check for Existing Configuration

Search for an existing config file in these locations (in priority order):

1. Check if `$CLAUDE_PRETOOL_SIDECAR_CONFIG` environment variable is set
2. `.claude-pretool-sidecar.toml` in the current project directory
3. `~/.config/claude-pretool-sidecar/config.toml` (XDG standard)
4. `~/.claude-pretool-sidecar.toml` (home directory fallback)

If a config file exists, read it and show the user what is currently configured.

## 2. Determine What to Configure

Ask the user what they want to set up:

* **Providers** -- external scripts/binaries that vote on tool approval
* **Quorum rules** -- how votes are aggregated (min_allow, max_deny, error_policy)
* **Timeouts** -- per-provider and total timeout limits
* **Audit logging** -- where to write decision audit logs

## 3. Create or Edit Configuration

Use the TOML config format documented in `resources/config-schema.md` (relative to plugin root).

### Minimal config (logging only):

```toml
[quorum]
min_allow = 0
default_decision = "passthrough"

[[providers]]
name = "logger"
command = "claude-pretool-logger"
mode = "fyi"
```

### Typical config (one voter + logger):

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
command = "/path/to/your/checker"
mode = "vote"

[[providers]]
name = "audit-logger"
command = "claude-pretool-logger"
args = ["--output", "~/.local/share/claude-pretool-sidecar/audit.jsonl"]
mode = "fyi"
```

## 4. Validate Configuration

After writing the config file, verify it by running:

```bash
claude-pretool-sidecar --validate
```

If the binary is not installed, remind the user to install it first (see `resources/hook-setup.md`).

## 5. Verify Hook Registration

Check that the sidecar is registered as a hook in Claude Code settings. Look at:

* `.claude/settings.json` or `.claude/settings.local.json` (project level)
* `~/.claude/settings.json` (user level)

If hooks are not configured and the plugin is not installed, guide the user through manual hook setup.

## Important Notes

* Provider commands must be executable and in PATH (or use absolute paths)
* The `mode` field must be either `"vote"` (participates in quorum) or `"fyi"` (logging only, output ignored)
* Environment variables with prefix `CPTS_` can override config values (e.g., `CPTS_MIN_ALLOW`, `CPTS_TIMEOUT`)
* Config file uses TOML format -- comments are supported with `#`
