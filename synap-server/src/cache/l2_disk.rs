//! L2 Disk Cache
//!
//! Persistent disk-backed cache for overflow from L1 memory cache
//! Uses memory-mapped files for fast access

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use crate::core::error::{Result, SynapError};

/// L2 cache entry metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    key: String,
    offset: u64,
    size: u64,
    timestamp: u64,
    frequency: u32,
}

/// L2 Disk Cache configuration
#[derive(Debug, Clone)]
pub struct L2CacheConfig {
    pub directory: PathBuf,
    pub max_size_mb: usize,
    pub max_entries: usize,
}

impl Default for L2CacheConfig {
    fn default() -> Self {
        Self {
            directory: PathBuf::from("./data/cache/l2"),
            max_size_mb: 1024, // 1GB default
            max_entries: 100_000,
        }
    }
}

/// L2 Disk Cache
pub struct L2DiskCache {
    config: L2CacheConfig,
    index: Arc<RwLock<HashMap<String, CacheEntry>>>,
    data_file: Arc<RwLock<File>>,
    current_offset: Arc<RwLock<u64>>,
    current_size: Arc<RwLock<u64>>,
}

impl L2DiskCache {
    /// Create or open L2 cache
    pub fn new(config: L2CacheConfig) -> Result<Self> {
        // Create cache directory
        fs::create_dir_all(&config.directory)
            .map_err(|e| SynapError::IoError(e.to_string()))?;

        // Open or create data file
        let data_path = config.directory.join("cache.dat");
        let data_file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&data_path)
            .map_err(|e| SynapError::IoError(e.to_string()))?;

        // Load index
        let index = Self::load_index(&config.directory)?;
        
        // Calculate current offset
        let current_offset = data_file
            .metadata()
            .map(|m| m.len())
            .unwrap_or(0);

        Ok(Self {
            config,
            index: Arc::new(RwLock::new(index)),
            data_file: Arc::new(RwLock::new(data_file)),
            current_offset: Arc::new(RwLock::new(current_offset)),
            current_size: Arc::new(RwLock::new(current_offset)),
        })
    }

    /// Load index from disk
    fn load_index(directory: &Path) -> Result<HashMap<String, CacheEntry>> {
        let index_path = directory.join("index.json");
        
        if !index_path.exists() {
            return Ok(HashMap::new());
        }

        let mut file = File::open(&index_path)
            .map_err(|e| SynapError::IoError(e.to_string()))?;
        
        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .map_err(|e| SynapError::IoError(e.to_string()))?;

        serde_json::from_str(&contents)
            .map_err(|e| SynapError::InvalidValue(format!("Failed to parse index: {}", e)))
    }

    /// Save index to disk
    fn save_index(&self) -> Result<()> {
        let index_path = self.config.directory.join("index.json");
        let index = self.index.read();
        
        let json = serde_json::to_string_pretty(&*index)
            .map_err(|e| SynapError::InvalidValue(format!("Failed to serialize index: {}", e)))?;

        fs::write(&index_path, json)
            .map_err(|e| SynapError::IoError(e.to_string()))?;

        Ok(())
    }

    /// Get value from L2 cache
    pub async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        // Look up in index
        let entry = {
            let index = self.index.read();
            index.get(key).cloned()
        };

        match entry {
            Some(entry) => {
                // Read from data file
                let mut file = self.data_file.write();
                file.seek(SeekFrom::Start(entry.offset))
                    .map_err(|e| SynapError::IoError(e.to_string()))?;

                let mut buffer = vec![0u8; entry.size as usize];
                file.read_exact(&mut buffer)
                    .map_err(|e| SynapError::IoError(e.to_string()))?;

                // Update frequency
                let mut index = self.index.write();
                if let Some(e) = index.get_mut(key) {
                    e.frequency += 1;
                }

                Ok(Some(buffer))
            }
            None => Ok(None),
        }
    }

    /// Insert value into L2 cache
    pub async fn insert(&self, key: String, value: Vec<u8>) -> Result<()> {
        let size = value.len() as u64;
        
        // Check if we need to evict
        let current_size = *self.current_size.read();
        let max_size = (self.config.max_size_mb * 1024 * 1024) as u64;
        
        if current_size + size > max_size || self.index.read().len() >= self.config.max_entries {
            self.evict_lfu().await?;
        }

        // Write to data file
        let offset = {
            let mut file = self.data_file.write();
            let offset = *self.current_offset.read();
            
            file.seek(SeekFrom::Start(offset))
                .map_err(|e| SynapError::IoError(e.to_string()))?;
            
            file.write_all(&value)
                .map_err(|e| SynapError::IoError(e.to_string()))?;
            
            file.flush()
                .map_err(|e| SynapError::IoError(e.to_string()))?;

            offset
        };

        // Update offset and size
        *self.current_offset.write() += size;
        *self.current_size.write() += size;

        // Update index
        let entry = CacheEntry {
            key: key.clone(),
            offset,
            size,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            frequency: 1,
        };

        self.index.write().insert(key, entry);

        // Save index periodically (every 100 inserts)
        if self.index.read().len() % 100 == 0 {
            self.save_index()?;
        }

        Ok(())
    }

    /// Evict least frequently used entry
    async fn evict_lfu(&self) -> Result<()> {
        let evict_key = {
            let index = self.index.read();
            
            index
                .iter()
                .min_by_key(|(_, entry)| entry.frequency)
                .map(|(key, _)| key.clone())
        };

        if let Some(key) = evict_key {
            let entry = self.index.write().remove(&key);
            
            if let Some(entry) = entry {
                let current_size = *self.current_size.read();
                *self.current_size.write() = current_size.saturating_sub(entry.size);
            }
        }

        Ok(())
    }

    /// Clear all cache entries
    pub async fn clear(&self) -> Result<()> {
        self.index.write().clear();
        *self.current_offset.write() = 0;
        *self.current_size.write() = 0;
        
        // Truncate data file
        let mut file = self.data_file.write();
        file.set_len(0)
            .map_err(|e| SynapError::IoError(e.to_string()))?;
        
        self.save_index()?;
        
        Ok(())
    }

    /// Get cache statistics
    pub fn stats(&self) -> L2CacheStats {
        let index = self.index.read();
        let current_size = *self.current_size.read();
        
        L2CacheStats {
            entries: index.len(),
            size_bytes: current_size,
            size_mb: current_size as f64 / (1024.0 * 1024.0),
            capacity_mb: self.config.max_size_mb,
            utilization: (current_size as f64 / (self.config.max_size_mb * 1024 * 1024) as f64) * 100.0,
        }
    }
}

/// L2 cache statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L2CacheStats {
    pub entries: usize,
    pub size_bytes: u64,
    pub size_mb: f64,
    pub capacity_mb: usize,
    pub utilization: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_l2_cache_basic() {
        let dir = tempdir().unwrap();
        let config = L2CacheConfig {
            directory: dir.path().to_path_buf(),
            max_size_mb: 10,
            max_entries: 1000,
        };

        let cache = L2DiskCache::new(config).unwrap();
        
        // Insert
        cache.insert("key1".to_string(), b"value1".to_vec()).await.unwrap();
        cache.insert("key2".to_string(), b"value2".to_vec()).await.unwrap();
        
        // Get
        let val1 = cache.get("key1").await.unwrap();
        assert_eq!(val1, Some(b"value1".to_vec()));
        
        let val2 = cache.get("key2").await.unwrap();
        assert_eq!(val2, Some(b"value2".to_vec()));
        
        // Stats
        let stats = cache.stats();
        assert_eq!(stats.entries, 2);
    }

    #[tokio::test]
    async fn test_l2_cache_eviction() {
        let dir = tempdir().unwrap();
        let config = L2CacheConfig {
            directory: dir.path().to_path_buf(),
            max_size_mb: 1, // Very small for testing
            max_entries: 3,
        };

        let cache = L2DiskCache::new(config).unwrap();
        
        // Fill cache
        cache.insert("a".to_string(), vec![0u8; 1024]).await.unwrap();
        cache.insert("b".to_string(), vec![0u8; 1024]).await.unwrap();
        cache.insert("c".to_string(), vec![0u8; 1024]).await.unwrap();
        
        assert_eq!(cache.stats().entries, 3);
    }
}

