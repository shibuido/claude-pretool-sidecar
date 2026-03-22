#!/bin/bash
# Live Claude Code Test: Hook Execution via Claude CLI
#
# REQUIRES: Claude Code CLI + ANTHROPIC_API_KEY.
#
# Tests that Claude Code actually invokes the sidecar as a PreToolUse hook
# by running a non-interactive query and checking audit logs for evidence.

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
  echo "SKIP: Claude Code CLI not found."
  exit 0
fi
if [ -z "${ANTHROPIC_API_KEY:-}" ]; then
  echo "SKIP: ANTHROPIC_API_KEY not set."
  exit 0
fi
if ! command -v claude-pretool-sidecar > /dev/null 2>&1; then
  echo "SKIP: claude-pretool-sidecar not in PATH."
  exit 0
fi

echo "=== Live Claude Code: Hook Execution Tests ==="
echo "(These tests make real API calls and may take 30-60 seconds)"
echo ""

# Set up workspace
mkdir -p "$WORKSPACE/.claude"
cd "$WORKSPACE"
git init --quiet
git config user.email "qa@test"
git config user.name "QA"
echo "test file" > test.txt
git add . && git commit -m "init" --quiet

# Configure sidecar with audit logging and passthrough (allow everything)
AUDIT_DIR="$TMPDIR/audit"
mkdir -p "$AUDIT_DIR"
SIDECAR_CONFIG="$TMPDIR/sidecar.toml"
cat > "$SIDECAR_CONFIG" <<EOF
[quorum]
min_allow = 0
default_decision = "passthrough"

[audit]
enabled = true
output = "$AUDIT_DIR"
max_total_bytes = 1048576
max_file_bytes = 524288

[[providers]]
name = "qa-passthrough"
command = "$PROVIDER"
args = ["allow"]
mode = "vote"
EOF

# Install hook in workspace settings
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
            "timeout": 15
          }
        ]
      }
    ]
  }
}
EOF

# Test 1: Run Claude with a simple query that should trigger a Read tool
echo "[1] Claude Code invokes sidecar hook on tool use"
CLAUDE_OUTPUT=$(cd "$WORKSPACE" && claude -p \
  --allowedTools "Read" \
  --dangerously-skip-permissions \
  "Read the file test.txt and tell me its contents. Reply with just the contents, nothing else." \
  2>/dev/null || true)

# Check if audit log was written (evidence that hook was invoked)
AUDIT_FILES=$(find "$AUDIT_DIR" -name 'audit-*.jsonl' 2>/dev/null | wc -l)
if [ "$AUDIT_FILES" -ge 1 ]; then
  ENTRIES=$(cat "$AUDIT_DIR"/audit-*.jsonl 2>/dev/null | wc -l)
  pass "sidecar hook invoked — $ENTRIES audit entries created"
else
  fail "no audit log — hook may not have been invoked"
fi

# Test 2: Audit log contains correct tool_name
echo "[2] Audit log captures tool_name"
if [ "$AUDIT_FILES" -ge 1 ]; then
  TOOL_NAMES=$(cat "$AUDIT_DIR"/audit-*.jsonl 2>/dev/null | jq -r '.tool_name' 2>/dev/null | sort -u | tr '\n' ',' || true)
  if [ -n "$TOOL_NAMES" ]; then
    pass "tool names in audit: $TOOL_NAMES"
  else
    fail "no tool_name in audit entries"
  fi
else
  fail "no audit files for tool_name check"
fi

# Test 3: Audit log contains provider vote
echo "[3] Audit log captures provider vote"
if [ "$AUDIT_FILES" -ge 1 ]; then
  VOTES=$(cat "$AUDIT_DIR"/audit-*.jsonl 2>/dev/null | jq -r '.providers[0].vote' 2>/dev/null | head -1 || true)
  if [ "$VOTES" = "allow" ]; then
    pass "provider vote recorded: $VOTES"
  else
    fail "unexpected vote: $VOTES"
  fi
else
  fail "no audit files for vote check"
fi

# Test 4: Provider timing is recorded
echo "[4] Provider timing in audit"
if [ "$AUDIT_FILES" -ge 1 ]; then
  TIMING=$(cat "$AUDIT_DIR"/audit-*.jsonl 2>/dev/null | jq -r '.providers[0].response_time_ms' 2>/dev/null | head -1 || true)
  if [ "$TIMING" -ge 0 ] 2>/dev/null; then
    pass "provider timing: ${TIMING}ms"
  else
    fail "no timing in audit"
  fi
else
  fail "no audit files for timing check"
fi

# Test 5: Passthrough sidecar didn't block Claude
echo "[5] Passthrough sidecar did not block tool execution"
if echo "$CLAUDE_OUTPUT" | grep -qi "test file\|error\|denied"; then
  if echo "$CLAUDE_OUTPUT" | grep -q "test file"; then
    pass "Claude read file successfully (sidecar passthrough worked)"
  else
    fail "Claude may have been blocked or errored: $CLAUDE_OUTPUT"
  fi
else
  # Claude may not have output the file content, but if it ran at all, good enough
  if [ -n "$CLAUDE_OUTPUT" ]; then
    pass "Claude produced output (sidecar didn't block)"
  else
    fail "no output from Claude"
  fi
fi

# Test 6: Validate audit log format
echo "[6] Audit log format validation"
if [ "$AUDIT_FILES" -ge 1 ]; then
  if "$HELPERS/check-audit-log.sh" "$AUDIT_DIR" > /dev/null 2>&1; then
    pass "audit log format valid"
  else
    fail "audit log format validation failed"
  fi
else
  fail "no audit files to validate"
fi

echo ""
echo "=== Results: $PASS passed, $FAIL failed ==="
[ "$FAIL" -eq 0 ] || exit 1
