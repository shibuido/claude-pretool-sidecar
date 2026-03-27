# Analysis: --permission-prompt-tool MCP Integration

*Researched: 2026-03-27*

## Conclusion

**Hooks are the better primary path.** The MCP `--permission-prompt-tool` is non-interactive only (`-p` mode), has an underdocumented API (reverse-engineered by community), and offers limited benefit over PreToolUse hooks.

## Key Findings

### When --permission-prompt-tool Fires

Only in non-interactive (`-p`) mode, only when a tool needs permission approval, and only AFTER PreToolUse hooks and permission rules have already been evaluated.

**Execution order:**
1. PreToolUse hook (runs first, always)
2. Permission rules (deny/ask/allow)
3. If ask + non-interactive → calls MCP permission-prompt-tool
4. If hook denies at step 1, MCP tool never runs

### Response Format (reverse-engineered)

```json
// Allow
{"behavior": "allow", "updatedInput": {}}

// Deny
{"behavior": "deny", "message": "reason"}
```

**Warning:** This format was reverse-engineered (GitHub issue #1175), not officially documented. Brittleness risk.

### Comparison with PreToolUse Hooks

| Aspect | PreToolUse Hook | MCP permission-prompt-tool |
|--------|----------------|---------------------------|
| Interactive sessions | Yes | No |
| Non-interactive (-p) | Yes | Yes |
| Runs when | Before ANY tool | Only when permission needed |
| Priority | Runs first | Runs after hooks + rules |
| Decision options | allow/deny/ask | allow/deny only |
| Documentation | Official, detailed | Minimal, community-reverse-engineered |

### Recommendation

* **Interactive use (primary):** Stay with PreToolUse hooks
* **Headless/CI use:** Consider MCP wrapper as optional add-on
* **Hybrid:** Share core voting logic, expose via both hooks and MCP

### Decision

Defer MCP wrapper to Phase 5. Current hook-based approach covers the primary use case. If headless demand emerges, build a thin MCP server that calls the same sidecar binary internally.

## Sources

* GitHub Issue #1175 — community reverse-engineering of the API
* code.claude.com/docs/en/permissions.md
* code.claude.com/docs/en/cli-reference.md
