---
name: file-issue
description: Help file GitHub issues for claude-pretool-sidecar. Triggers on "file issue", "report bug", "sidecar bug", "create issue for sidecar".
---

# File an Issue for claude-pretool-sidecar

You are helping the user file a GitHub issue for `claude-pretool-sidecar`. Gather context, format the issue, and create it via the `gh` CLI.

## Step 1: Gather Context

Collect the following information automatically:

```bash
# Sidecar version
claude-pretool-sidecar --version 2>&1 || echo "not installed"

# Claude Code version
claude --version 2>&1 || echo "not found"

# OS information
uname -a

# Config file location and contents (redact sensitive values)
echo $CLAUDE_PRETOOL_SIDECAR_CONFIG
cat .claude-pretool-sidecar.toml 2>/dev/null || \
  cat ~/.config/claude-pretool-sidecar/config.toml 2>/dev/null || \
  cat ~/.claude-pretool-sidecar.toml 2>/dev/null || \
  echo "no config found"
```

## Step 2: Ask the User

Ask the user to describe:

1. **What happened** -- the observed behavior
2. **What was expected** -- the desired behavior
3. **Steps to reproduce** -- how to trigger the issue
4. **Severity** -- is it a crash, incorrect behavior, or enhancement request?

## Step 3: Check Audit Logs

If audit logging is enabled, look at recent log entries for relevant errors:

```bash
# Check common audit log locations
tail -20 ~/.local/share/claude-pretool-sidecar/audit.jsonl 2>/dev/null
```

## Step 4: Create the Issue

Format and create the issue using `gh`. Always include `--label byAI`.

```bash
gh issue create \
  --repo shibuido/claude-pretool-sidecar \
  --title "Bug: <short description>" \
  --label byAI \
  --body '## Description

<What happened vs what was expected>

## Steps to Reproduce

1. <step>
2. <step>

## Environment

* **Sidecar version:** <version>
* **Claude Code version:** <version>
* **OS:** <os info>

## Configuration

```toml
<redacted config>
```

## Audit Log (if relevant)

```json
<relevant log entries>
```

## Additional Context

<any other details>'
```

## Important Notes

* Always use `--label byAI` when creating issues via CLI
* Redact any sensitive values from the config (API keys, tokens, internal paths)
* If the user mentions provider-specific issues, note the provider name and command
* Reference related issues if the user mentions them
* Use single quotes for the `--body` content to preserve markdown formatting
