//! API key pool with round-robin rotation and dead-key handling.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// A pool of API keys with round-robin selection and failure tracking.
pub struct KeyPool {
    keys: Vec<KeyEntry>,
    index: AtomicUsize,
}

struct KeyEntry {
    key: String,
    dead: AtomicBool,
}

impl KeyPool {
    /// Create a new key pool from a list of API keys.
    ///
    /// # Panics
    /// Panics if `keys` is empty.
    pub fn new(keys: Vec<String>) -> Self {
        assert!(!keys.is_empty(), "KeyPool requires at least one key");
        Self {
            keys: keys
                .into_iter()
                .map(|key| KeyEntry {
                    key,
                    dead: AtomicBool::new(false),
                })
                .collect(),
            index: AtomicUsize::new(0),
        }
    }

    /// Get the next live API key via round-robin.
    ///
    /// Returns `None` if all keys are dead.
    pub fn next_key(&self) -> Option<&str> {
        let len = self.keys.len();
        let start = self.index.fetch_add(1, Ordering::Relaxed) % len;
        for offset in 0..len {
            let i = (start + offset) % len;
            if !self.keys[i].dead.load(Ordering::Relaxed) {
                return Some(&self.keys[i].key);
            }
        }
        None
    }

    /// Mark a key as dead (e.g. after 401/403).
    pub fn mark_dead(&self, key: &str) {
        for entry in &self.keys {
            if entry.key == key {
                entry.dead.store(true, Ordering::Relaxed);
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_round_robin() {
        let pool = KeyPool::new(vec!["a".into(), "b".into(), "c".into()]);
        let k1 = pool.next_key().unwrap();
        let k2 = pool.next_key().unwrap();
        let k3 = pool.next_key().unwrap();
        // Should cycle through keys
        assert_ne!(k1, k2);
        assert_ne!(k2, k3);
    }

    #[test]
    fn test_dead_key_skip() {
        let pool = KeyPool::new(vec!["a".into(), "b".into()]);
        pool.mark_dead("a");
        assert_eq!(pool.next_key(), Some("b"));
        assert_eq!(pool.next_key(), Some("b"));
    }

    #[test]
    fn test_all_dead() {
        let pool = KeyPool::new(vec!["a".into()]);
        pool.mark_dead("a");
        assert_eq!(pool.next_key(), None);
    }
}
