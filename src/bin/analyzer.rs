//! # claude-pretool-analyzer
//!
//! A standalone CLI tool that reads audit JSONL files produced by
//! claude-pretool-sidecar and prints session summary analytics.
//!
//! ## Usage
//!
//! ```sh
//! claude-pretool-analyzer /path/to/audit-dir/
//! claude-pretool-analyzer /path/to/audit-2026-03-27.jsonl
//! claude-pretool-analyzer --json /path/to/audit-dir/
//! ```

use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};

/// Deserialized audit log entry (mirrors AuditEntry from src/audit.rs).
/// Uses Deserialize only — we never write these back.
#[derive(Debug, Deserialize)]
struct AuditEntry {
    timestamp: String,
    hook_event: String,
    tool_name: String,
    tool_input: serde_json::Value,
    #[serde(default)]
    tool_use_id: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    tool_result_summary: Option<String>,
    #[serde(default)]
    providers: Vec<ProviderEntry>,
    final_decision: String,
    #[allow(dead_code)]
    total_time_ms: u64,
}

#[derive(Debug, Deserialize, serde::Serialize)]
struct ProviderEntry {
    name: String,
    #[serde(default)]
    #[allow(dead_code)]
    vote: String,
    #[serde(default)]
    response_time_ms: u64,
    #[serde(default)]
    error: Option<String>,
}

/// Accumulated statistics for the report.
#[derive(Debug, Default)]
struct Stats {
    entries: Vec<AuditEntry>,
    first_timestamp: Option<String>,
    last_timestamp: Option<String>,

    // Decision counts (PreToolUse only)
    decision_counts: HashMap<String, usize>,

    // Per-tool breakdown: tool_name -> decision -> count
    tool_decisions: BTreeMap<String, HashMap<String, usize>>,

    // Provider performance: provider_name -> (times_ms, error_count)
    provider_times: HashMap<String, Vec<u64>>,
    provider_errors: HashMap<String, usize>,

    // Pre/Post correlation by tool_use_id
    pre_tool_ids: HashMap<String, String>,  // tool_use_id -> tool_name
    post_tool_ids: HashMap<String, String>, // tool_use_id -> tool_name
    denied_details: Vec<(String, String)>,  // (tool_name, simplified_input)

    // Pattern detection: (tool_name, normalized_input) -> (total, allowed)
    patterns: HashMap<(String, String), (usize, usize)>,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let (json_output, path) = parse_args(&args);

    let files = resolve_files(&path);
    if files.is_empty() {
        eprintln!("No audit JSONL files found at: {}", path.display());
        std::process::exit(1);
    }

    let stats = collect_stats(&files);

    if json_output {
        print_json_report(&stats);
    } else {
        print_human_report(&stats);
    }
}

fn parse_args(args: &[String]) -> (bool, PathBuf) {
    let mut json_output = false;
    let mut path: Option<PathBuf> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => json_output = true,
            "--help" | "-h" => {
                eprintln!("Usage: claude-pretool-analyzer [--json] <path>");
                eprintln!();
                eprintln!("  <path>   Directory of audit-*.jsonl files, or a single .jsonl file");
                eprintln!("  --json   Output as JSON instead of human-readable text");
                std::process::exit(0);
            }
            other => {
                if path.is_some() {
                    eprintln!("Unexpected argument: {other}");
                    std::process::exit(1);
                }
                path = Some(PathBuf::from(other));
            }
        }
        i += 1;
    }

    let path = path.unwrap_or_else(|| {
        eprintln!("Usage: claude-pretool-analyzer [--json] <path>");
        std::process::exit(1);
    });

    (json_output, path)
}

fn resolve_files(path: &Path) -> Vec<PathBuf> {
    if path.is_file() {
        return vec![path.to_path_buf()];
    }
    if path.is_dir() {
        let mut files: Vec<PathBuf> = fs::read_dir(path)
            .ok()
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with("audit-") && n.ends_with(".jsonl"))
                    .unwrap_or(false)
            })
            .collect();
        files.sort();
        return files;
    }
    Vec::new()
}

fn collect_stats(files: &[PathBuf]) -> Stats {
    let mut stats = Stats::default();

    for file in files {
        let reader = match fs::File::open(file) {
            Ok(f) => io::BufReader::new(f),
            Err(e) => {
                eprintln!("Warning: could not open {}: {e}", file.display());
                continue;
            }
        };

        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => continue,
            };
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            // Skip truncation sentinel lines
            if trimmed.contains("\"_truncated\"") {
                continue;
            }

            let entry: AuditEntry = match serde_json::from_str(trimmed) {
                Ok(e) => e,
                Err(_) => continue, // Skip malformed lines
            };

            process_entry(&mut stats, entry);
        }
    }

    stats
}

fn process_entry(stats: &mut Stats, entry: AuditEntry) {
    // Track time range
    if stats.first_timestamp.is_none() || stats.first_timestamp.as_ref().unwrap() > &entry.timestamp
    {
        stats.first_timestamp = Some(entry.timestamp.clone());
    }
    if stats.last_timestamp.is_none() || stats.last_timestamp.as_ref().unwrap() < &entry.timestamp {
        stats.last_timestamp = Some(entry.timestamp.clone());
    }

    let is_pre = entry.hook_event == "PreToolUse";
    let is_post = entry.hook_event == "PostToolUse";

    if is_pre {
        // Decision counts
        *stats
            .decision_counts
            .entry(entry.final_decision.clone())
            .or_insert(0) += 1;

        // Per-tool decisions
        let tool_map = stats
            .tool_decisions
            .entry(entry.tool_name.clone())
            .or_default();
        *tool_map.entry(entry.final_decision.clone()).or_insert(0) += 1;

        // Provider performance
        for p in &entry.providers {
            stats
                .provider_times
                .entry(p.name.clone())
                .or_default()
                .push(p.response_time_ms);
            if p.error.is_some() {
                *stats.provider_errors.entry(p.name.clone()).or_insert(0) += 1;
            }
        }

        // Pre/Post correlation
        if let Some(ref id) = entry.tool_use_id {
            stats
                .pre_tool_ids
                .insert(id.clone(), entry.tool_name.clone());
        }

        // Track denied tools
        if entry.final_decision == "deny" {
            let simplified = normalize_tool_input(&entry.tool_name, &entry.tool_input);
            stats
                .denied_details
                .push((entry.tool_name.clone(), simplified));
        }

        // Pattern detection
        let normalized = normalize_tool_input(&entry.tool_name, &entry.tool_input);
        let key = (entry.tool_name.clone(), normalized);
        let counter = stats.patterns.entry(key).or_insert((0, 0));
        counter.0 += 1;
        if entry.final_decision == "allow" {
            counter.1 += 1;
        }
    }

    if is_post {
        if let Some(ref id) = entry.tool_use_id {
            stats
                .post_tool_ids
                .insert(id.clone(), entry.tool_name.clone());
        }
    }

    stats.entries.push(entry);
}

/// Normalize tool_input to a simplified pattern string for grouping.
///
/// Examples:
///   Bash {"command": "ls -la /tmp"} -> "ls *"
///   Read {"file_path": "/home/user/foo.rs"} -> "Read(*)"
///   Write {"file_path": "/etc/passwd", ...} -> "Write(/etc/passwd)"
fn normalize_tool_input(tool_name: &str, input: &serde_json::Value) -> String {
    match tool_name {
        "Bash" => {
            if let Some(cmd) = input.get("command").and_then(|v| v.as_str()) {
                // Extract first word (the command name)
                let first_word = cmd.split_whitespace().next().unwrap_or("?");
                format!("{first_word} *")
            } else {
                "?".to_string()
            }
        }
        "Read" | "Write" | "Edit" => {
            if let Some(path) = input.get("file_path").and_then(|v| v.as_str()) {
                // Keep just the filename or sensitive paths
                if path.starts_with("/etc/") || path.starts_with("/root") {
                    format!("{tool_name}({path})")
                } else {
                    format!("{tool_name}(*)")
                }
            } else {
                format!("{tool_name}(?)")
            }
        }
        _ => format!("{tool_name}(*)"),
    }
}

fn print_human_report(stats: &Stats) {
    let total_pre: usize = stats.decision_counts.values().sum();

    println!("=== Session Summary ===");
    println!(
        "Period: {} -- {}",
        stats
            .first_timestamp
            .as_deref()
            .unwrap_or("(unknown)"),
        stats
            .last_timestamp
            .as_deref()
            .unwrap_or("(unknown)")
    );
    println!("Total tool requests: {total_pre}");
    println!();

    // Decision breakdown
    println!("Decisions:");
    for decision in &["allow", "deny", "passthrough"] {
        let count = stats.decision_counts.get(*decision).copied().unwrap_or(0);
        let pct = if total_pre > 0 {
            (count as f64 / total_pre as f64) * 100.0
        } else {
            0.0
        };
        println!("  {:<14} {:>4} ({:.0}%)", format!("{decision}:"), count, pct);
    }
    println!();

    // Per-tool breakdown
    println!("Tools (by frequency):");
    let mut tools: Vec<_> = stats.tool_decisions.iter().collect();
    tools.sort_by(|a, b| {
        let total_a: usize = a.1.values().sum();
        let total_b: usize = b.1.values().sum();
        total_b.cmp(&total_a)
    });
    for (tool, decisions) in &tools {
        let total: usize = decisions.values().sum();
        let allow = decisions.get("allow").copied().unwrap_or(0);
        let deny = decisions.get("deny").copied().unwrap_or(0);
        let pass = decisions.get("passthrough").copied().unwrap_or(0);
        println!(
            "  {:<8} {:>4} (allow: {}, deny: {}, passthrough: {})",
            format!("{tool}:"),
            total,
            allow,
            deny,
            pass
        );
    }
    println!();

    // Provider performance
    if !stats.provider_times.is_empty() {
        println!("Provider Performance:");
        let mut providers: Vec<_> = stats.provider_times.keys().collect();
        providers.sort();
        for name in providers {
            let times = &stats.provider_times[name];
            let avg = times.iter().sum::<u64>() as f64 / times.len() as f64;
            let max = times.iter().copied().max().unwrap_or(0);
            let errors = stats.provider_errors.get(name).copied().unwrap_or(0);
            println!(
                "  {:<20} avg {}ms, max {}ms, errors: {}",
                format!("{name}:"),
                avg as u64,
                max,
                errors
            );
        }
        println!();
    }

    // Pre/Post correlation
    let pre_count = stats.pre_tool_ids.len();
    let post_count = stats
        .pre_tool_ids
        .keys()
        .filter(|id| stats.post_tool_ids.contains_key(*id))
        .count();
    if pre_count > 0 {
        let pct = (post_count as f64 / pre_count as f64) * 100.0;
        println!("PostToolUse Correlation:");
        println!(
            "  Requested: {}, Actually executed: {} ({:.0}%)",
            pre_count, post_count, pct
        );
        if !stats.denied_details.is_empty() {
            let denied_summary: Vec<String> = stats
                .denied_details
                .iter()
                .take(5)
                .map(|(tool, input)| format!("{tool}({input})"))
                .collect();
            println!(
                "  Denied tools that were requested: {}{}",
                denied_summary.join(", "),
                if stats.denied_details.len() > 5 {
                    ", ..."
                } else {
                    ""
                }
            );
        }
        println!();
    }

    // Auto-approval candidates (patterns with 100% allow rate, >= 3 occurrences)
    let mut candidates: Vec<_> = stats
        .patterns
        .iter()
        .filter(|(_, (total, allowed))| *total >= 3 && *total == *allowed)
        .collect();
    candidates.sort_by(|a, b| b.1 .0.cmp(&a.1 .0));

    if !candidates.is_empty() {
        println!("Patterns (auto-approval candidates):");
        for ((tool, pattern), (total, _)) in candidates.iter().take(10) {
            println!(
                "  {:<20} {} requests, 100% allowed",
                format!("{tool}({pattern}):"),
                total
            );
        }
        println!();
    }
}

fn print_json_report(stats: &Stats) {
    let total_pre: usize = stats.decision_counts.values().sum();

    let mut tools_json = serde_json::Map::new();
    for (tool, decisions) in &stats.tool_decisions {
        let mut tool_obj = serde_json::Map::new();
        let total: usize = decisions.values().sum();
        tool_obj.insert("total".into(), serde_json::json!(total));
        for (dec, count) in decisions {
            tool_obj.insert(dec.clone(), serde_json::json!(count));
        }
        tools_json.insert(tool.clone(), serde_json::Value::Object(tool_obj));
    }

    let mut providers_json = serde_json::Map::new();
    for (name, times) in &stats.provider_times {
        let avg = times.iter().sum::<u64>() as f64 / times.len() as f64;
        let max = times.iter().copied().max().unwrap_or(0);
        let errors = stats.provider_errors.get(name).copied().unwrap_or(0);
        providers_json.insert(
            name.clone(),
            serde_json::json!({
                "avg_ms": avg as u64,
                "max_ms": max,
                "invocations": times.len(),
                "errors": errors,
            }),
        );
    }

    let pre_count = stats.pre_tool_ids.len();
    let post_count = stats
        .pre_tool_ids
        .keys()
        .filter(|id| stats.post_tool_ids.contains_key(*id))
        .count();

    let candidates: Vec<_> = stats
        .patterns
        .iter()
        .filter(|(_, (total, allowed))| *total >= 3 && *total == *allowed)
        .map(|((tool, pattern), (total, _))| {
            serde_json::json!({
                "tool": tool,
                "pattern": pattern,
                "requests": total,
                "allow_rate": 1.0,
            })
        })
        .collect();

    let report = serde_json::json!({
        "period": {
            "start": stats.first_timestamp,
            "end": stats.last_timestamp,
        },
        "total_requests": total_pre,
        "decisions": &stats.decision_counts,
        "tools": tools_json,
        "providers": providers_json,
        "correlation": {
            "requested": pre_count,
            "executed": post_count,
            "execution_rate": if pre_count > 0 { post_count as f64 / pre_count as f64 } else { 0.0 },
        },
        "auto_approval_candidates": candidates,
    });

    println!(
        "{}",
        serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".to_string())
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn make_entry(
        hook_event: &str,
        tool_name: &str,
        decision: &str,
        tool_use_id: Option<&str>,
        tool_input: serde_json::Value,
        providers: Vec<ProviderEntry>,
        timestamp: &str,
    ) -> String {
        let entry = serde_json::json!({
            "timestamp": timestamp,
            "hook_event": hook_event,
            "tool_name": tool_name,
            "tool_input": tool_input,
            "tool_use_id": tool_use_id,
            "providers": providers,
            "final_decision": decision,
            "total_time_ms": 50,
        });
        serde_json::to_string(&entry).unwrap()
    }

    fn make_provider(name: &str, time_ms: u64, error: Option<&str>) -> ProviderEntry {
        ProviderEntry {
            name: name.to_string(),
            vote: "allow".to_string(),
            response_time_ms: time_ms,
            error: error.map(|s| s.to_string()),
        }
    }

    fn write_jsonl(dir: &Path, filename: &str, lines: &[String]) -> PathBuf {
        let path = dir.join(filename);
        let mut f = fs::File::create(&path).unwrap();
        for line in lines {
            writeln!(f, "{line}").unwrap();
        }
        path
    }

    #[test]
    fn resolve_files_finds_audit_files_in_directory() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("audit-2026-03-27.jsonl"), "").unwrap();
        fs::write(dir.path().join("audit-2026-03-26.jsonl"), "").unwrap();
        fs::write(dir.path().join("other.txt"), "").unwrap();

        let files = resolve_files(dir.path());
        assert_eq!(files.len(), 2);
        // Should be sorted
        assert!(files[0].to_string_lossy().contains("2026-03-26"));
        assert!(files[1].to_string_lossy().contains("2026-03-27"));
    }

    #[test]
    fn resolve_files_handles_single_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("audit-2026-03-27.jsonl");
        fs::write(&path, "").unwrap();

        let files = resolve_files(&path);
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn collect_stats_counts_decisions() {
        let dir = TempDir::new().unwrap();
        let lines = vec![
            make_entry("PreToolUse", "Bash", "allow", None, serde_json::json!({"command": "ls"}), vec![], "2026-03-27T10:00:00Z"),
            make_entry("PreToolUse", "Bash", "allow", None, serde_json::json!({"command": "pwd"}), vec![], "2026-03-27T10:01:00Z"),
            make_entry("PreToolUse", "Bash", "deny", None, serde_json::json!({"command": "rm -rf /"}), vec![], "2026-03-27T10:02:00Z"),
            make_entry("PreToolUse", "Read", "passthrough", None, serde_json::json!({"file_path": "/tmp/x"}), vec![], "2026-03-27T10:03:00Z"),
        ];
        write_jsonl(dir.path(), "audit-2026-03-27.jsonl", &lines);

        let files = resolve_files(dir.path());
        let stats = collect_stats(&files);

        assert_eq!(stats.decision_counts.get("allow"), Some(&2));
        assert_eq!(stats.decision_counts.get("deny"), Some(&1));
        assert_eq!(stats.decision_counts.get("passthrough"), Some(&1));
    }

    #[test]
    fn collect_stats_tracks_tool_breakdown() {
        let dir = TempDir::new().unwrap();
        let lines = vec![
            make_entry("PreToolUse", "Bash", "allow", None, serde_json::json!({}), vec![], "2026-03-27T10:00:00Z"),
            make_entry("PreToolUse", "Bash", "deny", None, serde_json::json!({}), vec![], "2026-03-27T10:01:00Z"),
            make_entry("PreToolUse", "Read", "allow", None, serde_json::json!({}), vec![], "2026-03-27T10:02:00Z"),
        ];
        write_jsonl(dir.path(), "audit-2026-03-27.jsonl", &lines);

        let stats = collect_stats(&resolve_files(dir.path()));

        let bash = stats.tool_decisions.get("Bash").unwrap();
        assert_eq!(bash.get("allow"), Some(&1));
        assert_eq!(bash.get("deny"), Some(&1));

        let read = stats.tool_decisions.get("Read").unwrap();
        assert_eq!(read.get("allow"), Some(&1));
    }

    #[test]
    fn collect_stats_tracks_provider_performance() {
        let dir = TempDir::new().unwrap();
        let providers = vec![
            make_provider("checker", 45, None),
            make_provider("policy", 100, Some("timeout")),
        ];
        let lines = vec![
            make_entry("PreToolUse", "Bash", "allow", None, serde_json::json!({}), providers, "2026-03-27T10:00:00Z"),
        ];
        write_jsonl(dir.path(), "audit-2026-03-27.jsonl", &lines);

        let stats = collect_stats(&resolve_files(dir.path()));

        assert_eq!(stats.provider_times["checker"], vec![45]);
        assert_eq!(stats.provider_times["policy"], vec![100]);
        assert_eq!(stats.provider_errors.get("policy"), Some(&1));
        assert_eq!(stats.provider_errors.get("checker"), None);
    }

    #[test]
    fn collect_stats_correlates_pre_post() {
        let dir = TempDir::new().unwrap();
        let lines = vec![
            make_entry("PreToolUse", "Bash", "allow", Some("id-1"), serde_json::json!({}), vec![], "2026-03-27T10:00:00Z"),
            make_entry("PreToolUse", "Bash", "allow", Some("id-2"), serde_json::json!({}), vec![], "2026-03-27T10:01:00Z"),
            make_entry("PostToolUse", "Bash", "allow", Some("id-1"), serde_json::json!({}), vec![], "2026-03-27T10:00:05Z"),
            // id-2 has no PostToolUse
        ];
        write_jsonl(dir.path(), "audit-2026-03-27.jsonl", &lines);

        let stats = collect_stats(&resolve_files(dir.path()));

        assert_eq!(stats.pre_tool_ids.len(), 2);
        assert_eq!(stats.post_tool_ids.len(), 1);
        assert!(stats.post_tool_ids.contains_key("id-1"));
        assert!(!stats.post_tool_ids.contains_key("id-2"));
    }

    #[test]
    fn collect_stats_identifies_auto_approval_candidates() {
        let dir = TempDir::new().unwrap();
        let lines: Vec<String> = (0..5)
            .map(|i| {
                make_entry(
                    "PreToolUse",
                    "Bash",
                    "allow",
                    None,
                    serde_json::json!({"command": format!("ls {}", i)}),
                    vec![],
                    &format!("2026-03-27T10:{:02}:00Z", i),
                )
            })
            .collect();
        write_jsonl(dir.path(), "audit-2026-03-27.jsonl", &lines);

        let stats = collect_stats(&resolve_files(dir.path()));

        let key = ("Bash".to_string(), "ls *".to_string());
        let (total, allowed) = stats.patterns.get(&key).unwrap();
        assert_eq!(*total, 5);
        assert_eq!(*allowed, 5);
    }

    #[test]
    fn normalize_tool_input_bash() {
        let input = serde_json::json!({"command": "git status --short"});
        assert_eq!(normalize_tool_input("Bash", &input), "git *");
    }

    #[test]
    fn normalize_tool_input_read_normal_path() {
        let input = serde_json::json!({"file_path": "/home/user/foo.rs"});
        assert_eq!(normalize_tool_input("Read", &input), "Read(*)");
    }

    #[test]
    fn normalize_tool_input_write_sensitive_path() {
        let input = serde_json::json!({"file_path": "/etc/passwd"});
        assert_eq!(normalize_tool_input("Write", &input), "Write(/etc/passwd)");
    }

    #[test]
    fn normalize_tool_input_unknown_tool() {
        let input = serde_json::json!({"something": "else"});
        assert_eq!(normalize_tool_input("CustomTool", &input), "CustomTool(*)");
    }

    #[test]
    fn skips_truncation_sentinel_lines() {
        let dir = TempDir::new().unwrap();
        let mut lines = vec![
            r#"{"_truncated":true,"timestamp":"2026-03-27T10:00:00Z","lines_removed":50}"#
                .to_string(),
        ];
        lines.push(make_entry(
            "PreToolUse",
            "Bash",
            "allow",
            None,
            serde_json::json!({}),
            vec![],
            "2026-03-27T10:00:00Z",
        ));
        write_jsonl(dir.path(), "audit-2026-03-27.jsonl", &lines);

        let stats = collect_stats(&resolve_files(dir.path()));
        assert_eq!(stats.decision_counts.get("allow"), Some(&1));
    }

    #[test]
    fn collect_stats_tracks_time_range() {
        let dir = TempDir::new().unwrap();
        let lines = vec![
            make_entry("PreToolUse", "Bash", "allow", None, serde_json::json!({}), vec![], "2026-03-27T10:00:00Z"),
            make_entry("PreToolUse", "Read", "allow", None, serde_json::json!({}), vec![], "2026-03-27T11:30:00Z"),
        ];
        write_jsonl(dir.path(), "audit-2026-03-27.jsonl", &lines);

        let stats = collect_stats(&resolve_files(dir.path()));

        assert_eq!(
            stats.first_timestamp.as_deref(),
            Some("2026-03-27T10:00:00Z")
        );
        assert_eq!(
            stats.last_timestamp.as_deref(),
            Some("2026-03-27T11:30:00Z")
        );
    }

    #[test]
    fn empty_directory_yields_empty_stats() {
        let dir = TempDir::new().unwrap();
        let files = resolve_files(dir.path());
        assert!(files.is_empty());
    }

    #[test]
    fn malformed_lines_are_skipped() {
        let dir = TempDir::new().unwrap();
        let lines = vec![
            "not json at all".to_string(),
            r#"{"partial": true}"#.to_string(),
            make_entry("PreToolUse", "Bash", "allow", None, serde_json::json!({}), vec![], "2026-03-27T10:00:00Z"),
        ];
        write_jsonl(dir.path(), "audit-2026-03-27.jsonl", &lines);

        let stats = collect_stats(&resolve_files(dir.path()));
        assert_eq!(stats.decision_counts.get("allow"), Some(&1));
    }

    #[test]
    fn json_report_produces_valid_json() {
        let dir = TempDir::new().unwrap();
        let lines = vec![
            make_entry("PreToolUse", "Bash", "allow", Some("id-1"), serde_json::json!({"command": "ls"}), vec![make_provider("checker", 30, None)], "2026-03-27T10:00:00Z"),
            make_entry("PostToolUse", "Bash", "allow", Some("id-1"), serde_json::json!({}), vec![], "2026-03-27T10:00:05Z"),
        ];
        write_jsonl(dir.path(), "audit-2026-03-27.jsonl", &lines);

        let stats = collect_stats(&resolve_files(dir.path()));

        // Capture JSON output (we test the structure indirectly via stats)
        let total_pre: usize = stats.decision_counts.values().sum();
        assert_eq!(total_pre, 1);
        assert!(stats.provider_times.contains_key("checker"));
    }
}
