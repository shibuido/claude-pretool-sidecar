#!/bin/bash
# Generate Claude Code hook payloads for QA testing.
#
# Usage:
#   gen-payload.sh bash "ls -la"           # Bash tool payload
#   gen-payload.sh write "/tmp/f" "hello"  # Write tool payload
#   gen-payload.sh read "/tmp/f"           # Read tool payload
#   gen-payload.sh edit "/tmp/f"           # Edit tool payload
#   gen-payload.sh raw                     # Minimal raw payload
#
# Outputs JSON to stdout.

set -euo pipefail

TOOL_TYPE="${1:-bash}"
SESSION_ID="${CPTS_SESSION_ID:-qa-session-$(date +%s)}"

case "$TOOL_TYPE" in
  bash)
    COMMAND="${2:-echo hello}"
    cat <<EOF
{"tool_name":"Bash","tool_input":{"command":"$COMMAND","description":"QA test command"},"hook_event_name":"PreToolUse","session_id":"$SESSION_ID","cwd":"$(pwd)","permission_mode":"default"}
EOF
    ;;
  write)
    FILE_PATH="${2:-/tmp/qa-test-file.txt}"
    CONTENT="${3:-test content}"
    cat <<EOF
{"tool_name":"Write","tool_input":{"file_path":"$FILE_PATH","content":"$CONTENT"},"hook_event_name":"PreToolUse","session_id":"$SESSION_ID","cwd":"$(pwd)","permission_mode":"default"}
EOF
    ;;
  read)
    FILE_PATH="${2:-/tmp/qa-test-file.txt}"
    cat <<EOF
{"tool_name":"Read","tool_input":{"file_path":"$FILE_PATH"},"hook_event_name":"PreToolUse","session_id":"$SESSION_ID","cwd":"$(pwd)","permission_mode":"default"}
EOF
    ;;
  edit)
    FILE_PATH="${2:-/tmp/qa-test-file.txt}"
    cat <<EOF
{"tool_name":"Edit","tool_input":{"file_path":"$FILE_PATH","old_string":"old","new_string":"new"},"hook_event_name":"PreToolUse","session_id":"$SESSION_ID","cwd":"$(pwd)","permission_mode":"default"}
EOF
    ;;
  post)
    COMMAND="${2:-echo hello}"
    cat <<EOF
{"tool_name":"Bash","tool_input":{"command":"$COMMAND"},"hook_event_name":"PostToolUse","session_id":"$SESSION_ID","cwd":"$(pwd)","tool_result":{"type":"text","content":"output"}}
EOF
    ;;
  raw)
    cat <<EOF
{"tool_name":"Bash","tool_input":{"command":"ls"}}
EOF
    ;;
  *)
    echo "Unknown tool type: $TOOL_TYPE" >&2
    echo "Usage: gen-payload.sh {bash|write|read|edit|post|raw} [args...]" >&2
    exit 1
    ;;
esac
