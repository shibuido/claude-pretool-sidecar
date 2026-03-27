#!/usr/bin/env bash
# check-sidecar.sh — Verify claude-pretool-sidecar installation and configuration
#
# Usage:
#   bash check-sidecar.sh                  # Full health check with all output
#   bash check-sidecar.sh --quiet          # Only output errors (for SessionStart hook)
#   bash check-sidecar.sh --install-hint   # Suggest install-hooks.sh if hooks not configured
#
# Or if installed as a plugin:
#   bash "${CLAUDE_PLUGIN_ROOT}/scripts/check-sidecar.sh"

set -euo pipefail

QUIET=false
INSTALL_HINT=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --quiet)
            QUIET=true
            shift
            ;;
        --install-hint)
            INSTALL_HINT=true
            shift
            ;;
        -h|--help)
            echo "Usage: $0 [--quiet] [--install-hint]"
            echo
            echo "  --quiet          Only output errors (for use in SessionStart hooks)"
            echo "  --install-hint   Suggest install-hooks.sh if hooks aren't configured"
            exit 0
            ;;
        *)
            echo "Error: Unknown argument: $1" >&2
            exit 1
            ;;
    esac
done

PASS=0
WARN=0
FAIL=0

pass() {
    PASS=$((PASS + 1))
    if [[ "$QUIET" == false ]]; then
        echo "  [OK]   $1"
    fi
}

warn() {
    WARN=$((WARN + 1))
    if [[ "$QUIET" == false ]]; then
        echo "  [WARN] $1"
    fi
}

fail() {
    FAIL=$((FAIL + 1))
    # Always output failures, even in quiet mode
    echo "  [FAIL] $1"
}

info() {
    if [[ "$QUIET" == false ]]; then
        echo "$1"
    fi
}

info "=== claude-pretool-sidecar health check ==="
info ""

# 1. Check binary in PATH
info "--- Binary ---"
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
    if [[ "$QUIET" == false ]]; then
        echo "       Install with: cargo install --path <source-dir>"
        echo "       Or add ~/.cargo/bin to PATH"
    fi
fi
info ""

# 2. Check config file
info "--- Configuration ---"
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
    if [[ "$QUIET" == false ]]; then
        echo "       Searched: .claude-pretool-sidecar.toml, ~/.config/claude-pretool-sidecar/config.toml, ~/.claude-pretool-sidecar.toml"
    fi
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
info ""

# 4. Check hook registration
info "--- Hook Registration ---"
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
    if [[ "$QUIET" == false ]]; then
        echo "       If using the plugin system, this is expected"
        echo "       Otherwise, add hooks to .claude/settings.json (see resources/hook-setup.md)"
    fi
    if [[ "$INSTALL_HINT" == true ]]; then
        HINT_SCRIPT=""
        if [[ -n "${CLAUDE_PLUGIN_ROOT:-}" ]]; then
            HINT_SCRIPT="${CLAUDE_PLUGIN_ROOT}/scripts/install-hooks.sh"
        else
            HINT_SCRIPT="plugin/scripts/install-hooks.sh"
        fi
        echo
        echo "  Hint: Run the install script to register hooks:"
        echo "    bash ${HINT_SCRIPT} --scope project"
        echo "    bash ${HINT_SCRIPT} --scope user"
    fi
fi
info ""

# 5. Summary
info "=== Summary ==="
info "  Passed: ${PASS}  Warnings: ${WARN}  Failed: ${FAIL}"

if [ "$FAIL" -gt 0 ]; then
    info ""
    info "Some checks failed. See above for details."
    exit 1
elif [ "$WARN" -gt 0 ]; then
    info ""
    info "Some warnings. The sidecar may work but review the warnings above."
    exit 0
else
    info ""
    info "All checks passed."
    exit 0
fi
