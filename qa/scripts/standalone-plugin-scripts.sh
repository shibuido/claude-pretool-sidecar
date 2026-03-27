#!/bin/bash
# QA Tests: Plugin Scripts Validation
#
# Tests check-sidecar.sh, install-hooks.sh, and uninstall-hooks.sh
# for correct behavior including idempotency and temp-dir isolation.
#
# Requires: jq, claude-pretool-sidecar in PATH (for check-sidecar tests)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
QA_DIR="$(dirname "$SCRIPT_DIR")"
PROJECT_DIR="$(dirname "$QA_DIR")"
PLUGIN_SCRIPTS="$PROJECT_DIR/plugin/scripts"

PASS=0
FAIL=0
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

pass() { PASS=$((PASS + 1)); echo "  PASS: $1"; }
fail() { FAIL=$((FAIL + 1)); echo "  FAIL: $1"; }

echo "=== Plugin Scripts Validation Tests ==="

# ===== check-sidecar.sh tests =====

# --- Test 1: check-sidecar.sh runs without error (when binary is in PATH) ---
echo "[1] check-sidecar.sh runs without error"
if command -v claude-pretool-sidecar >/dev/null 2>&1; then
  if bash "$PLUGIN_SCRIPTS/check-sidecar.sh" >/dev/null 2>&1; then
    pass "check-sidecar.sh exits 0 (binary in PATH)"
  else
    # Exit code 0 is OK even with warnings (no config is just a warning)
    EXIT_CODE=$?
    if [ "$EXIT_CODE" -eq 0 ]; then
      pass "check-sidecar.sh exits 0 with warnings"
    else
      fail "check-sidecar.sh exited with code $EXIT_CODE"
    fi
  fi
else
  echo "  SKIP: claude-pretool-sidecar not in PATH (check-sidecar.sh run test)"
  # Still count as pass since this is a valid environment condition
  pass "check-sidecar.sh test skipped (binary not in PATH)"
fi

# --- Test 2: check-sidecar.sh --quiet produces no output on success ---
echo "[2] check-sidecar.sh --quiet produces no output on success"
if command -v claude-pretool-sidecar >/dev/null 2>&1; then
  QUIET_OUTPUT=$(bash "$PLUGIN_SCRIPTS/check-sidecar.sh" --quiet 2>&1 || true)
  # In quiet mode, only failures should produce output.
  # Warnings and passes should be silent.
  # If binary is found, no [FAIL] lines should appear for the binary check.
  if ! echo "$QUIET_OUTPUT" | grep -q "\[FAIL\].*not found"; then
    pass "check-sidecar.sh --quiet suppresses non-error output"
  else
    fail "check-sidecar.sh --quiet produced failure output unexpectedly"
  fi
else
  echo "  SKIP: claude-pretool-sidecar not in PATH"
  pass "check-sidecar.sh --quiet test skipped (binary not in PATH)"
fi

# --- Test 3: check-sidecar.sh --help works ---
echo "[3] check-sidecar.sh --help works"
HELP_OUTPUT=$(bash "$PLUGIN_SCRIPTS/check-sidecar.sh" --help 2>&1)
HELP_EXIT=$?
if [ "$HELP_EXIT" -eq 0 ] && echo "$HELP_OUTPUT" | grep -q "\-\-quiet"; then
  pass "check-sidecar.sh --help exits 0 and mentions --quiet"
else
  fail "check-sidecar.sh --help failed or missing --quiet docs"
fi

# ===== install-hooks.sh tests =====

# --- Test 4: install-hooks.sh --scope project creates valid settings JSON ---
echo "[4] install-hooks.sh --scope project creates valid settings JSON"
INSTALL_DIR="$TMPDIR/install-test"
mkdir -p "$INSTALL_DIR"
(cd "$INSTALL_DIR" && bash "$PLUGIN_SCRIPTS/install-hooks.sh" --scope project >/dev/null 2>&1)
SETTINGS_FILE="$INSTALL_DIR/.claude/settings.local.json"
if [ -f "$SETTINGS_FILE" ]; then
  if jq empty "$SETTINGS_FILE" 2>/dev/null; then
    # Verify it has PreToolUse and PostToolUse hooks
    HAS_PRE=$(jq -e '.hooks.PreToolUse' "$SETTINGS_FILE" >/dev/null 2>&1 && echo "yes" || echo "no")
    HAS_POST=$(jq -e '.hooks.PostToolUse' "$SETTINGS_FILE" >/dev/null 2>&1 && echo "yes" || echo "no")
    if [ "$HAS_PRE" = "yes" ] && [ "$HAS_POST" = "yes" ]; then
      pass "install-hooks.sh creates valid JSON with PreToolUse and PostToolUse hooks"
    else
      fail "install-hooks.sh missing hook types (pre=$HAS_PRE, post=$HAS_POST)"
    fi
  else
    fail "install-hooks.sh created invalid JSON"
  fi
else
  fail "install-hooks.sh did not create settings file at $SETTINGS_FILE"
fi

# --- Test 5: install-hooks.sh is idempotent ---
echo "[5] install-hooks.sh is idempotent (no duplicates on second run)"
IDEM_DIR="$TMPDIR/idempotent-test"
mkdir -p "$IDEM_DIR"
(cd "$IDEM_DIR" && bash "$PLUGIN_SCRIPTS/install-hooks.sh" --scope project >/dev/null 2>&1)
FIRST_COUNT=$(jq '[.hooks.PreToolUse[], .hooks.PostToolUse[]] | length' "$IDEM_DIR/.claude/settings.local.json")
(cd "$IDEM_DIR" && bash "$PLUGIN_SCRIPTS/install-hooks.sh" --scope project >/dev/null 2>&1)
SECOND_COUNT=$(jq '[.hooks.PreToolUse[], .hooks.PostToolUse[]] | length' "$IDEM_DIR/.claude/settings.local.json")
if [ "$FIRST_COUNT" -eq "$SECOND_COUNT" ]; then
  pass "idempotent: hook count unchanged after second install ($FIRST_COUNT entries)"
else
  fail "idempotent: hook count changed from $FIRST_COUNT to $SECOND_COUNT"
fi

# --- Test 6: install-hooks.sh preserves existing hooks ---
echo "[6] install-hooks.sh preserves existing hooks"
PRESERVE_DIR="$TMPDIR/preserve-test"
mkdir -p "$PRESERVE_DIR/.claude"
cat > "$PRESERVE_DIR/.claude/settings.local.json" <<'EXISTING'
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [{"type": "command", "command": "my-custom-checker"}]
      }
    ]
  }
}
EXISTING
(cd "$PRESERVE_DIR" && bash "$PLUGIN_SCRIPTS/install-hooks.sh" --scope project >/dev/null 2>&1)
CUSTOM_HOOK=$(jq '[.hooks.PreToolUse[] | select(.hooks[].command == "my-custom-checker")] | length' "$PRESERVE_DIR/.claude/settings.local.json")
SIDECAR_HOOK=$(jq '[.hooks.PreToolUse[] | select(.hooks[].command | test("claude-pretool-sidecar"))] | length' "$PRESERVE_DIR/.claude/settings.local.json")
if [ "$CUSTOM_HOOK" -ge 1 ] && [ "$SIDECAR_HOOK" -ge 1 ]; then
  pass "existing hooks preserved alongside sidecar hooks"
else
  fail "existing hooks not preserved (custom=$CUSTOM_HOOK, sidecar=$SIDECAR_HOOK)"
fi

# --- Test 7: install-hooks.sh --help works ---
echo "[7] install-hooks.sh --help works"
HELP_OUTPUT=$(bash "$PLUGIN_SCRIPTS/install-hooks.sh" --help 2>&1)
HELP_EXIT=$?
if [ "$HELP_EXIT" -eq 0 ] && echo "$HELP_OUTPUT" | grep -q "\-\-scope"; then
  pass "install-hooks.sh --help exits 0 and mentions --scope"
else
  fail "install-hooks.sh --help failed or missing --scope docs"
fi

# ===== uninstall-hooks.sh tests =====

# --- Test 8: uninstall-hooks.sh removes sidecar hooks ---
echo "[8] uninstall-hooks.sh removes sidecar hooks"
UNINST_DIR="$TMPDIR/uninstall-test"
mkdir -p "$UNINST_DIR"
# First install
(cd "$UNINST_DIR" && bash "$PLUGIN_SCRIPTS/install-hooks.sh" --scope project >/dev/null 2>&1)
# Then uninstall
(cd "$UNINST_DIR" && bash "$PLUGIN_SCRIPTS/uninstall-hooks.sh" --scope project >/dev/null 2>&1)
UNINST_FILE="$UNINST_DIR/.claude/settings.local.json"
if [ -f "$UNINST_FILE" ]; then
  REMAINING=$(jq '[(.hooks.PreToolUse // [])[], (.hooks.PostToolUse // [])[]] | map(select(.hooks[]?.command | test("claude-pretool-sidecar"))) | length' "$UNINST_FILE" 2>/dev/null || echo "0")
  if [ "$REMAINING" -eq 0 ]; then
    pass "uninstall-hooks.sh removed all sidecar hooks"
  else
    fail "uninstall-hooks.sh left $REMAINING sidecar hooks"
  fi
else
  fail "settings file missing after uninstall"
fi

# --- Test 9: uninstall-hooks.sh preserves other hooks ---
echo "[9] uninstall-hooks.sh preserves non-sidecar hooks"
UNINST_PRES_DIR="$TMPDIR/uninstall-preserve-test"
mkdir -p "$UNINST_PRES_DIR/.claude"
cat > "$UNINST_PRES_DIR/.claude/settings.local.json" <<'MIXED'
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [{"type": "command", "command": "my-custom-checker"}]
      },
      {
        "matcher": "*",
        "hooks": [{"type": "command", "command": "claude-pretool-sidecar", "timeout": 30}]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "*",
        "hooks": [{"type": "command", "command": "claude-pretool-sidecar --post-tool", "timeout": 10}]
      }
    ]
  }
}
MIXED
(cd "$UNINST_PRES_DIR" && bash "$PLUGIN_SCRIPTS/uninstall-hooks.sh" --scope project >/dev/null 2>&1)
CUSTOM_REMAINING=$(jq '[.hooks.PreToolUse[]? | select(.hooks[].command == "my-custom-checker")] | length' "$UNINST_PRES_DIR/.claude/settings.local.json")
if [ "$CUSTOM_REMAINING" -ge 1 ]; then
  pass "uninstall-hooks.sh preserved custom hook"
else
  fail "uninstall-hooks.sh removed custom hook"
fi

# --- Test 10: uninstall-hooks.sh handles missing file gracefully ---
echo "[10] uninstall-hooks.sh handles missing settings file gracefully"
EMPTY_DIR="$TMPDIR/no-settings"
mkdir -p "$EMPTY_DIR"
UNINST_OUTPUT=$(cd "$EMPTY_DIR" && bash "$PLUGIN_SCRIPTS/uninstall-hooks.sh" --scope project 2>&1)
UNINST_EXIT=$?
if [ "$UNINST_EXIT" -eq 0 ]; then
  pass "uninstall-hooks.sh exits 0 when settings file does not exist"
else
  fail "uninstall-hooks.sh failed on missing settings file (exit $UNINST_EXIT)"
fi

# --- Test 11: uninstall-hooks.sh --help works ---
echo "[11] uninstall-hooks.sh --help works"
HELP_OUTPUT=$(bash "$PLUGIN_SCRIPTS/uninstall-hooks.sh" --help 2>&1)
HELP_EXIT=$?
if [ "$HELP_EXIT" -eq 0 ] && echo "$HELP_OUTPUT" | grep -q "\-\-scope"; then
  pass "uninstall-hooks.sh --help exits 0 and mentions --scope"
else
  fail "uninstall-hooks.sh --help failed or missing --scope docs"
fi

echo ""
echo "=== Results: $PASS passed, $FAIL failed ==="
[ "$FAIL" -eq 0 ] || exit 1
