//! # Rule-Based Auto-Approval Engine
//!
//! A built-in rules engine that matches tool_name + tool_input against
//! user-configured patterns and returns allow/deny/passthrough without
//! needing external scripts. Serves as a lightweight fast-path shortcut
//! for common policies.
//!
//! ## How it works
//!
//! 1. Rules are evaluated in order — first match wins
//! 2. `tool_pattern` is a regex matched against `tool_name`
//! 3. `input_pattern` (optional) is a regex matched against the
//!    JSON-serialized `tool_input`
//! 4. If no rule matches, returns `None` (engine has no opinion)
//!
//! ## Configuration
//!
//! ```toml
//! [[rules]]
//! tool = "Bash"
//! input = "^ls |^pwd$"
//! decision = "allow"
//! reason = "Safe read-only commands"
//! ```

use crate::config::RuleConfig;
use crate::hook::Decision;
use regex::Regex;

/// A compiled rule ready for matching.
#[derive(Debug)]
struct CompiledRule {
    tool_regex: Regex,
    input_regex: Option<Regex>,
    decision: Decision,
    reason: Option<String>,
}

/// The rules engine: evaluates tool calls against configured patterns.
#[derive(Debug)]
pub struct RulesEngine {
    rules: Vec<CompiledRule>,
}

/// Error from compiling rule patterns.
#[derive(Debug)]
pub struct RuleCompileError {
    pub rule_index: usize,
    pub field: String,
    pub message: String,
}

impl std::fmt::Display for RuleCompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "rule[{}] {}: {}",
            self.rule_index, self.field, self.message
        )
    }
}

impl RulesEngine {
    /// Compile rule configurations into a rules engine.
    ///
    /// Returns an error if any regex pattern fails to compile.
    /// An empty rules list produces an engine that always returns None.
    pub fn new(configs: &[RuleConfig]) -> Result<Self, RuleCompileError> {
        let mut rules = Vec::with_capacity(configs.len());

        for (i, cfg) in configs.iter().enumerate() {
            // Compile tool pattern — anchor to full match
            let tool_pattern = if cfg.tool == "*" {
                ".*".to_string()
            } else {
                format!("^(?:{})$", cfg.tool)
            };

            let tool_regex = Regex::new(&tool_pattern).map_err(|e| RuleCompileError {
                rule_index: i,
                field: "tool".to_string(),
                message: e.to_string(),
            })?;

            // Compile optional input pattern (searched, not anchored)
            let input_regex = match &cfg.input {
                Some(pattern) => {
                    let re = Regex::new(pattern).map_err(|e| RuleCompileError {
                        rule_index: i,
                        field: "input".to_string(),
                        message: e.to_string(),
                    })?;
                    Some(re)
                }
                None => None,
            };

            rules.push(CompiledRule {
                tool_regex,
                input_regex,
                decision: cfg.decision,
                reason: cfg.reason.clone(),
            });
        }

        Ok(Self { rules })
    }

    /// Evaluate a tool call against the rules.
    ///
    /// Returns the decision and optional reason from the first matching rule,
    /// or None if no rule matches.
    pub fn evaluate(
        &self,
        tool_name: &str,
        tool_input: &serde_json::Value,
    ) -> Option<(Decision, Option<String>)> {
        for rule in &self.rules {
            // Check tool_name against tool pattern
            if !rule.tool_regex.is_match(tool_name) {
                continue;
            }

            // Check tool_input against input pattern (if specified)
            if let Some(ref input_regex) = rule.input_regex {
                let input_str = serde_json::to_string(tool_input).unwrap_or_default();
                if !input_regex.is_match(&input_str) {
                    continue;
                }
            }

            // Match found — return decision
            return Some((rule.decision, rule.reason.clone()));
        }

        // No rule matched
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RuleConfig;
    use serde_json::json;

    /// Helper: create a RuleConfig for testing.
    fn rule(tool: &str, input: Option<&str>, decision: Decision, reason: Option<&str>) -> RuleConfig {
        RuleConfig {
            tool: tool.to_string(),
            input: input.map(|s| s.to_string()),
            decision,
            reason: reason.map(|s| s.to_string()),
        }
    }

    /// Rule matching "Bash" should match the Bash tool.
    #[test]
    fn exact_tool_match() {
        let engine = RulesEngine::new(&[rule("Bash", None, Decision::Allow, None)]).unwrap();
        let result = engine.evaluate("Bash", &json!({}));
        assert!(result.is_some());
        assert_eq!(result.unwrap().0, Decision::Allow);
    }

    /// Rule matching "Bash" should NOT match "Write".
    #[test]
    fn exact_tool_no_match() {
        let engine = RulesEngine::new(&[rule("Bash", None, Decision::Allow, None)]).unwrap();
        let result = engine.evaluate("Write", &json!({}));
        assert!(result.is_none());
    }

    /// Rule matching "Write|Edit" should match both Write and Edit tools.
    #[test]
    fn regex_tool_match() {
        let engine = RulesEngine::new(&[rule("Write|Edit", None, Decision::Deny, None)]).unwrap();

        let write_result = engine.evaluate("Write", &json!({}));
        assert!(write_result.is_some());
        assert_eq!(write_result.unwrap().0, Decision::Deny);

        let edit_result = engine.evaluate("Edit", &json!({}));
        assert!(edit_result.is_some());
        assert_eq!(edit_result.unwrap().0, Decision::Deny);

        // Should not match Bash
        let bash_result = engine.evaluate("Bash", &json!({}));
        assert!(bash_result.is_none());
    }

    /// Rule matching "*" should match everything.
    #[test]
    fn wildcard_tool_match() {
        let engine =
            RulesEngine::new(&[rule("*", None, Decision::Passthrough, Some("catch-all"))]).unwrap();

        let result = engine.evaluate("Bash", &json!({}));
        assert!(result.is_some());
        let (decision, reason) = result.unwrap();
        assert_eq!(decision, Decision::Passthrough);
        assert_eq!(reason, Some("catch-all".to_string()));

        let result2 = engine.evaluate("Write", &json!({}));
        assert!(result2.is_some());
    }

    /// Rule with input regex should match against serialized tool_input.
    #[test]
    fn input_pattern_match() {
        let engine = RulesEngine::new(&[rule(
            "Bash",
            Some("rm -rf"),
            Decision::Deny,
            Some("Dangerous command"),
        )])
        .unwrap();

        // Should match when tool_input contains "rm -rf"
        let result = engine.evaluate("Bash", &json!({"command": "rm -rf /"}));
        assert!(result.is_some());
        assert_eq!(result.unwrap().0, Decision::Deny);

        // Should NOT match when tool_input is safe
        let result2 = engine.evaluate("Bash", &json!({"command": "ls -la"}));
        assert!(result2.is_none());
    }

    /// Earlier rules should take priority over later ones (first match wins).
    #[test]
    fn first_match_wins() {
        let engine = RulesEngine::new(&[
            rule("Bash", Some("^.*ls.*$"), Decision::Allow, Some("ls is safe")),
            rule("Bash", None, Decision::Deny, Some("deny all bash")),
        ])
        .unwrap();

        // "ls" command should match the first rule (allow)
        let result = engine.evaluate("Bash", &json!({"command": "ls -la"}));
        assert!(result.is_some());
        let (decision, reason) = result.unwrap();
        assert_eq!(decision, Decision::Allow);
        assert_eq!(reason, Some("ls is safe".to_string()));

        // Other bash commands should match the second rule (deny)
        let result2 = engine.evaluate("Bash", &json!({"command": "cat /etc/passwd"}));
        assert!(result2.is_some());
        let (decision2, reason2) = result2.unwrap();
        assert_eq!(decision2, Decision::Deny);
        assert_eq!(reason2, Some("deny all bash".to_string()));
    }

    /// When no rules match, the engine should return None.
    #[test]
    fn no_match_returns_none() {
        let engine = RulesEngine::new(&[rule("Write", None, Decision::Deny, None)]).unwrap();

        let result = engine.evaluate("Bash", &json!({"command": "ls"}));
        assert!(result.is_none());
    }

    /// An empty rules engine should always return None.
    #[test]
    fn empty_engine_returns_none() {
        let engine = RulesEngine::new(&[]).unwrap();
        let result = engine.evaluate("Bash", &json!({}));
        assert!(result.is_none());
    }

    /// A deny rule should return Decision::Deny.
    #[test]
    fn deny_rule_blocks() {
        let engine = RulesEngine::new(&[rule(
            "Bash",
            Some("rm -rf"),
            Decision::Deny,
            Some("Blocked"),
        )])
        .unwrap();

        let result = engine.evaluate("Bash", &json!({"command": "rm -rf /tmp/data"}));
        assert!(result.is_some());
        let (decision, reason) = result.unwrap();
        assert_eq!(decision, Decision::Deny);
        assert_eq!(reason, Some("Blocked".to_string()));
    }

    /// An allow rule should return Decision::Allow.
    #[test]
    fn allow_rule_permits() {
        let engine = RulesEngine::new(&[rule(
            "Bash",
            Some("^.*pwd.*$"),
            Decision::Allow,
            Some("Safe command"),
        )])
        .unwrap();

        let result = engine.evaluate("Bash", &json!({"command": "pwd"}));
        assert!(result.is_some());
        let (decision, reason) = result.unwrap();
        assert_eq!(decision, Decision::Allow);
        assert_eq!(reason, Some("Safe command".to_string()));
    }

    /// Invalid regex in tool pattern should produce a compile error.
    #[test]
    fn invalid_tool_regex_errors() {
        let result = RulesEngine::new(&[rule("[invalid", None, Decision::Allow, None)]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.rule_index, 0);
        assert_eq!(err.field, "tool");
    }

    /// Invalid regex in input pattern should produce a compile error.
    #[test]
    fn invalid_input_regex_errors() {
        let result = RulesEngine::new(&[rule("Bash", Some("[invalid"), Decision::Allow, None)]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.rule_index, 0);
        assert_eq!(err.field, "input");
    }

    /// Input pattern matching against .env files in Write tool_input.
    #[test]
    fn input_pattern_matches_sensitive_files() {
        let engine = RulesEngine::new(&[rule(
            "Write|Edit",
            Some(r"\.env"),
            Decision::Deny,
            Some("Sensitive file"),
        )])
        .unwrap();

        let result = engine.evaluate("Write", &json!({"file_path": "/home/user/.env", "content": "SECRET=abc"}));
        assert!(result.is_some());
        assert_eq!(result.unwrap().0, Decision::Deny);

        // Should not match normal files
        let result2 = engine.evaluate("Write", &json!({"file_path": "/tmp/test.txt", "content": "hello"}));
        assert!(result2.is_none());
    }
}
