#!/bin/bash
# rate-limiter.sh — Provider for claude-pretool-sidecar
#
# Tracks tool call frequency and denies if calls exceed a threshold
# per minute. Uses temp files to persist state between invocations.
#
# HOW TO CUSTOMIZE:
#   CPTS_RATE_LIMIT       Max calls per minute (default: 30)
#   CPTS_RATE_STATE_DIR   Directory for state files (default: /tmp/claude-pretool-rate)
#
# HOW IT WORKS:
#   Each invocation appends a timestamp to a state file. On each call,
#   timestamps older than 60 seconds are pruned. If the remaining count
#   exceeds the limit, the call is denied.
#
# USAGE:
#   Configure in .claude-pretool-sidecar.toml:
#     [[providers]]
#     name = "rate-limiter"
#     command = "/path/to/rate-limiter.sh"
#
# PROTOCOL:
#   stdin:  JSON with tool_name, tool_input, etc.
#   stdout: JSON with decision (deny|passthrough) and optional reason

set -euo pipefail

# --- Customize these settings ---
MAX_PER_MINUTE="${CPTS_RATE_LIMIT:-30}"
STATE_DIR="${CPTS_RATE_STATE_DIR:-/tmp/claude-pretool-rate}"
# --- End of customizable settings ---

# Read the full JSON payload from stdin (protocol requirement)
INPUT=$(cat)

# Ensure state directory exists
mkdir -p "$STATE_DIR"

# State file — one file tracks all calls (simple approach)
STATE_FILE="${STATE_DIR}/calls.log"

# Current time in epoch seconds
NOW=$(date +%s)

# Cutoff: 60 seconds ago
CUTOFF=$((NOW - 60))

# Create state file if it doesn't exist
touch "$STATE_FILE"

# Prune old entries and count recent ones (atomic-ish via temp file)
TEMP_FILE="${STATE_DIR}/calls.tmp.$$"
awk -v cutoff="$CUTOFF" '$1 >= cutoff' "$STATE_FILE" > "$TEMP_FILE" 2>/dev/null || true
mv "$TEMP_FILE" "$STATE_FILE"

# Count calls in the current window
CALL_COUNT=$(wc -l < "$STATE_FILE" | tr -d ' ')

if [ "$CALL_COUNT" -ge "$MAX_PER_MINUTE" ]; then
  echo "{\"decision\": \"deny\", \"reason\": \"Rate limit exceeded: ${CALL_COUNT}/${MAX_PER_MINUTE} calls in the last 60 seconds\"}"
  exit 0
fi

# Record this call
echo "$NOW" >> "$STATE_FILE"

# Under the limit — passthrough (let other providers decide)
echo "{\"decision\": \"passthrough\", \"reason\": \"Rate OK: $((CALL_COUNT + 1))/${MAX_PER_MINUTE} calls in window\"}"
