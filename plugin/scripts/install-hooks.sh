#!/usr/bin/env bash
# install-hooks.sh — Install claude-pretool-sidecar hooks into Claude Code settings
#
# Usage:
#   bash install-hooks.sh --scope project   # writes to .claude/settings.local.json
#   bash install-hooks.sh --scope user      # writes to ~/.claude/settings.json
#
# Requires: jq

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# --- Argument parsing ---
SCOPE=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --scope)
            SCOPE="$2"
            shift 2
            ;;
        --scope=*)
            SCOPE="${1#--scope=}"
            shift
            ;;
        -h|--help)
            echo "Usage: $0 --scope project|user"
            echo
            echo "  project  Install hooks to .claude/settings.local.json (current project)"
            echo "  user     Install hooks to ~/.claude/settings.json (all projects)"
            exit 0
            ;;
        *)
            echo "Error: Unknown argument: $1" >&2
            echo "Usage: $0 --scope project|user" >&2
            exit 1
            ;;
    esac
done

if [[ -z "$SCOPE" ]]; then
    echo "Error: --scope is required (project or user)" >&2
    echo "Usage: $0 --scope project|user" >&2
    exit 1
fi

# --- Dependency check ---
if ! command -v jq >/dev/null 2>&1; then
    echo "Error: jq is required but not found in PATH" >&2
    echo "Install it: https://jqlang.github.io/jq/download/" >&2
    exit 1
fi

# --- Binary check ---
if ! command -v claude-pretool-sidecar >/dev/null 2>&1; then
    echo "Warning: claude-pretool-sidecar binary not found in PATH" >&2
    echo "         Hooks will be installed but won't work until the binary is available." >&2
    echo "         Install with: cargo install --path <source-dir>" >&2
    echo
fi

# --- Determine settings file path ---
case "$SCOPE" in
    project)
        SETTINGS_FILE=".claude/settings.local.json"
        ;;
    user)
        SETTINGS_FILE="${HOME}/.claude/settings.json"
        ;;
    *)
        echo "Error: --scope must be 'project' or 'user', got: $SCOPE" >&2
        exit 1
        ;;
esac

echo "=== Installing claude-pretool-sidecar hooks ==="
echo "  Scope: $SCOPE"
echo "  Settings file: $SETTINGS_FILE"
echo

# --- Hook definitions (as jq-compatible JSON) ---
PRETOOL_HOOK='{
  "matcher": "*",
  "hooks": [
    {
      "type": "command",
      "command": "claude-pretool-sidecar",
      "timeout": 30
    }
  ]
}'

POSTTOOL_HOOK='{
  "matcher": "*",
  "hooks": [
    {
      "type": "command",
      "command": "claude-pretool-sidecar --post-tool",
      "timeout": 10
    }
  ]
}'

# Marker used to detect our hooks (to avoid duplicates)
HOOK_MARKER="claude-pretool-sidecar"

# --- Ensure parent directory exists ---
SETTINGS_DIR="$(dirname "$SETTINGS_FILE")"
if [[ ! -d "$SETTINGS_DIR" ]]; then
    echo "  Creating directory: $SETTINGS_DIR"
    mkdir -p "$SETTINGS_DIR"
fi

# --- Load or create settings ---
if [[ -f "$SETTINGS_FILE" ]]; then
    SETTINGS="$(cat "$SETTINGS_FILE")"
    # Validate existing JSON
    if ! echo "$SETTINGS" | jq empty 2>/dev/null; then
        echo "Error: Existing settings file is not valid JSON: $SETTINGS_FILE" >&2
        exit 1
    fi
    echo "  Loaded existing settings"
else
    SETTINGS='{}'
    echo "  Creating new settings file"
fi

# --- Check for existing sidecar hooks (idempotency) ---
HAS_PRETOOL=false
HAS_POSTTOOL=false

if echo "$SETTINGS" | jq -e '.hooks.PreToolUse // [] | map(select(.hooks[]?.command | test("claude-pretool-sidecar"))) | length > 0' >/dev/null 2>&1; then
    HAS_PRETOOL=true
fi

if echo "$SETTINGS" | jq -e '.hooks.PostToolUse // [] | map(select(.hooks[]?.command | test("claude-pretool-sidecar"))) | length > 0' >/dev/null 2>&1; then
    HAS_POSTTOOL=true
fi

if [[ "$HAS_PRETOOL" == true && "$HAS_POSTTOOL" == true ]]; then
    echo "  Sidecar hooks are already installed. Nothing to do."
    echo
    echo "=== Done (no changes) ==="
    exit 0
fi

# --- Build updated settings ---
UPDATED="$SETTINGS"

# Ensure hooks object exists
UPDATED="$(echo "$UPDATED" | jq '.hooks //= {}')"

# Add PreToolUse hook if not present
if [[ "$HAS_PRETOOL" == false ]]; then
    UPDATED="$(echo "$UPDATED" | jq --argjson hook "$PRETOOL_HOOK" '.hooks.PreToolUse = (.hooks.PreToolUse // []) + [$hook]')"
    echo "  Added PreToolUse hook"
else
    echo "  PreToolUse hook already present (skipped)"
fi

# Add PostToolUse hook if not present
if [[ "$HAS_POSTTOOL" == false ]]; then
    UPDATED="$(echo "$UPDATED" | jq --argjson hook "$POSTTOOL_HOOK" '.hooks.PostToolUse = (.hooks.PostToolUse // []) + [$hook]')"
    echo "  Added PostToolUse hook"
else
    echo "  PostToolUse hook already present (skipped)"
fi

# --- Validate output JSON ---
if ! echo "$UPDATED" | jq empty 2>/dev/null; then
    echo "Error: Generated JSON is invalid. Aborting without writing." >&2
    exit 1
fi

# --- Write settings file ---
echo "$UPDATED" | jq '.' > "$SETTINGS_FILE"
echo
echo "  Written to: $SETTINGS_FILE"

# --- Final validation ---
if jq empty "$SETTINGS_FILE" 2>/dev/null; then
    echo "  JSON validation: OK"
else
    echo "  JSON validation: FAILED (file may be corrupted)" >&2
    exit 1
fi

echo
echo "=== Installation complete ==="
echo
echo "Next steps:"
echo "  1. Ensure claude-pretool-sidecar binary is in PATH"
echo "  2. Create a config file (.claude-pretool-sidecar.toml)"
echo "  3. Start a new Claude Code session to activate the hooks"
