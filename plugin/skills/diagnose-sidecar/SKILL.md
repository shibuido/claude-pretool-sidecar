---
name: diagnose-sidecar
description: Help users troubleshoot claude-pretool-sidecar issues. Triggers on "sidecar not working", "hook not firing", "provider error", "debug sidecar".
---

# Diagnose claude-pretool-sidecar

You are helping the user troubleshoot issues with `claude-pretool-sidecar`. Follow this diagnostic checklist systematically.

## Step 1: Check Binary Installation

```bash
which claude-pretool-sidecar
claude-pretool-sidecar --version
```

If the binary is not found, the user needs to install it. Options:

* `cargo install --path .` from the source repository
* Download a prebuilt binary and add to PATH
* Check if installed but not in the current shell's PATH

## Step 2: Check Configuration

Find and validate the config file:

```bash
# Check environment variable override
echo $CLAUDE_PRETOOL_SIDECAR_CONFIG

# Check standard locations
ls -la .claude-pretool-sidecar.toml 2>/dev/null
ls -la ~/.config/claude-pretool-sidecar/config.toml 2>/dev/null
ls -la ~/.claude-pretool-sidecar.toml 2>/dev/null
```

If a config file is found, validate it:

```bash
claude-pretool-sidecar --validate
```

Read the config file and check for common issues:

* Provider commands that do not exist or are not executable
* Contradictory quorum settings (e.g., `min_allow` > number of vote providers)
* Invalid enum values for `error_policy` or `default_decision`

## Step 3: Check Hook Registration

Check if the sidecar is registered as a Claude Code hook:

* Read `.claude/settings.json` and `.claude/settings.local.json` in the project
* Read `~/.claude/settings.json` for user-level hooks
* If installed as a plugin, check `plugin/hooks/hooks.json`

The hooks should look like:

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
    ]
  }
}
```

## Step 4: Check Provider Scripts

For each provider in the config:

* Verify the command exists and is executable
* Test running it manually with a sample payload:

```bash
echo '{"tool_name":"Bash","tool_input":{"command":"echo hello"},"hook_event_name":"PreToolUse","session_id":"test","cwd":"/tmp"}' | /path/to/provider
```

* Check that it returns valid JSON with a `vote` field

## Step 5: Check Audit Logs

If audit logging is enabled, check the audit log file for errors:

* Look for the configured `[audit].output` path
* Check recent entries for provider errors, timeouts, or unexpected votes
* Look for patterns (e.g., one provider always timing out)

## Step 6: Check Exit Codes

Run the sidecar manually with a test payload and check the exit code:

```bash
echo '{"tool_name":"Bash","tool_input":{"command":"ls"},"hook_event_name":"PreToolUse","session_id":"test","cwd":"/tmp"}' | claude-pretool-sidecar
echo "Exit code: $?"
```

Exit codes:

* `0` -- Success, stdout contains JSON response
* `2` -- Blocking error, stderr contains error message
* Other -- Non-blocking error, logged and operation continues

## Common Issues

Refer to `resources/troubleshooting.md` (relative to plugin root) for detailed solutions to common problems including:

* Binary not found in PATH
* Config file not loading
* Provider crashing or timing out
* Hook not firing
* Audit log not writing
