//! # Hook Payload Types
//!
//! Defines the data structures for Claude Code hook events (input from stdin)
//! and hook responses (output to stdout).
//!
//! The exact fields are based on the Claude Code Hooks specification.
//! See `docs/design/architecture.md` for the mapping from Claude Code's
//! hook format to our internal types.

use serde::{Deserialize, Serialize};

/// The hook event payload received from Claude Code on stdin.
///
/// This represents the PreToolUse (or PostToolUse) hook invocation.
/// Claude Code sends this as JSON when the hook fires.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HookEvent {
    /// The name of the tool being invoked (e.g., "Bash", "Read", "Write")
    pub tool_name: String,

    /// The tool's input parameters (tool-specific JSON object)
    pub tool_input: serde_json::Value,

    /// The type of hook event
    #[serde(default = "default_hook_type")]
    pub hook_event: String,

    /// Session identifier (if provided by Claude Code)
    #[serde(default)]
    pub session_id: Option<String>,
}

fn default_hook_type() -> String {
    "PreToolUse".to_string()
}

impl HookEvent {
    /// Parse a hook event from JSON string.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Serialize the hook event to JSON (for passing to providers).
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

/// The decision returned by the sidecar to Claude Code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Decision {
    Allow,
    Deny,
    Passthrough,
}

impl std::fmt::Display for Decision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Decision::Allow => write!(f, "allow"),
            Decision::Deny => write!(f, "deny"),
            Decision::Passthrough => write!(f, "passthrough"),
        }
    }
}

/// The hook response written to stdout for Claude Code.
///
/// The exact format depends on Claude Code's hook response specification.
/// This will be refined once research confirms the expected output format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookResponse {
    pub decision: Decision,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl HookResponse {
    pub fn new(decision: Decision) -> Self {
        Self {
            decision,
            reason: None,
        }
    }

    pub fn with_reason(decision: Decision, reason: String) -> Self {
        Self {
            decision,
            reason: Some(reason),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// # HookEvent Parsing Tests
    ///
    /// These tests verify that we can correctly parse the JSON payload
    /// that Claude Code sends to PreToolUse hooks via stdin.

    /// A minimal hook event with just tool_name and tool_input should parse.
    /// The hook_event field defaults to "PreToolUse" when absent.
    #[test]
    fn parse_minimal_hook_event() {
        let json = r#"{"tool_name": "Bash", "tool_input": {"command": "ls"}}"#;
        let event = HookEvent::from_json(json).unwrap();
        assert_eq!(event.tool_name, "Bash");
        assert_eq!(event.hook_event, "PreToolUse");
        assert!(event.session_id.is_none());
    }

    /// A full hook event with all fields should parse correctly.
    #[test]
    fn parse_full_hook_event() {
        let json = r#"{
            "tool_name": "Write",
            "tool_input": {"file_path": "/tmp/test.txt", "content": "hello"},
            "hook_event": "PreToolUse",
            "session_id": "sess-abc123"
        }"#;
        let event = HookEvent::from_json(json).unwrap();
        assert_eq!(event.tool_name, "Write");
        assert_eq!(event.session_id, Some("sess-abc123".to_string()));
    }

    /// Unknown extra fields should be silently ignored (forward compatibility).
    #[test]
    fn parse_ignores_unknown_fields() {
        let json = r#"{
            "tool_name": "Bash",
            "tool_input": {},
            "unknown_field": "value"
        }"#;
        let event = HookEvent::from_json(json).unwrap();
        assert_eq!(event.tool_name, "Bash");
    }

    /// # Decision Serialization Tests

    /// Decision values serialize to lowercase strings.
    #[test]
    fn decision_serializes_lowercase() {
        assert_eq!(
            serde_json::to_string(&Decision::Allow).unwrap(),
            r#""allow""#
        );
        assert_eq!(
            serde_json::to_string(&Decision::Deny).unwrap(),
            r#""deny""#
        );
        assert_eq!(
            serde_json::to_string(&Decision::Passthrough).unwrap(),
            r#""passthrough""#
        );
    }

    /// HookResponse serializes correctly, omitting reason when None.
    #[test]
    fn response_without_reason_omits_field() {
        let resp = HookResponse::new(Decision::Allow);
        let json = serde_json::to_string(&resp).unwrap();
        assert!(!json.contains("reason"));
        assert!(json.contains(r#""decision":"allow""#));
    }

    /// HookResponse includes reason when provided.
    #[test]
    fn response_with_reason_includes_field() {
        let resp = HookResponse::with_reason(Decision::Deny, "dangerous command".to_string());
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains(r#""reason":"dangerous command""#));
    }
}
