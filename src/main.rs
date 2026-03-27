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
//! ## CLI Flags
//!
//! - `--config <path>` — explicit config file path (highest priority)
//! - `--validate` — validate config and exit
//! - `--post-tool` — PostToolUse mode: audit-log only, output `{}`
//! - `--passthrough` — return passthrough when no config file found
//! - `--version` — show version
//!
//! See `docs/design/` for detailed design documents.

mod audit;
mod cli;
mod config;
mod hook;
mod provider;
mod quorum;

use clap::Parser;
use std::io::{self, Read};
use std::process;
use std::time::Instant;

fn main() {
    let cli = cli::Cli::parse();
    let start = Instant::now();

    // Load configuration (--config flag has highest priority)
    let config = match config::Config::load(cli.config.as_deref()) {
        Ok(cfg) => cfg,
        Err(config::ConfigError::NotFound) if cli.passthrough => {
            // --passthrough: use empty config instead of erroring
            config::Config::empty()
        }
        Err(e) => {
            eprintln!("claude-pretool-sidecar: config error: {e}");
            process::exit(1);
        }
    };

    // Apply CPTS_* environment variable overrides
    let mut config = config;
    config.apply_env_overrides();

    // --validate: check config and exit
    if cli.validate {
        let result = config.validate();

        for warning in &result.warnings {
            eprintln!("warning: {warning}");
        }
        for error in &result.errors {
            eprintln!("error: {error}");
        }

        if result.is_ok() {
            if result.warnings.is_empty() {
                eprintln!("config valid");
            } else {
                eprintln!("config valid (with warnings)");
            }
            process::exit(0);
        } else {
            eprintln!("config invalid");
            process::exit(1);
        }
    }

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

    // --post-tool: audit-log only, skip provider voting, output passthrough
    if cli.post_tool {
        let total_time_ms = start.elapsed().as_millis() as u64;
        audit::log_decision(
            &config.audit,
            &hook_event,
            &[],
            hook::Decision::Passthrough,
            total_time_ms,
        );
        println!("{{}}");
        return;
    }

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
