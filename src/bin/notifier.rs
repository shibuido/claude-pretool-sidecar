//! # claude-pretool-notifier
//!
//! A companion tool that sends desktop notifications when Claude Code requests
//! tool approval. Designed for users who multitask — get a notification when
//! Claude is waiting for permission.
//!
//! This is an FYI provider: it always outputs `{"decision": "passthrough"}`
//! and never blocks tool execution.
//!
//! ## Usage
//!
//! ```toml
//! [[providers]]
//! name = "desktop-notify"
//! command = "claude-pretool-notifier"
//! mode = "fyi"
//! ```
//!
//! ## Environment Variables
//!
//! * `CPTS_NOTIFY_URGENCY` — low/normal/critical (default: normal)
//! * `CPTS_NOTIFY_TIMEOUT` — display time in ms (default: 5000)
//! * `CPTS_NOTIFY_DISABLE` — set to "1" to disable notifications (still passthrough)

use std::io::Read;
use std::process::Command;

const TITLE: &str = "Claude Code \u{2014} Tool Request";
const DEFAULT_URGENCY: &str = "normal";
const DEFAULT_TIMEOUT_MS: &str = "5000";

fn main() {
    // Read hook payload from stdin
    let mut input = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut input) {
        eprintln!("claude-pretool-notifier: failed to read stdin: {e}");
        print_passthrough();
        return;
    }

    // Extract notification body from the hook payload
    let body = extract_notification_body(&input);

    // Send notification unless disabled
    let disabled = std::env::var("CPTS_NOTIFY_DISABLE")
        .map(|v| v == "1")
        .unwrap_or(false);

    if !disabled {
        if let Err(e) = send_notification(TITLE, &body) {
            eprintln!("claude-pretool-notifier: notification failed: {e}");
        }
    }

    // Always output passthrough — FYI provider never blocks
    print_passthrough();
}

/// Extract a human-readable notification body from the hook JSON payload.
///
/// Format varies by tool:
/// * Bash: shows the command
/// * Write/Edit: shows the file path
/// * Read: shows the file path
/// * Other tools: shows tool name only
fn extract_notification_body(input: &str) -> String {
    let trimmed = input.trim();

    let parsed: serde_json::Value = match serde_json::from_str(trimmed) {
        Ok(v) => v,
        Err(_) => return "Unknown tool request".to_string(),
    };

    let tool_name = parsed
        .get("tool_name")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown");

    let tool_input = parsed.get("tool_input");

    let detail = match tool_name {
        "Bash" => tool_input
            .and_then(|ti| ti.get("command"))
            .and_then(|v| v.as_str())
            .map(|cmd| truncate(cmd, 120))
            .unwrap_or_default(),
        "Write" | "Edit" => tool_input
            .and_then(|ti| ti.get("file_path"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        "Read" => tool_input
            .and_then(|ti| ti.get("file_path"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        _ => String::new(),
    };

    if detail.is_empty() {
        tool_name.to_string()
    } else {
        format!("{tool_name}: {detail}")
    }
}

/// Truncate a string to max_len, appending "..." if truncated.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let end = s
            .char_indices()
            .take_while(|(i, _)| *i < max_len.saturating_sub(3))
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(0);
        format!("{}...", &s[..end])
    }
}

/// Send a desktop notification, choosing the platform-appropriate method.
fn send_notification(title: &str, body: &str) -> Result<(), String> {
    let urgency = std::env::var("CPTS_NOTIFY_URGENCY").unwrap_or_else(|_| DEFAULT_URGENCY.into());
    let timeout = std::env::var("CPTS_NOTIFY_TIMEOUT")
        .unwrap_or_else(|_| DEFAULT_TIMEOUT_MS.into());

    if cfg!(target_os = "linux") {
        send_linux(title, body, &urgency, &timeout)
    } else if cfg!(target_os = "macos") {
        send_macos(title, body)
    } else {
        // Fallback: print to stderr
        eprintln!("claude-pretool-notifier: [{title}] {body}");
        Ok(())
    }
}

/// Linux: use notify-send (libnotify).
fn send_linux(title: &str, body: &str, urgency: &str, timeout_ms: &str) -> Result<(), String> {
    let status = Command::new("notify-send")
        .arg("--urgency")
        .arg(urgency)
        .arg("--expire-time")
        .arg(timeout_ms)
        .arg("--app-name")
        .arg("claude-pretool-notifier")
        .arg(title)
        .arg(body)
        .status()
        .map_err(|e| format!("failed to run notify-send: {e}"))?;

    if status.success() {
        Ok(())
    } else {
        // Fallback to stderr if notify-send fails
        eprintln!("claude-pretool-notifier: [{title}] {body}");
        Ok(())
    }
}

/// macOS: use osascript to display a notification.
fn send_macos(title: &str, body: &str) -> Result<(), String> {
    let script = format!(
        "display notification \"{}\" with title \"{}\"",
        body.replace('\\', "\\\\").replace('"', "\\\""),
        title.replace('\\', "\\\\").replace('"', "\\\""),
    );

    let status = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .status()
        .map_err(|e| format!("failed to run osascript: {e}"))?;

    if status.success() {
        Ok(())
    } else {
        eprintln!("claude-pretool-notifier: [{title}] {body}");
        Ok(())
    }
}

/// Print passthrough JSON to stdout. Always succeeds.
fn print_passthrough() {
    println!(r#"{{"decision": "passthrough"}}"#);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bash_command_extracted() {
        let json = r#"{"tool_name": "Bash", "tool_input": {"command": "ls -la /tmp"}}"#;
        let body = extract_notification_body(json);
        assert_eq!(body, "Bash: ls -la /tmp");
    }

    #[test]
    fn write_file_path_extracted() {
        let json =
            r#"{"tool_name": "Write", "tool_input": {"file_path": "/tmp/test.txt", "content": "hi"}}"#;
        let body = extract_notification_body(json);
        assert_eq!(body, "Write: /tmp/test.txt");
    }

    #[test]
    fn edit_file_path_extracted() {
        let json =
            r#"{"tool_name": "Edit", "tool_input": {"file_path": "/src/main.rs", "old_string": "a", "new_string": "b"}}"#;
        let body = extract_notification_body(json);
        assert_eq!(body, "Edit: /src/main.rs");
    }

    #[test]
    fn read_file_path_extracted() {
        let json = r#"{"tool_name": "Read", "tool_input": {"file_path": "/etc/passwd"}}"#;
        let body = extract_notification_body(json);
        assert_eq!(body, "Read: /etc/passwd");
    }

    #[test]
    fn unknown_tool_shows_name_only() {
        let json = r#"{"tool_name": "WebSearch", "tool_input": {"query": "rust lang"}}"#;
        let body = extract_notification_body(json);
        assert_eq!(body, "WebSearch");
    }

    #[test]
    fn invalid_json_handled() {
        let body = extract_notification_body("not json at all");
        assert_eq!(body, "Unknown tool request");
    }

    #[test]
    fn long_command_truncated() {
        let long_cmd = "x".repeat(200);
        let truncated = truncate(&long_cmd, 120);
        assert!(truncated.len() <= 123); // 120 - 3 for "..." + possible char boundary
        assert!(truncated.ends_with("..."));
    }

    #[test]
    fn short_command_not_truncated() {
        let short = "ls -la";
        assert_eq!(truncate(short, 120), "ls -la");
    }
}
