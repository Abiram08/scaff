use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};

const CACHE_TTL_SECS: u64 = 7 * 24 * 3600;

#[derive(Debug, Serialize, Deserialize)]
struct CacheEntry {
    created_at: u64,
    model: String,
    response: String,
}

pub struct Cache {
    cache_dir: PathBuf,
}

impl Cache {
    pub fn new() -> Self {
        let base = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."));
        let cache_dir = base.join(".scaff").join("cache");
        let _ = fs::create_dir_all(&cache_dir);
        Self { cache_dir }
    }

    fn key(description: &str, model: &str) -> String {
        let mut hasher = Md5::new();
        hasher.update(description.as_bytes());
        hasher.update(b":");
        hasher.update(model.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    fn path(&self, description: &str, model: &str) -> PathBuf {
        self.cache_dir.join(Self::key(description, model))
    }

    pub fn get(&self, description: &str, model: &str) -> Option<String> {
        let path = self.path(description, model);
        if !path.exists() {
            return None;
        }
        match fs::read_to_string(&path) {
            Ok(content) => {
                match serde_json::from_str::<CacheEntry>(&content) {
                    Ok(entry) => {
                        let now = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        if entry.model == model && now - entry.created_at < CACHE_TTL_SECS {
                            Some(entry.response)
                        } else {
                            let _ = fs::remove_file(&path);
                            None
                        }
                    }
                    Err(_) => None,
                }
            }
            Err(_) => None,
        }
    }

    pub fn set(&self, description: &str, model: &str, response: &str) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let entry = CacheEntry {
            created_at: now,
            model: model.to_string(),
            response: response.to_string(),
        };
        if let Ok(content) = serde_json::to_string(&entry) {
            let _ = fs::write(self.path(description, model), content);
        }
    }

    pub fn status(&self) -> CacheStatus {
        let mut entries = 0u64;
        let mut size_bytes = 0u64;
        let mut oldest_hours = 0f64;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if let Ok(read_dir) = fs::read_dir(&self.cache_dir) {
            for entry in read_dir.flatten() {
                if entry.path().is_file() {
                    entries += 1;
                    if let Ok(meta) = entry.metadata() {
                        size_bytes += meta.len();
                    }
                    if let Ok(content) = fs::read_to_string(entry.path()) {
                        if let Ok(cache_entry) = serde_json::from_str::<CacheEntry>(&content) {
                            let age_secs = now - cache_entry.created_at;
                            let age_hours = age_secs as f64 / 3600.0;
                            if age_hours > oldest_hours {
                                oldest_hours = age_hours;
                            }
                        }
                    }
                }
            }
        }

        CacheStatus {
            entries,
            size_bytes,
            oldest_hours,
        }
    }

    pub fn clear(&self) -> u64 {
        let mut count = 0u64;
        if let Ok(read_dir) = fs::read_dir(&self.cache_dir) {
            for entry in read_dir.flatten() {
                if entry.path().is_file() {
                    let _ = fs::remove_file(entry.path());
                    count += 1;
                }
            }
        }
        count
    }
}

#[derive(Debug)]
pub struct CacheStatus {
    pub entries: u64,
    pub size_bytes: u64,
    pub oldest_hours: f64,
}
