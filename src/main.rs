//! # claude-pretool-sidecar
//!
//! A composable sidecar for Claude Code's PreToolUse hook that aggregates
//! tool-approval votes from multiple external providers.
//!
//! ## How it works
//!
//! 1. Claude Code invokes this binary as a PreToolUse hook
//! 2. The sidecar reads the hook payload (JSON) from stdin
//! 3. It fans out the payload to all configured providers via stdio
//! 4. Each provider returns a vote: allow, deny, or passthrough
//! 5. The sidecar aggregates votes using configurable quorum rules
//! 6. It returns the final decision (JSON) on stdout to Claude Code
//! 7. It writes an audit log entry with per-provider details
//!
//! See `docs/design/` for detailed design documents.

mod audit;
mod config;
mod hook;
mod provider;
mod quorum;

use std::io::{self, Read};
use std::process;
use std::time::Instant;

fn main() {
    let start = Instant::now();

    // Read hook payload from stdin
    let mut input = String::new();
    if let Err(e) = io::stdin().read_to_string(&mut input) {
        eprintln!("claude-pretool-sidecar: failed to read stdin: {e}");
        process::exit(1);
    }

    // Parse the hook event
    let hook_event = match hook::HookEvent::from_json(&input) {
        Ok(event) => event,
        Err(e) => {
            eprintln!("claude-pretool-sidecar: failed to parse hook payload: {e}");
            process::exit(1);
        }
    };

    // Load configuration
    let config = match config::Config::load() {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("claude-pretool-sidecar: config error: {e}");
            process::exit(1);
        }
    };

    // Execute providers and collect detailed results
    let results = provider::execute_all(&config.providers, &hook_event, &config.timeout);

    // Extract votes from non-FYI providers for quorum aggregation
    let votes = provider::votes_from_results(&results);

    // Aggregate votes using quorum rules
    let decision = quorum::aggregate(&config.quorum, &votes);

    let total_time_ms = start.elapsed().as_millis() as u64;

    // Write audit log entry (if configured)
    audit::log_decision(&config.audit, &hook_event, &results, decision, total_time_ms);

    // Output decision as Claude Code hook response format on stdout
    let output = hook::HookResponse::from_decision(decision, None);
    match serde_json::to_string(&output) {
        Ok(json) => println!("{json}"),
        Err(e) => {
            eprintln!("claude-pretool-sidecar: failed to serialize response: {e}");
            process::exit(1);
        }
    }
}
