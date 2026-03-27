#!/bin/bash
# dangerous-command-blocker.sh — Provider for claude-pretool-sidecar
#
# Denies Bash commands matching dangerous patterns, allows everything else.
# Non-Bash tools are passed through (no opinion).
#
# HOW TO CUSTOMIZE:
#   Edit the DANGEROUS_PATTERNS array below to add/remove patterns.
#   Each pattern is a grep -E (extended regex) expression matched against
#   the full command string.
#
# USAGE:
#   Configure in .claude-pretool-sidecar.toml:
#     [[providers]]
#     name = "dangerous-command-blocker"
#     command = "/path/to/dangerous-command-blocker.sh"
#
# PROTOCOL:
#   stdin:  JSON with tool_name, tool_input, etc.
#   stdout: JSON with decision (allow|deny|passthrough) and optional reason

set -euo pipefail

# --- Customize these patterns ---
# Each line is an extended regex matched against the command string.
# Add or remove patterns as needed for your environment.
DANGEROUS_PATTERNS=(
  'rm\s+(-[a-zA-Z]*f[a-zA-Z]*\s+)*-[a-zA-Z]*r'   # rm -rf, rm -fr, rm -r -f, etc.
  'rm\s+(-[a-zA-Z]*r[a-zA-Z]*\s+)*-[a-zA-Z]*f'   # rm -rf in reversed flag order
  '\bdd\s+if='                                      # dd if=... (raw disk writes)
  '\bmkfs\b'                                        # mkfs (format filesystem)
  '\bformat\b'                                      # format command
  'chmod\s+777'                                     # chmod 777 (world-writable)
  'chmod\s+-R\s+777'                                # recursive chmod 777
  '\b:\(\)\{.*\|.*:;'                               # fork bomb pattern
  '>\s*/dev/sd[a-z]'                                # redirect to raw disk device
  '\bshred\b'                                       # shred (secure delete)
)
# --- End of customizable patterns ---

# Read the full JSON payload from stdin (protocol requirement)
INPUT=$(cat)

# Extract tool_name — only inspect Bash commands
TOOL_NAME=$(echo "$INPUT" | python3 -c "import sys,json; print(json.load(sys.stdin).get('tool_name',''))" 2>/dev/null || echo "")

if [ "$TOOL_NAME" != "Bash" ]; then
  # Not a Bash command — no opinion
  echo '{"decision": "passthrough", "reason": "not a Bash tool call"}'
  exit 0
fi

# Extract the command string
COMMAND=$(echo "$INPUT" | python3 -c "import sys,json; print(json.load(sys.stdin).get('tool_input',{}).get('command',''))" 2>/dev/null || echo "")

if [ -z "$COMMAND" ]; then
  echo '{"decision": "passthrough", "reason": "no command found in input"}'
  exit 0
fi

# Check each pattern
for pattern in "${DANGEROUS_PATTERNS[@]}"; do
  if echo "$COMMAND" | grep -qE "$pattern"; then
    # Escape the command for JSON output
    ESCAPED_CMD=$(echo "$COMMAND" | head -c 200 | sed 's/"/\\"/g' | tr '\n' ' ')
    echo "{\"decision\": \"deny\", \"reason\": \"Blocked dangerous pattern: ${pattern} in command: ${ESCAPED_CMD}\"}"
    exit 0
  fi
done

# No dangerous patterns matched — allow
echo '{"decision": "allow", "reason": "command passed safety checks"}'
