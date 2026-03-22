#!/bin/bash
# Configurable mock provider for QA testing.
#
# Returns a configurable decision. Reads stdin (for protocol compliance).
#
# Usage:
#   echo '{"tool_name":"Bash",...}' | provider-echo.sh allow
#   echo '{"tool_name":"Bash",...}' | provider-echo.sh deny "reason text"
#   echo '{"tool_name":"Bash",...}' | provider-echo.sh passthrough
#   echo '{"tool_name":"Bash",...}' | provider-echo.sh crash
#   echo '{"tool_name":"Bash",...}' | provider-echo.sh bad-json
#   echo '{"tool_name":"Bash",...}' | provider-echo.sh slow 3   # delay 3 seconds
#   echo '{"tool_name":"Bash",...}' | provider-echo.sh dump     # dump stdin to stderr
#
# Environment:
#   CPTS_PROVIDER_DELAY=N   Override delay in seconds (for any mode)

set -euo pipefail

DECISION="${1:-passthrough}"
REASON="${2:-}"
DELAY="${CPTS_PROVIDER_DELAY:-0}"

# Read stdin (required by protocol — provider must consume stdin)
INPUT=$(cat)

# Optional delay
if [ "$DELAY" -gt 0 ] 2>/dev/null; then
  sleep "$DELAY"
fi

case "$DECISION" in
  allow)
    echo '{"decision": "allow"}'
    ;;
  deny)
    if [ -n "$REASON" ]; then
      echo "{\"decision\": \"deny\", \"reason\": \"$REASON\"}"
    else
      echo '{"decision": "deny"}'
    fi
    ;;
  passthrough)
    echo '{"decision": "passthrough"}'
    ;;
  crash)
    exit 1
    ;;
  bad-json)
    echo "this is not valid json"
    ;;
  slow)
    SLEEP_TIME="${2:-5}"
    sleep "$SLEEP_TIME"
    echo '{"decision": "allow"}'
    ;;
  dump)
    echo "$INPUT" >&2
    echo '{"decision": "passthrough"}'
    ;;
  env-check)
    # Output env vars for testing env passthrough
    echo "{\"decision\": \"allow\", \"reason\": \"MY_VAR=$MY_VAR\"}"
    ;;
  *)
    echo '{"decision": "passthrough"}'
    ;;
esac
