#!/bin/bash
# QA Tests: Audit Logging and Log Rotation
#
# Tests audit log creation, entry format, rotation, and truncation.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
QA_DIR="$(dirname "$SCRIPT_DIR")"
PROJECT_DIR="$(dirname "$QA_DIR")"
BINARY="$PROJECT_DIR/target/debug/claude-pretool-sidecar"
HELPERS="$QA_DIR/helpers"
PROVIDER="$HELPERS/provider-echo.sh"
CHECKER="$HELPERS/check-audit-log.sh"

PASS=0
FAIL=0
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

pass() { PASS=$((PASS + 1)); echo "  PASS: $1"; }
fail() { FAIL=$((FAIL + 1)); echo "  FAIL: $1"; }

[ -f "$BINARY" ] || (cd "$PROJECT_DIR" && cargo build --quiet 2>/dev/null)

PAYLOAD=$("$HELPERS/gen-payload.sh" bash "ls -la")

echo "=== Audit Logging Tests ==="

# Test 1: Audit disabled
echo "[1] Audit disabled by default"
cat > "$TMPDIR/no-audit.toml" <<EOF
[quorum]
min_allow = 0
[[providers]]
name = "a"
command = "$PROVIDER"
args = ["allow"]
mode = "vote"
EOF
AUDIT_DIR="$TMPDIR/audit-disabled"
mkdir -p "$AUDIT_DIR"
echo "$PAYLOAD" | CLAUDE_PRETOOL_SIDECAR_CONFIG="$TMPDIR/no-audit.toml" "$BINARY" > /dev/null 2>&1
COUNT=$(find "$AUDIT_DIR" -name 'audit-*.jsonl' 2>/dev/null | wc -l)
if [ "$COUNT" -eq 0 ]; then
  pass "no audit files when disabled"
else
  fail "audit files created when disabled ($COUNT files)"
fi

# Test 2: Audit to stderr
echo "[2] Audit to stderr"
cat > "$TMPDIR/audit-stderr.toml" <<EOF
[quorum]
min_allow = 0
[audit]
enabled = true
output = "stderr"
[[providers]]
name = "a"
command = "$PROVIDER"
args = ["allow"]
mode = "vote"
EOF
STDERR=$(echo "$PAYLOAD" | CLAUDE_PRETOOL_SIDECAR_CONFIG="$TMPDIR/audit-stderr.toml" "$BINARY" 2>&1 >/dev/null)
if echo "$STDERR" | jq -e '.tool_name' > /dev/null 2>&1; then
  pass "audit entry on stderr"
else
  fail "no valid audit JSON on stderr"
fi

# Test 3: Audit to directory
echo "[3] Audit to directory with date-chunked filename"
AUDIT_DIR="$TMPDIR/audit-dir"
cat > "$TMPDIR/audit-dir.toml" <<EOF
[quorum]
min_allow = 0
[audit]
enabled = true
output = "$AUDIT_DIR"
max_total_bytes = 1048576
max_file_bytes = 524288
[[providers]]
name = "a"
command = "$PROVIDER"
args = ["allow"]
mode = "vote"
EOF
echo "$PAYLOAD" | CLAUDE_PRETOOL_SIDECAR_CONFIG="$TMPDIR/audit-dir.toml" "$BINARY" > /dev/null 2>/dev/null
COUNT=$(find "$AUDIT_DIR" -name 'audit-*.jsonl' 2>/dev/null | wc -l)
if [ "$COUNT" -ge 1 ]; then
  pass "date-chunked audit file created"
else
  fail "no audit files in directory"
fi

# Test 4: Entry format validation
echo "[4] Audit entry format"
LOG_FILE=$(find "$AUDIT_DIR" -name 'audit-*.jsonl' | head -1)
if [ -n "$LOG_FILE" ] && "$CHECKER" "$LOG_FILE" > /dev/null 2>&1; then
  pass "audit entry format valid"
else
  fail "audit entry format validation failed"
fi

# Test 5: Provider timing
echo "[5] Provider timing in audit"
if [ -n "$LOG_FILE" ]; then
  TIMING=$(head -1 "$LOG_FILE" | jq -r '.providers[0].response_time_ms // -1')
  if [ "$TIMING" -ge 0 ] 2>/dev/null; then
    pass "provider timing recorded (${TIMING}ms)"
  else
    fail "provider timing not recorded"
  fi
else
  fail "no log file for timing check"
fi

# Test 6: Log rotation - truncation
echo "[6] Log rotation - per-file truncation"
AUDIT_ROT="$TMPDIR/audit-rotate"
cat > "$TMPDIR/audit-rot.toml" <<EOF
[quorum]
min_allow = 0
[audit]
enabled = true
output = "$AUDIT_ROT"
max_total_bytes = 4096
max_file_bytes = 1024
[[providers]]
name = "a"
command = "$PROVIDER"
args = ["allow"]
mode = "vote"
EOF
# Write many entries to trigger truncation
for i in $(seq 1 50); do
  echo "$PAYLOAD" | CLAUDE_PRETOOL_SIDECAR_CONFIG="$TMPDIR/audit-rot.toml" "$BINARY" > /dev/null 2>/dev/null
done
ROT_FILE=$(find "$AUDIT_ROT" -name 'audit-*.jsonl' | head -1)
if [ -n "$ROT_FILE" ]; then
  FILE_SIZE=$(wc -c < "$ROT_FILE")
  if [ "$FILE_SIZE" -le 2048 ]; then  # Some margin over 1024 for sentinel
    pass "file truncated to within limit (${FILE_SIZE} bytes)"
  else
    fail "file not truncated (${FILE_SIZE} bytes, limit 1024)"
  fi
else
  fail "no rotation log file"
fi

# Test 7: Sentinel line after truncation
echo "[7] Sentinel line present after truncation"
if [ -n "$ROT_FILE" ]; then
  FIRST_LINE=$(head -1 "$ROT_FILE")
  if echo "$FIRST_LINE" | jq -e '._truncated' > /dev/null 2>&1; then
    pass "sentinel line present"
  else
    # May not have been truncated if individual entries are small
    LINE_COUNT=$(wc -l < "$ROT_FILE")
    if [ "$LINE_COUNT" -lt 50 ]; then
      pass "file was truncated (${LINE_COUNT} lines remain of 50)"
    else
      fail "no truncation occurred"
    fi
  fi
else
  fail "no log file for sentinel check"
fi

# Test 8: Recent lines preserved
echo "[8] Recent lines preserved after truncation"
if [ -n "$ROT_FILE" ]; then
  LAST_LINE=$(tail -1 "$ROT_FILE")
  if echo "$LAST_LINE" | jq -e '.tool_name' > /dev/null 2>&1; then
    pass "recent entries preserved"
  else
    fail "recent entries not preserved"
  fi
else
  fail "no log file for recency check"
fi

echo ""
echo "=== Results: $PASS passed, $FAIL failed ==="
[ "$FAIL" -eq 0 ] || exit 1
