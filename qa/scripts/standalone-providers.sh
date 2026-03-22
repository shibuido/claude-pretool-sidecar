#!/bin/bash
# QA Tests: Provider Execution and Communication
#
# Tests provider spawning, stdin/stdout protocol, vote parsing, error handling.
# Requires: claude-pretool-sidecar binary built

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
QA_DIR="$(dirname "$SCRIPT_DIR")"
PROJECT_DIR="$(dirname "$QA_DIR")"
BINARY=$(command -v claude-pretool-sidecar 2>/dev/null || echo "$PROJECT_DIR/target/debug/claude-pretool-sidecar")
HELPERS="$QA_DIR/helpers"
PROVIDER="$HELPERS/provider-echo.sh"

PASS=0
FAIL=0
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

pass() { PASS=$((PASS + 1)); echo "  PASS: $1"; }
fail() { FAIL=$((FAIL + 1)); echo "  FAIL: $1"; }

# Build if needed
[ -x "$BINARY" ] || { command -v cargo > /dev/null 2>&1 && (cd "$PROJECT_DIR" && cargo build --quiet 2>/dev/null); }

PAYLOAD=$("$HELPERS/gen-payload.sh" bash "ls -la")

echo "=== Provider Execution Tests ==="

# Test 1: Allow provider
echo "[1] Allow provider"
"$HELPERS/gen-config.sh" single-allow "$PROVIDER" > "$TMPDIR/allow.toml"
# Override provider args to pass "allow" to provider-echo.sh
cat > "$TMPDIR/allow.toml" <<EOF
[quorum]
min_allow = 1
max_deny = 0
[[providers]]
name = "allower"
command = "$PROVIDER"
args = ["allow"]
mode = "vote"
EOF
OUTPUT=$(echo "$PAYLOAD" | CLAUDE_PRETOOL_SIDECAR_CONFIG="$TMPDIR/allow.toml" "$BINARY" 2>/dev/null)
DECISION=$(echo "$OUTPUT" | jq -r '.hookSpecificOutput.permissionDecision // "none"')
if [ "$DECISION" = "allow" ]; then
  pass "allow provider returns allow"
else
  fail "expected allow, got: $OUTPUT"
fi

# Test 2: Deny provider
echo "[2] Deny provider"
cat > "$TMPDIR/deny.toml" <<EOF
[quorum]
min_allow = 1
max_deny = 0
[[providers]]
name = "denier"
command = "$PROVIDER"
args = ["deny", "test reason"]
mode = "vote"
EOF
OUTPUT=$(echo "$PAYLOAD" | CLAUDE_PRETOOL_SIDECAR_CONFIG="$TMPDIR/deny.toml" "$BINARY" 2>/dev/null)
DECISION=$(echo "$OUTPUT" | jq -r '.hookSpecificOutput.permissionDecision // "none"')
if [ "$DECISION" = "deny" ]; then
  pass "deny provider returns deny"
else
  fail "expected deny, got: $OUTPUT"
fi

# Test 3: Passthrough provider
echo "[3] Passthrough provider"
cat > "$TMPDIR/pass.toml" <<EOF
[quorum]
min_allow = 1
max_deny = 0
default_decision = "passthrough"
[[providers]]
name = "passer"
command = "$PROVIDER"
args = ["passthrough"]
mode = "vote"
EOF
OUTPUT=$(echo "$PAYLOAD" | CLAUDE_PRETOOL_SIDECAR_CONFIG="$TMPDIR/pass.toml" "$BINARY" 2>/dev/null)
if [ "$OUTPUT" = "{}" ]; then
  pass "passthrough returns empty object"
else
  fail "expected {}, got: $OUTPUT"
fi

# Test 4: Crash provider
echo "[4] Crash provider"
cat > "$TMPDIR/crash.toml" <<EOF
[quorum]
min_allow = 1
max_deny = 0
error_policy = "deny"
[[providers]]
name = "crasher"
command = "$PROVIDER"
args = ["crash"]
mode = "vote"
EOF
OUTPUT=$(echo "$PAYLOAD" | CLAUDE_PRETOOL_SIDECAR_CONFIG="$TMPDIR/crash.toml" "$BINARY" 2>/dev/null)
DECISION=$(echo "$OUTPUT" | jq -r '.hookSpecificOutput.permissionDecision // "none"')
if [ "$DECISION" = "deny" ]; then
  pass "crash with deny error_policy returns deny"
else
  fail "expected deny for crash, got: $OUTPUT"
fi

# Test 5: Bad JSON provider
echo "[5] Bad JSON provider"
cat > "$TMPDIR/bad.toml" <<EOF
[quorum]
min_allow = 1
max_deny = 0
error_policy = "passthrough"
default_decision = "passthrough"
[[providers]]
name = "bad-json"
command = "$PROVIDER"
args = ["bad-json"]
mode = "vote"
EOF
OUTPUT=$(echo "$PAYLOAD" | CLAUDE_PRETOOL_SIDECAR_CONFIG="$TMPDIR/bad.toml" "$BINARY" 2>/dev/null)
if [ "$OUTPUT" = "{}" ]; then
  pass "bad JSON treated as error → passthrough"
else
  fail "expected {} for bad JSON, got: $OUTPUT"
fi

# Test 6: FYI provider ignored
echo "[6] FYI provider ignored"
cat > "$TMPDIR/fyi.toml" <<EOF
[quorum]
min_allow = 0
default_decision = "passthrough"
[[providers]]
name = "fyi-denier"
command = "$PROVIDER"
args = ["deny"]
mode = "fyi"
EOF
OUTPUT=$(echo "$PAYLOAD" | CLAUDE_PRETOOL_SIDECAR_CONFIG="$TMPDIR/fyi.toml" "$BINARY" 2>/dev/null)
DECISION=$(echo "$OUTPUT" | jq -r '.hookSpecificOutput.permissionDecision // "none"')
# min_allow=0, so 0>=0 → allow (FYI deny ignored)
if [ "$DECISION" = "allow" ]; then
  pass "FYI provider deny ignored"
else
  fail "expected allow (FYI ignored), got: $OUTPUT"
fi

# Test 7: Provider receives stdin content
echo "[7] Provider receives hook payload on stdin"
DUMP_FILE="$TMPDIR/provider-stdin-dump.json"
cat > "$TMPDIR/dump-provider.sh" <<SCRIPT
#!/bin/bash
cat > "$DUMP_FILE"
echo '{"decision": "allow"}'
SCRIPT
chmod +x "$TMPDIR/dump-provider.sh"
cat > "$TMPDIR/dump.toml" <<EOF
[quorum]
min_allow = 1
[[providers]]
name = "dumper"
command = "$TMPDIR/dump-provider.sh"
mode = "vote"
EOF
echo "$PAYLOAD" | CLAUDE_PRETOOL_SIDECAR_CONFIG="$TMPDIR/dump.toml" "$BINARY" > /dev/null 2>/dev/null
if [ -f "$DUMP_FILE" ] && grep -q "tool_name" "$DUMP_FILE"; then
  pass "provider received hook payload on stdin"
else
  fail "provider did not receive payload"
fi

# Test 8: Provider env vars
echo "[8] Provider environment variables"
cat > "$TMPDIR/env.toml" <<EOF
[quorum]
min_allow = 1
max_deny = 0
[[providers]]
name = "env-checker"
command = "$PROVIDER"
args = ["env-check"]
mode = "vote"

[providers.env]
MY_VAR = "qa-test-value"
EOF
OUTPUT=$(echo "$PAYLOAD" | CLAUDE_PRETOOL_SIDECAR_CONFIG="$TMPDIR/env.toml" "$BINARY" 2>/dev/null)
DECISION=$(echo "$OUTPUT" | jq -r '.hookSpecificOutput.permissionDecision // "none"')
if [ "$DECISION" = "allow" ]; then
  pass "provider with env vars executed successfully"
else
  fail "provider env var test failed: $OUTPUT"
fi

echo ""
echo "=== Results: $PASS passed, $FAIL failed ==="
[ "$FAIL" -eq 0 ] || exit 1
