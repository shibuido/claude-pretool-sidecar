//! # Audit Logging
//!
//! Built-in audit logging for the sidecar's decision-making process.
//! Captures per-provider vote details, timing, and the final decision
//! for each hook invocation.
//!
//! This is separate from FYI providers — this logs the sidecar's own
//! internal decision process for auditability and pattern analysis.

use crate::config::AuditConfig;
use crate::hook::{Decision, HookEvent};
use crate::provider::ProviderResult;
use serde::Serialize;
use std::io::Write;

/// A single audit log entry capturing the full decision lifecycle.
#[derive(Debug, Serialize)]
pub struct AuditEntry {
    /// Unix timestamp (seconds)
    pub timestamp: u64,
    /// Hook event type (e.g., "PreToolUse", "PostToolUse")
    pub hook_event: String,
    /// Tool name
    pub tool_name: String,
    /// Tool input (the full tool_input object)
    pub tool_input: serde_json::Value,
    /// Session ID (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Per-provider results with timing
    pub providers: Vec<ProviderResult>,
    /// Final aggregated decision
    pub final_decision: String,
    /// Total processing time in milliseconds
    pub total_time_ms: u64,
}

/// Log a decision to the configured audit output.
///
/// Does nothing if audit logging is disabled.
pub fn log_decision(
    config: &AuditConfig,
    event: &HookEvent,
    results: &[ProviderResult],
    decision: Decision,
    total_time_ms: u64,
) {
    if !config.enabled {
        return;
    }

    let entry = AuditEntry {
        timestamp: current_timestamp(),
        hook_event: event.hook_event_name.clone(),
        tool_name: event.tool_name.clone(),
        tool_input: event.tool_input.clone(),
        session_id: event.session_id.clone(),
        providers: results.to_vec(),
        final_decision: decision.to_string(),
        total_time_ms,
    };

    let json = match serde_json::to_string(&entry) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("claude-pretool-sidecar: audit serialization error: {e}");
            return;
        }
    };

    match config.output.as_str() {
        "stderr" => {
            eprintln!("{json}");
        }
        path => {
            if let Err(e) = append_to_file(path, &json) {
                eprintln!("claude-pretool-sidecar: audit log write error ({path}): {e}");
            }
        }
    }
}

fn current_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn append_to_file(path: &str, entry: &str) -> std::io::Result<()> {
    use std::fs::OpenOptions;
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{entry}")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::Vote;

    /// # Audit Entry Serialization Tests

    /// An audit entry should serialize to valid JSON with all fields.
    #[test]
    fn audit_entry_serializes_to_json() {
        let entry = AuditEntry {
            timestamp: 1711111200,
            hook_event: "PreToolUse".to_string(),
            tool_name: "Bash".to_string(),
            tool_input: serde_json::json!({"command": "ls"}),
            session_id: Some("sess-123".to_string()),
            providers: vec![
                ProviderResult {
                    name: "checker".to_string(),
                    vote: Vote::Allow,
                    mode: "vote".to_string(),
                    response_time_ms: 45,
                    reason: None,
                    error: None,
                },
            ],
            final_decision: "allow".to_string(),
            total_time_ms: 50,
        };

        let json = serde_json::to_string(&entry).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["tool_name"], "Bash");
        assert_eq!(parsed["final_decision"], "allow");
        assert_eq!(parsed["providers"][0]["name"], "checker");
        assert_eq!(parsed["providers"][0]["response_time_ms"], 45);
        assert_eq!(parsed["total_time_ms"], 50);
    }

    /// Session ID should be omitted when None.
    #[test]
    fn audit_entry_omits_none_session_id() {
        let entry = AuditEntry {
            timestamp: 0,
            hook_event: "PreToolUse".to_string(),
            tool_name: "Bash".to_string(),
            tool_input: serde_json::json!({}),
            session_id: None,
            providers: vec![],
            final_decision: "passthrough".to_string(),
            total_time_ms: 0,
        };

        let json = serde_json::to_string(&entry).unwrap();
        assert!(!json.contains("session_id"));
    }
}
