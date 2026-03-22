#!/bin/bash
# Docker management for LIVE CLAUDE CODE QA environment.
#
# This environment includes Claude Code CLI for testing the sidecar
# as a real PreToolUse/PostToolUse hook.
#
# IMPORTANT: Requires ANTHROPIC_API_KEY environment variable.
#
# Usage:
#   cpts-claude-code.sh build                 # Build the image
#   cpts-claude-code.sh test                  # Run live Claude Code tests
#   cpts-claude-code.sh test-standalone       # Run standalone tests (no API key needed)
#   cpts-claude-code.sh shell                 # Interactive shell
#   cpts-claude-code.sh exec <cmd> [args...]  # Run arbitrary command
#   cpts-claude-code.sh status               # Show image/container status
#   cpts-claude-code.sh logs                  # Show logs from last run
#   cpts-claude-code.sh destroy              # Remove all containers and images
#
# Environment:
#   ANTHROPIC_API_KEY       Required for live tests (not for standalone)
#   CPTS_DOCKER_PREFIX      Artifact prefix (default: cpts-claude-code)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
QA_DIR="$(dirname "$SCRIPT_DIR")"
PROJECT_DIR="$(dirname "$QA_DIR")"

PREFIX="${CPTS_DOCKER_PREFIX:-cpts-claude-code}"
IMAGE="${PREFIX}-image"
CONTAINER="${PREFIX}-runner"

ensure_image() {
  if ! docker image inspect "$IMAGE" > /dev/null 2>&1; then
    echo "Image not found, building first..."
    cmd_build
  fi
}

check_api_key() {
  if [ -z "${ANTHROPIC_API_KEY:-}" ]; then
    echo "ERROR: ANTHROPIC_API_KEY is required for live Claude Code tests."
    echo ""
    echo "Set it before running:"
    echo "  export ANTHROPIC_API_KEY='sk-ant-...'"
    echo "  $0 $1"
    echo ""
    echo "For standalone tests (no API key needed), use:"
    echo "  $0 test-standalone"
    exit 1
  fi
}

api_key_flags() {
  if [ -n "${ANTHROPIC_API_KEY:-}" ]; then
    echo "-e ANTHROPIC_API_KEY=${ANTHROPIC_API_KEY}"
  fi
}

cmd_build() {
  echo "Building Claude Code QA image: $IMAGE"
  docker build \
    -t "$IMAGE" \
    -f "$SCRIPT_DIR/Dockerfile.claude-code" \
    "$PROJECT_DIR"
  echo "Build complete: $IMAGE"
}

cmd_test() {
  check_api_key test
  ensure_image
  echo "Running live Claude Code QA tests in: $CONTAINER"
  docker run \
    --rm \
    --name "$CONTAINER" \
    -e ANTHROPIC_API_KEY="${ANTHROPIC_API_KEY}" \
    --entrypoint /qa/scripts/run-all-live-claude-code.sh \
    "$IMAGE"
  echo "Live Claude Code QA tests complete."
}

cmd_test_standalone() {
  ensure_image
  echo "Running standalone tests in Claude Code image: $CONTAINER"
  docker run \
    --rm \
    --name "${CONTAINER}-standalone" \
    --entrypoint /qa/scripts/run-all-standalone.sh \
    "$IMAGE" \
    --skip-cargo
  echo "Standalone tests complete."
}

cmd_shell() {
  ensure_image
  echo "Opening shell in Claude Code QA container..."
  echo "(ANTHROPIC_API_KEY $([ -n "${ANTHROPIC_API_KEY:-}" ] && echo 'is set' || echo 'NOT set — live tests will fail'))"
  docker run \
    --rm -it \
    --name "${CONTAINER}-shell" \
    $(api_key_flags) \
    --entrypoint /bin/bash \
    "$IMAGE"
}

cmd_exec() {
  ensure_image
  shift  # remove 'exec' from args
  echo "Executing in Claude Code container: $*"
  docker run \
    --rm \
    --name "${CONTAINER}-exec" \
    $(api_key_flags) \
    --entrypoint "" \
    "$IMAGE" \
    "$@"
}

cmd_status() {
  echo "=== Claude Code QA Images ==="
  docker images --filter "reference=${PREFIX}*" 2>/dev/null || echo "  (none)"
  echo ""
  echo "=== Claude Code QA Containers ==="
  docker ps -a --filter "name=${PREFIX}" 2>/dev/null || echo "  (none)"
}

cmd_logs() {
  local cid
  cid=$(docker ps -aq --filter "name=${PREFIX}" --latest 2>/dev/null | head -1)
  if [ -n "$cid" ]; then
    docker logs "$cid"
  else
    echo "No containers found with prefix ${PREFIX}"
  fi
}

cmd_destroy() {
  echo "Destroying Claude Code QA artifacts (prefix: $PREFIX)..."
  local containers
  containers=$(docker ps -aq --filter "name=${PREFIX}" 2>/dev/null || true)
  if [ -n "$containers" ]; then
    echo "  Stopping and removing containers..."
    echo "$containers" | xargs docker rm -f 2>/dev/null || true
  fi
  local images
  images=$(docker images -q --filter "reference=${PREFIX}*" 2>/dev/null || true)
  if [ -n "$images" ]; then
    echo "  Removing images..."
    echo "$images" | xargs docker rmi -f 2>/dev/null || true
  fi
  echo "Destroy complete."
}

case "${1:-help}" in
  build)           cmd_build ;;
  test)            cmd_test ;;
  test-standalone) cmd_test_standalone ;;
  shell)           cmd_shell ;;
  exec)            cmd_exec "$@" ;;
  status)          cmd_status ;;
  logs)            cmd_logs ;;
  destroy)         cmd_destroy ;;
  help|*)
    cat <<EOF
Usage: $0 {build|test|test-standalone|shell|exec|status|logs|destroy}

  build            Build the Claude Code QA Docker image
  test             Run live Claude Code integration tests (needs API key)
  test-standalone  Run standalone tests (no API key needed)
  shell            Interactive bash shell (API key forwarded if set)
  exec <cmd>       Run command: $0 exec claude --version
  status           Show image and container status
  logs             Show logs from most recent container
  destroy          Remove all containers and images

Environment:
  ANTHROPIC_API_KEY      Required for 'test' and 'shell' (live tests)
  CPTS_DOCKER_PREFIX     Artifact prefix (default: cpts-claude-code)

This environment includes Claude Code CLI.
For standalone-only tests, use: cpts-standalone.sh
EOF
    ;;
esac
