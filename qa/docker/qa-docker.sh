#!/bin/bash
# Docker orchestration for claude-pretool-sidecar QA testing.
#
# Usage:
#   qa-docker.sh build     # Build the QA Docker image
#   qa-docker.sh test      # Run all QA tests in container
#   qa-docker.sh shell     # Open interactive shell in container
#   qa-docker.sh status    # Show container/image status
#   qa-docker.sh cleanup   # Remove containers and images
#
# Environment:
#   CPTS_DOCKER_PREFIX    Container/image prefix (default: cpts-qa)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
QA_DIR="$(dirname "$SCRIPT_DIR")"
PROJECT_DIR="$(dirname "$QA_DIR")"

# Configurable prefix per CLAUDE.md conventions
PREFIX="${CPTS_DOCKER_PREFIX:-cpts-qa}"
IMAGE="${PREFIX}-image"
CONTAINER="${PREFIX}-runner"

cmd_build() {
  echo "Building QA image: $IMAGE"
  docker build \
    -t "$IMAGE" \
    -f "$SCRIPT_DIR/Dockerfile" \
    "$PROJECT_DIR"
  echo "Build complete: $IMAGE"
}

cmd_test() {
  # Ensure image exists
  if ! docker image inspect "$IMAGE" > /dev/null 2>&1; then
    echo "Image not found, building first..."
    cmd_build
  fi

  echo "Running QA tests in container: $CONTAINER"
  docker run \
    --rm \
    --name "$CONTAINER" \
    "$IMAGE"
  echo ""
  echo "QA tests complete."
}

cmd_shell() {
  if ! docker image inspect "$IMAGE" > /dev/null 2>&1; then
    echo "Image not found, building first..."
    cmd_build
  fi

  echo "Opening shell in QA container..."
  docker run \
    --rm \
    -it \
    --name "${CONTAINER}-shell" \
    --entrypoint /bin/bash \
    "$IMAGE"
}

cmd_status() {
  echo "=== Images ==="
  docker images --filter "reference=${PREFIX}*" 2>/dev/null || echo "(none)"
  echo ""
  echo "=== Containers ==="
  docker ps -a --filter "name=${PREFIX}" 2>/dev/null || echo "(none)"
}

cmd_cleanup() {
  echo "Cleaning up QA Docker artifacts (prefix: $PREFIX)..."

  # Stop and remove containers
  local containers
  containers=$(docker ps -aq --filter "name=${PREFIX}" 2>/dev/null || true)
  if [ -n "$containers" ]; then
    echo "Removing containers..."
    echo "$containers" | xargs docker rm -f 2>/dev/null || true
  fi

  # Remove images
  local images
  images=$(docker images -q --filter "reference=${PREFIX}*" 2>/dev/null || true)
  if [ -n "$images" ]; then
    echo "Removing images..."
    echo "$images" | xargs docker rmi -f 2>/dev/null || true
  fi

  echo "Cleanup complete."
}

# Main dispatch
case "${1:-help}" in
  build)   cmd_build ;;
  test)    cmd_test ;;
  shell)   cmd_shell ;;
  status)  cmd_status ;;
  cleanup) cmd_cleanup ;;
  help|*)
    echo "Usage: $0 {build|test|shell|status|cleanup}"
    echo ""
    echo "  build    Build the QA Docker image"
    echo "  test     Run all QA tests in container"
    echo "  shell    Open interactive shell in container"
    echo "  status   Show container/image status"
    echo "  cleanup  Remove containers and images"
    echo ""
    echo "Environment:"
    echo "  CPTS_DOCKER_PREFIX   Container/image prefix (default: cpts-qa)"
    ;;
esac
