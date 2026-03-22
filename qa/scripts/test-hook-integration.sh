#!/bin/bash
# QA Tests: Claude Code Hook Format Compliance
#
# Verifies that input parsing and output format match the Claude Code
# hooks specification documented in docs/design/claude-code-hooks-reference.md.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
QA_DIR="$(dirname "$SCRIPT_DIR")"
PROJECT_DIR="$(dirname "$QA_DIR")"
BINARY="$PROJECT_DIR/target/debug/claude-pretool-sidecar"
HELPERS="$QA_DIR/helpers"
PROVIDER="$HELPERS/provider-echo.sh"
FIXTURES="$QA_DIR/fixtures/payloads"

PASS=0
FAIL=0
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

pass() { PASS=$((PASS + 1)); echo "  PASS: $1"; }
fail() { FAIL=$((FAIL + 1)); echo "  FAIL: $1"; }

[ -f "$BINARY" ] || (cd "$PROJECT_DIR" && cargo build --quiet 2>/dev/null)

# Config for allow response
cat > "$TMPDIR/allow.toml" <<EOF
[quorum]
min_allow = 1
[[providers]]
name = "a"
command = "$PROVIDER"
args = ["allow"]
EOF

# Config for deny response
cat > "$TMPDIR/deny.toml" <<EOF
[quorum]
min_allow = 1
max_deny = 0
[[providers]]
name = "d"
command = "$PROVIDER"
args = ["deny", "policy violation"]
EOF

# Config for passthrough
cat > "$TMPDIR/pass.toml" <<EOF
[quorum]
min_allow = 1
default_decision = "passthrough"
[[providers]]
name = "p"
command = "$PROVIDER"
args = ["passthrough"]
EOF

echo "=== Claude Code Hook Format Compliance Tests ==="

# Test 1: Allow format
echo "[1] Allow response format"
OUTPUT=$(cat "$FIXTURES/bash-ls.json" | CLAUDE_PRETOOL_SIDECAR_CONFIG="$TMPDIR/allow.toml" "$BINARY" 2>/dev/null)
# Must have hookSpecificOutput.permissionDecision = "allow"
HSO=$(echo "$OUTPUT" | jq -r '.hookSpecificOutput.permissionDecision // "missing"')
if [ "$HSO" = "allow" ]; then
  pass "allow format: hookSpecificOutput.permissionDecision=allow"
else
  fail "allow format wrong: $OUTPUT"
fi

# Test 2: Deny format
echo "[2] Deny response format"
OUTPUT=$(cat "$FIXTURES/bash-dangerous.json" | CLAUDE_PRETOOL_SIDECAR_CONFIG="$TMPDIR/deny.toml" "$BINARY" 2>/dev/null)
HSO=$(echo "$OUTPUT" | jq -r '.hookSpecificOutput.permissionDecision // "missing"')
if [ "$HSO" = "deny" ]; then
  pass "deny format: hookSpecificOutput.permissionDecision=deny"
else
  fail "deny format wrong: $OUTPUT"
fi

# Test 3: Passthrough format
echo "[3] Passthrough response format"
OUTPUT=$(cat "$FIXTURES/bash-ls.json" | CLAUDE_PRETOOL_SIDECAR_CONFIG="$TMPDIR/pass.toml" "$BINARY" 2>/dev/null)
if [ "$OUTPUT" = "{}" ]; then
  pass "passthrough format: empty object {}"
else
  fail "passthrough format wrong (expected {}, got $OUTPUT)"
fi

# Test 4: Parses all Claude Code input fields
echo "[4] Full Claude Code input parsing"
OUTPUT=$(cat "$FIXTURES/bash-ls.json" | CLAUDE_PRETOOL_SIDECAR_CONFIG="$TMPDIR/allow.toml" "$BINARY" 2>/dev/null)
EXIT_CODE=$?
if [ "$EXIT_CODE" -eq 0 ]; then
  pass "full input payload parsed (exit 0)"
else
  fail "full input payload failed (exit $EXIT_CODE)"
fi

# Test 5: Unknown fields ignored
echo "[5] Unknown input fields ignored"
OUTPUT=$(cat "$FIXTURES/extra-fields.json" | CLAUDE_PRETOOL_SIDECAR_CONFIG="$TMPDIR/allow.toml" "$BINARY" 2>/dev/null)
EXIT_CODE=$?
if [ "$EXIT_CODE" -eq 0 ]; then
  pass "unknown fields silently ignored"
else
  fail "unknown fields caused error (exit $EXIT_CODE)"
fi

# Test 6: Minimal payload (just tool_name + tool_input)
echo "[6] Minimal payload works"
OUTPUT=$(cat "$FIXTURES/minimal.json" | CLAUDE_PRETOOL_SIDECAR_CONFIG="$TMPDIR/allow.toml" "$BINARY" 2>/dev/null)
EXIT_CODE=$?
if [ "$EXIT_CODE" -eq 0 ]; then
  pass "minimal payload accepted"
else
  fail "minimal payload rejected (exit $EXIT_CODE)"
fi

# Test 7: Write tool payload
echo "[7] Write tool payload"
OUTPUT=$(cat "$FIXTURES/write-file.json" | CLAUDE_PRETOOL_SIDECAR_CONFIG="$TMPDIR/allow.toml" "$BINARY" 2>/dev/null)
HSO=$(echo "$OUTPUT" | jq -r '.hookSpecificOutput.permissionDecision // "missing"')
if [ "$HSO" = "allow" ]; then
  pass "Write tool payload processed"
else
  fail "Write tool payload failed: $OUTPUT"
fi

# Test 8: Exit code is always 0 on success
echo "[8] Exit code 0 on success"
cat "$FIXTURES/bash-ls.json" | CLAUDE_PRETOOL_SIDECAR_CONFIG="$TMPDIR/allow.toml" "$BINARY" > /dev/null 2>/dev/null
if [ $? -eq 0 ]; then
  pass "exit code 0"
else
  fail "non-zero exit code on success"
fi

echo ""
echo "=== Results: $PASS passed, $FAIL failed ==="
[ "$FAIL" -eq 0 ] || exit 1
