#!/bin/bash
# Master runner for LIVE Claude Code QA tests.
#
# REQUIRES: Claude Code CLI installed and ANTHROPIC_API_KEY set.
# These tests make real API calls.
#
# Usage:
#   ./run-all-live-claude-code.sh
#
# For standalone tests (no Claude Code needed), see: run-all-standalone.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

SUITES_PASSED=0
SUITES_FAILED=0
SUITES_SKIPPED=0

run_suite() {
  local name="$1"
  local cmd="$2"

  echo ""
  echo "╔══════════════════════════════════════════════════════════╗"
  echo "║  $name"
  echo "╚══════════════════════════════════════════════════════════╝"
  echo ""

  local result
  result=$(eval "$cmd" 2>&1) || true
  local exit_code=$?

  echo "$result"

  if echo "$result" | grep -q "^SKIP:"; then
    SUITES_SKIPPED=$((SUITES_SKIPPED + 1))
    echo "  ⊘ Suite skipped"
  elif [ "$exit_code" -eq 0 ]; then
    SUITES_PASSED=$((SUITES_PASSED + 1))
    echo "  ✓ Suite passed"
  else
    SUITES_FAILED=$((SUITES_FAILED + 1))
    echo "  ✗ Suite FAILED"
  fi
}

echo "╔══════════════════════════════════════════════════════════╗"
echo "║  claude-pretool-sidecar — Live Claude Code QA Suite      ║"
echo "║  (requires Claude Code CLI + ANTHROPIC_API_KEY)          ║"
echo "╚══════════════════════════════════════════════════════════╝"
echo ""
echo "Date: $(date -Iseconds)"
echo "Claude Code: $(claude --version 2>/dev/null || echo 'NOT FOUND')"
echo "API Key: $([ -n "${ANTHROPIC_API_KEY:-}" ] && echo 'set' || echo 'NOT SET')"
echo ""

# Prerequisite check
if ! command -v claude > /dev/null 2>&1; then
  echo "ERROR: Claude Code CLI not found."
  echo "Install: curl -fsSL https://claude.ai/install.sh | bash"
  exit 1
fi
if [ -z "${ANTHROPIC_API_KEY:-}" ]; then
  echo "ERROR: ANTHROPIC_API_KEY not set."
  echo "Export your API key before running live tests."
  exit 1
fi

# Live test suites
run_suite "Hook Installation & Settings" "$SCRIPT_DIR/live-claude-code-hook-install.sh"
run_suite "Hook Execution via Claude CLI" "$SCRIPT_DIR/live-claude-code-hook-execution.sh"

# Summary
echo ""
echo "╔══════════════════════════════════════════════════════════╗"
echo "║  Live Claude Code QA Summary                             ║"
echo "╚══════════════════════════════════════════════════════════╝"
echo ""
echo "  Suites passed:  $SUITES_PASSED"
echo "  Suites failed:  $SUITES_FAILED"
echo "  Suites skipped: $SUITES_SKIPPED"
echo ""

if [ "$SUITES_FAILED" -gt 0 ]; then
  echo "  RESULT: FAIL ($SUITES_FAILED suite(s) failed)"
  exit 1
else
  echo "  RESULT: ALL PASS"
  exit 0
fi
