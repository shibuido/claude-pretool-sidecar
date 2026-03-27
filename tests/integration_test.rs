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
use predicates::prelude::*;
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

/// # CLI: --config flag
///
/// When --config is provided, it should use that config file
/// instead of environment variable or file discovery.
#[test]
fn cli_config_flag_overrides_env() {
    let allow_config = format!(
        r#"
        [quorum]
        min_allow = 1

        [[providers]]
        name = "allower"
        command = "{}"
        mode = "vote"
        "#,
        fixtures_dir().join("provider-always-allow.sh").display()
    );
    let deny_config = format!(
        r#"
        [quorum]
        min_allow = 1

        [[providers]]
        name = "denier"
        command = "{}"
        mode = "vote"
        "#,
        fixtures_dir().join("provider-always-deny.sh").display()
    );

    let allow_file = write_temp_config(&allow_config);
    let deny_file = write_temp_config(&deny_config);
    let payload = read_payload("hook-bash-payload.json");

    // --config should take priority over env var
    let output = Command::cargo_bin("claude-pretool-sidecar")
        .unwrap()
        .args(["--config", &allow_file.path().to_string_lossy()])
        .env("CLAUDE_PRETOOL_SIDECAR_CONFIG", deny_file.path())
        .write_stdin(payload)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let response: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    // --config pointed to allow config, so should be allow (not deny from env)
    assert_eq!(extract_decision(&response), Some("allow"));
}

/// # CLI: --validate with valid config
///
/// When --validate is set with a valid config, it should exit 0
/// and print "config valid" to stderr.
#[test]
fn cli_validate_valid_config_exits_zero() {
    let config = format!(
        r#"
        [quorum]
        min_allow = 1

        [[providers]]
        name = "checker"
        command = "{}"
        mode = "vote"
        "#,
        fixtures_dir().join("provider-always-allow.sh").display()
    );
    let config_file = write_temp_config(&config);

    Command::cargo_bin("claude-pretool-sidecar")
        .unwrap()
        .args(["--validate", "--config", &config_file.path().to_string_lossy()])
        .assert()
        .success()
        .stderr(predicate::str::contains("config valid"));
}

/// # CLI: --validate with warnings
///
/// When min_allow exceeds vote provider count, --validate should still
/// exit 0 but show a warning.
#[test]
fn cli_validate_with_warnings_exits_zero() {
    let config = r#"
        [quorum]
        min_allow = 5

        [[providers]]
        name = "checker"
        command = "echo"
        mode = "vote"
    "#;
    let config_file = write_temp_config(config);

    Command::cargo_bin("claude-pretool-sidecar")
        .unwrap()
        .args(["--validate", "--config", &config_file.path().to_string_lossy()])
        .assert()
        .success()
        .stderr(predicate::str::contains("warning"))
        .stderr(predicate::str::contains("min_allow"));
}

/// # CLI: --validate with errors
///
/// When config has duplicate provider names, --validate should exit 1.
#[test]
fn cli_validate_with_errors_exits_one() {
    let config = r#"
        [[providers]]
        name = "checker"
        command = "echo"

        [[providers]]
        name = "checker"
        command = "true"
    "#;
    let config_file = write_temp_config(config);

    Command::cargo_bin("claude-pretool-sidecar")
        .unwrap()
        .args(["--validate", "--config", &config_file.path().to_string_lossy()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("duplicate provider name"));
}

/// # CLI: --post-tool outputs passthrough
///
/// When --post-tool is set, the sidecar should skip provider voting
/// and output `{}` (passthrough).
#[test]
fn cli_post_tool_outputs_passthrough() {
    let config = format!(
        r#"
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
        .args(["--post-tool", "--config", &config_file.path().to_string_lossy()])
        .write_stdin(payload)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.trim(), "{}");
}

/// # CLI: --passthrough when no config found
///
/// When --passthrough is set and no config file exists, the sidecar
/// should use an empty config and output passthrough instead of erroring.
#[test]
fn cli_passthrough_no_config_outputs_passthrough() {
    let payload = read_payload("hook-bash-payload.json");
    let tmp = tempfile::TempDir::new().unwrap();

    let output = Command::cargo_bin("claude-pretool-sidecar")
        .unwrap()
        .args(["--passthrough"])
        // Point HOME and config away from any real config files
        .env("HOME", tmp.path())
        .env_remove("CLAUDE_PRETOOL_SIDECAR_CONFIG")
        .env_remove("XDG_CONFIG_HOME")
        .current_dir(tmp.path())
        .write_stdin(payload)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let response: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    // Empty config with default quorum (min_allow=1, 0 providers) → default_decision = passthrough
    assert_eq!(response, serde_json::json!({}));
}

/// # CLI: no --passthrough and no config errors out
///
/// Without --passthrough flag, a missing config should cause exit 1.
#[test]
fn cli_no_passthrough_no_config_errors() {
    let payload = read_payload("hook-bash-payload.json");
    let tmp = tempfile::TempDir::new().unwrap();

    Command::cargo_bin("claude-pretool-sidecar")
        .unwrap()
        .env("HOME", tmp.path())
        .env_remove("CLAUDE_PRETOOL_SIDECAR_CONFIG")
        .env_remove("XDG_CONFIG_HOME")
        .current_dir(tmp.path())
        .write_stdin(payload)
        .assert()
        .failure()
        .stderr(predicate::str::contains("config error"));
}

/// # ENV: CPTS_MIN_ALLOW changes voting behavior
///
/// When config has min_allow=1 (met by one allow provider), but
/// CPTS_MIN_ALLOW=3 overrides it, quorum can't be met with one provider,
/// so the default_decision (passthrough) is used instead of allow.
#[test]
fn env_cpts_min_allow_overrides_config() {
    let config = format!(
        r#"
        [quorum]
        min_allow = 1
        default_decision = "passthrough"

        [[providers]]
        name = "allower"
        command = "{}"
        mode = "vote"
        "#,
        fixtures_dir().join("provider-always-allow.sh").display()
    );
    let config_file = write_temp_config(&config);
    let payload = read_payload("hook-bash-payload.json");

    // Without env override: min_allow=1, one allow provider → allow
    let output_no_env = Command::cargo_bin("claude-pretool-sidecar")
        .unwrap()
        .env("CLAUDE_PRETOOL_SIDECAR_CONFIG", config_file.path())
        .write_stdin(payload.clone())
        .output()
        .unwrap();

    assert!(output_no_env.status.success());
    let stdout = String::from_utf8(output_no_env.stdout).unwrap();
    let response: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(extract_decision(&response), Some("allow"));

    // With CPTS_MIN_ALLOW=3: quorum can't be met → passthrough
    let output_with_env = Command::cargo_bin("claude-pretool-sidecar")
        .unwrap()
        .env("CLAUDE_PRETOOL_SIDECAR_CONFIG", config_file.path())
        .env("CPTS_MIN_ALLOW", "3")
        .write_stdin(payload)
        .output()
        .unwrap();

    assert!(output_with_env.status.success());
    let stdout = String::from_utf8(output_with_env.stdout).unwrap();
    let response: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    // Passthrough = empty object
    assert_eq!(response, serde_json::json!({}));
}

/// # CLI: --version shows version
///
/// The --version flag should output the version from Cargo.toml.
#[test]
fn cli_version_shows_version() {
    Command::cargo_bin("claude-pretool-sidecar")
        .unwrap()
        .args(["--version"])
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}
