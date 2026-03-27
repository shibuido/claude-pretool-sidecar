#!/usr/bin/env python3
"""
file-path-policy.py — Provider for claude-pretool-sidecar

Denies writes to sensitive file paths, allows reads everywhere.
Non-file tools (Bash, etc.) are passed through.

HOW TO CUSTOMIZE:
  Option 1: Edit the SENSITIVE_PATTERNS list below.
  Option 2: Set the CPTS_SENSITIVE_PATHS env var to a colon-separated
            list of patterns (merged with the built-in list).
            Example: CPTS_SENSITIVE_PATHS=".secret:/opt/prod:passwords"

USAGE:
  Configure in .claude-pretool-sidecar.toml:
    [[providers]]
    name = "file-path-policy"
    command = "python3 /path/to/file-path-policy.py"

PROTOCOL:
  stdin:  JSON with tool_name, tool_input, etc.
  stdout: JSON with decision (allow|deny|passthrough) and optional reason
"""

import json
import os
import sys

# --- Customize these patterns ---
# Substrings matched case-insensitively against file paths in tool_input.
# Any file path containing one of these triggers a deny for write operations.
SENSITIVE_PATTERNS = [
    ".env",
    "/etc/",
    "credentials",
    "secrets",
    ".ssh/",
    ".gnupg/",
    ".aws/",
    "id_rsa",
    "id_ed25519",
    ".pem",
    "private_key",
    "token",
    "/passwd",
    "/shadow",
]
# --- End of customizable patterns ---

# Tools that write files (deny if path is sensitive)
WRITE_TOOLS = {"Write", "Edit", "NotebookEdit"}

# Tools that read files (always allow)
READ_TOOLS = {"Read"}


def load_extra_patterns():
    """Load additional patterns from CPTS_SENSITIVE_PATHS env var."""
    env_val = os.environ.get("CPTS_SENSITIVE_PATHS", "")
    if env_val:
        return [p.strip() for p in env_val.split(":") if p.strip()]
    return []


def extract_paths(tool_input):
    """Extract file paths from tool_input dict."""
    paths = []
    for key in ("file_path", "path", "filename"):
        val = tool_input.get(key, "")
        if val:
            paths.append(val)
    # Also check old_string/new_string for Edit tool — the file_path is the key one
    return paths


def is_sensitive(path, patterns):
    """Check if a path matches any sensitive pattern (case-insensitive)."""
    path_lower = path.lower()
    for pattern in patterns:
        if pattern.lower() in path_lower:
            return pattern
    return None


def main():
    # Read full JSON from stdin (protocol requirement)
    try:
        payload = json.load(sys.stdin)
    except (json.JSONDecodeError, ValueError):
        # Graceful handling of bad input
        print(json.dumps({"decision": "passthrough", "reason": "could not parse input JSON"}))
        return

    tool_name = payload.get("tool_name", "")
    tool_input = payload.get("tool_input", {})

    # Read tools: always allow
    if tool_name in READ_TOOLS:
        print(json.dumps({"decision": "allow", "reason": "read operations are always allowed"}))
        return

    # Non-file tools: no opinion
    if tool_name not in WRITE_TOOLS:
        print(json.dumps({"decision": "passthrough", "reason": f"not a file-write tool: {tool_name}"}))
        return

    # File-write tool: check paths against sensitive patterns
    all_patterns = SENSITIVE_PATTERNS + load_extra_patterns()
    paths = extract_paths(tool_input)

    if not paths:
        print(json.dumps({"decision": "passthrough", "reason": "no file path found in tool input"}))
        return

    for path in paths:
        matched = is_sensitive(path, all_patterns)
        if matched:
            print(json.dumps({
                "decision": "deny",
                "reason": f"Write to sensitive path blocked: '{path}' matches pattern '{matched}'"
            }))
            return

    # Path is not sensitive — allow the write
    print(json.dumps({"decision": "allow", "reason": "file path passed sensitivity checks"}))


if __name__ == "__main__":
    main()
