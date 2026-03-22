//! # claude-pretool-logger
//!
//! A simple companion tool that logs Claude Code hook events to a file or stdout.
//!
//! Designed to be used as an FYI provider in the sidecar's configuration.
//! It reads the hook payload from stdin and writes it to the configured output.
//!
//! ## Usage
//!
//! ```toml
//! [[providers]]
//! name = "logger"
//! command = "claude-pretool-logger"
//! args = ["--output", "/var/log/claude-tools.jsonl"]
//! mode = "fyi"
//! ```
//!
//! When no `--output` is specified, logs to stderr (so it doesn't interfere
//! with the sidecar's stdout protocol).

use std::io::{self, Read, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Parse --output flag
    let output_path = parse_output_arg(&args);

    // Read hook payload from stdin
    let mut input = String::new();
    if let Err(e) = io::stdin().read_to_string(&mut input) {
        eprintln!("claude-pretool-logger: failed to read stdin: {e}");
        std::process::exit(1);
    }

    // Format the log entry (JSON line with timestamp)
    let log_entry = format_log_entry(&input);

    // Write to output
    if let Some(path) = output_path {
        if let Err(e) = append_to_file(&path, &log_entry) {
            eprintln!("claude-pretool-logger: failed to write to {path}: {e}");
            std::process::exit(1);
        }
    } else {
        // Default: write to stderr
        eprintln!("{log_entry}");
    }

    // Return passthrough decision (in case someone accidentally configures as "vote")
    println!(r#"{{"decision": "passthrough"}}"#);
}

fn parse_output_arg(args: &[String]) -> Option<String> {
    for i in 0..args.len() {
        if args[i] == "--output" || args[i] == "-o" {
            return args.get(i + 1).cloned();
        }
    }
    None
}

fn format_log_entry(input: &str) -> String {
    // Try to parse as JSON to create a structured log entry
    let trimmed = input.trim();
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
        // Wrap in a log envelope with timestamp
        let entry = serde_json::json!({
            "timestamp": chrono_like_timestamp(),
            "event": value,
        });
        serde_json::to_string(&entry).unwrap_or_else(|_| trimmed.to_string())
    } else {
        // If not valid JSON, log as raw text
        format!(
            "{{\"timestamp\":\"{}\",\"raw\":{}}}",
            chrono_like_timestamp(),
            serde_json::to_string(trimmed).unwrap_or_else(|_| "\"<invalid>\"".to_string())
        )
    }
}

/// Simple timestamp without external dependency.
/// Format: seconds since Unix epoch (good enough for v0.1).
fn chrono_like_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    secs.to_string()
}

fn append_to_file(path: &str, entry: &str) -> io::Result<()> {
    use std::fs::OpenOptions;
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{entry}")?;
    Ok(())
}
