#!/usr/bin/env bash
# uninstall-hooks.sh — Remove claude-pretool-sidecar hooks from Claude Code settings
#
# Usage:
#   bash uninstall-hooks.sh --scope project   # from .claude/settings.local.json
#   bash uninstall-hooks.sh --scope user      # from ~/.claude/settings.json
#
# Requires: jq

set -euo pipefail

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
            echo "  project  Remove hooks from .claude/settings.local.json"
            echo "  user     Remove hooks from ~/.claude/settings.json"
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
    exit 1
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

echo "=== Removing claude-pretool-sidecar hooks ==="
echo "  Scope: $SCOPE"
echo "  Settings file: $SETTINGS_FILE"
echo

# --- Check file exists ---
if [[ ! -f "$SETTINGS_FILE" ]]; then
    echo "  Settings file does not exist. Nothing to remove."
    echo
    echo "=== Done (no changes) ==="
    exit 0
fi

SETTINGS="$(cat "$SETTINGS_FILE")"

# Validate JSON
if ! echo "$SETTINGS" | jq empty 2>/dev/null; then
    echo "Error: Settings file is not valid JSON: $SETTINGS_FILE" >&2
    exit 1
fi

# --- Check if any sidecar hooks exist ---
HAS_PRETOOL=false
HAS_POSTTOOL=false

if echo "$SETTINGS" | jq -e '.hooks.PreToolUse // [] | map(select(.hooks[]?.command | test("claude-pretool-sidecar"))) | length > 0' >/dev/null 2>&1; then
    HAS_PRETOOL=true
fi

if echo "$SETTINGS" | jq -e '.hooks.PostToolUse // [] | map(select(.hooks[]?.command | test("claude-pretool-sidecar"))) | length > 0' >/dev/null 2>&1; then
    HAS_POSTTOOL=true
fi

if [[ "$HAS_PRETOOL" == false && "$HAS_POSTTOOL" == false ]]; then
    echo "  No sidecar hooks found. Nothing to remove."
    echo
    echo "=== Done (no changes) ==="
    exit 0
fi

# --- Remove sidecar hooks, keeping all others ---
UPDATED="$SETTINGS"

if [[ "$HAS_PRETOOL" == true ]]; then
    UPDATED="$(echo "$UPDATED" | jq '.hooks.PreToolUse = [.hooks.PreToolUse[]? | select(.hooks | all(.command | test("claude-pretool-sidecar") | not))]')"
    echo "  Removed PreToolUse sidecar hook"
fi

if [[ "$HAS_POSTTOOL" == true ]]; then
    UPDATED="$(echo "$UPDATED" | jq '.hooks.PostToolUse = [.hooks.PostToolUse[]? | select(.hooks | all(.command | test("claude-pretool-sidecar") | not))]')"
    echo "  Removed PostToolUse sidecar hook"
fi

# Clean up empty arrays
UPDATED="$(echo "$UPDATED" | jq '
    if (.hooks.PreToolUse | length) == 0 then del(.hooks.PreToolUse) else . end |
    if (.hooks.PostToolUse | length) == 0 then del(.hooks.PostToolUse) else . end |
    if (.hooks | length) == 0 then del(.hooks) else . end
')"

# --- Validate and write ---
if ! echo "$UPDATED" | jq empty 2>/dev/null; then
    echo "Error: Generated JSON is invalid. Aborting without writing." >&2
    exit 1
fi

echo "$UPDATED" | jq '.' > "$SETTINGS_FILE"
echo
echo "  Written to: $SETTINGS_FILE"

# Final validation
if jq empty "$SETTINGS_FILE" 2>/dev/null; then
    echo "  JSON validation: OK"
else
    echo "  JSON validation: FAILED" >&2
    exit 1
fi

echo
echo "=== Uninstall complete ==="
echo
echo "Sidecar hooks have been removed. Other hooks are preserved."
echo "Start a new Claude Code session for changes to take effect."
