//! RocksDB-backed persistent block storage
//!
//! Provides CID-indexed block storage with BLAKE3 verification,
//! persistent storage via RocksDB, and optimized configuration
//! for content-addressed blocks (1KB - 10MB+).

use cid::Cid;
use rocksdb::{Options, WriteBatch, DB};
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, info};

use crate::cid_blake3::{blake3_cid, verify_blake3, CidError};

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("Block not found: {0}")]
    BlockNotFound(String),

    #[error("CID verification failed: {0}")]
    VerificationFailed(#[from] CidError),

    #[error("Block already exists: {0}")]
    BlockExists(String),

    #[error("Database error: {0}")]
    DatabaseError(#[from] rocksdb::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// A block with its CID and data
#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub cid: Cid,
    pub data: Vec<u8>,
}

impl Block {
    /// Create a new block from data, computing its CID
    pub fn new(data: Vec<u8>) -> Result<Self, CidError> {
        let cid = blake3_cid(&data)?;
        Ok(Self { cid, data })
    }

    /// Create a block from data and verify it matches the expected CID
    pub fn from_cid_and_data(cid: Cid, data: Vec<u8>) -> Result<Self, CidError> {
        verify_blake3(&data, &cid)?;
        Ok(Self { cid, data })
    }

    /// Get the size of the block in bytes
    pub fn size(&self) -> usize {
        self.data.len()
    }
}

/// RocksDB-backed persistent block storage with CID-based indexing
pub struct BlockStore {
    /// RocksDB database handle
    db: Arc<DB>,
}

impl BlockStore {
    /// Create a new block store with in-memory backend (for testing)
    pub fn new() -> Self {
        // Use a temporary directory for in-memory testing
        let temp_dir =
            std::env::temp_dir().join(format!("neverust-test-{}", rand::random::<u64>()));
        Self::new_with_path(&temp_dir).expect("Failed to create test BlockStore")
    }

    /// Create a new block store with persistent RocksDB backend
    pub fn new_with_path<P: AsRef<Path>>(path: P) -> Result<Self, StorageError> {
        let mut opts = Options::default();
        opts.create_if_missing(true);

        // Optimize for point lookups (CID -> block)
        opts.optimize_for_point_lookup(256); // 256MB block cache

        // Enable pipelined writes for better throughput
        opts.set_enable_pipelined_write(true);

        // Compression - disable for already-compressed content blocks
        opts.set_compression_type(rocksdb::DBCompressionType::None);

        // Performance tuning
        opts.increase_parallelism(num_cpus::get() as i32);
        opts.set_max_background_jobs(4);

        // Write buffer and compaction
        opts.set_write_buffer_size(64 * 1024 * 1024); // 64MB write buffer
        opts.set_target_file_size_base(128 * 1024 * 1024); // 128MB SST files

        let db = DB::open(&opts, path.as_ref())?;

        info!("Opened RocksDB block store at {:?}", path.as_ref());
        Ok(Self { db: Arc::new(db) })
    }

    /// Store a block, verifying its CID
    pub async fn put(&self, block: Block) -> Result<(), StorageError> {
        let cid_str = block.cid.to_string();

        // Verify block integrity
        verify_blake3(&block.data, &block.cid)?;

        let db = Arc::clone(&self.db);
        let key = cid_str.clone();
        let value = block.data.clone();

        tokio::task::spawn_blocking(move || {
            // Check if block already exists (idempotent)
            if db.get(&key)?.is_some() {
                debug!("Block already exists: {}", key);
                return Ok::<(), StorageError>(());
            }

            // Store block
            db.put(&key, &value)?;
            Ok(())
        })
        .await
        .map_err(|e| StorageError::IoError(std::io::Error::other(e.to_string())))??;

        info!("Stored block {}, size: {} bytes", cid_str, block.data.len());
        Ok(())
    }

    /// Store raw data, computing and verifying CID
    pub async fn put_data(&self, data: Vec<u8>) -> Result<Cid, StorageError> {
        let block = Block::new(data)?;
        let cid = block.cid;
        self.put(block).await?;
        Ok(cid)
    }

    /// Retrieve a block by CID
    pub async fn get(&self, cid: &Cid) -> Result<Block, StorageError> {
        let cid_str = cid.to_string();
        let db = Arc::clone(&self.db);
        let key = cid_str.clone();
        let cid_copy = *cid;

        let data = tokio::task::spawn_blocking(move || db.get(&key))
            .await
            .map_err(|e| StorageError::IoError(std::io::Error::other(e.to_string())))??
            .ok_or(StorageError::BlockNotFound(cid_str))?;

        Ok(Block {
            cid: cid_copy,
            data,
        })
    }

    /// Check if a block exists
    pub async fn has(&self, cid: &Cid) -> bool {
        let cid_str = cid.to_string();
        let db = Arc::clone(&self.db);

        tokio::task::spawn_blocking(move || {
            db.get(&cid_str).map(|opt| opt.is_some()).unwrap_or(false)
        })
        .await
        .unwrap_or(false)
    }

    /// Delete a block
    pub async fn delete(&self, cid: &Cid) -> Result<(), StorageError> {
        let cid_str = cid.to_string();
        let db = Arc::clone(&self.db);
        let key = cid_str.clone();

        tokio::task::spawn_blocking(move || {
            // Check if block exists
            if db.get(&key)?.is_none() {
                return Err(StorageError::BlockNotFound(key.clone()));
            }

            db.delete(&key)?;
            Ok::<(), StorageError>(())
        })
        .await
        .map_err(|e| StorageError::IoError(std::io::Error::other(e.to_string())))??;

        info!("Deleted block {}", cid_str);
        Ok(())
    }

    /// Get all CIDs in the store
    pub async fn list_cids(&self) -> Vec<Cid> {
        let db = Arc::clone(&self.db);

        tokio::task::spawn_blocking(move || {
            let mut cids = Vec::new();
            let iter = db.iterator(rocksdb::IteratorMode::Start);

            for (key, _) in iter.flatten() {
                if let Ok(key_str) = String::from_utf8(key.to_vec()) {
                    if let Ok(cid) = key_str.parse::<Cid>() {
                        cids.push(cid);
                    }
                }
            }

            cids
        })
        .await
        .unwrap_or_default()
    }

    /// Get statistics about the block store
    pub async fn stats(&self) -> BlockStoreStats {
        let db = Arc::clone(&self.db);

        tokio::task::spawn_blocking(move || {
            let mut block_count = 0;
            let mut total_size = 0;

            let iter = db.iterator(rocksdb::IteratorMode::Start);
            for (_, value) in iter.flatten() {
                block_count += 1;
                total_size += value.len();
            }

            BlockStoreStats {
                block_count,
                total_size,
            }
        })
        .await
        .unwrap_or(BlockStoreStats {
            block_count: 0,
            total_size: 0,
        })
    }

    /// Clear all blocks
    pub async fn clear(&self) {
        let db = Arc::clone(&self.db);

        tokio::task::spawn_blocking(move || {
            let mut batch = WriteBatch::default();
            let iter = db.iterator(rocksdb::IteratorMode::Start);

            for (key, _) in iter.flatten() {
                batch.delete(&key);
            }

            let _ = db.write(batch);
        })
        .await
        .ok();

        info!("Cleared all blocks from store");
    }
}

impl Default for BlockStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about the block store
#[derive(Debug, Clone)]
pub struct BlockStoreStats {
    pub block_count: usize,
    pub total_size: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_block_new() {
        let data = b"hello world".to_vec();
        let block = Block::new(data.clone()).unwrap();

        assert_eq!(block.data, data);
        assert_eq!(block.size(), data.len());
    }

    #[tokio::test]
    async fn test_block_from_cid_and_data() {
        let data = b"hello world".to_vec();
        let block1 = Block::new(data.clone()).unwrap();

        // Should succeed with matching CID
        let block2 = Block::from_cid_and_data(block1.cid, data.clone()).unwrap();
        assert_eq!(block1, block2);

        // Should fail with mismatched CID
        let other_data = b"goodbye world".to_vec();
        let result = Block::from_cid_and_data(block1.cid, other_data);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_store_put_get() {
        let store = BlockStore::new();
        let data = b"hello world".to_vec();
        let block = Block::new(data).unwrap();
        let cid = block.cid;

        // Store block
        store.put(block.clone()).await.unwrap();

        // Retrieve block
        let retrieved = store.get(&cid).await.unwrap();
        assert_eq!(retrieved, block);
    }

    #[tokio::test]
    async fn test_store_put_data() {
        let store = BlockStore::new();
        let data = b"hello world".to_vec();

        // Store raw data
        let cid = store.put_data(data.clone()).await.unwrap();

        // Retrieve block
        let block = store.get(&cid).await.unwrap();
        assert_eq!(block.data, data);
    }

    #[tokio::test]
    async fn test_store_has() {
        let store = BlockStore::new();
        let data = b"hello world".to_vec();
        let block = Block::new(data).unwrap();
        let cid = block.cid;

        // Should not exist yet
        assert!(!store.has(&cid).await);

        // Store block
        store.put(block).await.unwrap();

        // Should exist now
        assert!(store.has(&cid).await);
    }

    #[tokio::test]
    async fn test_store_delete() {
        let store = BlockStore::new();
        let data = b"hello world".to_vec();
        let block = Block::new(data).unwrap();
        let cid = block.cid;

        // Store block
        store.put(block).await.unwrap();
        assert!(store.has(&cid).await);

        // Delete block
        store.delete(&cid).await.unwrap();
        assert!(!store.has(&cid).await);

        // Should fail to get deleted block
        let result = store.get(&cid).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_store_list_cids() {
        let store = BlockStore::new();

        // Store multiple blocks
        let data1 = b"block 1".to_vec();
        let data2 = b"block 2".to_vec();
        let data3 = b"block 3".to_vec();

        let cid1 = store.put_data(data1).await.unwrap();
        let cid2 = store.put_data(data2).await.unwrap();
        let cid3 = store.put_data(data3).await.unwrap();

        // List CIDs
        let cids = store.list_cids().await;
        assert_eq!(cids.len(), 3);
        assert!(cids.contains(&cid1));
        assert!(cids.contains(&cid2));
        assert!(cids.contains(&cid3));
    }

    #[tokio::test]
    async fn test_store_stats() {
        let store = BlockStore::new();

        // Initially empty
        let stats = store.stats().await;
        assert_eq!(stats.block_count, 0);
        assert_eq!(stats.total_size, 0);

        // Store some blocks
        let data1 = vec![1u8; 100];
        let data2 = vec![2u8; 200];

        store.put_data(data1).await.unwrap();
        store.put_data(data2).await.unwrap();

        // Check stats
        let stats = store.stats().await;
        assert_eq!(stats.block_count, 2);
        assert_eq!(stats.total_size, 300);
    }

    #[tokio::test]
    async fn test_store_clear() {
        let store = BlockStore::new();

        // Store some blocks
        store.put_data(b"block 1".to_vec()).await.unwrap();
        store.put_data(b"block 2".to_vec()).await.unwrap();

        let stats = store.stats().await;
        assert_eq!(stats.block_count, 2);

        // Clear store
        store.clear().await;

        let stats = store.stats().await;
        assert_eq!(stats.block_count, 0);
        assert_eq!(stats.total_size, 0);
    }

    #[tokio::test]
    async fn test_store_idempotent_put() {
        let store = BlockStore::new();
        let data = b"hello world".to_vec();
        let block = Block::new(data).unwrap();

        // Store same block twice
        store.put(block.clone()).await.unwrap();
        store.put(block.clone()).await.unwrap();

        // Should only count once
        let stats = store.stats().await;
        assert_eq!(stats.block_count, 1);
    }

    #[tokio::test]
    async fn test_large_blocks() {
        let store = BlockStore::new();

        // Store a large block (1MB)
        let data = vec![0x42u8; 1024 * 1024];
        let cid = store.put_data(data.clone()).await.unwrap();

        // Retrieve and verify
        let block = store.get(&cid).await.unwrap();
        assert_eq!(block.data.len(), 1024 * 1024);
        assert_eq!(block.data, data);

        // Check stats
        let stats = store.stats().await;
        assert_eq!(stats.block_count, 1);
        assert_eq!(stats.total_size, 1024 * 1024);
    }
}
