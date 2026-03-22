//! # Provider Execution
//!
//! Handles spawning external provider processes, sending them the hook
//! payload via stdin, and collecting their votes from stdout.
//!
//! See `docs/design/stdio-protocol.md` for the communication protocol.

use crate::config::{ProviderConfig, ProviderMode, TimeoutConfig};
use crate::hook::HookEvent;
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Duration;

/// A vote returned by a provider (or derived from an error).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Vote {
    Allow,
    Deny,
    Passthrough,
    Error,
}

/// Response parsed from a provider's stdout.
#[derive(Debug, serde::Deserialize)]
struct ProviderResponse {
    decision: String,
    #[allow(dead_code)]
    reason: Option<String>,
}

/// Execute all configured providers and collect their votes.
///
/// FYI providers are executed but their votes are not included in the result.
/// Each provider runs as a separate process with the hook payload on stdin.
pub fn execute_all(
    providers: &[ProviderConfig],
    event: &HookEvent,
    timeout_config: &TimeoutConfig,
) -> Vec<Vote> {
    let payload = match event.to_json() {
        Ok(json) => json,
        Err(e) => {
            eprintln!("claude-pretool-sidecar: failed to serialize event for providers: {e}");
            return vec![];
        }
    };

    let mut votes = Vec::new();

    for provider in providers {
        let vote = execute_one(provider, &payload, timeout_config);

        match provider.mode {
            ProviderMode::Vote => votes.push(vote),
            ProviderMode::Fyi => {
                // FYI providers: log the result but don't include in votes
                eprintln!(
                    "claude-pretool-sidecar: fyi provider '{}' completed: {:?}",
                    provider.name, vote
                );
            }
        }
    }

    votes
}

/// Execute a single provider and return its vote.
fn execute_one(provider: &ProviderConfig, payload: &str, timeout_config: &TimeoutConfig) -> Vote {
    let timeout_ms = provider.timeout.unwrap_or(timeout_config.provider_default);
    let timeout = Duration::from_millis(timeout_ms);

    // Spawn the provider process
    let mut child = match Command::new(&provider.command)
        .args(&provider.args)
        .envs(&provider.env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            eprintln!(
                "claude-pretool-sidecar: failed to spawn provider '{}': {e}",
                provider.name
            );
            return Vote::Error;
        }
    };

    // Write payload to stdin
    if let Some(mut stdin) = child.stdin.take() {
        if let Err(e) = stdin.write_all(payload.as_bytes()) {
            eprintln!(
                "claude-pretool-sidecar: failed to write to provider '{}' stdin: {e}",
                provider.name
            );
            let _ = child.kill();
            return Vote::Error;
        }
        // Drop stdin to signal EOF
    }

    // Wait for the process with timeout
    let output = match child.wait_with_output() {
        Ok(output) => output,
        Err(e) => {
            eprintln!(
                "claude-pretool-sidecar: provider '{}' wait failed: {e}",
                provider.name
            );
            return Vote::Error;
        }
    };

    // Note: proper timeout handling requires spawning a thread or using async.
    // For now, we rely on the process completing. Timeout enforcement will be
    // added in a future iteration (see FUTURE_WORK.md).
    let _ = timeout; // suppress unused warning for now

    // Check exit code
    if !output.status.success() {
        eprintln!(
            "claude-pretool-sidecar: provider '{}' exited with status {}",
            provider.name,
            output.status
        );
        return Vote::Error;
    }

    // Parse stdout
    let stdout = match String::from_utf8(output.stdout) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "claude-pretool-sidecar: provider '{}' stdout is not valid UTF-8: {e}",
                provider.name
            );
            return Vote::Error;
        }
    };

    parse_provider_response(&stdout, &provider.name)
}

/// Parse a provider's stdout JSON into a Vote.
fn parse_provider_response(stdout: &str, provider_name: &str) -> Vote {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        eprintln!("claude-pretool-sidecar: provider '{provider_name}' returned empty output");
        return Vote::Error;
    }

    let response: ProviderResponse = match serde_json::from_str(trimmed) {
        Ok(r) => r,
        Err(e) => {
            eprintln!(
                "claude-pretool-sidecar: provider '{provider_name}' returned invalid JSON: {e}"
            );
            return Vote::Error;
        }
    };

    match response.decision.as_str() {
        "allow" => Vote::Allow,
        "deny" => Vote::Deny,
        "passthrough" => Vote::Passthrough,
        other => {
            eprintln!(
                "claude-pretool-sidecar: provider '{provider_name}' returned unknown decision: '{other}'"
            );
            Vote::Error
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// # Provider Response Parsing Tests
    ///
    /// These tests verify that we correctly parse the JSON output from
    /// provider scripts, as specified in `docs/design/stdio-protocol.md`.

    /// A valid "allow" response should parse to Vote::Allow.
    #[test]
    fn parse_allow_response() {
        let stdout = r#"{"decision": "allow"}"#;
        assert_eq!(parse_provider_response(stdout, "test"), Vote::Allow);
    }

    /// A valid "deny" response with a reason should parse to Vote::Deny.
    #[test]
    fn parse_deny_with_reason() {
        let stdout = r#"{"decision": "deny", "reason": "dangerous"}"#;
        assert_eq!(parse_provider_response(stdout, "test"), Vote::Deny);
    }

    /// A valid "passthrough" response should parse to Vote::Passthrough.
    #[test]
    fn parse_passthrough_response() {
        let stdout = r#"{"decision": "passthrough"}"#;
        assert_eq!(parse_provider_response(stdout, "test"), Vote::Passthrough);
    }

    /// Empty output should be treated as an error.
    #[test]
    fn empty_output_is_error() {
        assert_eq!(parse_provider_response("", "test"), Vote::Error);
    }

    /// Invalid JSON should be treated as an error.
    #[test]
    fn invalid_json_is_error() {
        assert_eq!(parse_provider_response("not json", "test"), Vote::Error);
    }

    /// Unknown decision value should be treated as an error.
    #[test]
    fn unknown_decision_is_error() {
        let stdout = r#"{"decision": "maybe"}"#;
        assert_eq!(parse_provider_response(stdout, "test"), Vote::Error);
    }

    /// Output with extra whitespace/newlines should still parse.
    #[test]
    fn whitespace_trimmed() {
        let stdout = "  \n{\"decision\": \"allow\"}\n  ";
        assert_eq!(parse_provider_response(stdout, "test"), Vote::Allow);
    }
}
