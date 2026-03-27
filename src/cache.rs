//! # Provider Decision Caching
//!
//! File-based cache that avoids redundant provider invocations when the same
//! (tool_name, tool_input) combination is seen within a configurable TTL.
//!
//! Cache files are scoped per Claude Code session and stored in /tmp so the OS
//! handles cleanup. Format: `/tmp/cpts-cache-{session_id}.json`

use crate::config::CacheConfig;
use crate::hook::Decision;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// A cached decision with the timestamp it was stored.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    decision: Decision,
    /// Unix timestamp in seconds when this entry was stored.
    stored_at: u64,
}

/// On-disk cache mapping hash keys to cached decisions.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CacheFile {
    entries: HashMap<String, CacheEntry>,
}

/// File-based decision cache scoped to a Claude Code session.
pub struct DecisionCache {
    config: CacheConfig,
    path: PathBuf,
}

impl DecisionCache {
    /// Create a new cache for the given session.
    ///
    /// `session_id` is used to scope the cache file; pass `None` to use a
    /// default scope.
    pub fn new(config: &CacheConfig, session_id: Option<&str>) -> Self {
        let sid = session_id.unwrap_or("default");
        let path = PathBuf::from(format!("/tmp/cpts-cache-{sid}.json"));
        Self {
            config: config.clone(),
            path,
        }
    }

    /// Look up a cached decision for the given tool call.
    ///
    /// Returns `Some(Decision)` if the cache is enabled, a matching entry
    /// exists, and the entry has not expired.
    pub fn get(&self, tool_name: &str, tool_input: &serde_json::Value) -> Option<Decision> {
        if !self.config.enabled {
            return None;
        }

        let key = Self::cache_key(tool_name, tool_input);
        let cache_file = self.load_file()?;
        let entry = cache_file.entries.get(&key)?;

        let now = now_unix_secs();
        if now.saturating_sub(entry.stored_at) > self.config.ttl_seconds {
            return None;
        }

        Some(entry.decision)
    }

    /// Store a decision in the cache.
    pub fn put(&self, tool_name: &str, tool_input: &serde_json::Value, decision: Decision) {
        if !self.config.enabled {
            return;
        }

        let key = Self::cache_key(tool_name, tool_input);
        let mut cache_file = self.load_file().unwrap_or_default();

        // Evict expired entries while we're here
        let now = now_unix_secs();
        let ttl = self.config.ttl_seconds;
        cache_file
            .entries
            .retain(|_, e| now.saturating_sub(e.stored_at) <= ttl);

        cache_file.entries.insert(
            key,
            CacheEntry {
                decision,
                stored_at: now,
            },
        );

        self.save_file(&cache_file);
    }

    /// Flush the entire cache.
    #[allow(dead_code)] // Public API, not called from main binary
    pub fn clear(&self) {
        let _ = std::fs::remove_file(&self.path);
    }

    /// Compute a deterministic cache key from tool name and canonical input JSON.
    fn cache_key(tool_name: &str, tool_input: &serde_json::Value) -> String {
        // Canonical JSON (serde_json sorts nothing, but Value is deterministic
        // for the same parsed input).
        let canonical = serde_json::to_string(tool_input).unwrap_or_default();
        let mut hasher = DefaultHasher::new();
        tool_name.hash(&mut hasher);
        canonical.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }

    /// Load the cache file from disk, returning `None` if it doesn't exist or
    /// is malformed.
    fn load_file(&self) -> Option<CacheFile> {
        let data = std::fs::read_to_string(&self.path).ok()?;
        serde_json::from_str(&data).ok()
    }

    /// Persist the cache file to disk. Errors are silently ignored (cache is
    /// best-effort).
    fn save_file(&self, cache_file: &CacheFile) {
        if let Ok(data) = serde_json::to_string(cache_file) {
            let _ = std::fs::write(&self.path, data);
        }
    }
}

/// Current time as Unix seconds.
fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Helper: create a CacheConfig with short TTL for testing.
    fn test_config(enabled: bool, ttl: u64) -> CacheConfig {
        CacheConfig {
            enabled,
            ttl_seconds: ttl,
        }
    }

    /// Helper: create a DecisionCache pointing at a temp file.
    fn test_cache(config: &CacheConfig) -> DecisionCache {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("test-cache.json");
        DecisionCache {
            config: config.clone(),
            // Use the temp path instead of /tmp/cpts-cache-...
            // We leak the TempDir so the file stays around for the test.
            path: {
                // Keep dir alive by leaking it
                let p = path.clone();
                std::mem::forget(dir);
                p
            },
        }
    }

    /// put then get returns the same decision.
    #[test]
    fn cache_stores_and_retrieves() {
        let config = test_config(true, 60);
        let cache = test_cache(&config);

        let input = json!({"command": "ls -la"});
        cache.put("Bash", &input, Decision::Allow);

        let result = cache.get("Bash", &input);
        assert_eq!(result, Some(Decision::Allow));

        cache.clear();
    }

    /// Entry older than TTL is not returned.
    #[test]
    fn cache_expires_after_ttl() {
        let config = test_config(true, 60);
        let cache = test_cache(&config);

        let input = json!({"command": "ls"});

        // Manually write an entry with a timestamp far in the past
        let key = DecisionCache::cache_key("Bash", &input);
        let mut cf = CacheFile::default();
        cf.entries.insert(
            key,
            CacheEntry {
                decision: Decision::Allow,
                stored_at: 1000, // way in the past
            },
        );
        cache.save_file(&cf);

        let result = cache.get("Bash", &input);
        assert_eq!(result, None, "expired entry should not be returned");

        cache.clear();
    }

    /// Same tool name but different inputs produce different cache keys.
    #[test]
    fn cache_different_inputs_different_entries() {
        let config = test_config(true, 60);
        let cache = test_cache(&config);

        let input_a = json!({"command": "ls"});
        let input_b = json!({"command": "rm -rf /"});

        cache.put("Bash", &input_a, Decision::Allow);
        cache.put("Bash", &input_b, Decision::Deny);

        assert_eq!(cache.get("Bash", &input_a), Some(Decision::Allow));
        assert_eq!(cache.get("Bash", &input_b), Some(Decision::Deny));

        cache.clear();
    }

    /// When cache is disabled, get always returns None and put is a no-op.
    #[test]
    fn cache_disabled_returns_none() {
        let config = test_config(false, 60);
        let cache = test_cache(&config);

        let input = json!({"command": "ls"});
        cache.put("Bash", &input, Decision::Allow);

        assert_eq!(cache.get("Bash", &input), None);

        cache.clear();
    }
}
