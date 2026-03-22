#!/bin/bash
# Master Standalone QA Runner — runs all tests that do NOT require Claude Code CLI.
#
# Usage:
#   ./run-all-standalone.sh              # Run all standalone tests
#   ./run-all-standalone.sh --skip-cargo # Skip cargo test (run only QA scripts)
#
# For tests requiring live Claude Code CLI, see: run-all-live-claude-code.sh
#
# Exit code: 0 if all pass, 1 if any fail.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
QA_DIR="$(dirname "$SCRIPT_DIR")"
PROJECT_DIR="$(dirname "$QA_DIR")"

SKIP_CARGO=false
[ "${1:-}" = "--skip-cargo" ] && SKIP_CARGO=true

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
echo "║  claude-pretool-sidecar — Standalone QA Suite            ║"
echo "║  (no Claude Code CLI required)                           ║"
echo "╚══════════════════════════════════════════════════════════╝"
echo ""
echo "Project: $PROJECT_DIR"
echo "Date:    $(date -Iseconds)"
echo ""

# Step 1: Build (skip if binary already exists, e.g. in Docker)
BINARY=$(command -v claude-pretool-sidecar 2>/dev/null || echo "$PROJECT_DIR/target/debug/claude-pretool-sidecar")
if [ -x "$BINARY" ]; then
  echo "Binary found: $BINARY"
elif command -v cargo > /dev/null 2>&1; then
  echo "Building project..."
  (cd "$PROJECT_DIR" && cargo build --quiet 2>/dev/null)
  echo "Build complete."
else
  echo "ERROR: No binary found and cargo not available."
  echo "Run 'cargo build' first or use the Docker environment."
  exit 1
fi

# Step 2: Cargo tests (unit + integration)
if [ "$SKIP_CARGO" = false ]; then
  run_suite "Rust Unit + Integration Tests (cargo test)" \
    "cd '$PROJECT_DIR' && cargo test --quiet 2>&1"
fi

# Step 3: Standalone QA test suites
run_suite "Config Loading Tests"      "$SCRIPT_DIR/standalone-config.sh"
run_suite "Provider Execution Tests"  "$SCRIPT_DIR/standalone-providers.sh"
run_suite "Quorum Logic Tests"        "$SCRIPT_DIR/standalone-quorum.sh"
run_suite "Audit Logging Tests"       "$SCRIPT_DIR/standalone-audit.sh"
run_suite "Hook Format Compliance"    "$SCRIPT_DIR/standalone-hook-format.sh"

# Summary
echo ""
echo "╔══════════════════════════════════════════════════════════╗"
echo "║  Standalone QA Summary                                   ║"
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
