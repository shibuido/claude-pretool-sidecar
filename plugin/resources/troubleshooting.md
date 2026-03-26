# Troubleshooting Guide

Common issues and solutions for `claude-pretool-sidecar`.

## Binary Not Found

**Symptom:** Hook fails with "command not found" or similar error.

**Solutions:**

* Verify installation: `which claude-pretool-sidecar`
* If installed via `cargo install`, ensure `~/.cargo/bin` is in your PATH
* Use an absolute path in the hook command instead of relying on PATH:
  ```json
  "command": "/home/user/.cargo/bin/claude-pretool-sidecar"
  ```
* If using the plugin, run `scripts/check-sidecar.sh` to verify

## Config File Not Loading

**Symptom:** Sidecar starts but uses default behavior instead of your configuration.

**Solutions:**

* Check config file locations in priority order:
  1. `$CLAUDE_PRETOOL_SIDECAR_CONFIG` environment variable
  2. `.claude-pretool-sidecar.toml` in current directory
  3. `~/.config/claude-pretool-sidecar/config.toml`
  4. `~/.claude-pretool-sidecar.toml`
* Validate your config: `claude-pretool-sidecar --validate`
* Check file permissions: the config must be readable by the user running Claude Code
* Ensure the TOML syntax is valid (no trailing commas, proper quoting)

## Provider Crashing or Timing Out

**Symptom:** Audit log shows provider errors or timeouts. Decisions default unexpectedly.

**Solutions:**

* Test the provider manually:
  ```bash
  echo '{"tool_name":"Bash","tool_input":{"command":"echo test"},"hook_event_name":"PreToolUse","session_id":"test","cwd":"/tmp"}' | /path/to/provider
  ```
* Check provider command exists and is executable: `ls -la /path/to/provider`
* Increase the provider timeout in config:
  ```toml
  [[providers]]
  name = "slow-checker"
  command = "/path/to/checker"
  timeout = 15000  # 15 seconds
  ```
* Check provider stderr output for error messages
* Verify provider returns valid JSON with a `vote` field

## Hook Not Firing

**Symptom:** Tool calls proceed without any sidecar involvement. No audit log entries.

**Solutions:**

* Check hook registration in Claude Code settings:
  ```bash
  cat .claude/settings.json 2>/dev/null
  cat .claude/settings.local.json 2>/dev/null
  cat ~/.claude/settings.json 2>/dev/null
  ```
* Verify the hook format is correct. For settings.json:
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
* If installed as a plugin, verify the plugin is loaded (check `--plugin-dir` flag)
* Restart Claude Code after changing hook configuration

## Audit Log Not Writing

**Symptom:** Sidecar appears to work but no audit entries are created.

**Solutions:**

* Check audit configuration:
  ```toml
  [audit]
  enabled = true
  output = "/path/to/audit.jsonl"
  ```
* Verify the output directory exists and is writable
* If using `"stderr"`, check Claude Code's hook error output
* Check file permissions on the audit log file

## All Tools Being Blocked

**Symptom:** Every tool call is denied.

**Solutions:**

* Check quorum settings -- `min_allow` may be higher than the number of vote-mode providers
* Check `error_policy` -- if set to `"deny"` and providers are crashing, all errors become denials
* Check `default_decision` -- if set to `"deny"`, any unmet quorum results in denial
* Test individual providers to see if they are all voting "deny"

## All Tools Passing Through

**Symptom:** No tool calls are being blocked even when they should be.

**Solutions:**

* Verify `min_allow` is greater than 0 (if 0, the quorum is always met trivially)
* Check that providers have `mode = "vote"` not `mode = "fyi"`
* Verify providers are returning `"vote": "allow"` or `"vote": "deny"` (not empty responses)
* Check the audit log to see what votes providers are actually returning

## Permission Errors

**Symptom:** Sidecar fails with permission-related errors.

**Solutions:**

* Check that the sidecar binary is executable: `chmod +x /path/to/claude-pretool-sidecar`
* Check that provider scripts are executable: `chmod +x /path/to/provider`
* Check that log directories exist and are writable
* On SELinux/AppArmor systems, check security policy constraints
