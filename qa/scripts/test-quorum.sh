#!/bin/bash
# QA Tests: Quorum Logic (End-to-End)
#
# Tests vote aggregation with real providers through the binary.
# Complements the unit tests in src/quorum.rs.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
QA_DIR="$(dirname "$SCRIPT_DIR")"
PROJECT_DIR="$(dirname "$QA_DIR")"
BINARY="$PROJECT_DIR/target/debug/claude-pretool-sidecar"
HELPERS="$QA_DIR/helpers"
PROVIDER="$HELPERS/provider-echo.sh"

PASS=0
FAIL=0
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

pass() { PASS=$((PASS + 1)); echo "  PASS: $1"; }
fail() { FAIL=$((FAIL + 1)); echo "  FAIL: $1"; }

[ -f "$BINARY" ] || (cd "$PROJECT_DIR" && cargo build --quiet 2>/dev/null)

PAYLOAD=$("$HELPERS/gen-payload.sh" bash "ls")

# Helper: run sidecar with config and extract decision
run_test() {
  local config_file="$1"
  local output
  output=$(echo "$PAYLOAD" | CLAUDE_PRETOOL_SIDECAR_CONFIG="$config_file" "$BINARY" 2>/dev/null)
  local decision
  decision=$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecision // "passthrough"')
  echo "$decision"
}

echo "=== Quorum Logic Tests ==="

# Test 1: Single allow quorum
echo "[1] Single allow, min_allow=1 → allow"
cat > "$TMPDIR/t1.toml" <<EOF
[quorum]
min_allow = 1
[[providers]]
name = "a"
command = "$PROVIDER"
args = ["allow"]
EOF
RESULT=$(run_test "$TMPDIR/t1.toml")
[ "$RESULT" = "allow" ] && pass "single allow" || fail "expected allow, got $RESULT"

# Test 2: Single deny blocks
echo "[2] Single deny, max_deny=0 → deny"
cat > "$TMPDIR/t2.toml" <<EOF
[quorum]
min_allow = 1
max_deny = 0
[[providers]]
name = "d"
command = "$PROVIDER"
args = ["deny"]
EOF
RESULT=$(run_test "$TMPDIR/t2.toml")
[ "$RESULT" = "deny" ] && pass "single deny" || fail "expected deny, got $RESULT"

# Test 3: Deny priority over allows
echo "[3] 2 allow + 1 deny, max_deny=0 → deny (priority)"
cat > "$TMPDIR/t3.toml" <<EOF
[quorum]
min_allow = 2
max_deny = 0
[[providers]]
name = "a1"
command = "$PROVIDER"
args = ["allow"]
[[providers]]
name = "a2"
command = "$PROVIDER"
args = ["allow"]
[[providers]]
name = "d1"
command = "$PROVIDER"
args = ["deny"]
EOF
RESULT=$(run_test "$TMPDIR/t3.toml")
[ "$RESULT" = "deny" ] && pass "deny priority" || fail "expected deny, got $RESULT"

# Test 4: Tolerated deny
echo "[4] 2 allow + 1 deny, max_deny=1 → allow"
cat > "$TMPDIR/t4.toml" <<EOF
[quorum]
min_allow = 2
max_deny = 1
[[providers]]
name = "a1"
command = "$PROVIDER"
args = ["allow"]
[[providers]]
name = "a2"
command = "$PROVIDER"
args = ["allow"]
[[providers]]
name = "d1"
command = "$PROVIDER"
args = ["deny"]
EOF
RESULT=$(run_test "$TMPDIR/t4.toml")
[ "$RESULT" = "allow" ] && pass "tolerated deny" || fail "expected allow, got $RESULT"

# Test 5: Quorum not met
echo "[5] 1 allow, min_allow=2, default=passthrough → passthrough"
cat > "$TMPDIR/t5.toml" <<EOF
[quorum]
min_allow = 2
max_deny = 0
default_decision = "passthrough"
[[providers]]
name = "a1"
command = "$PROVIDER"
args = ["allow"]
[[providers]]
name = "p1"
command = "$PROVIDER"
args = ["passthrough"]
EOF
RESULT=$(run_test "$TMPDIR/t5.toml")
[ "$RESULT" = "passthrough" ] && pass "quorum not met" || fail "expected passthrough, got $RESULT"

# Test 6: Error as deny
echo "[6] Crash + error_policy=deny → deny"
cat > "$TMPDIR/t6.toml" <<EOF
[quorum]
min_allow = 1
max_deny = 0
error_policy = "deny"
[[providers]]
name = "c1"
command = "$PROVIDER"
args = ["crash"]
EOF
RESULT=$(run_test "$TMPDIR/t6.toml")
[ "$RESULT" = "deny" ] && pass "error as deny" || fail "expected deny, got $RESULT"

# Test 7: Error as passthrough
echo "[7] Crash + error_policy=passthrough, min_allow=1 → passthrough (default)"
cat > "$TMPDIR/t7.toml" <<EOF
[quorum]
min_allow = 1
max_deny = 0
error_policy = "passthrough"
default_decision = "passthrough"
[[providers]]
name = "c1"
command = "$PROVIDER"
args = ["crash"]
EOF
RESULT=$(run_test "$TMPDIR/t7.toml")
[ "$RESULT" = "passthrough" ] && pass "error as passthrough" || fail "expected passthrough, got $RESULT"

# Test 8: Zero providers, min_allow=0 → allow
echo "[8] Zero providers, min_allow=0 → allow"
cat > "$TMPDIR/t8.toml" <<EOF
[quorum]
min_allow = 0
default_decision = "passthrough"
EOF
RESULT=$(run_test "$TMPDIR/t8.toml")
[ "$RESULT" = "allow" ] && pass "zero providers allow" || fail "expected allow, got $RESULT"

# Test 9: Mixed with FYI
echo "[9] 1 allow + 1 FYI-deny, min_allow=1 → allow (FYI ignored)"
cat > "$TMPDIR/t9.toml" <<EOF
[quorum]
min_allow = 1
max_deny = 0
[[providers]]
name = "a1"
command = "$PROVIDER"
args = ["allow"]
mode = "vote"
[[providers]]
name = "fyi1"
command = "$PROVIDER"
args = ["deny"]
mode = "fyi"
EOF
RESULT=$(run_test "$TMPDIR/t9.toml")
[ "$RESULT" = "allow" ] && pass "FYI ignored in quorum" || fail "expected allow, got $RESULT"

echo ""
echo "=== Results: $PASS passed, $FAIL failed ==="
[ "$FAIL" -eq 0 ] || exit 1
