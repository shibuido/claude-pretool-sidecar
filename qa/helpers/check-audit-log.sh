#!/bin/bash
# Validate audit log entries from claude-pretool-sidecar.
#
# Usage:
#   check-audit-log.sh /path/to/audit-2026-03-22.jsonl
#   check-audit-log.sh /path/to/dir/  # Checks all .jsonl files in dir
#
# Checks:
#   - Each line is valid JSON
#   - Required fields present: timestamp, hook_event, tool_name, providers, final_decision
#   - Provider entries have: name, vote, mode, response_time_ms
#   - Sentinel lines (if any) have: _truncated, lines_removed
#
# Exits 0 if all valid, 1 if any errors found.

set -euo pipefail

TARGET="${1:?Usage: check-audit-log.sh <file-or-directory>}"
ERRORS=0
CHECKED=0

check_file() {
  local file="$1"
  local line_num=0

  while IFS= read -r line; do
    line_num=$((line_num + 1))

    # Skip empty lines
    [ -z "$line" ] && continue

    # Check valid JSON
    if ! echo "$line" | jq empty 2>/dev/null; then
      echo "FAIL: $file:$line_num — invalid JSON"
      ERRORS=$((ERRORS + 1))
      continue
    fi

    # Check if sentinel line
    local is_truncated
    is_truncated=$(echo "$line" | jq -r '._truncated // false')
    if [ "$is_truncated" = "true" ]; then
      # Validate sentinel
      local lines_removed
      lines_removed=$(echo "$line" | jq -r '.lines_removed // "missing"')
      if [ "$lines_removed" = "missing" ]; then
        echo "FAIL: $file:$line_num — sentinel missing lines_removed"
        ERRORS=$((ERRORS + 1))
      fi
      CHECKED=$((CHECKED + 1))
      continue
    fi

    # Validate regular audit entry
    local missing=""
    for field in timestamp hook_event tool_name final_decision; do
      local val
      val=$(echo "$line" | jq -r ".$field // \"__MISSING__\"")
      if [ "$val" = "__MISSING__" ]; then
        missing="$missing $field"
      fi
    done

    if [ -n "$missing" ]; then
      echo "FAIL: $file:$line_num — missing fields:$missing"
      ERRORS=$((ERRORS + 1))
    fi

    # Check providers array
    local provider_count
    provider_count=$(echo "$line" | jq '.providers | length')
    local i=0
    while [ "$i" -lt "$provider_count" ]; do
      for pfield in name vote mode response_time_ms; do
        local pval
        pval=$(echo "$line" | jq -r ".providers[$i].$pfield // \"__MISSING__\"")
        if [ "$pval" = "__MISSING__" ]; then
          echo "FAIL: $file:$line_num — provider[$i] missing $pfield"
          ERRORS=$((ERRORS + 1))
        fi
      done
      i=$((i + 1))
    done

    CHECKED=$((CHECKED + 1))
  done < "$file"
}

# Process target
if [ -d "$TARGET" ]; then
  for f in "$TARGET"/audit-*.jsonl; do
    [ -f "$f" ] && check_file "$f"
  done
else
  check_file "$TARGET"
fi

echo "Checked $CHECKED entries, $ERRORS errors"
if [ "$ERRORS" -gt 0 ]; then
  exit 1
else
  echo "All entries valid"
  exit 0
fi
