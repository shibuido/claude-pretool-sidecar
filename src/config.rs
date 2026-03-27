//! # Configuration
//!
//! Handles loading and parsing the sidecar configuration file.
//! See `docs/design/configuration.md` for the config format specification.

use crate::hook::Decision;
use serde::Deserialize;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("no config file found (searched: CLI flag, env var, .claude-pretool-sidecar.toml, ~/.config/claude-pretool-sidecar/config.toml)")]
    NotFound,

    #[error("failed to read config file {path}: {source}")]
    ReadError {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to parse config file {path}: {source}")]
    ParseError {
        path: PathBuf,
        source: toml::de::Error,
    },
}

/// Top-level configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub quorum: QuorumConfig,

    #[serde(default)]
    pub timeout: TimeoutConfig,

    #[serde(default)]
    pub providers: Vec<ProviderConfig>,

    #[serde(default)]
    pub audit: AuditConfig,

    #[serde(default)]
    pub cache: CacheConfig,

    #[serde(default)]
    pub health: HealthConfig,

    #[serde(default)]
    pub rules: Vec<RuleConfig>,
}

/// Configuration for a single rule in the rules engine.
///
/// Rules are evaluated in order — first match wins.
/// The `tool` field is a regex matched against `tool_name`.
/// The `input` field (optional) is a regex matched against serialized `tool_input`.
///
/// ```toml
/// [[rules]]
/// tool = "Bash"
/// input = "^ls |^pwd$"
/// decision = "allow"
/// reason = "Safe read-only commands"
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct RuleConfig {
    /// Regex pattern matched against tool_name (e.g., "Bash", "Write|Edit", "*").
    pub tool: String,

    /// Optional regex pattern matched against serialized tool_input JSON.
    pub input: Option<String>,

    /// Decision to return when this rule matches: allow, deny, or passthrough.
    pub decision: Decision,

    /// Optional human-readable reason explaining the rule.
    pub reason: Option<String>,
}

/// Audit logging configuration.
///
/// Supports date-based log file chunking and automatic size management.
/// See `docs/design/log-rotation.md` for the rotation algorithm.
#[derive(Debug, Clone, Deserialize)]
pub struct AuditConfig {
    /// Whether audit logging is enabled.
    #[serde(default)]
    pub enabled: bool,

    /// Output destination: directory path for date-chunked files, or "stderr".
    /// When a directory, files are named `audit-YYYY-MM-DD.jsonl`.
    #[serde(default = "default_audit_output")]
    pub output: String,

    /// Maximum total size across all log files in bytes (default: 10 MB).
    #[serde(default = "default_max_total_bytes")]
    pub max_total_bytes: u64,

    /// Maximum size per individual log file in bytes (default: 5 MB).
    #[serde(default = "default_max_file_bytes")]
    pub max_file_bytes: u64,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            output: default_audit_output(),
            max_total_bytes: default_max_total_bytes(),
            max_file_bytes: default_max_file_bytes(),
        }
    }
}

fn default_audit_output() -> String {
    "stderr".to_string()
}

fn default_max_total_bytes() -> u64 {
    10 * 1024 * 1024 // 10 MB
}

fn default_max_file_bytes() -> u64 {
    5 * 1024 * 1024 // 5 MB
}

/// Cache configuration for provider decision caching.
///
/// When enabled, identical (tool_name, tool_input) pairs within the TTL
/// return a cached decision without re-invoking providers.
/// Cache is file-based, scoped per session, stored in /tmp.
///
/// ```toml
/// [cache]
/// enabled = false
/// ttl_seconds = 60
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct CacheConfig {
    /// Whether caching is enabled (opt-in, default false).
    #[serde(default)]
    pub enabled: bool,

    /// How long cached decisions remain valid, in seconds.
    #[serde(default = "default_cache_ttl")]
    pub ttl_seconds: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            ttl_seconds: default_cache_ttl(),
        }
    }
}

fn default_cache_ttl() -> u64 {
    60
}

/// Health monitoring configuration for provider error tracking.
///
/// When enabled, tracks provider error rates across invocations within
/// a session and auto-disables consistently failing providers.
///
/// ```toml
/// [health]
/// enabled = false
/// max_error_rate = 0.5
/// min_calls_before_disable = 3
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct HealthConfig {
    /// Whether health monitoring is enabled (opt-in, default false).
    #[serde(default)]
    pub enabled: bool,

    /// Disable provider if its error rate exceeds this threshold (0.0 to 1.0).
    #[serde(default = "default_max_error_rate")]
    pub max_error_rate: f64,

    /// Minimum number of calls before a provider can be disabled.
    #[serde(default = "default_min_calls_before_disable")]
    pub min_calls_before_disable: u32,
}

impl Default for HealthConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_error_rate: default_max_error_rate(),
            min_calls_before_disable: default_min_calls_before_disable(),
        }
    }
}

fn default_max_error_rate() -> f64 {
    0.5
}

fn default_min_calls_before_disable() -> u32 {
    3
}

/// Quorum rules for vote aggregation.
#[derive(Debug, Clone, Deserialize)]
pub struct QuorumConfig {
    /// Minimum number of "allow" votes required.
    #[serde(default = "default_min_allow")]
    pub min_allow: u32,

    /// Maximum number of "deny" votes tolerated.
    #[serde(default)]
    pub max_deny: u32,

    /// How to treat provider errors.
    #[serde(default = "default_passthrough")]
    pub error_policy: Decision,

    /// Decision when quorum is not met and no deny threshold exceeded.
    #[serde(default = "default_passthrough")]
    pub default_decision: Decision,
}

impl Default for QuorumConfig {
    fn default() -> Self {
        Self {
            min_allow: default_min_allow(),
            max_deny: 0,
            error_policy: Decision::Passthrough,
            default_decision: Decision::Passthrough,
        }
    }
}

fn default_min_allow() -> u32 {
    1
}

fn default_passthrough() -> Decision {
    Decision::Passthrough
}

/// Timeout settings.
#[derive(Debug, Clone, Deserialize)]
pub struct TimeoutConfig {
    /// Default timeout per provider in milliseconds.
    #[serde(default = "default_provider_timeout")]
    pub provider_default: u64,

    /// Total timeout for all providers combined in milliseconds.
    #[serde(default = "default_total_timeout")]
    pub total: u64,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            provider_default: default_provider_timeout(),
            total: default_total_timeout(),
        }
    }
}

fn default_provider_timeout() -> u64 {
    5000
}

fn default_total_timeout() -> u64 {
    30000
}

/// Configuration for a single provider.
#[derive(Debug, Clone, Deserialize)]
pub struct ProviderConfig {
    /// Human-readable name for logging.
    pub name: String,

    /// Command to execute.
    pub command: String,

    /// Command arguments.
    #[serde(default)]
    pub args: Vec<String>,

    /// Provider mode: "vote" or "fyi".
    #[serde(default = "default_mode")]
    pub mode: ProviderMode,

    /// Per-provider timeout override (milliseconds).
    pub timeout: Option<u64>,

    /// Vote weight for quorum aggregation (default: 1).
    /// Only meaningful for vote-mode providers; ignored for FYI providers.
    #[serde(default = "default_weight")]
    pub weight: u32,

    /// Additional environment variables.
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
}

fn default_weight() -> u32 {
    1
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderMode {
    Vote,
    Fyi,
}

fn default_mode() -> ProviderMode {
    ProviderMode::Vote
}

/// Result of a config validation check.
#[derive(Debug)]
pub struct ValidationResult {
    /// Warning messages (non-fatal issues).
    pub warnings: Vec<String>,
    /// Error messages (fatal issues).
    pub errors: Vec<String>,
}

impl ValidationResult {
    /// Returns true if validation passed (no errors).
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
}

impl Config {
    /// Load configuration from the first available source.
    ///
    /// Search order:
    /// 1. `explicit_path` from `--config <path>` CLI flag (highest priority)
    /// 2. `$CLAUDE_PRETOOL_SIDECAR_CONFIG` environment variable
    /// 3. `.claude-pretool-sidecar.toml` in current directory
    /// 4. `~/.config/claude-pretool-sidecar/config.toml` (XDG)
    /// 5. `~/.claude-pretool-sidecar.toml` (home fallback)
    pub fn load(explicit_path: Option<&std::path::Path>) -> Result<Self, ConfigError> {
        // Try explicit CLI path first (highest priority)
        if let Some(path) = explicit_path {
            return Self::load_from(&path.to_path_buf());
        }

        // Try environment variable
        if let Ok(path) = std::env::var("CLAUDE_PRETOOL_SIDECAR_CONFIG") {
            let path = PathBuf::from(path);
            return Self::load_from(&path);
        }

        // Try current directory
        let local = PathBuf::from(".claude-pretool-sidecar.toml");
        if local.exists() {
            return Self::load_from(&local);
        }

        // Try XDG config
        if let Some(config_dir) = dirs_path() {
            let xdg = config_dir.join("claude-pretool-sidecar/config.toml");
            if xdg.exists() {
                return Self::load_from(&xdg);
            }
        }

        // Try home directory fallback
        if let Some(home) = home_path() {
            let home_config = home.join(".claude-pretool-sidecar.toml");
            if home_config.exists() {
                return Self::load_from(&home_config);
            }
        }

        Err(ConfigError::NotFound)
    }

    /// Load configuration from a specific file path.
    pub fn load_from(path: &PathBuf) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path).map_err(|e| ConfigError::ReadError {
            path: path.clone(),
            source: e,
        })?;

        let config: Config =
            toml::from_str(&content).map_err(|e| ConfigError::ParseError {
                path: path.clone(),
                source: e,
            })?;

        Ok(config)
    }

    /// Returns a default empty config (no providers, passthrough defaults).
    ///
    /// Used when `--passthrough` flag is set and no config file is found.
    pub fn empty() -> Self {
        Config {
            quorum: QuorumConfig::default(),
            timeout: TimeoutConfig::default(),
            providers: Vec::new(),
            audit: AuditConfig::default(),
            cache: CacheConfig::default(),
            health: HealthConfig::default(),
            rules: Vec::new(),
        }
    }

    /// Apply environment variable overrides (CPTS_* prefix).
    ///
    /// Checks for `CPTS_MIN_ALLOW`, `CPTS_MAX_DENY`, `CPTS_ERROR_POLICY`,
    /// and `CPTS_TIMEOUT` and overrides the corresponding config fields.
    /// Invalid values print a warning to stderr and keep the config file value.
    pub fn apply_env_overrides(&mut self) {
        self.apply_env_overrides_from(
            std::env::var("CPTS_MIN_ALLOW").ok(),
            std::env::var("CPTS_MAX_DENY").ok(),
            std::env::var("CPTS_ERROR_POLICY").ok(),
            std::env::var("CPTS_TIMEOUT").ok(),
        );
    }

    /// Apply environment variable overrides from explicit Option values.
    ///
    /// This is the testable core — accepts parsed env values as parameters
    /// so unit tests don't need to mutate the process environment.
    pub fn apply_env_overrides_from(
        &mut self,
        min_allow: Option<String>,
        max_deny: Option<String>,
        error_policy: Option<String>,
        timeout: Option<String>,
    ) {
        if let Some(val) = min_allow {
            match val.parse::<u32>() {
                Ok(n) => self.quorum.min_allow = n,
                Err(_) => eprintln!(
                    "claude-pretool-sidecar: CPTS_MIN_ALLOW: expected integer, got '{val}'"
                ),
            }
        }

        if let Some(val) = max_deny {
            match val.parse::<u32>() {
                Ok(n) => self.quorum.max_deny = n,
                Err(_) => eprintln!(
                    "claude-pretool-sidecar: CPTS_MAX_DENY: expected integer, got '{val}'"
                ),
            }
        }

        if let Some(val) = error_policy {
            match val.as_str() {
                "allow" => self.quorum.error_policy = Decision::Allow,
                "deny" => self.quorum.error_policy = Decision::Deny,
                "passthrough" => self.quorum.error_policy = Decision::Passthrough,
                _ => eprintln!(
                    "claude-pretool-sidecar: CPTS_ERROR_POLICY: expected 'allow', 'deny', or 'passthrough', got '{val}'"
                ),
            }
        }

        if let Some(val) = timeout {
            match val.parse::<u64>() {
                Ok(n) => self.timeout.provider_default = n,
                Err(_) => eprintln!(
                    "claude-pretool-sidecar: CPTS_TIMEOUT: expected integer, got '{val}'"
                ),
            }
        }
    }

    /// Validate the loaded configuration for potential issues.
    ///
    /// Checks:
    /// - Provider command paths exist and are executable (warning if not)
    /// - Quorum min_allow doesn't exceed number of vote-mode providers (warning)
    pub fn validate(&self) -> ValidationResult {
        let mut warnings = Vec::new();
        let mut errors = Vec::new();

        // Count vote-mode providers
        let vote_provider_count = self
            .providers
            .iter()
            .filter(|p| p.mode == ProviderMode::Vote)
            .count() as u32;

        // Check if min_allow exceeds vote provider count
        if vote_provider_count > 0 && self.quorum.min_allow > vote_provider_count {
            warnings.push(format!(
                "quorum.min_allow ({}) exceeds number of vote-mode providers ({}); quorum can never be met",
                self.quorum.min_allow, vote_provider_count
            ));
        }

        // Check if min_allow > 0 but no vote providers exist
        if vote_provider_count == 0 && self.quorum.min_allow > 0 {
            warnings.push(format!(
                "quorum.min_allow is {} but no vote-mode providers are configured; quorum can never be met",
                self.quorum.min_allow
            ));
        }

        // Check provider commands
        for provider in &self.providers {
            let cmd_path = std::path::Path::new(&provider.command);

            // Only check absolute paths — relative/bare commands might be in PATH at runtime
            if cmd_path.is_absolute() {
                if !cmd_path.exists() {
                    warnings.push(format!(
                        "provider '{}': command '{}' does not exist",
                        provider.name, provider.command
                    ));
                } else {
                    // Check executable permission on Unix
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        if let Ok(meta) = std::fs::metadata(cmd_path) {
                            if meta.permissions().mode() & 0o111 == 0 {
                                warnings.push(format!(
                                    "provider '{}': command '{}' exists but is not executable",
                                    provider.name, provider.command
                                ));
                            }
                        }
                    }
                }
            }
        }

        // Check for duplicate provider names
        let mut seen_names = std::collections::HashSet::new();
        for provider in &self.providers {
            if !seen_names.insert(&provider.name) {
                errors.push(format!(
                    "duplicate provider name '{}'",
                    provider.name
                ));
            }
        }

        ValidationResult { warnings, errors }
    }
}

/// Get XDG config directory or fallback.
fn dirs_path() -> Option<PathBuf> {
    std::env::var("XDG_CONFIG_HOME")
        .ok()
        .map(PathBuf::from)
        .or_else(|| home_path().map(|h| h.join(".config")))
}

/// Get home directory.
fn home_path() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// # Config Parsing Tests
    ///
    /// These tests verify that TOML configuration files are correctly parsed
    /// into our Config struct, with proper defaults for omitted fields.

    /// A minimal config with just one provider should parse with all defaults.
    #[test]
    fn parse_minimal_config() {
        let toml = r#"
            [[providers]]
            name = "checker"
            command = "/usr/bin/check"
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.providers.len(), 1);
        assert_eq!(config.providers[0].name, "checker");
        assert_eq!(config.providers[0].mode, ProviderMode::Vote);
        assert_eq!(config.quorum.min_allow, 1);
        assert_eq!(config.quorum.max_deny, 0);
    }

    /// A config with all fields specified should parse correctly.
    #[test]
    fn parse_full_config() {
        let toml = r#"
            [quorum]
            min_allow = 2
            max_deny = 1
            error_policy = "deny"
            default_decision = "deny"

            [timeout]
            provider_default = 10000
            total = 60000

            [[providers]]
            name = "security"
            command = "/usr/bin/sec-check"
            args = ["--strict"]
            mode = "vote"
            timeout = 15000

            [[providers]]
            name = "logger"
            command = "claude-pretool-logger"
            mode = "fyi"
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.quorum.min_allow, 2);
        assert_eq!(config.quorum.max_deny, 1);
        assert_eq!(config.quorum.error_policy, Decision::Deny);
        assert_eq!(config.timeout.provider_default, 10000);
        assert_eq!(config.providers.len(), 2);
        assert_eq!(config.providers[0].args, vec!["--strict"]);
        assert_eq!(config.providers[1].mode, ProviderMode::Fyi);
    }

    /// An empty config should parse with all defaults.
    #[test]
    fn parse_empty_config() {
        let toml = "";
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.quorum.min_allow, 1);
        assert_eq!(config.providers.len(), 0);
    }

    /// FYI mode should be correctly deserialized.
    #[test]
    fn parse_fyi_provider() {
        let toml = r#"
            [[providers]]
            name = "audit"
            command = "logger"
            mode = "fyi"
        "#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.providers[0].mode, ProviderMode::Fyi);
    }

    /// # Config Loading Tests

    /// Load with explicit path should use that path.
    #[test]
    fn load_with_explicit_path_uses_path() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("test-config.toml");
        std::fs::write(
            &path,
            r#"
            [[providers]]
            name = "test"
            command = "echo"
            "#,
        )
        .unwrap();

        let config = Config::load(Some(&path)).unwrap();
        assert_eq!(config.providers.len(), 1);
        assert_eq!(config.providers[0].name, "test");
    }

    /// Load with None path should fall through to discovery.
    #[test]
    fn load_with_none_path_falls_through() {
        // Unset env var to avoid interference, use a temp dir as HOME
        // where no config exists
        let dir = tempfile::TempDir::new().unwrap();
        unsafe {
            std::env::set_var("HOME", dir.path());
            std::env::remove_var("CLAUDE_PRETOOL_SIDECAR_CONFIG");
        }

        let result = Config::load(None);
        assert!(matches!(result, Err(ConfigError::NotFound)));

        // Clean up
        unsafe {
            std::env::remove_var("HOME");
        }
    }

    /// Empty config should have zero providers and default quorum.
    #[test]
    fn empty_config_has_defaults() {
        let config = Config::empty();
        assert_eq!(config.providers.len(), 0);
        assert_eq!(config.quorum.min_allow, 1);
        assert_eq!(config.quorum.max_deny, 0);
    }

    /// # Config Validation Tests
    ///
    /// These tests verify that the validate() method correctly identifies
    /// potential issues in the configuration.

    /// Valid config with matching quorum and providers passes validation.
    #[test]
    fn validate_valid_config_passes() {
        let config = Config {
            quorum: QuorumConfig {
                min_allow: 1,
                max_deny: 0,
                error_policy: Decision::Passthrough,
                default_decision: Decision::Passthrough,
            },
            timeout: TimeoutConfig::default(),
            providers: vec![ProviderConfig {
                name: "checker".to_string(),
                command: "echo".to_string(),
                args: vec![],
                mode: ProviderMode::Vote,
                timeout: None,
                weight: 1,
                env: std::collections::HashMap::new(),
            }],
            audit: AuditConfig::default(),
            cache: CacheConfig::default(),
            health: HealthConfig::default(),
            rules: Vec::new(),
        };

        let result = config.validate();
        assert!(result.is_ok());
        assert!(result.warnings.is_empty());
    }

    /// min_allow exceeding vote provider count should produce a warning.
    #[test]
    fn validate_min_allow_exceeds_providers_warns() {
        let config = Config {
            quorum: QuorumConfig {
                min_allow: 3,
                max_deny: 0,
                error_policy: Decision::Passthrough,
                default_decision: Decision::Passthrough,
            },
            timeout: TimeoutConfig::default(),
            providers: vec![ProviderConfig {
                name: "checker".to_string(),
                command: "echo".to_string(),
                args: vec![],
                mode: ProviderMode::Vote,
                timeout: None,
                weight: 1,
                env: std::collections::HashMap::new(),
            }],
            audit: AuditConfig::default(),
            cache: CacheConfig::default(),
            health: HealthConfig::default(),
            rules: Vec::new(),
        };

        let result = config.validate();
        assert!(result.is_ok()); // warnings, not errors
        assert!(result.warnings.iter().any(|w| w.contains("min_allow (3) exceeds")));
    }

    /// min_allow > 0 with only fyi providers should warn.
    #[test]
    fn validate_no_vote_providers_with_min_allow_warns() {
        let config = Config {
            quorum: QuorumConfig {
                min_allow: 1,
                max_deny: 0,
                error_policy: Decision::Passthrough,
                default_decision: Decision::Passthrough,
            },
            timeout: TimeoutConfig::default(),
            providers: vec![ProviderConfig {
                name: "logger".to_string(),
                command: "echo".to_string(),
                args: vec![],
                mode: ProviderMode::Fyi,
                timeout: None,
                weight: 1,
                env: std::collections::HashMap::new(),
            }],
            audit: AuditConfig::default(),
            cache: CacheConfig::default(),
            health: HealthConfig::default(),
            rules: Vec::new(),
        };

        let result = config.validate();
        assert!(result.warnings.iter().any(|w| w.contains("no vote-mode providers")));
    }

    /// Non-existent absolute command path should produce a warning.
    #[test]
    fn validate_nonexistent_command_warns() {
        let config = Config {
            quorum: QuorumConfig::default(),
            timeout: TimeoutConfig::default(),
            providers: vec![ProviderConfig {
                name: "ghost".to_string(),
                command: "/nonexistent/path/to/binary".to_string(),
                args: vec![],
                mode: ProviderMode::Vote,
                timeout: None,
                weight: 1,
                env: std::collections::HashMap::new(),
            }],
            audit: AuditConfig::default(),
            cache: CacheConfig::default(),
            health: HealthConfig::default(),
            rules: Vec::new(),
        };

        let result = config.validate();
        assert!(result.warnings.iter().any(|w| w.contains("does not exist")));
    }

    /// Bare command names (not absolute paths) should not produce warnings.
    #[test]
    fn validate_bare_command_no_warning() {
        let config = Config {
            quorum: QuorumConfig::default(),
            timeout: TimeoutConfig::default(),
            providers: vec![ProviderConfig {
                name: "checker".to_string(),
                command: "echo".to_string(),
                args: vec![],
                mode: ProviderMode::Vote,
                timeout: None,
                weight: 1,
                env: std::collections::HashMap::new(),
            }],
            audit: AuditConfig::default(),
            cache: CacheConfig::default(),
            health: HealthConfig::default(),
            rules: Vec::new(),
        };

        let result = config.validate();
        assert!(result.warnings.is_empty());
    }

    /// # Environment Variable Override Tests
    ///
    /// These tests use `apply_env_overrides_from()` to avoid mutating
    /// process-global env vars, ensuring test isolation.

    /// CPTS_MIN_ALLOW overrides quorum.min_allow.
    #[test]
    fn env_override_min_allow() {
        let mut config = Config::empty();
        assert_eq!(config.quorum.min_allow, 1); // default
        config.apply_env_overrides_from(Some("3".to_string()), None, None, None);
        assert_eq!(config.quorum.min_allow, 3);
    }

    /// CPTS_MAX_DENY overrides quorum.max_deny.
    #[test]
    fn env_override_max_deny() {
        let mut config = Config::empty();
        assert_eq!(config.quorum.max_deny, 0); // default
        config.apply_env_overrides_from(None, Some("2".to_string()), None, None);
        assert_eq!(config.quorum.max_deny, 2);
    }

    /// CPTS_ERROR_POLICY overrides quorum.error_policy.
    #[test]
    fn env_override_error_policy() {
        let mut config = Config::empty();
        assert_eq!(config.quorum.error_policy, Decision::Passthrough); // default
        config.apply_env_overrides_from(None, None, Some("deny".to_string()), None);
        assert_eq!(config.quorum.error_policy, Decision::Deny);
    }

    /// CPTS_TIMEOUT overrides timeout.provider_default.
    #[test]
    fn env_override_timeout() {
        let mut config = Config::empty();
        assert_eq!(config.timeout.provider_default, 5000); // default
        config.apply_env_overrides_from(None, None, None, Some("10000".to_string()));
        assert_eq!(config.timeout.provider_default, 10000);
    }

    /// Invalid number for CPTS_MIN_ALLOW keeps the default value.
    #[test]
    fn env_override_invalid_number_keeps_default() {
        let mut config = Config::empty();
        config.apply_env_overrides_from(Some("abc".to_string()), None, None, None);
        assert_eq!(config.quorum.min_allow, 1); // unchanged
    }

    /// Invalid policy string for CPTS_ERROR_POLICY keeps the default value.
    #[test]
    fn env_override_invalid_policy_keeps_default() {
        let mut config = Config::empty();
        config.apply_env_overrides_from(None, None, Some("invalid".to_string()), None);
        assert_eq!(config.quorum.error_policy, Decision::Passthrough); // unchanged
    }

    /// Multiple env overrides can be applied simultaneously.
    #[test]
    fn env_override_multiple_values() {
        let mut config = Config::empty();
        config.apply_env_overrides_from(
            Some("5".to_string()),
            Some("3".to_string()),
            Some("allow".to_string()),
            Some("15000".to_string()),
        );
        assert_eq!(config.quorum.min_allow, 5);
        assert_eq!(config.quorum.max_deny, 3);
        assert_eq!(config.quorum.error_policy, Decision::Allow);
        assert_eq!(config.timeout.provider_default, 15000);
    }

    /// Weight field should be parsed from TOML config.
    #[test]
    fn parse_weight_from_config() {
        let toml = r#"
            [[providers]]
            name = "security"
            command = "/usr/bin/sec-check"
            mode = "vote"
            weight = 3
        "#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.providers[0].weight, 3);
    }

    /// Weight should default to 1 when not specified.
    #[test]
    fn parse_weight_defaults_to_one() {
        let toml = r#"
            [[providers]]
            name = "checker"
            command = "/usr/bin/check"
        "#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.providers[0].weight, 1);
    }

    /// Duplicate provider names should produce an error.
    #[test]
    fn validate_duplicate_provider_names_errors() {
        let config = Config {
            quorum: QuorumConfig::default(),
            timeout: TimeoutConfig::default(),
            providers: vec![
                ProviderConfig {
                    name: "checker".to_string(),
                    command: "echo".to_string(),
                    args: vec![],
                    mode: ProviderMode::Vote,
                    timeout: None,
                    weight: 1,
                    env: std::collections::HashMap::new(),
                },
                ProviderConfig {
                    name: "checker".to_string(),
                    command: "true".to_string(),
                    args: vec![],
                    mode: ProviderMode::Vote,
                    timeout: None,
                    weight: 1,
                    env: std::collections::HashMap::new(),
                },
            ],
            audit: AuditConfig::default(),
            cache: CacheConfig::default(),
            health: HealthConfig::default(),
            rules: Vec::new(),
        };

        let result = config.validate();
        assert!(!result.is_ok());
        assert!(result.errors.iter().any(|e| e.contains("duplicate provider name")));
    }

    /// Rules should be parsed from TOML config.
    #[test]
    fn parse_rules_from_config() {
        let toml = r#"
            [[rules]]
            tool = "Bash"
            input = "^ls "
            decision = "allow"
            reason = "Safe command"

            [[rules]]
            tool = "Write|Edit"
            decision = "deny"
        "#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.rules.len(), 2);
        assert_eq!(config.rules[0].tool, "Bash");
        assert_eq!(config.rules[0].input, Some("^ls ".to_string()));
        assert_eq!(config.rules[0].decision, Decision::Allow);
        assert_eq!(config.rules[0].reason, Some("Safe command".to_string()));
        assert_eq!(config.rules[1].tool, "Write|Edit");
        assert_eq!(config.rules[1].input, None);
        assert_eq!(config.rules[1].decision, Decision::Deny);
        assert!(config.rules[1].reason.is_none());
    }

    /// Config with no rules should default to empty list.
    #[test]
    fn parse_config_without_rules_defaults_to_empty() {
        let toml = r#"
            [[providers]]
            name = "checker"
            command = "echo"
        "#;
        let config: Config = toml::from_str(toml).unwrap();
        assert!(config.rules.is_empty());
    }

    /// Rules with passthrough decision should parse correctly.
    #[test]
    fn parse_rules_passthrough_decision() {
        let toml = r#"
            [[rules]]
            tool = "*"
            decision = "passthrough"
            reason = "Catch-all passthrough"
        "#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.rules.len(), 1);
        assert_eq!(config.rules[0].decision, Decision::Passthrough);
    }
}
