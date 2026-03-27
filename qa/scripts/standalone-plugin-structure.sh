#!/bin/bash
# QA Tests: Plugin Structure Validation
#
# Validates that the plugin/ directory has the correct structure,
# valid JSON files, proper SKILL.md frontmatter, executable scripts,
# and non-empty resource files.
#
# Requires: jq

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
QA_DIR="$(dirname "$SCRIPT_DIR")"
PROJECT_DIR="$(dirname "$QA_DIR")"
PLUGIN_DIR="$PROJECT_DIR/plugin"

PASS=0
FAIL=0

pass() { PASS=$((PASS + 1)); echo "  PASS: $1"; }
fail() { FAIL=$((FAIL + 1)); echo "  FAIL: $1"; }

echo "=== Plugin Structure Validation Tests ==="

# --- Test 1: plugin.json is valid JSON with required fields ---
echo "[1] plugin.json is valid JSON with required fields"
PLUGIN_JSON="$PLUGIN_DIR/.claude-plugin/plugin.json"
if [ ! -f "$PLUGIN_JSON" ]; then
  fail "plugin.json not found at $PLUGIN_JSON"
else
  if jq empty "$PLUGIN_JSON" 2>/dev/null; then
    NAME=$(jq -r '.name // empty' "$PLUGIN_JSON")
    DESC=$(jq -r '.description // empty' "$PLUGIN_JSON")
    if [ -n "$NAME" ] && [ -n "$DESC" ]; then
      pass "plugin.json valid with name='$NAME'"
    else
      fail "plugin.json missing required fields (name and/or description)"
    fi
  else
    fail "plugin.json is not valid JSON"
  fi
fi

# --- Test 2: hooks.json is valid JSON with correct wrapper format ---
echo "[2] hooks.json is valid JSON with hooks key"
HOOKS_JSON="$PLUGIN_DIR/hooks/hooks.json"
if [ ! -f "$HOOKS_JSON" ]; then
  fail "hooks.json not found at $HOOKS_JSON"
else
  if jq empty "$HOOKS_JSON" 2>/dev/null; then
    HAS_HOOKS=$(jq -e '.hooks' "$HOOKS_JSON" >/dev/null 2>&1 && echo "yes" || echo "no")
    if [ "$HAS_HOOKS" = "yes" ]; then
      pass "hooks.json valid with 'hooks' key"
    else
      fail "hooks.json missing 'hooks' key"
    fi
  else
    fail "hooks.json is not valid JSON"
  fi
fi

# --- Test 3: All SKILL.md files have YAML frontmatter ---
echo "[3] SKILL.md files have YAML frontmatter with name and description"
SKILL_COUNT=0
SKILL_OK=0
for skill_dir in "$PLUGIN_DIR"/skills/*/; do
  SKILL_FILE="$skill_dir/SKILL.md"
  SKILL_NAME="$(basename "$skill_dir")"
  SKILL_COUNT=$((SKILL_COUNT + 1))
  if [ ! -f "$SKILL_FILE" ]; then
    fail "SKILL.md missing for skill '$SKILL_NAME'"
    continue
  fi

  # Check for YAML frontmatter delimiters (--- at line 1 and another ---)
  FIRST_LINE=$(head -n 1 "$SKILL_FILE")
  if [ "$FIRST_LINE" != "---" ]; then
    fail "SKILL.md for '$SKILL_NAME' missing YAML frontmatter (no opening ---)"
    continue
  fi

  # Extract frontmatter (between first and second ---)
  FRONTMATTER=$(sed -n '2,/^---$/p' "$SKILL_FILE" | head -n -1)
  HAS_NAME=$(echo "$FRONTMATTER" | grep -c '^name:' || true)
  HAS_DESC=$(echo "$FRONTMATTER" | grep -c '^description:' || true)

  if [ "$HAS_NAME" -ge 1 ] && [ "$HAS_DESC" -ge 1 ]; then
    SKILL_OK=$((SKILL_OK + 1))
  else
    fail "SKILL.md for '$SKILL_NAME' frontmatter missing name or description"
  fi
done

if [ "$SKILL_COUNT" -eq 0 ]; then
  fail "no skill directories found under plugin/skills/"
elif [ "$SKILL_OK" -eq "$SKILL_COUNT" ]; then
  pass "all $SKILL_COUNT SKILL.md files have valid frontmatter"
fi

# --- Test 4: Expected skill directories exist ---
echo "[4] Expected skill directories exist"
EXPECTED_SKILLS=("configure-sidecar" "diagnose-sidecar" "file-issue")
ALL_SKILLS_OK=true
for skill in "${EXPECTED_SKILLS[@]}"; do
  if [ ! -d "$PLUGIN_DIR/skills/$skill" ]; then
    fail "expected skill directory missing: skills/$skill"
    ALL_SKILLS_OK=false
  fi
done
if [ "$ALL_SKILLS_OK" = true ]; then
  pass "all ${#EXPECTED_SKILLS[@]} expected skill directories present"
fi

# --- Test 5: All scripts are executable ---
echo "[5] All plugin scripts are executable"
SCRIPTS_OK=true
for script in "$PLUGIN_DIR"/scripts/*.sh; do
  if [ ! -x "$script" ]; then
    fail "script not executable: $(basename "$script")"
    SCRIPTS_OK=false
  fi
done
if [ "$SCRIPTS_OK" = true ]; then
  SCRIPT_COUNT=$(ls "$PLUGIN_DIR"/scripts/*.sh 2>/dev/null | wc -l)
  pass "all $SCRIPT_COUNT scripts are executable"
fi

# --- Test 6: All resource files exist and are non-empty ---
echo "[6] Resource files exist and are non-empty"
EXPECTED_RESOURCES=("config-schema.md" "hook-setup.md" "troubleshooting.md")
RES_OK=true
for res in "${EXPECTED_RESOURCES[@]}"; do
  RES_FILE="$PLUGIN_DIR/resources/$res"
  if [ ! -f "$RES_FILE" ]; then
    fail "resource file missing: resources/$res"
    RES_OK=false
  elif [ ! -s "$RES_FILE" ]; then
    fail "resource file empty: resources/$res"
    RES_OK=false
  fi
done
if [ "$RES_OK" = true ]; then
  pass "all ${#EXPECTED_RESOURCES[@]} resource files present and non-empty"
fi

# --- Test 7: No broken relative path references in SKILL.md files ---
echo "[7] No broken relative path references in skills"
REFS_OK=true
for skill_file in "$PLUGIN_DIR"/skills/*/SKILL.md; do
  # Look for references to resources/ paths in the SKILL.md files
  RESOURCE_REFS=$(grep -oP 'resources/[a-zA-Z0-9_-]+\.md' "$skill_file" 2>/dev/null || true)
  for ref in $RESOURCE_REFS; do
    # Resources should be relative to plugin root
    if [ ! -f "$PLUGIN_DIR/$ref" ]; then
      fail "broken resource reference '$ref' in $(basename "$(dirname "$skill_file")")/SKILL.md"
      REFS_OK=false
    fi
  done
done
if [ "$REFS_OK" = true ]; then
  pass "no broken relative path references in skills"
fi

# --- Test 8: README.md exists ---
echo "[8] Plugin README.md exists and is non-empty"
if [ -f "$PLUGIN_DIR/README.md" ] && [ -s "$PLUGIN_DIR/README.md" ]; then
  pass "plugin README.md present and non-empty"
else
  fail "plugin README.md missing or empty"
fi

echo ""
echo "=== Results: $PASS passed, $FAIL failed ==="
[ "$FAIL" -eq 0 ] || exit 1
