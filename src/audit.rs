//! # Audit Logging with Date-Based Chunking and Size Management
//!
//! Built-in audit logging for the sidecar's decision-making process.
//! Captures per-provider vote details, timing, and the final decision.
//!
//! ## Features
//!
//! - **Date-based chunking**: Log files named `audit-YYYY-MM-DD.jsonl`
//! - **Size limits**: Automatic cleanup when total size exceeds threshold
//! - **Graceful truncation**: When current file is too large, retain only recent lines
//!
//! See `docs/design/log-rotation.md` for the full rotation algorithm.

use crate::config::AuditConfig;
use crate::hook::{Decision, HookEvent};
use crate::provider::ProviderResult;
use serde::Serialize;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// A single audit log entry capturing the full decision lifecycle.
#[derive(Debug, Serialize)]
pub struct AuditEntry {
    /// ISO 8601 timestamp
    pub timestamp: String,
    /// Hook event type (e.g., "PreToolUse", "PostToolUse")
    pub hook_event: String,
    /// Tool name
    pub tool_name: String,
    /// Tool input (the full tool_input object)
    pub tool_input: serde_json::Value,
    /// Session ID (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Tool use ID for correlating Pre/Post entries
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,
    /// Brief summary of tool result (PostToolUse only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_result_summary: Option<String>,
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

    let now = current_datetime();
    let entry = AuditEntry {
        timestamp: now.clone(),
        hook_event: event.hook_event_name.clone(),
        tool_name: event.tool_name.clone(),
        tool_input: event.tool_input.clone(),
        session_id: event.session_id.clone(),
        tool_use_id: event.tool_use_id.clone(),
        tool_result_summary: event.tool_result.as_ref().map(summarize_tool_result),
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
        output_dir => {
            write_to_dated_log(output_dir, &now, &json, config);
        }
    }
}

/// Summarize a tool_result value into a brief string (max 200 chars).
///
/// Produces summaries like:
/// - `"success (85 bytes)"` for a result with text content
/// - `"error: <message>"` for error results
/// - First 200 chars of the JSON representation, truncated with "..." if longer
pub fn summarize_tool_result(result: &serde_json::Value) -> String {
    // Check for error indicators
    if let Some(obj) = result.as_object() {
        // Check for explicit error fields
        if let Some(err) = obj.get("error").and_then(|v| v.as_str()) {
            let summary = format!("error: {err}");
            return truncate_to_max(&summary, 200);
        }
        if obj.get("type").and_then(|v| v.as_str()) == Some("error") {
            let msg = obj
                .get("message")
                .or_else(|| obj.get("content"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error");
            let summary = format!("error: {msg}");
            return truncate_to_max(&summary, 200);
        }
        // Success with content — report byte size
        if let Some(content) = obj.get("content").and_then(|v| v.as_str()) {
            return format!("success ({} bytes)", content.len());
        }
    }

    // For string results, report size
    if let Some(s) = result.as_str() {
        return format!("success ({} bytes)", s.len());
    }

    // Fallback: serialize and truncate
    let json = serde_json::to_string(result).unwrap_or_else(|_| "unknown".to_string());
    truncate_to_max(&json, 200)
}

/// Truncate a string to at most `max` characters, appending "..." if truncated.
fn truncate_to_max(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        let mut truncated = s[..max - 3].to_string();
        truncated.push_str("...");
        truncated
    }
}

/// Write a log entry to a date-chunked file, with size management.
fn write_to_dated_log(output_dir: &str, datetime: &str, entry: &str, config: &AuditConfig) {
    let dir = Path::new(output_dir);

    // Ensure directory exists
    if let Err(e) = fs::create_dir_all(dir) {
        eprintln!("claude-pretool-sidecar: failed to create audit dir {output_dir}: {e}");
        return;
    }

    // Date portion for filename (YYYY-MM-DD)
    let date = &datetime[..10]; // "2026-03-22" from ISO 8601
    let filename = format!("audit-{date}.jsonl");
    let filepath = dir.join(&filename);

    // Append entry
    if let Err(e) = append_to_file(&filepath, entry) {
        eprintln!("claude-pretool-sidecar: audit write error ({}): {e}", filepath.display());
        return;
    }

    // Check if current file exceeds per-file limit
    if let Ok(meta) = fs::metadata(&filepath) {
        if meta.len() > config.max_file_bytes {
            truncate_to_recent(&filepath, config.max_file_bytes);
        }
    }

    // Periodically check total directory size (use simple heuristic:
    // check when the current file is at least 10% of max_total_bytes)
    if let Ok(meta) = fs::metadata(&filepath) {
        if meta.len() > config.max_total_bytes / 10 {
            enforce_total_size_limit(dir, config.max_total_bytes, config.max_file_bytes);
        }
    }
}

/// Append a line to a file.
fn append_to_file(path: &Path, entry: &str) -> std::io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{entry}")?;
    Ok(())
}

/// Truncate a file to keep only the most recent lines within the size limit.
///
/// Adds a sentinel line at the top indicating truncation occurred.
fn truncate_to_recent(path: &Path, max_bytes: u64) {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return,
    };

    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return;
    }

    // Find how many recent lines fit within the limit
    // Reserve ~200 bytes for the sentinel line
    let target_bytes = max_bytes.saturating_sub(200) as usize;
    let mut kept_size = 0usize;
    let mut keep_from = lines.len();

    for i in (0..lines.len()).rev() {
        let line_size = lines[i].len() + 1; // +1 for newline
        if kept_size + line_size > target_bytes {
            break;
        }
        kept_size += line_size;
        keep_from = i;
    }

    let lines_removed = keep_from;
    if lines_removed == 0 {
        return; // Nothing to truncate
    }

    let sentinel = format!(
        r#"{{"_truncated":true,"timestamp":"{}","lines_removed":{}}}"#,
        current_datetime(),
        lines_removed
    );

    let mut output = sentinel;
    output.push('\n');
    for line in &lines[keep_from..] {
        output.push_str(line);
        output.push('\n');
    }

    if let Err(e) = fs::write(path, output) {
        eprintln!(
            "claude-pretool-sidecar: failed to truncate audit log {}: {e}",
            path.display()
        );
    }
}

/// Enforce total size limit across all audit log files in a directory.
///
/// Deletes oldest files first. If the last remaining file is still too large,
/// truncates it to fit within the limit.
fn enforce_total_size_limit(dir: &Path, max_total_bytes: u64, max_file_bytes: u64) {
    let mut log_files = match collect_audit_files(dir) {
        Ok(files) => files,
        Err(_) => return,
    };

    // Sort by name (which sorts by date since format is audit-YYYY-MM-DD.jsonl)
    log_files.sort();

    let total_size: u64 = log_files
        .iter()
        .filter_map(|f| fs::metadata(f).ok())
        .map(|m| m.len())
        .sum();

    if total_size <= max_total_bytes {
        return;
    }

    let mut freed: u64 = 0;
    let excess = total_size - max_total_bytes;

    // Delete oldest files until under limit
    for file in &log_files[..log_files.len().saturating_sub(1)] {
        if freed >= excess {
            break;
        }
        if let Ok(meta) = fs::metadata(file) {
            freed += meta.len();
            if let Err(e) = fs::remove_file(file) {
                eprintln!(
                    "claude-pretool-sidecar: failed to delete old audit log {}: {e}",
                    file.display()
                );
            }
        }
    }

    // If still over limit with only the current file, truncate it
    if freed < excess {
        if let Some(last) = log_files.last() {
            if last.exists() {
                truncate_to_recent(last, max_file_bytes);
            }
        }
    }
}

/// Collect all audit-*.jsonl files in a directory.
fn collect_audit_files(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with("audit-") && name_str.ends_with(".jsonl") {
            files.push(entry.path());
        }
    }
    Ok(files)
}

/// Get current datetime as ISO 8601 string.
fn current_datetime() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Convert to YYYY-MM-DD HH:MM:SS format (UTC)
    // Simple implementation without chrono dependency
    let days_since_epoch = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    // Civil date from days since 1970-01-01 (algorithm from Howard Hinnant)
    let z = days_since_epoch as i64 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    format!("{y:04}-{m:02}-{d:02}T{hours:02}:{minutes:02}:{seconds:02}Z")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::Vote;
    use tempfile::TempDir;

    /// # Audit Entry Serialization Tests

    /// An audit entry should serialize to valid JSON with all fields.
    #[test]
    fn audit_entry_serializes_to_json() {
        let entry = AuditEntry {
            timestamp: "2026-03-22T14:30:00Z".to_string(),
            hook_event: "PreToolUse".to_string(),
            tool_name: "Bash".to_string(),
            tool_input: serde_json::json!({"command": "ls"}),
            session_id: Some("sess-123".to_string()),
            tool_use_id: Some("toolu_01ABC123".to_string()),
            tool_result_summary: None,
            providers: vec![ProviderResult {
                name: "checker".to_string(),
                vote: Vote::Allow,
                mode: "vote".to_string(),
                response_time_ms: 45,
                reason: None,
                error: None,
            }],
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
        assert_eq!(parsed["timestamp"], "2026-03-22T14:30:00Z");
    }

    /// Session ID should be omitted when None.
    #[test]
    fn audit_entry_omits_none_session_id() {
        let entry = AuditEntry {
            timestamp: "2026-03-22T00:00:00Z".to_string(),
            hook_event: "PreToolUse".to_string(),
            tool_name: "Bash".to_string(),
            tool_input: serde_json::json!({}),
            session_id: None,
            tool_use_id: None,
            tool_result_summary: None,
            providers: vec![],
            final_decision: "passthrough".to_string(),
            total_time_ms: 0,
        };

        let json = serde_json::to_string(&entry).unwrap();
        assert!(!json.contains("session_id"));
    }

    /// # Date-Based Log File Tests

    /// Writing to a dated log should create a file named audit-YYYY-MM-DD.jsonl.
    #[test]
    fn dated_log_creates_file_with_date() {
        let dir = TempDir::new().unwrap();
        let config = AuditConfig {
            enabled: true,
            output: dir.path().to_string_lossy().to_string(),
            max_total_bytes: 10 * 1024 * 1024,
            max_file_bytes: 5 * 1024 * 1024,
        };

        let datetime = "2026-03-22T14:30:00Z";
        write_to_dated_log(&config.output, datetime, r#"{"test": true}"#, &config);

        let expected = dir.path().join("audit-2026-03-22.jsonl");
        assert!(expected.exists());
        let content = fs::read_to_string(expected).unwrap();
        assert!(content.contains(r#"{"test": true}"#));
    }

    /// # Truncation Tests

    /// Truncating a file should keep only recent lines and add a sentinel.
    #[test]
    fn truncate_keeps_recent_lines() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.jsonl");

        // Write 100 lines, each ~50 bytes
        let mut content = String::new();
        for i in 0..100 {
            content.push_str(&format!(r#"{{"line": {i}, "data": "padding-data-here"}}"#));
            content.push('\n');
        }
        fs::write(&path, &content).unwrap();

        // Truncate to ~500 bytes (should keep ~10 lines)
        truncate_to_recent(&path, 500);

        let result = fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = result.lines().collect();

        // Should have sentinel line + some recent lines
        assert!(lines[0].contains("_truncated"));
        assert!(lines.len() < 100);
        // Last line should be from the end of the original
        assert!(lines.last().unwrap().contains("\"line\": 99"));
    }

    /// # Total Size Enforcement Tests

    /// When total size exceeds limit, oldest files should be deleted.
    #[test]
    fn enforce_total_size_deletes_oldest() {
        let dir = TempDir::new().unwrap();

        // Create 3 files: old, middle, new (1KB each)
        let data = "x".repeat(1024);
        fs::write(dir.path().join("audit-2026-03-20.jsonl"), &data).unwrap();
        fs::write(dir.path().join("audit-2026-03-21.jsonl"), &data).unwrap();
        fs::write(dir.path().join("audit-2026-03-22.jsonl"), &data).unwrap();

        // Enforce 2KB limit (should delete oldest)
        enforce_total_size_limit(dir.path(), 2048, 1024);

        assert!(!dir.path().join("audit-2026-03-20.jsonl").exists());
        assert!(dir.path().join("audit-2026-03-22.jsonl").exists());
    }

    /// # Current Datetime Tests

    /// current_datetime should produce valid ISO 8601 format.
    #[test]
    fn datetime_format_is_valid() {
        let dt = current_datetime();
        // Should match YYYY-MM-DDTHH:MM:SSZ pattern
        assert_eq!(dt.len(), 20);
        assert_eq!(&dt[4..5], "-");
        assert_eq!(&dt[7..8], "-");
        assert_eq!(&dt[10..11], "T");
        assert_eq!(&dt[19..20], "Z");
    }

    /// # Tool Use ID Correlation Tests

    /// tool_use_id should appear in audit entry JSON when present.
    #[test]
    fn audit_entry_includes_tool_use_id_when_present() {
        let entry = AuditEntry {
            timestamp: "2026-03-27T10:00:00Z".to_string(),
            hook_event: "PreToolUse".to_string(),
            tool_name: "Bash".to_string(),
            tool_input: serde_json::json!({"command": "ls"}),
            session_id: Some("sess-123".to_string()),
            tool_use_id: Some("toolu_01ABC123".to_string()),
            tool_result_summary: None,
            providers: vec![],
            final_decision: "allow".to_string(),
            total_time_ms: 5,
        };

        let json = serde_json::to_string(&entry).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["tool_use_id"], "toolu_01ABC123");
        // tool_result_summary should be omitted when None
        assert!(parsed.get("tool_result_summary").is_none());
    }

    /// tool_use_id should be omitted from audit entry JSON when None.
    #[test]
    fn audit_entry_omits_tool_use_id_when_none() {
        let entry = AuditEntry {
            timestamp: "2026-03-27T10:00:00Z".to_string(),
            hook_event: "PreToolUse".to_string(),
            tool_name: "Bash".to_string(),
            tool_input: serde_json::json!({}),
            session_id: None,
            tool_use_id: None,
            tool_result_summary: None,
            providers: vec![],
            final_decision: "passthrough".to_string(),
            total_time_ms: 0,
        };

        let json = serde_json::to_string(&entry).unwrap();
        assert!(!json.contains("tool_use_id"));
        assert!(!json.contains("tool_result_summary"));
    }

    /// # Tool Result Summary Tests

    /// Summarize a successful text result with byte count.
    #[test]
    fn summarize_tool_result_success_with_content() {
        let result = serde_json::json!({
            "type": "text",
            "content": "total 42\ndrwxr-xr-x ..."
        });
        let summary = summarize_tool_result(&result);
        assert_eq!(summary, "success (23 bytes)");
    }

    /// Summarize an error result.
    #[test]
    fn summarize_tool_result_error_field() {
        let result = serde_json::json!({"error": "permission denied"});
        let summary = summarize_tool_result(&result);
        assert_eq!(summary, "error: permission denied");
    }

    /// Summarize an error result with type=error.
    #[test]
    fn summarize_tool_result_error_type() {
        let result = serde_json::json!({"type": "error", "message": "file not found"});
        let summary = summarize_tool_result(&result);
        assert_eq!(summary, "error: file not found");
    }

    /// Summarize a plain string result.
    #[test]
    fn summarize_tool_result_string() {
        let result = serde_json::json!("hello world");
        let summary = summarize_tool_result(&result);
        assert_eq!(summary, "success (11 bytes)");
    }

    /// Summary should be truncated to 200 chars max.
    #[test]
    fn summarize_tool_result_truncation() {
        let long_error = "x".repeat(300);
        let result = serde_json::json!({"error": long_error});
        let summary = summarize_tool_result(&result);
        assert!(summary.len() <= 200);
        assert!(summary.ends_with("..."));
        assert!(summary.starts_with("error: "));
    }

    /// Summarize a non-object, non-string result (fallback to JSON).
    #[test]
    fn summarize_tool_result_fallback() {
        let result = serde_json::json!([1, 2, 3]);
        let summary = summarize_tool_result(&result);
        assert_eq!(summary, "[1,2,3]");
    }
}
