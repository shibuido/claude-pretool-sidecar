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

    /// Additional environment variables.
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
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

impl Config {
    /// Load configuration from the first available source.
    ///
    /// Search order:
    /// 1. `--config <path>` CLI flag (TODO: CLI arg parsing)
    /// 2. `$CLAUDE_PRETOOL_SIDECAR_CONFIG` environment variable
    /// 3. `.claude-pretool-sidecar.toml` in current directory
    /// 4. `~/.config/claude-pretool-sidecar/config.toml` (XDG)
    /// 5. `~/.claude-pretool-sidecar.toml` (home fallback)
    pub fn load() -> Result<Self, ConfigError> {
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
}
