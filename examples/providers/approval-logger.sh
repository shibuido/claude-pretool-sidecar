#!/bin/bash
# approval-logger.sh — FYI provider for claude-pretool-sidecar
#
# Always returns "passthrough" (FYI providers have their output ignored anyway).
# Logs every tool invocation with timestamp to a file for audit purposes.
#
# This is designed to be configured as a FYI provider — it observes but
# never blocks. Useful as a session audit trail.
#
# HOW TO CUSTOMIZE:
#   Set CPTS_AUDIT_LOG to change the output file path.
#   Default: /tmp/claude-pretool-audit.log
#
# USAGE:
#   Configure in .claude-pretool-sidecar.toml:
#     [[providers]]
#     name = "approval-logger"
#     mode = "fyi"
#     command = "/path/to/approval-logger.sh"
#
# PROTOCOL:
#   stdin:  JSON with tool_name, tool_input, etc.
#   stdout: JSON with decision "passthrough" (ignored in FYI mode)

set -euo pipefail

# --- Customize the log output path ---
LOG_FILE="${CPTS_AUDIT_LOG:-/tmp/claude-pretool-audit.log}"
# --- End of customizable settings ---

# Read the full JSON payload from stdin (protocol requirement)
INPUT=$(cat)

# Extract fields for logging
TOOL_NAME=$(echo "$INPUT" | python3 -c "import sys,json; print(json.load(sys.stdin).get('tool_name','unknown'))" 2>/dev/null || echo "unknown")
HOOK_EVENT=$(echo "$INPUT" | python3 -c "import sys,json; print(json.load(sys.stdin).get('hook_event_name','unknown'))" 2>/dev/null || echo "unknown")

# Extract a short summary of the tool input (first 200 chars of command or file_path)
SUMMARY=$(echo "$INPUT" | python3 -c "
import sys, json
data = json.load(sys.stdin)
ti = data.get('tool_input', {})
s = ti.get('command', '') or ti.get('file_path', '') or str(ti)[:200]
print(s[:200].replace(chr(10), ' '))
" 2>/dev/null || echo "(could not extract)")

TIMESTAMP=$(date -u '+%Y-%m-%dT%H:%M:%SZ')

# Append to log file (create if needed)
echo "${TIMESTAMP} [${HOOK_EVENT}] ${TOOL_NAME}: ${SUMMARY}" >> "$LOG_FILE"

# Always passthrough — this is a FYI/logging provider
echo '{"decision": "passthrough"}'
