//! # Hook Payload Types
//!
//! Defines the data structures for Claude Code hook events (input from stdin)
//! and hook responses (output to stdout).
//!
//! Based on the Claude Code Hooks specification:
//! - Input: JSON with tool_name, tool_input, hook_event_name, session_id, etc.
//! - Output: JSON with hookSpecificOutput.permissionDecision for PreToolUse
//! - Exit code 0 = success, exit code 2 = blocking error

use serde::{Deserialize, Serialize};

/// The hook event payload received from Claude Code on stdin.
///
/// Claude Code sends this JSON to all hook commands via stdin.
/// Fields vary by hook event type; we capture the common ones
/// and preserve the full payload for forwarding to providers.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HookEvent {
    /// The name of the tool being invoked (e.g., "Bash", "Read", "Write")
    pub tool_name: String,

    /// The tool's input parameters (tool-specific JSON object)
    pub tool_input: serde_json::Value,

    /// The hook event name (e.g., "PreToolUse", "PostToolUse")
    #[serde(default = "default_hook_event_name")]
    pub hook_event_name: String,

    /// Session identifier
    #[serde(default)]
    pub session_id: Option<String>,

    /// Path to the transcript file
    #[serde(default)]
    pub transcript_path: Option<String>,

    /// Current working directory
    #[serde(default)]
    pub cwd: Option<String>,

    /// Permission mode ("ask", "allow", etc.)
    #[serde(default)]
    pub permission_mode: Option<String>,

    /// Unique identifier for the tool call (for correlating Pre/Post events)
    #[serde(default)]
    pub tool_use_id: Option<String>,

    /// Tool result (present in PostToolUse events)
    #[serde(default)]
    pub tool_result: Option<serde_json::Value>,
}

fn default_hook_event_name() -> String {
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

/// Internal decision type used by the sidecar's quorum logic.
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

/// The hook-specific output for PreToolUse events.
///
/// Claude Code expects this nested structure inside the response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookSpecificOutput {
    /// The permission decision: "allow", "deny", or "ask"
    pub permission_decision: String,

    /// Optional: modified tool input (can alter what the tool receives)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_input: Option<serde_json::Value>,
}

/// The hook response written to stdout for Claude Code.
///
/// This matches the Claude Code hook output format:
/// - hookSpecificOutput: contains the permission decision
/// - systemMessage: optional message shown to Claude
///
/// For passthrough, we output an empty JSON object `{}` (exit 0, no decision).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookResponse {
    /// Hook-specific output with permission decision
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hook_specific_output: Option<HookSpecificOutput>,

    /// Optional message for Claude's context
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_message: Option<String>,
}

impl HookResponse {
    /// Create a response that allows the tool call.
    pub fn allow() -> Self {
        Self {
            hook_specific_output: Some(HookSpecificOutput {
                permission_decision: "allow".to_string(),
                updated_input: None,
            }),
            system_message: None,
        }
    }

    /// Create a response that denies the tool call.
    pub fn deny(reason: Option<String>) -> Self {
        Self {
            hook_specific_output: Some(HookSpecificOutput {
                permission_decision: "deny".to_string(),
                updated_input: None,
            }),
            system_message: reason,
        }
    }

    /// Create a passthrough response (empty object — no decision).
    /// Claude Code treats this as "hook has no opinion."
    pub fn passthrough() -> Self {
        Self {
            hook_specific_output: None,
            system_message: None,
        }
    }

    /// Create a response from an internal Decision.
    pub fn from_decision(decision: Decision, reason: Option<String>) -> Self {
        match decision {
            Decision::Allow => Self::allow(),
            Decision::Deny => Self::deny(reason),
            Decision::Passthrough => Self::passthrough(),
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
    /// The hook_event_name field defaults to "PreToolUse" when absent.
    #[test]
    fn parse_minimal_hook_event() {
        let json = r#"{"tool_name": "Bash", "tool_input": {"command": "ls"}}"#;
        let event = HookEvent::from_json(json).unwrap();
        assert_eq!(event.tool_name, "Bash");
        assert_eq!(event.hook_event_name, "PreToolUse");
        assert!(event.session_id.is_none());
    }

    /// A full hook event with all Claude Code fields should parse correctly.
    #[test]
    fn parse_full_hook_event() {
        let json = r#"{
            "tool_name": "Write",
            "tool_input": {"file_path": "/tmp/test.txt", "content": "hello"},
            "hook_event_name": "PreToolUse",
            "session_id": "sess-abc123",
            "transcript_path": "/tmp/transcript.txt",
            "cwd": "/home/user/project",
            "permission_mode": "ask"
        }"#;
        let event = HookEvent::from_json(json).unwrap();
        assert_eq!(event.tool_name, "Write");
        assert_eq!(event.session_id, Some("sess-abc123".to_string()));
        assert_eq!(event.cwd, Some("/home/user/project".to_string()));
        assert_eq!(event.permission_mode, Some("ask".to_string()));
    }

    /// A PostToolUse payload with tool_use_id and tool_result should parse.
    #[test]
    fn parse_post_tool_use_payload() {
        let json = r#"{
            "tool_name": "Bash",
            "tool_input": {"command": "ls -la"},
            "tool_use_id": "toolu_01ABC123",
            "hook_event_name": "PostToolUse",
            "session_id": "sess-123",
            "tool_result": {"type": "text", "content": "total 42\ndrwxr-xr-x ..."}
        }"#;
        let event = HookEvent::from_json(json).unwrap();
        assert_eq!(event.tool_name, "Bash");
        assert_eq!(event.hook_event_name, "PostToolUse");
        assert_eq!(event.tool_use_id, Some("toolu_01ABC123".to_string()));
        assert!(event.tool_result.is_some());
        let result = event.tool_result.unwrap();
        assert_eq!(result["type"], "text");
        assert_eq!(result["content"], "total 42\ndrwxr-xr-x ...");
    }

    /// PreToolUse payloads without tool_use_id and tool_result should still parse.
    #[test]
    fn parse_pre_tool_use_without_post_fields() {
        let json = r#"{"tool_name": "Bash", "tool_input": {"command": "ls"}}"#;
        let event = HookEvent::from_json(json).unwrap();
        assert!(event.tool_use_id.is_none());
        assert!(event.tool_result.is_none());
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

    /// # HookResponse Serialization Tests
    ///
    /// These tests verify that responses match Claude Code's expected format.
    /// Allow response produces correct hookSpecificOutput format.
    #[test]
    fn allow_response_format() {
        let resp = HookResponse::allow();
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(
            parsed["hookSpecificOutput"]["permissionDecision"],
            "allow"
        );
        assert!(parsed.get("systemMessage").is_none());
    }

    /// Deny response includes hookSpecificOutput and optional systemMessage.
    #[test]
    fn deny_response_with_reason() {
        let resp = HookResponse::deny(Some("dangerous command".to_string()));
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["hookSpecificOutput"]["permissionDecision"], "deny");
        assert_eq!(parsed["systemMessage"], "dangerous command");
    }

    /// Passthrough response is an empty JSON object (no hookSpecificOutput).
    #[test]
    fn passthrough_response_is_empty_object() {
        let resp = HookResponse::passthrough();
        let json = serde_json::to_string(&resp).unwrap();
        assert_eq!(json, "{}");
    }

    /// from_decision correctly maps internal decisions to hook responses.
    #[test]
    fn from_decision_maps_correctly() {
        let allow = HookResponse::from_decision(Decision::Allow, None);
        let json = serde_json::to_string(&allow).unwrap();
        assert!(json.contains("permissionDecision"));

        let passthrough = HookResponse::from_decision(Decision::Passthrough, None);
        let json = serde_json::to_string(&passthrough).unwrap();
        assert_eq!(json, "{}");
    }
}
