//! # Integration Tests
//!
//! These tests verify the claude-pretool-sidecar binary end-to-end:
//! reading a hook payload from stdin, executing mock provider scripts,
//! and returning the aggregated decision on stdout.
//!
//! The output format matches Claude Code's hook response specification:
//! - Allow/Deny: `{"hookSpecificOutput":{"permissionDecision":"allow|deny"}}`
//! - Passthrough: `{}` (empty JSON object, no decision)
//!
//! Each test creates a temporary config file pointing to mock provider
//! scripts in `tests/fixtures/`, then runs the sidecar binary with
//! a hook payload on stdin.

use assert_cmd::Command;
use std::fs;
use tempfile::NamedTempFile;

/// Helper: path to the project root (for finding fixture files).
fn fixtures_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Helper: create a temp config file with given TOML content.
fn write_temp_config(content: &str) -> NamedTempFile {
    let file = NamedTempFile::new().unwrap();
    fs::write(file.path(), content).unwrap();
    file
}

/// Helper: read a fixture JSON payload.
fn read_payload(name: &str) -> String {
    fs::read_to_string(fixtures_dir().join(name)).unwrap()
}

/// Helper: extract the permissionDecision from a Claude Code hook response.
/// Returns None for passthrough (empty object).
fn extract_decision(response: &serde_json::Value) -> Option<&str> {
    response
        .get("hookSpecificOutput")
        .and_then(|h| h.get("permissionDecision"))
        .and_then(|d| d.as_str())
}

/// # Single Allow Provider
///
/// When one provider always allows and quorum requires min_allow=1,
/// the sidecar should output a Claude Code hook response with
/// hookSpecificOutput.permissionDecision = "allow".
#[test]
fn single_allow_provider_returns_allow() {
    let config = format!(
        r#"
        [quorum]
        min_allow = 1
        max_deny = 0

        [[providers]]
        name = "allower"
        command = "{}"
        mode = "vote"
        "#,
        fixtures_dir().join("provider-always-allow.sh").display()
    );
    let config_file = write_temp_config(&config);
    let payload = read_payload("hook-bash-payload.json");

    let output = Command::cargo_bin("claude-pretool-sidecar")
        .unwrap()
        .env("CLAUDE_PRETOOL_SIDECAR_CONFIG", config_file.path())
        .write_stdin(payload)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let response: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(extract_decision(&response), Some("allow"));
}

/// # Single Deny Provider
///
/// When one provider always denies and max_deny=0,
/// the sidecar should output permissionDecision = "deny".
#[test]
fn single_deny_provider_returns_deny() {
    let config = format!(
        r#"
        [quorum]
        min_allow = 1
        max_deny = 0

        [[providers]]
        name = "denier"
        command = "{}"
        mode = "vote"
        "#,
        fixtures_dir().join("provider-always-deny.sh").display()
    );
    let config_file = write_temp_config(&config);
    let payload = read_payload("hook-bash-payload.json");

    let output = Command::cargo_bin("claude-pretool-sidecar")
        .unwrap()
        .env("CLAUDE_PRETOOL_SIDECAR_CONFIG", config_file.path())
        .write_stdin(payload)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let response: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(extract_decision(&response), Some("deny"));
}

/// # Crashing Provider with deny error_policy
///
/// When a provider crashes (non-zero exit) and error_policy is "deny",
/// the error should count as a deny vote, causing overall denial.
#[test]
fn crashing_provider_with_deny_error_policy_returns_deny() {
    let config = format!(
        r#"
        [quorum]
        min_allow = 1
        max_deny = 0
        error_policy = "deny"

        [[providers]]
        name = "crasher"
        command = "{}"
        mode = "vote"
        "#,
        fixtures_dir().join("provider-crash.sh").display()
    );
    let config_file = write_temp_config(&config);
    let payload = read_payload("hook-bash-payload.json");

    let output = Command::cargo_bin("claude-pretool-sidecar")
        .unwrap()
        .env("CLAUDE_PRETOOL_SIDECAR_CONFIG", config_file.path())
        .write_stdin(payload)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let response: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(extract_decision(&response), Some("deny"));
}

/// # FYI Provider Does Not Affect Voting
///
/// An FYI provider's output is ignored. With only FYI providers and
/// min_allow=0, quorum is trivially met (0 >= 0) → allow.
#[test]
fn fyi_provider_does_not_affect_votes() {
    let config = format!(
        r#"
        [quorum]
        min_allow = 0
        default_decision = "passthrough"

        [[providers]]
        name = "fyi-logger"
        command = "{}"
        mode = "fyi"
        "#,
        fixtures_dir().join("provider-always-deny.sh").display()
    );
    let config_file = write_temp_config(&config);
    let payload = read_payload("hook-bash-payload.json");

    let output = Command::cargo_bin("claude-pretool-sidecar")
        .unwrap()
        .env("CLAUDE_PRETOOL_SIDECAR_CONFIG", config_file.path())
        .write_stdin(payload)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let response: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    // FYI deny provider is ignored; with min_allow=0 quorum is trivially met → allow
    assert_eq!(extract_decision(&response), Some("allow"));
}

/// # Mixed Providers: Two Allow, One Deny, max_deny=1
///
/// With three voting providers (2 allow, 1 deny) and max_deny=1,
/// the deny is tolerated and the allow quorum is met.
#[test]
fn mixed_providers_with_tolerated_deny() {
    let config = format!(
        r#"
        [quorum]
        min_allow = 2
        max_deny = 1

        [[providers]]
        name = "allower1"
        command = "{allow}"
        mode = "vote"

        [[providers]]
        name = "allower2"
        command = "{allow}"
        mode = "vote"

        [[providers]]
        name = "denier"
        command = "{deny}"
        mode = "vote"
        "#,
        allow = fixtures_dir().join("provider-always-allow.sh").display(),
        deny = fixtures_dir().join("provider-always-deny.sh").display(),
    );
    let config_file = write_temp_config(&config);
    let payload = read_payload("hook-bash-payload.json");

    let output = Command::cargo_bin("claude-pretool-sidecar")
        .unwrap()
        .env("CLAUDE_PRETOOL_SIDECAR_CONFIG", config_file.path())
        .write_stdin(payload)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let response: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(extract_decision(&response), Some("allow"));
}

/// # Bad JSON Provider → Passthrough
///
/// When a provider returns invalid JSON, it's treated as an error.
/// With error_policy=passthrough and min_allow=1, quorum is not met.
/// Passthrough produces an empty JSON object `{}` (no permissionDecision).
#[test]
fn bad_json_provider_treated_as_error() {
    let config = format!(
        r#"
        [quorum]
        min_allow = 1
        max_deny = 0
        error_policy = "passthrough"
        default_decision = "passthrough"

        [[providers]]
        name = "bad-json"
        command = "{}"
        mode = "vote"
        "#,
        fixtures_dir().join("provider-bad-json.sh").display()
    );
    let config_file = write_temp_config(&config);
    let payload = read_payload("hook-bash-payload.json");

    let output = Command::cargo_bin("claude-pretool-sidecar")
        .unwrap()
        .env("CLAUDE_PRETOOL_SIDECAR_CONFIG", config_file.path())
        .write_stdin(payload)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let response: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    // Passthrough = empty object, no hookSpecificOutput
    assert_eq!(extract_decision(&response), None);
    assert_eq!(response, serde_json::json!({}));
}
