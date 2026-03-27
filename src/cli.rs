//! # CLI Argument Parsing
//!
//! Defines the command-line interface for the sidecar binary using `clap`.
//! See `docs/design/configuration.md` for flag specifications.

use clap::Parser;
use std::path::PathBuf;

/// A composable sidecar for Claude Code's PreToolUse hook that aggregates
/// tool-approval votes from multiple external providers.
#[derive(Debug, Parser)]
#[command(name = "claude-pretool-sidecar", version, about)]
pub struct Cli {
    /// Explicit config file path (highest priority, overrides env var and file discovery).
    #[arg(long, value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// Validate config and exit (checks parsing, provider commands, quorum values).
    #[arg(long)]
    pub validate: bool,

    /// PostToolUse mode: audit-log the event, skip provider voting, output `{}` passthrough.
    #[arg(long)]
    pub post_tool: bool,

    /// When no config file is found, return passthrough instead of erroring.
    /// Useful for initial setup before a config file exists.
    #[arg(long)]
    pub passthrough: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// # CLI Parsing Tests
    ///
    /// These tests verify that command-line arguments are correctly parsed
    /// into the Cli struct with proper defaults.
    /// No arguments should parse with all defaults.
    #[test]
    fn no_args_parses_with_defaults() {
        let cli = Cli::parse_from(["claude-pretool-sidecar"]);
        assert!(cli.config.is_none());
        assert!(!cli.validate);
        assert!(!cli.post_tool);
        assert!(!cli.passthrough);
    }

    /// --config should accept a path.
    #[test]
    fn config_flag_accepts_path() {
        let cli = Cli::parse_from(["claude-pretool-sidecar", "--config", "/tmp/my-config.toml"]);
        assert_eq!(cli.config, Some(PathBuf::from("/tmp/my-config.toml")));
    }

    /// --validate flag should set validate to true.
    #[test]
    fn validate_flag_sets_true() {
        let cli = Cli::parse_from(["claude-pretool-sidecar", "--validate"]);
        assert!(cli.validate);
    }

    /// --post-tool flag should set post_tool to true.
    #[test]
    fn post_tool_flag_sets_true() {
        let cli = Cli::parse_from(["claude-pretool-sidecar", "--post-tool"]);
        assert!(cli.post_tool);
    }

    /// --passthrough flag should set passthrough to true.
    #[test]
    fn passthrough_flag_sets_true() {
        let cli = Cli::parse_from(["claude-pretool-sidecar", "--passthrough"]);
        assert!(cli.passthrough);
    }

    /// Multiple flags can be combined.
    #[test]
    fn multiple_flags_combine() {
        let cli = Cli::parse_from([
            "claude-pretool-sidecar",
            "--config",
            "/etc/sidecar.toml",
            "--passthrough",
            "--post-tool",
        ]);
        assert_eq!(cli.config, Some(PathBuf::from("/etc/sidecar.toml")));
        assert!(cli.passthrough);
        assert!(cli.post_tool);
        assert!(!cli.validate);
    }

    /// --version flag is handled by clap automatically (exits with version info).
    /// We just verify the version string is set from Cargo.toml.
    #[test]
    fn version_string_matches_cargo() {
        let cmd = <Cli as clap::CommandFactory>::command();
        let version = cmd.get_version().expect("version should be set");
        assert_eq!(version, env!("CARGO_PKG_VERSION"));
    }
}
