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
mod cache;
mod cli;
mod config;
mod health;
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

    // Initialize cache (file-based, scoped per session)
    let decision_cache = cache::DecisionCache::new(
        &config.cache,
        hook_event.session_id.as_deref(),
    );

    // Initialize health tracker (file-based, scoped per session)
    let mut health_tracker = if config.health.enabled {
        Some(health::HealthTracker::new(
            &config.health,
            hook_event.session_id.as_deref(),
        ))
    } else {
        None
    };

    // Check cache before invoking providers
    let (decision, results) =
        if let Some(cached_decision) = decision_cache.get(&hook_event.tool_name, &hook_event.tool_input) {
            eprintln!(
                "claude-pretool-sidecar: cache hit for {}(…) → {}",
                hook_event.tool_name, cached_decision
            );
            (cached_decision, vec![])
        } else {
            // Filter providers by health status, producing skip results for disabled ones
            let (healthy_providers, skip_results): (Vec<_>, Vec<_>) = config
                .providers
                .iter()
                .map(|p| {
                    if let Some(ref tracker) = health_tracker {
                        if !tracker.is_healthy(&p.name) {
                            let stats = tracker.get_stats(&p.name);
                            let (errors, total) = stats
                                .map(|s| (s.errors, s.total_calls))
                                .unwrap_or((0, 0));
                            return Err(provider::ProviderResult {
                                name: p.name.clone(),
                                vote: provider::Vote::Error,
                                mode: match p.mode {
                                    config::ProviderMode::Vote => "vote",
                                    config::ProviderMode::Fyi => "fyi",
                                }
                                .to_string(),
                                response_time_ms: 0,
                                reason: None,
                                error: Some(format!(
                                    "provider disabled (health: {} errors in {} calls)",
                                    errors, total
                                )),
                            });
                        }
                    }
                    Ok(p)
                })
                .partition::<Vec<_>, _>(|r| r.is_ok());

            let healthy_providers: Vec<_> = healthy_providers
                .into_iter()
                .map(|r| r.unwrap().clone())
                .collect();
            let mut skip_results: Vec<_> = skip_results
                .into_iter()
                .map(|r| r.unwrap_err())
                .collect();

            // Execute healthy providers and collect detailed results
            let mut results =
                provider::execute_all(&healthy_providers, &hook_event, &config.timeout);

            // Record results in health tracker
            if let Some(ref mut tracker) = health_tracker {
                for result in &results {
                    let is_error = result.vote == provider::Vote::Error;
                    tracker.record_result(
                        &result.name,
                        is_error,
                        result.error.as_deref(),
                    );
                }
            }

            // Combine executed results with skip results
            results.append(&mut skip_results);

            // Extract weighted votes from non-FYI providers for quorum aggregation
            let votes = provider::weighted_votes_from_results(&results);

            // Aggregate weighted votes using quorum rules
            let decision = quorum::aggregate_weighted(&config.quorum, &votes);

            // Store in cache for future identical calls
            decision_cache.put(&hook_event.tool_name, &hook_event.tool_input, decision);

            (decision, results)
        };

    let total_time_ms = start.elapsed().as_millis() as u64;

    // Save health state and print summary if any providers are degraded
    if let Some(ref tracker) = health_tracker {
        tracker.save();
        let summary = tracker.summary();
        if !summary.is_empty() {
            eprintln!("{}", summary);
        }
    }

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
