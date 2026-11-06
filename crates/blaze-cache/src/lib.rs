//! Bounded, content-addressed cache for re-creatable browser resources.
//!
//! Do not put notes, credentials, cookies, or other user-authored data in this
//! cache. Those belong in encrypted persistent storage.

use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CacheError {
    #[error("cache I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("item size {size} exceeds cache quota {limit}")]
    TooLarge { size: u64, limit: u64 },
    #[error("cache key must be a 64-character lowercase SHA-256 hex string")]
    InvalidKey,
}

#[derive(Clone, Debug)]
struct CacheEntry {
    size: u64,
    last_access: SystemTime,
}

/// Content-addressed cache with disk deduplication and a strict byte ceiling.
pub struct ContentCache {
    root: PathBuf,
    max_bytes: u64,
    total_bytes: u64,
    entries: HashMap<String, CacheEntry>,
}

impl ContentCache {
    /// Opens an existing cache directory and reconstructs its lightweight
    /// index from the on-disk shard directories.
    pub fn open(root: impl Into<PathBuf>, max_bytes: u64) -> Result<Self, CacheError> {
        let root = root.into();
        fs::create_dir_all(&root)?;
        let mut cache = Self {
            root,
            max_bytes,
            total_bytes: 0,
            entries: HashMap::new(),
        };
        cache.scan_existing()?;
        cache.evict_until_within_quota()?;
        Ok(cache)
    }

    /// Stores bytes by SHA-256. Identical content is stored only once.
    pub fn put(&mut self, bytes: &[u8]) -> Result<String, CacheError> {
        let size = bytes.len() as u64;
        if size > self.max_bytes {
            return Err(CacheError::TooLarge {
                size,
                limit: self.max_bytes,
            });
        }

        let key = sha256_hex(bytes);
        let path = self.path_for_key(&key)?;
        let now = SystemTime::now();

        if !path.exists() {
            let parent = path.parent().expect("sharded cache paths have a parent");
            fs::create_dir_all(parent)?;
            let temp_path = path.with_extension(format!("tmp-{}", std::process::id()));
            fs::write(&temp_path, bytes)?;
            if let Err(error) = fs::rename(&temp_path, &path) {
                let _ = fs::remove_file(&temp_path);
                return Err(CacheError::Io(error));
            }
            self.total_bytes = self.total_bytes.saturating_add(size);
        }

        self.entries.insert(
            key.clone(),
            CacheEntry {
                size,
                last_access: now,
            },
        );
        self.evict_until_within_quota()?;
        Ok(key)
    }

    /// Reads a cached object and refreshes its last-access timestamp.
    pub fn get(&mut self, key: &str) -> Result<Option<Vec<u8>>, CacheError> {
        let path = self.path_for_key(key)?;
        if !self.entries.contains_key(key) || !path.is_file() {
            return Ok(None);
        }

        let bytes = fs::read(&path)?;
        let now = SystemTime::now();
        if let Some(entry) = self.entries.get_mut(key) {
            entry.last_access = now;
        }
        let file = fs::OpenOptions::new().write(true).open(&path)?;
        file.set_modified(now)?;
        Ok(Some(bytes))
    }

    pub fn total_bytes(&self) -> u64 {
        self.total_bytes
    }

    pub fn max_bytes(&self) -> u64 {
        self.max_bytes
    }

    pub fn item_count(&self) -> usize {
        self.entries.len()
    }

    pub fn clear(&mut self) -> Result<(), CacheError> {
        if self.root.exists() {
            fs::remove_dir_all(&self.root)?;
        }
        fs::create_dir_all(&self.root)?;
        self.entries.clear();
        self.total_bytes = 0;
        Ok(())
    }

    fn scan_existing(&mut self) -> Result<(), CacheError> {
        for shard in fs::read_dir(&self.root)? {
            let shard = shard?;
            if !shard.file_type()?.is_dir() {
                continue;
            }
            let shard_name = shard.file_name().to_string_lossy().into_owned();
            if shard_name.len() != 2 || !is_lower_hex(&shard_name) {
                continue;
            }
            for item in fs::read_dir(shard.path())? {
                let item = item?;
                if !item.file_type()?.is_file() {
                    continue;
                }
                let key = format!("{}{}", shard_name, item.file_name().to_string_lossy());
                if key.len() != 64 || !is_lower_hex(&key) {
                    continue;
                }
                let metadata = item.metadata()?;
                let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                let size = metadata.len();
                self.total_bytes = self.total_bytes.saturating_add(size);
                self.entries.insert(
                    key,
                    CacheEntry {
                        size,
                        last_access: modified,
                    },
                );
            }
        }
        Ok(())
    }

    fn path_for_key(&self, key: &str) -> Result<PathBuf, CacheError> {
        if key.len() != 64 || !is_lower_hex(key) {
            return Err(CacheError::InvalidKey);
        }
        Ok(self.root.join(&key[..2]).join(&key[2..]))
    }

    fn evict_until_within_quota(&mut self) -> Result<(), CacheError> {
        while self.total_bytes > self.max_bytes {
            let Some(oldest_key) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_access)
                .map(|(key, _)| key.clone())
            else {
                break;
            };

            let path = self.path_for_key(&oldest_key)?;
            if path.exists() {
                fs::remove_file(path)?;
            }
            if let Some(entry) = self.entries.remove(&oldest_key) {
                self.total_bytes = self.total_bytes.saturating_sub(entry.size);
            }
        }
        Ok(())
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

fn is_lower_hex(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_DIR: AtomicU64 = AtomicU64::new(1);

    fn test_dir() -> PathBuf {
        std::env::temp_dir().join(format!(
            "blaze-cache-test-{}-{}",
            std::process::id(),
            NEXT_DIR.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn identical_content_is_deduplicated() {
        let root = test_dir();
        let mut cache = ContentCache::open(&root, 1024).unwrap();
        let first = cache.put(b"same bytes").unwrap();
        let second = cache.put(b"same bytes").unwrap();
        assert_eq!(first, second);
        assert_eq!(cache.item_count(), 1);
        assert_eq!(cache.total_bytes(), 10);
        assert_eq!(cache.get(&first).unwrap().unwrap(), b"same bytes");
        cache.clear().unwrap();
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn cache_enforces_quota() {
        let root = test_dir();
        let mut cache = ContentCache::open(&root, 4).unwrap();
        let first = cache.put(b"abc").unwrap();
        let _second = cache.put(b"de").unwrap();
        assert!(cache.total_bytes() <= 4);
        assert!(cache.get(&first).unwrap().is_none());
        cache.clear().unwrap();
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_oversized_items_and_invalid_keys() {
        let root = test_dir();
        let mut cache = ContentCache::open(&root, 2).unwrap();
        assert!(matches!(cache.put(b"too big"), Err(CacheError::TooLarge { .. })));
        assert!(matches!(cache.get("../secret"), Err(CacheError::InvalidKey)));
        cache.clear().unwrap();
        let _ = fs::remove_dir_all(root);
    }
}
