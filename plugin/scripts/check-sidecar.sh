#!/usr/bin/env bash
# check-sidecar.sh — Verify claude-pretool-sidecar installation and configuration
#
# Usage: bash check-sidecar.sh
# Or if installed as a plugin: bash "${CLAUDE_PLUGIN_ROOT}/scripts/check-sidecar.sh"

set -euo pipefail

PASS=0
WARN=0
FAIL=0

pass() { echo "  [OK]   $1"; PASS=$((PASS + 1)); }
warn() { echo "  [WARN] $1"; WARN=$((WARN + 1)); }
fail() { echo "  [FAIL] $1"; FAIL=$((FAIL + 1)); }

echo "=== claude-pretool-sidecar health check ==="
echo

# 1. Check binary in PATH
echo "--- Binary ---"
if command -v claude-pretool-sidecar >/dev/null 2>&1; then
    SIDECAR_PATH="$(command -v claude-pretool-sidecar)"
    pass "Binary found: ${SIDECAR_PATH}"

    if claude-pretool-sidecar --version >/dev/null 2>&1; then
        VERSION="$(claude-pretool-sidecar --version 2>&1)"
        pass "Version: ${VERSION}"
    else
        warn "Could not determine version (--version flag may not be implemented yet)"
    fi
else
    fail "claude-pretool-sidecar not found in PATH"
    echo "       Install with: cargo install --path <source-dir>"
    echo "       Or add ~/.cargo/bin to PATH"
fi
echo

# 2. Check config file
echo "--- Configuration ---"
CONFIG_FOUND=""

if [ -n "${CLAUDE_PRETOOL_SIDECAR_CONFIG:-}" ]; then
    if [ -f "$CLAUDE_PRETOOL_SIDECAR_CONFIG" ]; then
        pass "Config from \$CLAUDE_PRETOOL_SIDECAR_CONFIG: ${CLAUDE_PRETOOL_SIDECAR_CONFIG}"
        CONFIG_FOUND="$CLAUDE_PRETOOL_SIDECAR_CONFIG"
    else
        fail "\$CLAUDE_PRETOOL_SIDECAR_CONFIG set but file not found: ${CLAUDE_PRETOOL_SIDECAR_CONFIG}"
    fi
elif [ -f ".claude-pretool-sidecar.toml" ]; then
    pass "Config found: .claude-pretool-sidecar.toml (project-level)"
    CONFIG_FOUND=".claude-pretool-sidecar.toml"
elif [ -f "${HOME}/.config/claude-pretool-sidecar/config.toml" ]; then
    pass "Config found: ~/.config/claude-pretool-sidecar/config.toml (XDG)"
    CONFIG_FOUND="${HOME}/.config/claude-pretool-sidecar/config.toml"
elif [ -f "${HOME}/.claude-pretool-sidecar.toml" ]; then
    pass "Config found: ~/.claude-pretool-sidecar.toml (home)"
    CONFIG_FOUND="${HOME}/.claude-pretool-sidecar.toml"
else
    warn "No config file found in standard locations"
    echo "       Searched: .claude-pretool-sidecar.toml, ~/.config/claude-pretool-sidecar/config.toml, ~/.claude-pretool-sidecar.toml"
fi

# 3. Validate config if binary and config both exist
if [ -n "$CONFIG_FOUND" ] && command -v claude-pretool-sidecar >/dev/null 2>&1; then
    if claude-pretool-sidecar --validate --config "$CONFIG_FOUND" >/dev/null 2>&1; then
        pass "Config validation passed"
    else
        # Try without --config flag (let it find config itself)
        if claude-pretool-sidecar --validate >/dev/null 2>&1; then
            pass "Config validation passed"
        else
            warn "Config validation failed or --validate not yet implemented"
        fi
    fi
fi
echo

# 4. Check hook registration
echo "--- Hook Registration ---"
HOOKS_FOUND=false

for SETTINGS_FILE in ".claude/settings.json" ".claude/settings.local.json" "${HOME}/.claude/settings.json"; do
    if [ -f "$SETTINGS_FILE" ]; then
        if grep -q "claude-pretool-sidecar" "$SETTINGS_FILE" 2>/dev/null; then
            pass "Hook registered in: ${SETTINGS_FILE}"
            HOOKS_FOUND=true
        fi
    fi
done

if [ "$HOOKS_FOUND" = false ]; then
    warn "No hook registration found in Claude Code settings"
    echo "       If using the plugin system, this is expected"
    echo "       Otherwise, add hooks to .claude/settings.json (see resources/hook-setup.md)"
fi
echo

# 5. Summary
echo "=== Summary ==="
echo "  Passed: ${PASS}  Warnings: ${WARN}  Failed: ${FAIL}"

if [ "$FAIL" -gt 0 ]; then
    echo
    echo "Some checks failed. See above for details."
    exit 1
elif [ "$WARN" -gt 0 ]; then
    echo
    echo "Some warnings. The sidecar may work but review the warnings above."
    exit 0
else
    echo
    echo "All checks passed."
    exit 0
fi
