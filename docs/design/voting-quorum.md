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

## Weighted Voting

Providers can have configurable weights that affect how their votes are counted in quorum aggregation. By default, each provider has a weight of 1.

### Configuration

```toml
[[providers]]
name = "security-checker"
command = "/usr/bin/sec-check"
mode = "vote"
weight = 2    # This provider's vote counts double
```

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `weight` | u32 | 1 | How much this provider's vote counts in quorum |

* Weight is only meaningful for `vote`-mode providers; it is ignored for `fyi` providers.
* Weight defaults to 1, preserving full backward compatibility with existing configs.

### How Weights Affect the Algorithm

Instead of counting each vote as 1, each vote contributes its provider's weight:

```
1. Collect weighted votes from all non-FYI providers
2. For each vote:
   - allow → allow_count += weight
   - deny  → deny_count  += weight
   - passthrough → passthrough_count += weight
   - error → apply error_policy, then add weight to the resulting category
3. IF deny_count > max_deny → DENY
4. IF allow_count >= min_allow → ALLOW
5. ELSE → default_decision
```

### Examples

#### Example: Security checker with double weight

```toml
[quorum]
min_allow = 2
max_deny = 0

[[providers]]
name = "security-checker"
command = "/usr/bin/sec-check"
weight = 2

[[providers]]
name = "style-checker"
command = "/usr/bin/style-check"
weight = 1
```

If only `security-checker` votes allow, its weight of 2 satisfies `min_allow = 2` on its own.

#### Example: Heavy deny overrides tolerance

```toml
[quorum]
min_allow = 1
max_deny = 1

[[providers]]
name = "critical-scanner"
command = "/usr/bin/critical-scan"
weight = 2
```

If `critical-scanner` votes deny, its weighted deny count of 2 exceeds `max_deny = 1`, causing an overall deny even though only one provider denied.

## Edge Cases

* **Zero non-FYI providers**: Returns `default_decision` immediately
* **All providers error**: Depends on `error_policy` — if all convert to passthrough and `min_allow > 0`, returns `default_decision`
* **Provider timeout**: Treated as error, subject to `error_policy`
