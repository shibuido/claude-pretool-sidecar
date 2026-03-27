#!/bin/bash
# ask-user.sh — Provider for claude-pretool-sidecar
#
# Returns "ask" for dangerous tool patterns, triggering Claude Code's
# interactive approval prompt. Returns "passthrough" for everything else.
#
# The "ask" decision is different from "deny" — instead of blocking,
# it causes Claude Code to pause and ask the user for explicit approval.
# This is useful for operations that might be legitimate but deserve
# human review.
#
# HOW TO CUSTOMIZE:
#   Edit the tool/pattern checks below to match your workflow.
#   The "ask" decision works with Claude Code's permissionDecision system.
#
# USAGE:
#   Configure in .claude-pretool-sidecar.toml:
#     [[providers]]
#     name = "ask-user"
#     command = "/path/to/ask-user.sh"
#
# PROTOCOL:
#   stdin:  JSON with tool_name, tool_input, etc.
#   stdout: JSON with decision (ask|passthrough) and optional reason
#
# NOTE ON "ask" DECISION:
#   The sidecar translates provider decisions to Claude Code's hook format.
#   An "ask" decision becomes permissionDecision: "ask" in the hook output,
#   which triggers Claude Code's interactive user approval prompt.

set -euo pipefail

# Read the full JSON payload from stdin (protocol requirement)
INPUT=$(cat)

# Extract tool name and relevant input fields
TOOL_NAME=$(echo "$INPUT" | python3 -c "import sys,json; print(json.load(sys.stdin).get('tool_name',''))" 2>/dev/null || echo "")
COMMAND=$(echo "$INPUT" | python3 -c "import sys,json; print(json.load(sys.stdin).get('tool_input',{}).get('command',''))" 2>/dev/null || echo "")
FILE_PATH=$(echo "$INPUT" | python3 -c "import sys,json; print(json.load(sys.stdin).get('tool_input',{}).get('file_path',''))" 2>/dev/null || echo "")

# --- Customize: patterns that trigger interactive approval ---

# Bash commands that should prompt for user approval
if [ "$TOOL_NAME" = "Bash" ] && [ -n "$COMMAND" ]; then
  # Network-related commands (curl, wget, ssh, scp)
  if echo "$COMMAND" | grep -qE '\b(curl|wget|ssh|scp|rsync)\b'; then
    echo "{\"decision\": \"ask\", \"reason\": \"Network command detected: please confirm\"}"
    exit 0
  fi

  # Package installation commands
  if echo "$COMMAND" | grep -qE '\b(apt|apt-get|yum|dnf|pacman|pip|npm)\s+(install|remove|uninstall)'; then
    echo "{\"decision\": \"ask\", \"reason\": \"Package manager operation: please confirm\"}"
    exit 0
  fi

  # Git push/force operations
  if echo "$COMMAND" | grep -qE '\bgit\s+(push|reset\s+--hard|clean\s+-f)'; then
    echo "{\"decision\": \"ask\", \"reason\": \"Destructive git operation: please confirm\"}"
    exit 0
  fi

  # Docker operations that affect system state
  if echo "$COMMAND" | grep -qE '\bdocker\s+(rm|rmi|system\s+prune|volume\s+rm)'; then
    echo "{\"decision\": \"ask\", \"reason\": \"Docker cleanup operation: please confirm\"}"
    exit 0
  fi
fi

# File writes to important config files
if [ "$TOOL_NAME" = "Write" ] || [ "$TOOL_NAME" = "Edit" ]; then
  if [ -n "$FILE_PATH" ]; then
    # Writing to dotfiles, configs, or system paths
    if echo "$FILE_PATH" | grep -qE '(\.bashrc|\.zshrc|\.profile|\.gitconfig|Makefile|Dockerfile|\.github/)'; then
      echo "{\"decision\": \"ask\", \"reason\": \"Writing to important config file: please confirm\"}"
      exit 0
    fi
  fi
fi

# --- End of customizable patterns ---

# No dangerous pattern matched — passthrough (let other providers decide)
echo '{"decision": "passthrough"}'
