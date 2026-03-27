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
use wait_timeout::ChildExt;

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
    /// Vote weight from config (default 1, ignored for FYI providers)
    pub weight: u32,
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

/// A vote paired with its provider's weight for weighted quorum aggregation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeightedVote {
    pub vote: Vote,
    pub weight: u32,
}

/// Extract only the votes from non-FYI providers (for quorum aggregation).
/// Superseded by `weighted_votes_from_results` in main binary, but kept as public API.
#[allow(dead_code)]
pub fn votes_from_results(results: &[ProviderResult]) -> Vec<Vote> {
    results
        .iter()
        .filter(|r| r.mode == "vote")
        .map(|r| r.vote.clone())
        .collect()
}

/// Extract weighted votes from non-FYI providers (for weighted quorum aggregation).
pub fn weighted_votes_from_results(results: &[ProviderResult]) -> Vec<WeightedVote> {
    results
        .iter()
        .filter(|r| r.mode == "vote")
        .map(|r| WeightedVote {
            vote: r.vote.clone(),
            weight: r.weight,
        })
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
                weight: provider.weight,
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
                weight: provider.weight,
                response_time_ms: elapsed.as_millis() as u64,
                reason: None,
                error: Some(format!("failed to write stdin: {e}")),
            };
        }
        // Drop stdin to signal EOF
    }

    // Wait for the process with timeout enforcement.
    // We use wait-timeout to avoid blocking indefinitely on slow providers.
    let status = match child.wait_timeout(timeout) {
        Ok(Some(status)) => status,
        Ok(None) => {
            // Timed out — kill the child and return an error vote.
            let _ = child.kill();
            let _ = child.wait(); // Reap the zombie
            let elapsed = start.elapsed();
            return ProviderResult {
                name: provider.name.clone(),
                vote: Vote::Error,
                mode: mode.to_string(),
                weight: provider.weight,
                response_time_ms: elapsed.as_millis() as u64,
                reason: None,
                error: Some(format!(
                    "provider '{}' timed out after {}ms",
                    provider.name, timeout_ms
                )),
            };
        }
        Err(e) => {
            let elapsed = start.elapsed();
            return ProviderResult {
                name: provider.name.clone(),
                vote: Vote::Error,
                mode: mode.to_string(),
                weight: provider.weight,
                response_time_ms: elapsed.as_millis() as u64,
                reason: None,
                error: Some(format!("wait failed: {e}")),
            };
        }
    };

    let elapsed = start.elapsed();

    // Check exit code
    if !status.success() {
        return ProviderResult {
            name: provider.name.clone(),
            vote: Vote::Error,
            mode: mode.to_string(),
            weight: provider.weight,
            response_time_ms: elapsed.as_millis() as u64,
            reason: None,
            error: Some(format!("exited with status {}", status)),
        };
    }

    // Read and parse stdout
    let stdout = match child.stdout.take() {
        Some(stdout) => {
            use std::io::Read;
            let mut buf = String::new();
            let mut reader = stdout;
            match reader.read_to_string(&mut buf) {
                Ok(_) => buf,
                Err(e) => {
                    return ProviderResult {
                        name: provider.name.clone(),
                        vote: Vote::Error,
                        mode: mode.to_string(),
                        weight: provider.weight,
                        response_time_ms: elapsed.as_millis() as u64,
                        reason: None,
                        error: Some(format!("failed to read stdout: {e}")),
                    };
                }
            }
        }
        None => String::new(),
    };

    let (vote, reason, error) = parse_provider_response(&stdout, &provider.name);

    ProviderResult {
        name: provider.name.clone(),
        vote,
        mode: mode.to_string(),
        weight: provider.weight,
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

    /// # Provider Timeout Tests
    ///
    /// These tests verify that the timeout enforcement in `execute_one`
    /// correctly kills slow providers and returns Vote::Error.

    /// Helper: create a minimal ProviderConfig for testing.
    fn test_provider(command: &str, args: &[&str], timeout_ms: Option<u64>) -> ProviderConfig {
        ProviderConfig {
            name: "test-provider".to_string(),
            command: command.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
            mode: ProviderMode::Vote,
            timeout: timeout_ms,
            weight: 1,
            env: std::collections::HashMap::new(),
        }
    }

    fn default_timeout_config() -> TimeoutConfig {
        TimeoutConfig {
            provider_default: 5000,
            total: 30000,
        }
    }

    /// A fast provider that responds within timeout should return its vote normally.
    #[test]
    fn fast_provider_within_timeout_returns_vote() {
        // bash -c: echo JSON immediately and exit
        let provider = test_provider(
            "bash",
            &["-c", r#"echo '{"decision":"allow"}'"#],
            Some(2000),
        );
        let timeout_config = default_timeout_config();
        let payload = r#"{"hook":"PreToolUse","tool_name":"Bash","tool_input":{}}"#;

        let result = execute_one(&provider, payload, &timeout_config);
        assert_eq!(result.vote, Vote::Allow);
        assert!(result.error.is_none());
        assert!(result.response_time_ms < 2000);
    }

    /// A provider that sleeps longer than its timeout should be killed
    /// and return Vote::Error with a timeout message.
    #[test]
    fn slow_provider_exceeding_timeout_returns_error() {
        // bash -c: sleep 10 seconds (well beyond 200ms timeout)
        let provider = test_provider(
            "bash",
            &["-c", "sleep 10; echo '{\"decision\":\"allow\"}'"],
            Some(200),
        );
        let timeout_config = default_timeout_config();
        let payload = r#"{"hook":"PreToolUse","tool_name":"Bash","tool_input":{}}"#;

        let start = Instant::now();
        let result = execute_one(&provider, payload, &timeout_config);
        let wall_time = start.elapsed();

        assert_eq!(result.vote, Vote::Error);
        assert!(result.error.as_ref().unwrap().contains("timed out"));
        assert!(result.error.as_ref().unwrap().contains("200ms"));
        // Should have completed close to the 200ms timeout, not the 10s sleep
        assert!(wall_time < Duration::from_secs(2), "took too long: {:?}", wall_time);
    }

    /// Provider timeout should use the per-provider override when set,
    /// falling back to provider_default from TimeoutConfig.
    #[test]
    fn timeout_uses_per_provider_override() {
        // Provider with 100ms override; the default is 5000ms.
        // The provider sleeps 5s, so only the 100ms override should trigger.
        let provider = test_provider(
            "bash",
            &["-c", "sleep 5; echo '{\"decision\":\"allow\"}'"],
            Some(100),
        );
        let timeout_config = default_timeout_config();
        let payload = r#"{"hook":"PreToolUse","tool_name":"Bash","tool_input":{}}"#;

        let start = Instant::now();
        let result = execute_one(&provider, payload, &timeout_config);
        let wall_time = start.elapsed();

        assert_eq!(result.vote, Vote::Error);
        assert!(result.error.as_ref().unwrap().contains("timed out"));
        assert!(wall_time < Duration::from_secs(2), "should have timed out quickly");
    }

    /// Provider timeout should use the default when no per-provider override is set.
    #[test]
    fn timeout_falls_back_to_default_config() {
        // No per-provider timeout; config default is 200ms.
        let provider = test_provider(
            "bash",
            &["-c", "sleep 5; echo '{\"decision\":\"allow\"}'"],
            None,
        );
        let timeout_config = TimeoutConfig {
            provider_default: 200,
            total: 30000,
        };
        let payload = r#"{"hook":"PreToolUse","tool_name":"Bash","tool_input":{}}"#;

        let start = Instant::now();
        let result = execute_one(&provider, payload, &timeout_config);
        let wall_time = start.elapsed();

        assert_eq!(result.vote, Vote::Error);
        assert!(result.error.as_ref().unwrap().contains("timed out"));
        assert!(wall_time < Duration::from_secs(2));
    }

    /// Response time should be accurately recorded for both fast and timed-out providers.
    #[test]
    fn response_time_ms_accurate_for_fast_provider() {
        let provider = test_provider(
            "bash",
            &["-c", r#"echo '{"decision":"passthrough"}'"#],
            Some(5000),
        );
        let timeout_config = default_timeout_config();
        let payload = r#"{"hook":"PreToolUse","tool_name":"Bash","tool_input":{}}"#;

        let result = execute_one(&provider, payload, &timeout_config);
        assert_eq!(result.vote, Vote::Passthrough);
        // A fast echo should complete in well under a second
        assert!(result.response_time_ms < 1000);
    }
}
