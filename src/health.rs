//! # Provider Health Monitoring
//!
//! Tracks provider error rates across invocations within a session and
//! auto-disables consistently failing providers. Health state is persisted
//! to a file-based store in /tmp, scoped per session (similar to cache.rs).
//!
//! ## Disability Policy
//!
//! Once a provider is disabled within a session, it stays disabled for the
//! remainder of that session. This is a deliberate design choice: if a provider
//! is consistently failing (e.g., binary missing, permission denied, crashing),
//! retrying it on every invocation adds latency for no benefit. A new session
//! (new Claude Code conversation) resets all health state.
//!
//! File format: `/tmp/cpts-health-{session_id}.json`

use crate::config::HealthConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Per-provider health statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderHealth {
    /// Total number of times this provider was invoked.
    pub total_calls: u32,
    /// Number of invocations that resulted in an error.
    pub errors: u32,
    /// The most recent error message, if any.
    pub last_error: Option<String>,
    /// Whether this provider has been disabled due to excessive errors.
    pub disabled: bool,
}

impl ProviderHealth {
    fn new() -> Self {
        Self {
            total_calls: 0,
            errors: 0,
            last_error: None,
            disabled: false,
        }
    }

    /// Current error rate as a fraction (0.0 to 1.0).
    fn error_rate(&self) -> f64 {
        if self.total_calls == 0 {
            return 0.0;
        }
        self.errors as f64 / self.total_calls as f64
    }
}

/// On-disk health state mapping provider names to their health stats.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct HealthFile {
    providers: HashMap<String, ProviderHealth>,
}

/// File-based health tracker scoped to a Claude Code session.
pub struct HealthTracker {
    config: HealthConfig,
    path: PathBuf,
    state: HealthFile,
}

impl HealthTracker {
    /// Create a new health tracker for the given session.
    ///
    /// Loads existing health state from disk if available.
    pub fn new(config: &HealthConfig, session_id: Option<&str>) -> Self {
        let sid = session_id.unwrap_or("default");
        let path = PathBuf::from(format!("/tmp/cpts-health-{sid}.json"));
        let state = Self::load_file(&path).unwrap_or_default();
        Self {
            config: config.clone(),
            path,
            state,
        }
    }

    /// Record the result of a provider invocation.
    ///
    /// If `is_error` is true, increments the error counter and optionally
    /// stores the error message. Then checks whether the provider should
    /// be disabled based on the configured thresholds.
    pub fn record_result(&mut self, provider_name: &str, is_error: bool, error_msg: Option<&str>) {
        let entry = self
            .state
            .providers
            .entry(provider_name.to_string())
            .or_insert_with(ProviderHealth::new);

        entry.total_calls += 1;
        if is_error {
            entry.errors += 1;
            entry.last_error = error_msg.map(|s| s.to_string());
        }

        // Check if provider should be disabled (once disabled, stays disabled)
        if !entry.disabled
            && entry.total_calls >= self.config.min_calls_before_disable
            && entry.error_rate() > self.config.max_error_rate
        {
            entry.disabled = true;
            eprintln!(
                "claude-pretool-sidecar: health: disabled provider '{}' (error rate: {:.0}%, {} errors in {} calls)",
                provider_name,
                entry.error_rate() * 100.0,
                entry.errors,
                entry.total_calls,
            );
        }
    }

    /// Check whether a provider is healthy (not disabled).
    ///
    /// Returns `true` if the provider has not been disabled or has no
    /// recorded history. Returns `false` if the provider has been disabled.
    pub fn is_healthy(&self, provider_name: &str) -> bool {
        match self.state.providers.get(provider_name) {
            Some(health) => !health.disabled,
            None => true, // unknown provider is assumed healthy
        }
    }

    /// Get the health stats for a specific provider.
    pub fn get_stats(&self, provider_name: &str) -> Option<&ProviderHealth> {
        self.state.providers.get(provider_name)
    }

    /// Generate a human-readable health summary.
    ///
    /// Only includes providers that have degraded health (any errors).
    /// Returns an empty string if all providers are healthy.
    pub fn summary(&self) -> String {
        let degraded: Vec<_> = self
            .state
            .providers
            .iter()
            .filter(|(_, h)| h.errors > 0)
            .collect();

        if degraded.is_empty() {
            return String::new();
        }

        let mut lines = vec!["claude-pretool-sidecar: health summary:".to_string()];
        for (name, health) in &degraded {
            let status = if health.disabled {
                "DISABLED"
            } else {
                "degraded"
            };
            lines.push(format!(
                "  {}: {} ({} errors / {} calls, {:.0}% error rate)",
                name,
                status,
                health.errors,
                health.total_calls,
                health.error_rate() * 100.0,
            ));
            if let Some(ref err) = health.last_error {
                lines.push(format!("    last error: {}", err));
            }
        }

        lines.join("\n")
    }

    /// Persist current health state to disk.
    ///
    /// Errors are silently ignored (health tracking is best-effort).
    pub fn save(&self) {
        if let Ok(data) = serde_json::to_string(&self.state) {
            let _ = std::fs::write(&self.path, data);
        }
    }

    /// Load health state from disk.
    fn load_file(path: &PathBuf) -> Option<HealthFile> {
        let data = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&data).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a HealthConfig for testing.
    fn test_config() -> HealthConfig {
        HealthConfig {
            enabled: true,
            max_error_rate: 0.5,
            min_calls_before_disable: 3,
        }
    }

    /// Helper: create a HealthTracker with a temp file path.
    fn test_tracker(config: &HealthConfig) -> HealthTracker {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("test-health.json");
        let p = path.clone();
        std::mem::forget(dir); // keep the temp dir alive
        HealthTracker {
            config: config.clone(),
            path: p,
            state: HealthFile::default(),
        }
    }

    /// Provider stays healthy after recording only successes.
    #[test]
    fn record_success_keeps_healthy() {
        let config = test_config();
        let mut tracker = test_tracker(&config);

        for _ in 0..10 {
            tracker.record_result("good-provider", false, None);
        }

        assert!(tracker.is_healthy("good-provider"));
        let stats = tracker.get_stats("good-provider").unwrap();
        assert_eq!(stats.total_calls, 10);
        assert_eq!(stats.errors, 0);
        assert!(!stats.disabled);
    }

    /// Provider is disabled after exceeding the error rate threshold
    /// once min_calls_before_disable is reached.
    #[test]
    fn record_errors_disables_provider() {
        let config = test_config(); // max_error_rate=0.5, min_calls=3
        let mut tracker = test_tracker(&config);

        // 3 errors in 3 calls = 100% error rate > 50% threshold
        tracker.record_result("bad-provider", true, Some("spawn failed"));
        tracker.record_result("bad-provider", true, Some("spawn failed"));
        tracker.record_result("bad-provider", true, Some("spawn failed"));

        assert!(!tracker.is_healthy("bad-provider"));
        let stats = tracker.get_stats("bad-provider").unwrap();
        assert!(stats.disabled);
        assert_eq!(stats.errors, 3);
        assert_eq!(stats.total_calls, 3);
        assert_eq!(stats.last_error, Some("spawn failed".to_string()));
    }

    /// Provider is not disabled until min_calls_before_disable is reached,
    /// even if all calls so far have been errors.
    #[test]
    fn min_calls_before_disable() {
        let config = HealthConfig {
            enabled: true,
            max_error_rate: 0.5,
            min_calls_before_disable: 5,
        };
        let mut tracker = test_tracker(&config);

        // 3 errors in 3 calls (100% rate), but min_calls=5 not reached
        tracker.record_result("flaky", true, Some("err1"));
        tracker.record_result("flaky", true, Some("err2"));
        tracker.record_result("flaky", true, Some("err3"));

        assert!(
            tracker.is_healthy("flaky"),
            "should still be healthy: only 3 calls, need 5 before disabling"
        );

        // 2 more errors → 5 calls, all errors → now should be disabled
        tracker.record_result("flaky", true, Some("err4"));
        tracker.record_result("flaky", true, Some("err5"));

        assert!(!tracker.is_healthy("flaky"));
    }

    /// Once disabled, a provider stays disabled for the session even if
    /// subsequent calls would have been successes (sticky disability).
    #[test]
    fn disabled_stays_disabled_for_session() {
        let config = test_config();
        let mut tracker = test_tracker(&config);

        // Disable the provider
        tracker.record_result("sticky", true, None);
        tracker.record_result("sticky", true, None);
        tracker.record_result("sticky", true, None);
        assert!(!tracker.is_healthy("sticky"));

        // Record many successes — should remain disabled
        for _ in 0..20 {
            tracker.record_result("sticky", false, None);
        }
        assert!(
            !tracker.is_healthy("sticky"),
            "once disabled, provider stays disabled for the session"
        );
    }

    /// is_healthy returns false for a disabled provider.
    #[test]
    fn disabled_provider_returns_false() {
        let config = test_config();
        let mut tracker = test_tracker(&config);

        // Manually set disabled
        tracker.state.providers.insert(
            "manual".to_string(),
            ProviderHealth {
                total_calls: 10,
                errors: 8,
                last_error: Some("boom".to_string()),
                disabled: true,
            },
        );

        assert!(!tracker.is_healthy("manual"));
    }

    /// Unknown providers are considered healthy.
    #[test]
    fn unknown_provider_is_healthy() {
        let config = test_config();
        let tracker = test_tracker(&config);

        assert!(tracker.is_healthy("never-seen"));
        assert!(tracker.get_stats("never-seen").is_none());
    }

    /// Summary is empty when no providers have errors.
    #[test]
    fn summary_empty_when_all_healthy() {
        let config = test_config();
        let mut tracker = test_tracker(&config);

        tracker.record_result("good", false, None);
        assert!(tracker.summary().is_empty());
    }

    /// Summary includes degraded providers.
    #[test]
    fn summary_includes_degraded_providers() {
        let config = test_config();
        let mut tracker = test_tracker(&config);

        tracker.record_result("flaky", true, Some("timeout"));
        tracker.record_result("flaky", false, None);

        let summary = tracker.summary();
        assert!(summary.contains("flaky"));
        assert!(summary.contains("degraded"));
        assert!(summary.contains("1 errors / 2 calls"));
    }

    /// Summary shows DISABLED status for disabled providers.
    #[test]
    fn summary_shows_disabled_status() {
        let config = test_config();
        let mut tracker = test_tracker(&config);

        tracker.record_result("broken", true, Some("not found"));
        tracker.record_result("broken", true, Some("not found"));
        tracker.record_result("broken", true, Some("not found"));

        let summary = tracker.summary();
        assert!(summary.contains("DISABLED"));
        assert!(summary.contains("broken"));
    }

    /// Health state persists across save/load cycles.
    #[test]
    fn persistence_save_and_load() {
        let config = test_config();
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("persist-test.json");

        // Create tracker, record some data, save
        {
            let mut tracker = HealthTracker {
                config: config.clone(),
                path: path.clone(),
                state: HealthFile::default(),
            };
            tracker.record_result("p1", true, Some("error1"));
            tracker.record_result("p1", true, Some("error2"));
            tracker.record_result("p1", true, Some("error3"));
            tracker.save();
        }

        // Load into a new tracker and verify state was preserved
        {
            let loaded_state = HealthTracker::load_file(&path).unwrap();
            let tracker = HealthTracker {
                config: config.clone(),
                path: path.clone(),
                state: loaded_state,
            };
            assert!(!tracker.is_healthy("p1"));
            let stats = tracker.get_stats("p1").unwrap();
            assert_eq!(stats.total_calls, 3);
            assert_eq!(stats.errors, 3);
            assert!(stats.disabled);
        }
    }
}
