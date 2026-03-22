#!/bin/bash
# Master QA Runner — runs all test suites and reports results.
#
# Usage:
#   ./run-all-qa.sh          # Run all tests
#   ./run-all-qa.sh --skip-cargo  # Skip cargo test (run only QA scripts)
#
# Exit code: 0 if all pass, 1 if any fail.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
QA_DIR="$(dirname "$SCRIPT_DIR")"
PROJECT_DIR="$(dirname "$QA_DIR")"

SKIP_CARGO=false
[ "${1:-}" = "--skip-cargo" ] && SKIP_CARGO=true

TOTAL_PASS=0
TOTAL_FAIL=0
SUITES_PASSED=0
SUITES_FAILED=0

run_suite() {
  local name="$1"
  local cmd="$2"

  echo ""
  echo "╔══════════════════════════════════════════════════════════╗"
  echo "║  $name"
  echo "╚══════════════════════════════════════════════════════════╝"
  echo ""

  if eval "$cmd"; then
    SUITES_PASSED=$((SUITES_PASSED + 1))
    echo "  ✓ Suite passed"
  else
    SUITES_FAILED=$((SUITES_FAILED + 1))
    echo "  ✗ Suite FAILED"
  fi
}

echo "╔══════════════════════════════════════════════════════════╗"
echo "║  claude-pretool-sidecar — Full QA Suite                  ║"
echo "╚══════════════════════════════════════════════════════════╝"
echo ""
echo "Project: $PROJECT_DIR"
echo "Date:    $(date -Iseconds)"
echo ""

# Step 1: Build
echo "Building project..."
(cd "$PROJECT_DIR" && cargo build --quiet 2>/dev/null)
echo "Build complete."

# Step 2: Cargo tests (unit + integration)
if [ "$SKIP_CARGO" = false ]; then
  run_suite "Rust Unit + Integration Tests (cargo test)" \
    "cd '$PROJECT_DIR' && cargo test --quiet 2>&1"
fi

# Step 3: QA test suites
run_suite "Config Loading Tests" "$SCRIPT_DIR/test-config.sh"
run_suite "Provider Execution Tests" "$SCRIPT_DIR/test-providers.sh"
run_suite "Quorum Logic Tests" "$SCRIPT_DIR/test-quorum.sh"
run_suite "Audit Logging Tests" "$SCRIPT_DIR/test-audit.sh"
run_suite "Claude Code Hook Compliance" "$SCRIPT_DIR/test-hook-integration.sh"

# Summary
echo ""
echo "╔══════════════════════════════════════════════════════════╗"
echo "║  QA Summary                                              ║"
echo "╚══════════════════════════════════════════════════════════╝"
echo ""
echo "  Suites passed: $SUITES_PASSED"
echo "  Suites failed: $SUITES_FAILED"
echo ""

if [ "$SUITES_FAILED" -gt 0 ]; then
  echo "  RESULT: FAIL ($SUITES_FAILED suite(s) failed)"
  exit 1
else
  echo "  RESULT: ALL PASS"
  exit 0
fi
