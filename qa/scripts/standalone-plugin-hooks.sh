#!/bin/bash
# QA Tests: Plugin Hook Configuration Validation
#
# Validates hooks.json structure: matchers, commands, flags, timeouts,
# and hook event types.
#
# Requires: jq

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
QA_DIR="$(dirname "$SCRIPT_DIR")"
PROJECT_DIR="$(dirname "$QA_DIR")"
HOOKS_JSON="$PROJECT_DIR/plugin/hooks/hooks.json"

PASS=0
FAIL=0

pass() { PASS=$((PASS + 1)); echo "  PASS: $1"; }
fail() { FAIL=$((FAIL + 1)); echo "  FAIL: $1"; }

echo "=== Plugin Hook Configuration Tests ==="

if [ ! -f "$HOOKS_JSON" ]; then
  fail "hooks.json not found"
  echo ""
  echo "=== Results: $PASS passed, $FAIL failed ==="
  exit 1
fi

# --- Test 1: PreToolUse matcher is "*" ---
echo "[1] PreToolUse matcher is '*'"
PRE_MATCHER=$(jq -r '.hooks.PreToolUse[0].matcher // "missing"' "$HOOKS_JSON")
if [ "$PRE_MATCHER" = "*" ]; then
  pass "PreToolUse matcher is '*'"
else
  fail "PreToolUse matcher is '$PRE_MATCHER', expected '*'"
fi

# --- Test 2: PostToolUse matcher is "*" ---
echo "[2] PostToolUse matcher is '*'"
POST_MATCHER=$(jq -r '.hooks.PostToolUse[0].matcher // "missing"' "$HOOKS_JSON")
if [ "$POST_MATCHER" = "*" ]; then
  pass "PostToolUse matcher is '*'"
else
  fail "PostToolUse matcher is '$POST_MATCHER', expected '*'"
fi

# --- Test 3: PreToolUse hook command references claude-pretool-sidecar ---
echo "[3] PreToolUse hook command references claude-pretool-sidecar"
PRE_CMD=$(jq -r '.hooks.PreToolUse[0].hooks[0].command // "missing"' "$HOOKS_JSON")
if echo "$PRE_CMD" | grep -q "claude-pretool-sidecar"; then
  pass "PreToolUse command contains 'claude-pretool-sidecar'"
else
  fail "PreToolUse command '$PRE_CMD' does not reference claude-pretool-sidecar"
fi

# --- Test 4: PostToolUse hook command references claude-pretool-sidecar ---
echo "[4] PostToolUse hook command references claude-pretool-sidecar"
POST_CMD=$(jq -r '.hooks.PostToolUse[0].hooks[0].command // "missing"' "$HOOKS_JSON")
if echo "$POST_CMD" | grep -q "claude-pretool-sidecar"; then
  pass "PostToolUse command contains 'claude-pretool-sidecar'"
else
  fail "PostToolUse command '$POST_CMD' does not reference claude-pretool-sidecar"
fi

# --- Test 5: PostToolUse hook uses --post-tool flag ---
echo "[5] PostToolUse hook uses --post-tool flag"
if echo "$POST_CMD" | grep -q "\-\-post-tool"; then
  pass "PostToolUse command includes --post-tool flag"
else
  fail "PostToolUse command missing --post-tool flag: '$POST_CMD'"
fi

# --- Test 6: SessionStart hook exists ---
echo "[6] SessionStart hook exists"
SS_COUNT=$(jq '.hooks.SessionStart | length' "$HOOKS_JSON" 2>/dev/null || echo "0")
if [ "$SS_COUNT" -gt 0 ]; then
  # Check it references check-sidecar.sh
  SS_CMD=$(jq -r '.hooks.SessionStart[0].hooks[0].command // "missing"' "$HOOKS_JSON")
  if echo "$SS_CMD" | grep -q "check-sidecar"; then
    pass "SessionStart hook exists and references check-sidecar.sh"
  else
    pass "SessionStart hook exists (command: $SS_CMD)"
  fi
else
  fail "SessionStart hook not found"
fi

# --- Test 7: PreToolUse timeout is reasonable (5-30s) ---
echo "[7] PreToolUse timeout is reasonable (5-30s)"
PRE_TIMEOUT=$(jq '.hooks.PreToolUse[0].hooks[0].timeout // 0' "$HOOKS_JSON")
if [ "$PRE_TIMEOUT" -ge 5 ] && [ "$PRE_TIMEOUT" -le 30 ]; then
  pass "PreToolUse timeout is ${PRE_TIMEOUT}s (within 5-30s range)"
else
  fail "PreToolUse timeout is ${PRE_TIMEOUT}s (outside 5-30s range)"
fi

# --- Test 8: PostToolUse timeout is reasonable (5-30s) ---
echo "[8] PostToolUse timeout is reasonable (5-30s)"
POST_TIMEOUT=$(jq '.hooks.PostToolUse[0].hooks[0].timeout // 0' "$HOOKS_JSON")
if [ "$POST_TIMEOUT" -ge 5 ] && [ "$POST_TIMEOUT" -le 30 ]; then
  pass "PostToolUse timeout is ${POST_TIMEOUT}s (within 5-30s range)"
else
  fail "PostToolUse timeout is ${POST_TIMEOUT}s (outside 5-30s range)"
fi

# --- Test 9: SessionStart timeout is reasonable (1-10s) ---
echo "[9] SessionStart timeout is reasonable (1-10s)"
SS_TIMEOUT=$(jq '.hooks.SessionStart[0].hooks[0].timeout // 0' "$HOOKS_JSON")
if [ "$SS_TIMEOUT" -ge 1 ] && [ "$SS_TIMEOUT" -le 10 ]; then
  pass "SessionStart timeout is ${SS_TIMEOUT}s (within 1-10s range)"
else
  fail "SessionStart timeout is ${SS_TIMEOUT}s (outside 1-10s range)"
fi

# --- Test 10: All hook types are "command" ---
echo "[10] All hook types are 'command'"
ALL_TYPES=$(jq -r '[.hooks[][] | .hooks[]? | .type] | unique | .[]' "$HOOKS_JSON" 2>/dev/null)
if [ "$ALL_TYPES" = "command" ]; then
  pass "all hook types are 'command'"
else
  fail "unexpected hook types: $ALL_TYPES"
fi

# --- Test 11: Exactly 3 hook events defined ---
echo "[11] Exactly 3 hook events defined (SessionStart, PreToolUse, PostToolUse)"
EVENT_COUNT=$(jq '.hooks | keys | length' "$HOOKS_JSON")
if [ "$EVENT_COUNT" -eq 3 ]; then
  EVENTS=$(jq -r '.hooks | keys | sort | join(", ")' "$HOOKS_JSON")
  pass "3 hook events defined: $EVENTS"
else
  fail "expected 3 hook events, found $EVENT_COUNT"
fi

echo ""
echo "=== Results: $PASS passed, $FAIL failed ==="
[ "$FAIL" -eq 0 ] || exit 1
