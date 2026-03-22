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
use std::time::{Duration, Instant};

/// A vote returned by a provider (or derived from an error).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Vote {
    Allow,
    Deny,
    Passthrough,
    Error,
}

/// Detailed result from a single provider execution.
/// Used for audit logging — captures timing and vote details.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProviderResult {
    /// Provider name from config
    pub name: String,
    /// The vote (or error)
    pub vote: Vote,
    /// Provider mode (vote or fyi)
    pub mode: String,
    /// How long the provider took to respond (milliseconds)
    pub response_time_ms: u64,
    /// Optional reason from the provider
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Error message if the provider failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Response parsed from a provider's stdout.
#[derive(Debug, serde::Deserialize)]
struct ProviderResponse {
    decision: String,
    #[allow(dead_code)]
    reason: Option<String>,
}

/// Execute all configured providers and collect detailed results.
///
/// Returns ProviderResult for every provider (including FYI).
/// The caller uses mode to filter voting vs FYI results.
pub fn execute_all(
    providers: &[ProviderConfig],
    event: &HookEvent,
    timeout_config: &TimeoutConfig,
) -> Vec<ProviderResult> {
    let payload = match event.to_json() {
        Ok(json) => json,
        Err(e) => {
            eprintln!("claude-pretool-sidecar: failed to serialize event for providers: {e}");
            return vec![];
        }
    };

    let mut results = Vec::new();

    for provider in providers {
        let result = execute_one(provider, &payload, timeout_config);
        results.push(result);
    }

    results
}

/// Extract only the votes from non-FYI providers (for quorum aggregation).
pub fn votes_from_results(results: &[ProviderResult]) -> Vec<Vote> {
    results
        .iter()
        .filter(|r| r.mode == "vote")
        .map(|r| r.vote.clone())
        .collect()
}

/// Execute a single provider and return its detailed result.
fn execute_one(
    provider: &ProviderConfig,
    payload: &str,
    timeout_config: &TimeoutConfig,
) -> ProviderResult {
    let timeout_ms = provider.timeout.unwrap_or(timeout_config.provider_default);
    let timeout = Duration::from_millis(timeout_ms);
    let start = Instant::now();
    let mode = match provider.mode {
        ProviderMode::Vote => "vote",
        ProviderMode::Fyi => "fyi",
    };

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
            let elapsed = start.elapsed();
            return ProviderResult {
                name: provider.name.clone(),
                vote: Vote::Error,
                mode: mode.to_string(),
                response_time_ms: elapsed.as_millis() as u64,
                reason: None,
                error: Some(format!("failed to spawn: {e}")),
            };
        }
    };

    // Write payload to stdin
    if let Some(mut stdin) = child.stdin.take() {
        if let Err(e) = stdin.write_all(payload.as_bytes()) {
            let elapsed = start.elapsed();
            let _ = child.kill();
            return ProviderResult {
                name: provider.name.clone(),
                vote: Vote::Error,
                mode: mode.to_string(),
                response_time_ms: elapsed.as_millis() as u64,
                reason: None,
                error: Some(format!("failed to write stdin: {e}")),
            };
        }
        // Drop stdin to signal EOF
    }

    // Wait for the process with timeout
    let output = match child.wait_with_output() {
        Ok(output) => output,
        Err(e) => {
            let elapsed = start.elapsed();
            return ProviderResult {
                name: provider.name.clone(),
                vote: Vote::Error,
                mode: mode.to_string(),
                response_time_ms: elapsed.as_millis() as u64,
                reason: None,
                error: Some(format!("wait failed: {e}")),
            };
        }
    };

    let elapsed = start.elapsed();
    let _ = timeout; // TODO: proper timeout enforcement

    // Check exit code
    if !output.status.success() {
        return ProviderResult {
            name: provider.name.clone(),
            vote: Vote::Error,
            mode: mode.to_string(),
            response_time_ms: elapsed.as_millis() as u64,
            reason: None,
            error: Some(format!("exited with status {}", output.status)),
        };
    }

    // Parse stdout
    let stdout = match String::from_utf8(output.stdout) {
        Ok(s) => s,
        Err(e) => {
            return ProviderResult {
                name: provider.name.clone(),
                vote: Vote::Error,
                mode: mode.to_string(),
                response_time_ms: elapsed.as_millis() as u64,
                reason: None,
                error: Some(format!("stdout not valid UTF-8: {e}")),
            };
        }
    };

    let (vote, reason, error) = parse_provider_response(&stdout, &provider.name);

    ProviderResult {
        name: provider.name.clone(),
        vote,
        mode: mode.to_string(),
        response_time_ms: elapsed.as_millis() as u64,
        reason,
        error,
    }
}

/// Parse a provider's stdout JSON into a Vote, reason, and optional error.
fn parse_provider_response(stdout: &str, provider_name: &str) -> (Vote, Option<String>, Option<String>) {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return (Vote::Error, None, Some(format!("provider '{provider_name}' returned empty output")));
    }

    let response: ProviderResponse = match serde_json::from_str(trimmed) {
        Ok(r) => r,
        Err(e) => {
            return (Vote::Error, None, Some(format!("provider '{provider_name}' invalid JSON: {e}")));
        }
    };

    let vote = match response.decision.as_str() {
        "allow" => Vote::Allow,
        "deny" => Vote::Deny,
        "passthrough" => Vote::Passthrough,
        other => {
            return (Vote::Error, None, Some(format!("provider '{provider_name}' unknown decision: '{other}'")));
        }
    };

    (vote, response.reason, None)
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
        let (vote, _, error) = parse_provider_response(r#"{"decision": "allow"}"#, "test");
        assert_eq!(vote, Vote::Allow);
        assert!(error.is_none());
    }

    /// A valid "deny" response with a reason should parse to Vote::Deny.
    #[test]
    fn parse_deny_with_reason() {
        let (vote, reason, _) = parse_provider_response(
            r#"{"decision": "deny", "reason": "dangerous"}"#,
            "test",
        );
        assert_eq!(vote, Vote::Deny);
        assert_eq!(reason, Some("dangerous".to_string()));
    }

    /// A valid "passthrough" response should parse to Vote::Passthrough.
    #[test]
    fn parse_passthrough_response() {
        let (vote, _, _) = parse_provider_response(r#"{"decision": "passthrough"}"#, "test");
        assert_eq!(vote, Vote::Passthrough);
    }

    /// Empty output should be treated as an error.
    #[test]
    fn empty_output_is_error() {
        let (vote, _, error) = parse_provider_response("", "test");
        assert_eq!(vote, Vote::Error);
        assert!(error.is_some());
    }

    /// Invalid JSON should be treated as an error.
    #[test]
    fn invalid_json_is_error() {
        let (vote, _, error) = parse_provider_response("not json", "test");
        assert_eq!(vote, Vote::Error);
        assert!(error.is_some());
    }

    /// Unknown decision value should be treated as an error.
    #[test]
    fn unknown_decision_is_error() {
        let (vote, _, error) = parse_provider_response(r#"{"decision": "maybe"}"#, "test");
        assert_eq!(vote, Vote::Error);
        assert!(error.is_some());
    }

    /// Output with extra whitespace/newlines should still parse.
    #[test]
    fn whitespace_trimmed() {
        let (vote, _, _) = parse_provider_response("  \n{\"decision\": \"allow\"}\n  ", "test");
        assert_eq!(vote, Vote::Allow);
    }
}
