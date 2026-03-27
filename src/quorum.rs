//! # Quorum Logic
//!
//! Implements the vote aggregation algorithm described in
//! `docs/design/voting-quorum.md`.
//!
//! The algorithm:
//! 1. Collect votes from all non-FYI providers
//! 2. Apply error_policy to convert errors
//! 3. If deny_count > max_deny → DENY
//! 4. If allow_count >= min_allow → ALLOW
//! 5. Otherwise → default_decision

use crate::config::QuorumConfig;
use crate::hook::Decision;
use crate::provider::{Vote, WeightedVote};

/// Aggregate provider votes into a single decision using quorum rules.
///
/// Each vote counts as weight 1. For weighted voting, use `aggregate_weighted`.
/// See `docs/design/voting-quorum.md` for the full algorithm specification.
#[allow(dead_code)] // Used in tests; main binary calls aggregate_weighted directly
pub fn aggregate(config: &QuorumConfig, votes: &[Vote]) -> Decision {
    let weighted: Vec<WeightedVote> = votes
        .iter()
        .map(|v| WeightedVote {
            vote: v.clone(),
            weight: 1,
        })
        .collect();
    aggregate_weighted(config, &weighted)
}

/// Aggregate weighted provider votes into a single decision using quorum rules.
///
/// Each vote contributes its weight to the allow/deny/passthrough counts.
/// For example, a provider with weight=2 voting "allow" adds 2 to allow_count.
///
/// See `docs/design/voting-quorum.md` for the full algorithm specification.
pub fn aggregate_weighted(config: &QuorumConfig, votes: &[WeightedVote]) -> Decision {
    let mut allow_count: u32 = 0;
    let mut deny_count: u32 = 0;
    // passthrough_count tracked for completeness but not used in decision
    let mut _passthrough_count: u32 = 0;

    for wv in votes {
        match wv.vote {
            Vote::Allow => allow_count += wv.weight,
            Vote::Deny => deny_count += wv.weight,
            Vote::Passthrough => _passthrough_count += wv.weight,
            Vote::Error => {
                // Apply error_policy: convert error to the configured category
                match config.error_policy {
                    Decision::Allow => allow_count += wv.weight,
                    Decision::Deny => deny_count += wv.weight,
                    Decision::Passthrough => _passthrough_count += wv.weight,
                }
            }
        }
    }

    // Deny takes priority: if deny threshold exceeded, deny regardless
    if deny_count > config.max_deny {
        return Decision::Deny;
    }

    // Check if enough allow votes
    if allow_count >= config.min_allow {
        return Decision::Allow;
    }

    // Quorum not met — return default
    config.default_decision
}

#[cfg(test)]
mod tests {
    use super::*;

    /// # Quorum Logic Tests
    ///
    /// These tests verify the vote aggregation algorithm described in
    /// `docs/design/voting-quorum.md`. Each test corresponds to an example
    /// or edge case from that document.
    fn default_config() -> QuorumConfig {
        QuorumConfig {
            min_allow: 1,
            max_deny: 0,
            error_policy: Decision::Passthrough,
            default_decision: Decision::Passthrough,
        }
    }

    // --- Example 1: Single provider setups ---

    /// When a single provider votes "allow" and quorum requires min_allow=1,
    /// the sidecar should return "allow".
    #[test]
    fn single_provider_allow_returns_allow() {
        let config = default_config();
        let votes = vec![Vote::Allow];
        assert_eq!(aggregate(&config, &votes), Decision::Allow);
    }

    /// When a single provider votes "deny" and max_deny=0,
    /// the sidecar should return "deny".
    #[test]
    fn single_provider_deny_returns_deny() {
        let config = default_config();
        let votes = vec![Vote::Deny];
        assert_eq!(aggregate(&config, &votes), Decision::Deny);
    }

    /// When a single provider votes "passthrough" and min_allow=1,
    /// quorum is not met, so return default_decision (passthrough).
    #[test]
    fn single_provider_passthrough_returns_default() {
        let config = default_config();
        let votes = vec![Vote::Passthrough];
        assert_eq!(aggregate(&config, &votes), Decision::Passthrough);
    }

    // --- Example 2: Multiple providers ---

    /// Two of three providers allow, one passes through.
    /// With min_allow=2, max_deny=0: should allow.
    #[test]
    fn two_of_three_allow_with_one_passthrough_returns_allow() {
        let config = QuorumConfig {
            min_allow: 2,
            max_deny: 0,
            ..default_config()
        };
        let votes = vec![Vote::Allow, Vote::Allow, Vote::Passthrough];
        assert_eq!(aggregate(&config, &votes), Decision::Allow);
    }

    /// Two allow, one deny. With max_deny=0: deny wins.
    #[test]
    fn two_allow_one_deny_with_zero_max_deny_returns_deny() {
        let config = QuorumConfig {
            min_allow: 2,
            max_deny: 0,
            ..default_config()
        };
        let votes = vec![Vote::Allow, Vote::Allow, Vote::Deny];
        assert_eq!(aggregate(&config, &votes), Decision::Deny);
    }

    /// Two allow, one deny. With max_deny=1: deny is tolerated, allow wins.
    #[test]
    fn two_allow_one_deny_with_one_max_deny_returns_allow() {
        let config = QuorumConfig {
            min_allow: 2,
            max_deny: 1,
            ..default_config()
        };
        let votes = vec![Vote::Allow, Vote::Allow, Vote::Deny];
        assert_eq!(aggregate(&config, &votes), Decision::Allow);
    }

    // --- Edge cases ---

    /// No votes at all (all providers are FYI). With min_allow=0,
    /// quorum is trivially met (0 >= 0), so decision is Allow.
    #[test]
    fn zero_votes_with_min_allow_zero_returns_allow() {
        let config = QuorumConfig {
            min_allow: 0,
            default_decision: Decision::Passthrough,
            ..default_config()
        };
        let votes: Vec<Vote> = vec![];
        assert_eq!(aggregate(&config, &votes), Decision::Allow);
    }

    /// No votes at all with min_allow=1 means quorum not met → default_decision.
    #[test]
    fn zero_votes_with_min_allow_one_returns_default() {
        let config = QuorumConfig {
            min_allow: 1,
            default_decision: Decision::Passthrough,
            ..default_config()
        };
        let votes: Vec<Vote> = vec![];
        assert_eq!(aggregate(&config, &votes), Decision::Passthrough);
    }

    /// Provider error with error_policy=deny should count as deny.
    #[test]
    fn error_with_deny_policy_counts_as_deny() {
        let config = QuorumConfig {
            min_allow: 1,
            max_deny: 0,
            error_policy: Decision::Deny,
            ..default_config()
        };
        let votes = vec![Vote::Allow, Vote::Error];
        assert_eq!(aggregate(&config, &votes), Decision::Deny);
    }

    /// Provider error with error_policy=passthrough should not block allow.
    #[test]
    fn error_with_passthrough_policy_does_not_block_allow() {
        let config = QuorumConfig {
            min_allow: 1,
            max_deny: 0,
            error_policy: Decision::Passthrough,
            ..default_config()
        };
        let votes = vec![Vote::Allow, Vote::Error];
        assert_eq!(aggregate(&config, &votes), Decision::Allow);
    }

    /// Provider error with error_policy=allow should count as allow vote.
    #[test]
    fn error_with_allow_policy_counts_as_allow() {
        let config = QuorumConfig {
            min_allow: 2,
            max_deny: 0,
            error_policy: Decision::Allow,
            ..default_config()
        };
        let votes = vec![Vote::Allow, Vote::Error];
        assert_eq!(aggregate(&config, &votes), Decision::Allow);
    }

    /// All providers error with error_policy=passthrough and min_allow=1.
    /// Quorum not met → default_decision.
    #[test]
    fn all_errors_with_passthrough_policy_returns_default() {
        let config = QuorumConfig {
            min_allow: 1,
            max_deny: 0,
            error_policy: Decision::Passthrough,
            default_decision: Decision::Passthrough,
        };
        let votes = vec![Vote::Error, Vote::Error];
        assert_eq!(aggregate(&config, &votes), Decision::Passthrough);
    }

    /// Deny priority: even with enough allows, exceeding max_deny means deny.
    #[test]
    fn deny_takes_priority_over_sufficient_allows() {
        let config = QuorumConfig {
            min_allow: 1,
            max_deny: 0,
            ..default_config()
        };
        let votes = vec![Vote::Allow, Vote::Allow, Vote::Deny];
        assert_eq!(aggregate(&config, &votes), Decision::Deny);
    }

    // --- Weighted voting tests ---

    /// A single provider with weight=2 voting "allow" should meet min_allow=2.
    #[test]
    fn weighted_single_provider_weight2_meets_min_allow_2() {
        let config = QuorumConfig {
            min_allow: 2,
            max_deny: 0,
            ..default_config()
        };
        let votes = vec![WeightedVote {
            vote: Vote::Allow,
            weight: 2,
        }];
        assert_eq!(aggregate_weighted(&config, &votes), Decision::Allow);
    }

    /// A single provider with weight=1 voting "allow" should NOT meet min_allow=2.
    #[test]
    fn weighted_single_provider_weight1_does_not_meet_min_allow_2() {
        let config = QuorumConfig {
            min_allow: 2,
            max_deny: 0,
            ..default_config()
        };
        let votes = vec![WeightedVote {
            vote: Vote::Allow,
            weight: 1,
        }];
        assert_eq!(aggregate_weighted(&config, &votes), Decision::Passthrough);
    }

    /// A deny vote with weight=2 should exceed max_deny=1.
    #[test]
    fn weighted_deny_weight2_exceeds_max_deny_1() {
        let config = QuorumConfig {
            min_allow: 1,
            max_deny: 1,
            ..default_config()
        };
        let votes = vec![
            WeightedVote {
                vote: Vote::Allow,
                weight: 1,
            },
            WeightedVote {
                vote: Vote::Deny,
                weight: 2,
            },
        ];
        assert_eq!(aggregate_weighted(&config, &votes), Decision::Deny);
    }

    /// Mixed weights: weight=2 allow + weight=1 deny with max_deny=1.
    /// Deny count (1) <= max_deny (1), allow count (2) >= min_allow (2) → allow.
    #[test]
    fn weighted_mixed_weights_allow_wins() {
        let config = QuorumConfig {
            min_allow: 2,
            max_deny: 1,
            ..default_config()
        };
        let votes = vec![
            WeightedVote {
                vote: Vote::Allow,
                weight: 2,
            },
            WeightedVote {
                vote: Vote::Deny,
                weight: 1,
            },
        ];
        assert_eq!(aggregate_weighted(&config, &votes), Decision::Allow);
    }

    /// Weighted error with error_policy=deny: error weight=3 should exceed max_deny=2.
    #[test]
    fn weighted_error_with_deny_policy_uses_weight() {
        let config = QuorumConfig {
            min_allow: 1,
            max_deny: 2,
            error_policy: Decision::Deny,
            ..default_config()
        };
        let votes = vec![
            WeightedVote {
                vote: Vote::Allow,
                weight: 1,
            },
            WeightedVote {
                vote: Vote::Error,
                weight: 3,
            },
        ];
        assert_eq!(aggregate_weighted(&config, &votes), Decision::Deny);
    }

    /// Default weight (1) preserves backward compatibility with aggregate().
    #[test]
    fn weighted_default_weight_matches_unweighted() {
        let config = QuorumConfig {
            min_allow: 2,
            max_deny: 0,
            ..default_config()
        };
        let unweighted_votes = vec![Vote::Allow, Vote::Allow, Vote::Passthrough];
        let weighted_votes = vec![
            WeightedVote { vote: Vote::Allow, weight: 1 },
            WeightedVote { vote: Vote::Allow, weight: 1 },
            WeightedVote { vote: Vote::Passthrough, weight: 1 },
        ];
        assert_eq!(
            aggregate(&config, &unweighted_votes),
            aggregate_weighted(&config, &weighted_votes)
        );
    }
}
