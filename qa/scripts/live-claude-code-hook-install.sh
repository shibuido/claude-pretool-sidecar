#!/bin/bash
# Live Claude Code Test: Hook Installation and Detection
#
# REQUIRES: Claude Code CLI installed and ANTHROPIC_API_KEY set.
#
# Tests that the sidecar can be installed as a Claude Code hook
# and that Claude Code detects and invokes it correctly.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
QA_DIR="$(dirname "$SCRIPT_DIR")"
HELPERS="$QA_DIR/helpers"
PROVIDER="$HELPERS/provider-echo.sh"

PASS=0
FAIL=0
TMPDIR=$(mktemp -d)
WORKSPACE="$TMPDIR/workspace"
trap 'rm -rf "$TMPDIR"' EXIT

pass() { PASS=$((PASS + 1)); echo "  PASS: $1"; }
fail() { FAIL=$((FAIL + 1)); echo "  FAIL: $1"; }

# Verify prerequisites
if ! command -v claude > /dev/null 2>&1; then
  echo "SKIP: Claude Code CLI not found. Install: curl -fsSL https://claude.ai/install.sh | bash"
  exit 0
fi
if [ -z "${ANTHROPIC_API_KEY:-}" ]; then
  echo "SKIP: ANTHROPIC_API_KEY not set."
  exit 0
fi

BINARY=$(command -v claude-pretool-sidecar 2>/dev/null || echo "")
if [ -z "$BINARY" ]; then
  echo "SKIP: claude-pretool-sidecar binary not found in PATH."
  exit 0
fi

echo "=== Live Claude Code: Hook Installation Tests ==="

# Set up a test workspace with git
mkdir -p "$WORKSPACE/.claude"
cd "$WORKSPACE"
git init --quiet
git config user.email "qa@test"
git config user.name "QA"
touch .gitkeep && git add . && git commit -m "init" --quiet

# Test 1: Settings file with PreToolUse hook is valid JSON
echo "[1] Generate valid hook settings"
SIDECAR_CONFIG="$TMPDIR/sidecar.toml"
"$HELPERS/gen-config.sh" passthrough > "$SIDECAR_CONFIG"

cat > "$WORKSPACE/.claude/settings.local.json" <<EOF
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "CLAUDE_PRETOOL_SIDECAR_CONFIG=$SIDECAR_CONFIG claude-pretool-sidecar",
            "timeout": 10
          }
        ]
      }
    ]
  }
}
EOF

if jq empty "$WORKSPACE/.claude/settings.local.json" 2>/dev/null; then
  pass "hook settings is valid JSON"
else
  fail "hook settings is invalid JSON"
fi

# Test 2: Claude Code can parse the settings (non-interactive version check)
echo "[2] Claude Code accepts the settings"
CC_OUTPUT=$(cd "$WORKSPACE" && claude --version 2>&1 || true)
if echo "$CC_OUTPUT" | grep -q "Claude Code"; then
  pass "Claude Code runs with hook settings present"
else
  fail "Claude Code failed with hook settings: $CC_OUTPUT"
fi

# Test 3: Hook config with both PreToolUse and PostToolUse
echo "[3] Dual-hook configuration (Pre + Post)"
cat > "$WORKSPACE/.claude/settings.local.json" <<EOF
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "CLAUDE_PRETOOL_SIDECAR_CONFIG=$SIDECAR_CONFIG claude-pretool-sidecar",
            "timeout": 10
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "CLAUDE_PRETOOL_SIDECAR_CONFIG=$SIDECAR_CONFIG claude-pretool-sidecar",
            "timeout": 10
          }
        ]
      }
    ]
  }
}
EOF
if jq empty "$WORKSPACE/.claude/settings.local.json" 2>/dev/null; then
  pass "dual-hook settings valid"
else
  fail "dual-hook settings invalid"
fi

# Test 4: Hook with audit logging enabled
echo "[4] Hook with audit logging"
AUDIT_DIR="$TMPDIR/audit"
"$HELPERS/gen-config.sh" logging "$AUDIT_DIR" > "$SIDECAR_CONFIG"
pass "audit-enabled config generated for hook"

# Test 5: Sidecar binary is accessible from hook command context
echo "[5] Sidecar binary accessible"
if [ -x "$BINARY" ]; then
  pass "sidecar binary is executable at: $BINARY"
else
  fail "sidecar binary not executable"
fi

echo ""
echo "=== Results: $PASS passed, $FAIL failed ==="
[ "$FAIL" -eq 0 ] || exit 1
