#!/bin/bash
# Docker management for STANDALONE QA environment (no Claude Code CLI).
#
# Usage:
#   cpts-standalone.sh build     # Build the standalone QA image
#   cpts-standalone.sh test      # Run all standalone tests in container
#   cpts-standalone.sh shell     # Interactive shell in container
#   cpts-standalone.sh exec CMD  # Run arbitrary command in new container
#   cpts-standalone.sh status    # Show image/container status
#   cpts-standalone.sh logs      # Show logs from last test run
#   cpts-standalone.sh destroy   # Remove all containers and images
#
# Environment:
#   CPTS_DOCKER_PREFIX   Prefix for artifacts (default: cpts-standalone)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
QA_DIR="$(dirname "$SCRIPT_DIR")"
PROJECT_DIR="$(dirname "$QA_DIR")"

PREFIX="${CPTS_DOCKER_PREFIX:-cpts-standalone}"
IMAGE="${PREFIX}-image"
CONTAINER="${PREFIX}-runner"

ensure_image() {
  if ! docker image inspect "$IMAGE" > /dev/null 2>&1; then
    echo "Image not found, building first..."
    cmd_build
  fi
}

cmd_build() {
  echo "Building standalone QA image: $IMAGE"
  docker build \
    -t "$IMAGE" \
    -f "$SCRIPT_DIR/Dockerfile.standalone" \
    "$PROJECT_DIR"
  echo "Build complete: $IMAGE"
}

cmd_test() {
  ensure_image
  echo "Running standalone QA tests in: $CONTAINER"
  docker run \
    --rm \
    --name "$CONTAINER" \
    "$IMAGE"
  echo "Standalone QA tests complete."
}

cmd_shell() {
  ensure_image
  echo "Opening shell in standalone QA container..."
  docker run \
    --rm -it \
    --name "${CONTAINER}-shell" \
    --entrypoint /bin/bash \
    "$IMAGE"
}

cmd_exec() {
  ensure_image
  shift  # remove 'exec' from args
  echo "Executing in standalone container: $*"
  docker run \
    --rm \
    --name "${CONTAINER}-exec" \
    --entrypoint "" \
    "$IMAGE" \
    "$@"
}

cmd_status() {
  echo "=== Standalone QA Images ==="
  docker images --filter "reference=${PREFIX}*" 2>/dev/null || echo "  (none)"
  echo ""
  echo "=== Standalone QA Containers ==="
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
  echo "Destroying standalone QA artifacts (prefix: $PREFIX)..."
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
  build)   cmd_build ;;
  test)    cmd_test ;;
  shell)   cmd_shell ;;
  exec)    cmd_exec "$@" ;;
  status)  cmd_status ;;
  logs)    cmd_logs ;;
  destroy) cmd_destroy ;;
  help|*)
    cat <<EOF
Usage: $0 {build|test|shell|exec|status|logs|destroy}

  build    Build the standalone QA Docker image
  test     Run all standalone QA tests in container
  shell    Open interactive bash shell in container
  exec     Run arbitrary command: $0 exec <cmd> [args...]
  status   Show image and container status
  logs     Show logs from most recent container
  destroy  Remove all containers and images

Environment:
  CPTS_DOCKER_PREFIX   Artifact prefix (default: cpts-standalone)

This environment does NOT include Claude Code CLI.
For live Claude Code tests, use: cpts-claude-code.sh
EOF
    ;;
esac
