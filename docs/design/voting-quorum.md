# Design: Voting & Quorum Logic

*Date: 2026-03-22*

## Problem

When multiple decision-making providers are configured, we need a deterministic way to aggregate their votes (allow / deny / passthrough) into a single decision that the sidecar returns to Claude Code's hook system.

## Provider Response Types

Each provider returns one of:

* **allow** — provider approves the tool execution
* **deny** — provider rejects the tool execution
* **passthrough** — provider abstains (refuses to vote)
* **error** — provider failed to respond (timeout, crash, invalid output)

Additionally, providers can be configured as:

* **fyi** — "for your info" mode: provider receives the hook payload but its response is ignored. Used for logging, auditing, telemetry.

## Quorum Configuration

Users configure quorum rules via these parameters:

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `min_allow` | u32 | 1 | Minimum number of "allow" votes required |
| `max_deny` | u32 | 0 | Maximum number of "deny" votes tolerated |
| `error_policy` | enum | `passthrough` | How to treat provider errors: `passthrough`, `deny`, `allow` |
| `default_decision` | enum | `passthrough` | Decision when quorum is not met and no deny threshold exceeded |

### Shorthand Flags

For convenience, users can set a single `min_consensus` value that sets both `min_allow` and `max_deny=0`, meaning "at least N agree, zero disagree."

## Decision Algorithm

```
1. Collect votes from all non-FYI providers (respecting timeouts)
2. Classify votes: allow_count, deny_count, passthrough_count, error_count
3. Apply error_policy to convert error_count into the configured category
4. IF deny_count > max_deny → DENY
5. IF allow_count >= min_allow → ALLOW
6. ELSE → default_decision
```

**Deny takes priority**: if deny threshold is exceeded, the request is denied regardless of allow count. This is a security-conscious default.

## Examples

### Example 1: Simple majority (3 providers)
```toml
min_allow = 2
max_deny = 0
```
At least 2 must allow, zero denies tolerated. One passthrough is OK.

### Example 2: Any single gatekeeper (1 provider)
```toml
min_allow = 1
max_deny = 0
```
The single provider must allow. Default for single-provider setups.

### Example 3: Tolerant setup (5 providers)
```toml
min_allow = 3
max_deny = 1
```
At least 3 allow, up to 1 deny tolerated. Remaining can passthrough.

### Example 4: Pure logging (0 decision providers, all FYI)
```toml
min_allow = 0
default_decision = "passthrough"
```
No voting providers. All providers are FYI. Always passthrough.

## Edge Cases

* **Zero non-FYI providers**: Returns `default_decision` immediately
* **All providers error**: Depends on `error_policy` — if all convert to passthrough and `min_allow > 0`, returns `default_decision`
* **Provider timeout**: Treated as error, subject to `error_policy`
