#!/bin/bash
# Generate TOML config files for QA testing.
#
# Usage:
#   gen-config.sh passthrough                    # Pure passthrough (no providers)
#   gen-config.sh logging /tmp/audit             # Logging-only setup
#   gen-config.sh single-allow /path/to/allow.sh # Single allow provider
#   gen-config.sh single-deny /path/to/deny.sh   # Single deny provider
#   gen-config.sh multi /path/allow.sh /path/deny.sh  # Mixed providers
#   gen-config.sh error-deny /path/to/crash.sh   # Error policy = deny
#   gen-config.sh audit /tmp/audit               # With audit logging enabled
#
# Outputs TOML to stdout. Redirect to file for use.

set -euo pipefail

TEMPLATE="${1:-passthrough}"

case "$TEMPLATE" in
  passthrough)
    cat <<'EOF'
[quorum]
min_allow = 0
default_decision = "passthrough"
EOF
    ;;
  logging)
    AUDIT_DIR="${2:-/tmp/cpts-qa-audit}"
    cat <<EOF
[quorum]
min_allow = 0
default_decision = "passthrough"

[audit]
enabled = true
output = "$AUDIT_DIR"
max_total_bytes = 1048576
max_file_bytes = 524288
EOF
    ;;
  single-allow)
    PROVIDER_CMD="${2:?Usage: gen-config.sh single-allow /path/to/allow.sh}"
    cat <<EOF
[quorum]
min_allow = 1
max_deny = 0

[[providers]]
name = "qa-allower"
command = "$PROVIDER_CMD"
mode = "vote"
EOF
    ;;
  single-deny)
    PROVIDER_CMD="${2:?Usage: gen-config.sh single-deny /path/to/deny.sh}"
    cat <<EOF
[quorum]
min_allow = 1
max_deny = 0

[[providers]]
name = "qa-denier"
command = "$PROVIDER_CMD"
mode = "vote"
EOF
    ;;
  multi)
    ALLOW_CMD="${2:?Usage: gen-config.sh multi /path/allow.sh /path/deny.sh}"
    DENY_CMD="${3:?Usage: gen-config.sh multi /path/allow.sh /path/deny.sh}"
    cat <<EOF
[quorum]
min_allow = 1
max_deny = 1

[[providers]]
name = "qa-allower-1"
command = "$ALLOW_CMD"
mode = "vote"

[[providers]]
name = "qa-allower-2"
command = "$ALLOW_CMD"
mode = "vote"

[[providers]]
name = "qa-denier"
command = "$DENY_CMD"
mode = "vote"
EOF
    ;;
  error-deny)
    CRASH_CMD="${2:?Usage: gen-config.sh error-deny /path/to/crash.sh}"
    cat <<EOF
[quorum]
min_allow = 1
max_deny = 0
error_policy = "deny"

[[providers]]
name = "qa-crasher"
command = "$CRASH_CMD"
mode = "vote"
EOF
    ;;
  fyi)
    PROVIDER_CMD="${2:?Usage: gen-config.sh fyi /path/to/provider.sh}"
    cat <<EOF
[quorum]
min_allow = 0
default_decision = "passthrough"

[[providers]]
name = "qa-fyi"
command = "$PROVIDER_CMD"
mode = "fyi"
EOF
    ;;
  audit)
    AUDIT_DIR="${2:-/tmp/cpts-qa-audit}"
    ALLOW_CMD="${3:-}"
    cat <<EOF
[quorum]
min_allow = 0
default_decision = "passthrough"

[audit]
enabled = true
output = "$AUDIT_DIR"
max_total_bytes = 4096
max_file_bytes = 2048
EOF
    if [ -n "$ALLOW_CMD" ]; then
      cat <<EOF

[[providers]]
name = "qa-voter"
command = "$ALLOW_CMD"
mode = "vote"
EOF
    fi
    ;;
  *)
    echo "Unknown template: $TEMPLATE" >&2
    echo "Templates: passthrough, logging, single-allow, single-deny, multi, error-deny, fyi, audit" >&2
    exit 1
    ;;
esac
