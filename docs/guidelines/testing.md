# Guideline: Testing Philosophy

## Tests as Documentation (Knuth-inspired)

We treat tests as the primary documentation of behavior. Each test should be readable as a specification:

```rust
/// When a single provider votes "allow" and quorum requires min_allow=1,
/// the sidecar should return "allow" to Claude Code.
#[test]
fn single_provider_allow_meets_quorum() {
    // ... test body that reads like a story
}
```

### Naming Convention

Test names follow: `{scenario}_{expected_outcome}`

Examples:

* `single_provider_deny_returns_deny`
* `two_of_three_allow_with_zero_max_deny_returns_allow`
* `provider_timeout_with_deny_error_policy_returns_deny`

### Test Levels

1. **Unit tests** — Pure logic (vote aggregation, config parsing, JSON serialization)
2. **Integration tests** — Sidecar binary with mock provider scripts
3. **End-to-end tests** — Full hook lifecycle with simulated Claude Code hook payloads

### Documentation in Tests

Each test module starts with a doc comment explaining what aspect of behavior it covers:

```rust
/// # Quorum Logic Tests
///
/// These tests verify the vote aggregation algorithm described in
/// `docs/design/voting-quorum.md`. Each test corresponds to an example
/// or edge case from that document.
#[cfg(test)]
mod quorum_tests {
    // ...
}
```
