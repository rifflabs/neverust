//! Persistent block storage backends.
//!
//! Supports:
//! - `redb` (default): embedded key/value database.
//! - `geomtree`: FlatFS-style geometric directory tree.
//! - `deltastore`: classed blockfiles with geometric sharding + redb index.
//!
//! Backend selection:
//! - `NEVERUST_STORAGE_BACKEND=redb|geomtree|deltastore`

use cid::Cid;
use redb::{Database, ReadableTable, TableDefinition};
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::cid_blake3::{blake3_cid, verify_blake3, CidError};

const BLOCKS_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("blocks");
const DELTA_INDEX_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("delta_index");
const DELTA_CLASS_STATE_TABLE: TableDefinition<&str, &[u8]> =
    TableDefinition::new("delta_class_state");

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("Block not found: {0}")]
    BlockNotFound(String),

    #[error("CID verification failed: {0}")]
    VerificationFailed(#[from] CidError),

    #[error("Block already exists: {0}")]
    BlockExists(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

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

const DELTA_SIZE_CLASSES: [usize; 11] = [
    512 * 1024,
    1024 * 1024,
    2 * 1024 * 1024,
    4 * 1024 * 1024,
    8 * 1024 * 1024,
    16 * 1024 * 1024,
    32 * 1024 * 1024,
    64 * 1024 * 1024,
    128 * 1024 * 1024,
    256 * 1024 * 1024,
    512 * 1024 * 1024,
];

#[derive(Clone, Copy, Debug)]
struct DeltaLocation {
    class_id: u8,
    file_id: u32,
    offset: u64,
    len: u32,
}

#[derive(Clone, Copy, Debug)]
struct DeltaClassState {
    file_id: u32,
    next_offset: u64,
}

struct RedbStore {
    db: Arc<Database>,
    db_path: PathBuf,
}

#[derive(Clone)]
struct DeltaStore {
    root: PathBuf,
    blocks_root: PathBuf,
    db: Arc<Database>,
    fsync_writes: bool,
    max_file_bytes: u64,
}

#[derive(Clone)]
struct GeomTreeStore {
    root: PathBuf,
    fsync_writes: bool,
    shard_levels: usize,
    bytes_per_level: usize,
}

enum StoreBackend {
    Redb(RedbStore),
    DeltaStore(DeltaStore),
    GeomTree(GeomTreeStore),
}

/// Persistent block storage with pluggable backend.
pub struct BlockStore {
    backend: StoreBackend,
}

impl BlockStore {
    /// Create a new block store with a temp-file backend (for testing).
    pub fn new() -> Self {
        let temp_dir =
            std::env::temp_dir().join(format!("neverust-test-{}", rand::random::<u64>()));
        Self::new_with_path(&temp_dir).expect("Failed to create test BlockStore")
    }

    /// Create a new block store with persistent backend.
    pub fn new_with_path<P: AsRef<Path>>(path: P) -> Result<Self, StorageError> {
        let backend = std::env::var("NEVERUST_STORAGE_BACKEND")
            .unwrap_or_else(|_| "redb".to_string())
            .to_lowercase();
        Self::new_with_backend(path.as_ref(), &backend)
    }

    fn new_with_backend(path: &Path, backend: &str) -> Result<Self, StorageError> {
        let backend = backend.to_lowercase();

        match backend.as_str() {
            "deltastore" | "delta" | "delta-store" => {
                let delta = DeltaStore::open(path)?;
                Ok(Self {
                    backend: StoreBackend::DeltaStore(delta),
                })
            }
            "geomtree" => {
                let root = Self::resolve_geomtree_root(path);
                fs::create_dir_all(&root)?;
                let fsync_writes = Self::env_flag("NEVERUST_GEOMTREE_FSYNC", false);
                let shard_levels = Self::env_usize("NEVERUST_GEOMTREE_SHARD_LEVELS", 3)
                    .clamp(1, 8);
                let bytes_per_level = Self::env_usize("NEVERUST_GEOMTREE_BYTES_PER_LEVEL", 2)
                    .clamp(1, 8);
                info!(
                    "Opened geomtree block store at {:?} (fsync_writes={}, shard_levels={}, bytes_per_level={})",
                    root, fsync_writes, shard_levels, bytes_per_level
                );
                Ok(Self {
                    backend: StoreBackend::GeomTree(GeomTreeStore {
                        root,
                        fsync_writes,
                        shard_levels,
                        bytes_per_level,
                    }),
                })
            }
            "redb" => {
                let redb = RedbStore::open(path)?;
                Ok(Self {
                    backend: StoreBackend::Redb(redb),
                })
            }
            other => {
                warn!(
                    "Unknown NEVERUST_STORAGE_BACKEND='{}', falling back to redb",
                    other
                );
                let redb = RedbStore::open(path)?;
                Ok(Self {
                    backend: StoreBackend::Redb(redb),
                })
            }
        }
    }

    fn resolve_geomtree_root(path: &Path) -> PathBuf {
        if (path.exists() && path.is_dir()) || path.extension().is_none() {
            path.join("geomtree")
        } else {
            path.with_extension("geomtree")
        }
    }

    fn resolve_deltastore_root(path: &Path) -> PathBuf {
        if (path.exists() && path.is_dir()) || path.extension().is_none() {
            path.join("deltastore")
        } else {
            path.with_extension("deltastore")
        }
    }

    fn env_flag(name: &str, default: bool) -> bool {
        std::env::var(name)
            .ok()
            .map(|v| {
                let t = v.trim().to_ascii_lowercase();
                matches!(t.as_str(), "1" | "true" | "yes" | "on")
            })
            .unwrap_or(default)
    }

    fn env_usize(name: &str, default: usize) -> usize {
        std::env::var(name)
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|&v| v > 0)
            .unwrap_or(default)
    }

    fn env_u64(name: &str, default: u64) -> u64 {
        std::env::var(name)
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|&v| v > 0)
            .unwrap_or(default)
    }

    /// Store multiple blocks, verifying CID integrity.
    pub async fn put_many(&self, blocks: Vec<Block>) -> Result<(), StorageError> {
        match &self.backend {
            StoreBackend::Redb(redb) => redb.put_many(blocks).await,
            StoreBackend::DeltaStore(delta) => delta.put_many(blocks).await,
            StoreBackend::GeomTree(tree) => tree.put_many(blocks).await,
        }
    }

    /// Store a block, verifying its CID.
    pub async fn put(&self, block: Block) -> Result<(), StorageError> {
        self.put_many(vec![block]).await
    }

    /// Store raw data, computing and verifying CID.
    pub async fn put_data(&self, data: Vec<u8>) -> Result<Cid, StorageError> {
        let block = Block::new(data)?;
        let cid = block.cid;
        self.put(block).await?;
        Ok(cid)
    }

    /// Retrieve a block by CID.
    pub async fn get(&self, cid: &Cid) -> Result<Block, StorageError> {
        match &self.backend {
            StoreBackend::Redb(redb) => redb.get(cid).await,
            StoreBackend::DeltaStore(delta) => delta.get(cid).await,
            StoreBackend::GeomTree(tree) => tree.get(cid).await,
        }
    }

    /// Check if a block exists.
    pub async fn has(&self, cid: &Cid) -> bool {
        match &self.backend {
            StoreBackend::Redb(redb) => redb.has(cid).await,
            StoreBackend::DeltaStore(delta) => delta.has(cid).await,
            StoreBackend::GeomTree(tree) => tree.has(cid).await,
        }
    }

    /// Delete a block.
    pub async fn delete(&self, cid: &Cid) -> Result<(), StorageError> {
        match &self.backend {
            StoreBackend::Redb(redb) => redb.delete(cid).await,
            StoreBackend::DeltaStore(delta) => delta.delete(cid).await,
            StoreBackend::GeomTree(tree) => tree.delete(cid).await,
        }
    }

    /// Get all CIDs in the store.
    pub async fn list_cids(&self) -> Vec<Cid> {
        match &self.backend {
            StoreBackend::Redb(redb) => redb.list_cids().await,
            StoreBackend::DeltaStore(delta) => delta.list_cids().await,
            StoreBackend::GeomTree(tree) => tree.list_cids().await,
        }
    }

    /// Get statistics about the block store.
    pub async fn stats(&self) -> BlockStoreStats {
        match &self.backend {
            StoreBackend::Redb(redb) => redb.stats().await,
            StoreBackend::DeltaStore(delta) => delta.stats().await,
            StoreBackend::GeomTree(tree) => tree.stats().await,
        }
    }

    /// Clear all blocks.
    pub async fn clear(&self) {
        match &self.backend {
            StoreBackend::Redb(redb) => redb.clear().await,
            StoreBackend::DeltaStore(delta) => delta.clear().await,
            StoreBackend::GeomTree(tree) => tree.clear().await,
        }
    }
}

impl RedbStore {
    fn open(path: &Path) -> Result<Self, StorageError> {
        let db_path = Self::resolve_db_path(path);
        if let Some(parent) = db_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let db = if db_path.exists() {
            Database::open(&db_path).map_err(Self::db_err)?
        } else {
            Database::create(&db_path).map_err(Self::db_err)?
        };

        // Ensure the table exists.
        {
            let write_txn = db.begin_write().map_err(Self::db_err)?;
            write_txn.open_table(BLOCKS_TABLE).map_err(Self::db_err)?;
            write_txn.commit().map_err(Self::db_err)?;
        }

        info!("Opened redb block store at {:?}", db_path);
        Ok(Self {
            db: Arc::new(db),
            db_path,
        })
    }

    fn resolve_db_path(path: &Path) -> PathBuf {
        if (path.exists() && path.is_dir()) || path.extension().is_none() {
            path.join("store.redb")
        } else {
            path.to_path_buf()
        }
    }

    fn db_err<E: std::fmt::Display>(err: E) -> StorageError {
        StorageError::DatabaseError(err.to_string())
    }

    async fn put_many(&self, blocks: Vec<Block>) -> Result<(), StorageError> {
        if blocks.is_empty() {
            return Ok(());
        }

        let mut prepared = Vec::with_capacity(blocks.len());
        for block in blocks {
            // Verify block integrity (codec-aware)
            // - Data blocks (0xcd02): verify with blake3_cid
            // - Manifests (0xcd01): skip verification
            // - Tree roots (0xcd03): skip verification
            if block.cid.codec() == 0xcd02 {
                verify_blake3(&block.data, &block.cid)?;
            }
            prepared.push((block.cid.to_string(), block.data));
        }

        let stored_count = prepared.len();
        let db = Arc::clone(&self.db);
        tokio::task::spawn_blocking(move || {
            let write_txn = db.begin_write().map_err(Self::db_err)?;
            {
                let mut table = write_txn.open_table(BLOCKS_TABLE).map_err(Self::db_err)?;
                for (key, value) in &prepared {
                    if table.get(key.as_str()).map_err(Self::db_err)?.is_some() {
                        debug!("Block already exists: {}", key);
                        continue;
                    }
                    table
                        .insert(key.as_str(), value.as_slice())
                        .map_err(Self::db_err)?;
                }
            }
            write_txn.commit().map_err(Self::db_err)?;
            Ok::<(), StorageError>(())
        })
        .await
        .map_err(|e| StorageError::IoError(std::io::Error::other(e.to_string())))??;

        info!("Stored {} blocks in redb batch", stored_count);
        Ok(())
    }

    async fn get(&self, cid: &Cid) -> Result<Block, StorageError> {
        let cid_str = cid.to_string();
        let db = Arc::clone(&self.db);
        let key = cid_str.clone();
        let cid_copy = *cid;

        let data = tokio::task::spawn_blocking(move || {
            let read_txn = db.begin_read().map_err(Self::db_err)?;
            let table = read_txn.open_table(BLOCKS_TABLE).map_err(Self::db_err)?;
            table
                .get(key.as_str())
                .map_err(Self::db_err)?
                .map(|v| v.value().to_vec())
                .ok_or(StorageError::BlockNotFound(key))
        })
        .await
        .map_err(|e| StorageError::IoError(std::io::Error::other(e.to_string())))??;

        Ok(Block {
            cid: cid_copy,
            data,
        })
    }

    async fn has(&self, cid: &Cid) -> bool {
        let cid_str = cid.to_string();
        let db = Arc::clone(&self.db);

        tokio::task::spawn_blocking(move || {
            let read_txn = db.begin_read().map_err(Self::db_err)?;
            let table = read_txn.open_table(BLOCKS_TABLE).map_err(Self::db_err)?;
            Ok::<bool, StorageError>(table.get(cid_str.as_str()).map_err(Self::db_err)?.is_some())
        })
        .await
        .ok()
        .and_then(Result::ok)
        .unwrap_or(false)
    }

    async fn delete(&self, cid: &Cid) -> Result<(), StorageError> {
        let cid_str = cid.to_string();
        let db = Arc::clone(&self.db);
        let key = cid_str.clone();

        tokio::task::spawn_blocking(move || {
            let write_txn = db.begin_write().map_err(Self::db_err)?;
            {
                let mut table = write_txn.open_table(BLOCKS_TABLE).map_err(Self::db_err)?;
                if table.remove(key.as_str()).map_err(Self::db_err)?.is_none() {
                    return Err(StorageError::BlockNotFound(key));
                }
            }
            write_txn.commit().map_err(Self::db_err)?;
            Ok::<(), StorageError>(())
        })
        .await
        .map_err(|e| StorageError::IoError(std::io::Error::other(e.to_string())))??;

        info!("Deleted block {}", cid_str);
        Ok(())
    }

    async fn list_cids(&self) -> Vec<Cid> {
        let db = Arc::clone(&self.db);

        tokio::task::spawn_blocking(move || -> Result<Vec<Cid>, StorageError> {
            let read_txn = db.begin_read().map_err(Self::db_err)?;
            let table = read_txn.open_table(BLOCKS_TABLE).map_err(Self::db_err)?;
            let mut cids = Vec::new();

            for entry in table.iter().map_err(Self::db_err)? {
                let (key, _) = entry.map_err(Self::db_err)?;
                if let Ok(cid) = key.value().parse::<Cid>() {
                    cids.push(cid);
                }
            }

            Ok(cids)
        })
        .await
        .map_err(|e| StorageError::IoError(std::io::Error::other(e.to_string())))
        .and_then(|r| r)
        .unwrap_or_else(|e| {
            warn!("Failed to list CIDs from {:?}: {}", self.db_path, e);
            Vec::new()
        })
    }

    async fn stats(&self) -> BlockStoreStats {
        let db = Arc::clone(&self.db);

        tokio::task::spawn_blocking(move || -> Result<BlockStoreStats, StorageError> {
            let read_txn = db.begin_read().map_err(Self::db_err)?;
            let table = read_txn.open_table(BLOCKS_TABLE).map_err(Self::db_err)?;
            let mut block_count = 0usize;
            let mut total_size = 0usize;

            for entry in table.iter().map_err(Self::db_err)? {
                let (_, value) = entry.map_err(Self::db_err)?;
                block_count += 1;
                total_size += value.value().len();
            }

            Ok(BlockStoreStats {
                block_count,
                total_size,
            })
        })
        .await
        .map_err(|e| StorageError::IoError(std::io::Error::other(e.to_string())))
        .and_then(|r| r)
        .unwrap_or_else(|e| {
            warn!(
                "Failed to compute store stats from {:?}: {}",
                self.db_path, e
            );
            BlockStoreStats {
                block_count: 0,
                total_size: 0,
            }
        })
    }

    async fn clear(&self) {
        let db = Arc::clone(&self.db);
        let db_path = self.db_path.clone();

        let res = tokio::task::spawn_blocking(move || -> Result<(), StorageError> {
            let write_txn = db.begin_write().map_err(Self::db_err)?;
            {
                let mut table = write_txn.open_table(BLOCKS_TABLE).map_err(Self::db_err)?;
                let mut keys = Vec::new();
                for entry in table.iter().map_err(Self::db_err)? {
                    let (key, _) = entry.map_err(Self::db_err)?;
                    keys.push(key.value().to_string());
                }

                for key in keys {
                    let _ = table.remove(key.as_str()).map_err(Self::db_err)?;
                }
            }
            write_txn.commit().map_err(Self::db_err)?;
            Ok(())
        })
        .await
        .map_err(|e| StorageError::IoError(std::io::Error::other(e.to_string())))
        .and_then(|r| r);

        match res {
            Ok(()) => info!("Cleared all blocks from redb store"),
            Err(e) => warn!("Failed to clear redb store at {:?}: {}", db_path, e),
        }
    }
}

impl DeltaStore {
    const LOCATION_ENCODED_LEN: usize = 1 + 1 + 4 + 8 + 4;
    const CLASS_STATE_ENCODED_LEN: usize = 4 + 8;

    fn open(path: &Path) -> Result<Self, StorageError> {
        let root = BlockStore::resolve_deltastore_root(path);
        let blocks_root = root.join("blocks");
        fs::create_dir_all(&blocks_root)?;

        let db_path = root.join("index.redb");
        let db = if db_path.exists() {
            Database::open(&db_path).map_err(RedbStore::db_err)?
        } else {
            Database::create(&db_path).map_err(RedbStore::db_err)?
        };

        {
            let write_txn = db.begin_write().map_err(RedbStore::db_err)?;
            write_txn
                .open_table(DELTA_INDEX_TABLE)
                .map_err(RedbStore::db_err)?;
            write_txn
                .open_table(DELTA_CLASS_STATE_TABLE)
                .map_err(RedbStore::db_err)?;
            write_txn.commit().map_err(RedbStore::db_err)?;
        }

        let fsync_writes = BlockStore::env_flag("NEVERUST_DELTASTORE_FSYNC", false);
        let min_max_bytes = *DELTA_SIZE_CLASSES.last().unwrap_or(&(512 * 1024 * 1024)) as u64;
        let max_file_bytes = BlockStore::env_u64(
            "NEVERUST_DELTASTORE_MAX_FILE_BYTES",
            1024 * 1024 * 1024,
        )
        .max(min_max_bytes);

        info!(
            "Opened deltastore at {:?} (fsync_writes={}, max_file_bytes={})",
            root, fsync_writes, max_file_bytes
        );

        Ok(Self {
            root,
            blocks_root,
            db: Arc::new(db),
            fsync_writes,
            max_file_bytes,
        })
    }

    fn encode_location(loc: DeltaLocation) -> Vec<u8> {
        let mut out = Vec::with_capacity(Self::LOCATION_ENCODED_LEN);
        out.push(1u8);
        out.push(loc.class_id);
        out.extend_from_slice(&loc.file_id.to_le_bytes());
        out.extend_from_slice(&loc.offset.to_le_bytes());
        out.extend_from_slice(&loc.len.to_le_bytes());
        out
    }

    fn decode_location(bytes: &[u8]) -> Result<DeltaLocation, StorageError> {
        if bytes.len() != Self::LOCATION_ENCODED_LEN {
            return Err(StorageError::DatabaseError(format!(
                "delta location length mismatch: expected {}, got {}",
                Self::LOCATION_ENCODED_LEN,
                bytes.len()
            )));
        }
        if bytes[0] != 1 {
            return Err(StorageError::DatabaseError(format!(
                "unsupported delta location version {}",
                bytes[0]
            )));
        }
        let class_id = bytes[1];
        let file_id = u32::from_le_bytes(
            bytes[2..6]
                .try_into()
                .map_err(|_| StorageError::DatabaseError("bad file_id bytes".to_string()))?,
        );
        let offset = u64::from_le_bytes(
            bytes[6..14]
                .try_into()
                .map_err(|_| StorageError::DatabaseError("bad offset bytes".to_string()))?,
        );
        let len = u32::from_le_bytes(
            bytes[14..18]
                .try_into()
                .map_err(|_| StorageError::DatabaseError("bad len bytes".to_string()))?,
        );
        Ok(DeltaLocation {
            class_id,
            file_id,
            offset,
            len,
        })
    }

    fn encode_class_state(state: DeltaClassState) -> Vec<u8> {
        let mut out = Vec::with_capacity(Self::CLASS_STATE_ENCODED_LEN);
        out.extend_from_slice(&state.file_id.to_le_bytes());
        out.extend_from_slice(&state.next_offset.to_le_bytes());
        out
    }

    fn decode_class_state(bytes: &[u8]) -> Result<DeltaClassState, StorageError> {
        if bytes.len() != Self::CLASS_STATE_ENCODED_LEN {
            return Err(StorageError::DatabaseError(format!(
                "delta class state length mismatch: expected {}, got {}",
                Self::CLASS_STATE_ENCODED_LEN,
                bytes.len()
            )));
        }
        let file_id = u32::from_le_bytes(
            bytes[0..4]
                .try_into()
                .map_err(|_| StorageError::DatabaseError("bad class-state file_id".to_string()))?,
        );
        let next_offset =
            u64::from_le_bytes(bytes[4..12].try_into().map_err(|_| {
                StorageError::DatabaseError("bad class-state next_offset".to_string())
            })?);
        Ok(DeltaClassState {
            file_id,
            next_offset,
        })
    }

    fn class_for_len(len: usize) -> (u8, usize) {
        for (i, class) in DELTA_SIZE_CLASSES.iter().copied().enumerate() {
            if len <= class {
                return (i as u8, class);
            }
        }
        ((DELTA_SIZE_CLASSES.len() - 1) as u8, *DELTA_SIZE_CLASSES.last().unwrap())
    }

    fn class_state_key(class_id: u8) -> String {
        format!("c{:02}", class_id)
    }

    fn class_dir(&self, class_id: u8) -> PathBuf {
        self.blocks_root.join(format!("c{:02}", class_id))
    }

    fn blockfile_path(&self, class_id: u8, file_id: u32) -> PathBuf {
        self.class_dir(class_id)
            .join(format!("blk-{:08}.dat", file_id))
    }

    fn append_block_locked(
        &self,
        data: &[u8],
        class_id: u8,
        state: &mut DeltaClassState,
    ) -> Result<DeltaLocation, StorageError> {
        let class_dir = self.class_dir(class_id);
        fs::create_dir_all(&class_dir)?;

        loop {
            let path = class_dir.join(format!("blk-{:08}.dat", state.file_id));
            let mut file = fs::OpenOptions::new()
                .create(true)
                .read(true)
                .write(true)
                .open(&path)?;
            let mut file_len = file.metadata()?.len();

            // If there is unexpected trailing data (e.g. previous crash after write, before
            // index commit), append after current file end to avoid overwrite/corruption.
            if file_len != state.next_offset {
                state.next_offset = file_len;
            }

            if state.next_offset + data.len() as u64 > self.max_file_bytes {
                state.file_id = state
                    .file_id
                    .checked_add(1)
                    .ok_or_else(|| StorageError::DatabaseError("delta file_id overflow".to_string()))?;
                state.next_offset = 0;
                continue;
            }

            file.seek(SeekFrom::Start(state.next_offset))?;
            file.write_all(data)?;
            if self.fsync_writes {
                file.sync_data()?;
            }
            file_len = state.next_offset;
            let loc = DeltaLocation {
                class_id,
                file_id: state.file_id,
                offset: file_len,
                len: u32::try_from(data.len())
                    .map_err(|_| StorageError::DatabaseError("block length exceeds u32".to_string()))?,
            };
            state.next_offset = state
                .next_offset
                .checked_add(data.len() as u64)
                .ok_or_else(|| StorageError::DatabaseError("delta offset overflow".to_string()))?;
            return Ok(loc);
        }
    }

    async fn put_many(&self, blocks: Vec<Block>) -> Result<(), StorageError> {
        if blocks.is_empty() {
            return Ok(());
        }

        for block in &blocks {
            if block.cid.codec() == 0xcd02 {
                verify_blake3(&block.data, &block.cid)?;
            }
        }

        let store = self.clone();
        tokio::task::spawn_blocking(move || -> Result<(), StorageError> {
            let write_txn = store.db.begin_write().map_err(RedbStore::db_err)?;
            {
                let mut index = write_txn
                    .open_table(DELTA_INDEX_TABLE)
                    .map_err(RedbStore::db_err)?;
                let mut class_state = write_txn
                    .open_table(DELTA_CLASS_STATE_TABLE)
                    .map_err(RedbStore::db_err)?;

                for block in blocks {
                    let cid_key = block.cid.to_string();
                    if index.get(cid_key.as_str()).map_err(RedbStore::db_err)?.is_some() {
                        continue;
                    }

                    let (class_id, _class_bytes) = Self::class_for_len(block.data.len());
                    let state_key = Self::class_state_key(class_id);

                    let mut state = match class_state
                        .get(state_key.as_str())
                        .map_err(RedbStore::db_err)?
                    {
                        Some(v) => Self::decode_class_state(v.value())?,
                        None => DeltaClassState {
                            file_id: 0,
                            next_offset: 0,
                        },
                    };

                    let loc = store.append_block_locked(&block.data, class_id, &mut state)?;
                    let loc_bytes = Self::encode_location(loc);
                    let state_bytes = Self::encode_class_state(state);

                    index
                        .insert(cid_key.as_str(), loc_bytes.as_slice())
                        .map_err(RedbStore::db_err)?;
                    class_state
                        .insert(state_key.as_str(), state_bytes.as_slice())
                        .map_err(RedbStore::db_err)?;
                }
            }
            write_txn.commit().map_err(RedbStore::db_err)?;
            Ok(())
        })
        .await
        .map_err(|e| StorageError::IoError(std::io::Error::other(e.to_string())))?
    }

    async fn get(&self, cid: &Cid) -> Result<Block, StorageError> {
        let cid_copy = *cid;
        let cid_key = cid.to_string();
        let store = self.clone();

        tokio::task::spawn_blocking(move || -> Result<Block, StorageError> {
            let read_txn = store.db.begin_read().map_err(RedbStore::db_err)?;
            let index = read_txn
                .open_table(DELTA_INDEX_TABLE)
                .map_err(RedbStore::db_err)?;
            let loc_bytes = index
                .get(cid_key.as_str())
                .map_err(RedbStore::db_err)?
                .ok_or_else(|| StorageError::BlockNotFound(cid_key.clone()))?;
            let loc = Self::decode_location(loc_bytes.value())?;
            let path = store.blockfile_path(loc.class_id, loc.file_id);

            let mut file = fs::OpenOptions::new().read(true).open(&path).map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    StorageError::BlockNotFound(cid_key.clone())
                } else {
                    StorageError::IoError(e)
                }
            })?;
            file.seek(SeekFrom::Start(loc.offset))?;
            let mut data = vec![0u8; loc.len as usize];
            file.read_exact(&mut data)?;

            Ok(Block { cid: cid_copy, data })
        })
        .await
        .map_err(|e| StorageError::IoError(std::io::Error::other(e.to_string())))?
    }

    async fn has(&self, cid: &Cid) -> bool {
        let cid_key = cid.to_string();
        let db = Arc::clone(&self.db);
        tokio::task::spawn_blocking(move || -> Result<bool, StorageError> {
            let read_txn = db.begin_read().map_err(RedbStore::db_err)?;
            let index = read_txn
                .open_table(DELTA_INDEX_TABLE)
                .map_err(RedbStore::db_err)?;
            Ok(index
                .get(cid_key.as_str())
                .map_err(RedbStore::db_err)?
                .is_some())
        })
        .await
        .ok()
        .and_then(Result::ok)
        .unwrap_or(false)
    }

    async fn delete(&self, cid: &Cid) -> Result<(), StorageError> {
        let cid_key = cid.to_string();
        let db = Arc::clone(&self.db);
        tokio::task::spawn_blocking(move || -> Result<(), StorageError> {
            let write_txn = db.begin_write().map_err(RedbStore::db_err)?;
            {
                let mut index = write_txn
                    .open_table(DELTA_INDEX_TABLE)
                    .map_err(RedbStore::db_err)?;
                if index
                    .remove(cid_key.as_str())
                    .map_err(RedbStore::db_err)?
                    .is_none()
                {
                    return Err(StorageError::BlockNotFound(cid_key));
                }
            }
            write_txn.commit().map_err(RedbStore::db_err)?;
            Ok(())
        })
        .await
        .map_err(|e| StorageError::IoError(std::io::Error::other(e.to_string())))?
    }

    async fn list_cids(&self) -> Vec<Cid> {
        let db = Arc::clone(&self.db);
        tokio::task::spawn_blocking(move || -> Result<Vec<Cid>, StorageError> {
            let read_txn = db.begin_read().map_err(RedbStore::db_err)?;
            let index = read_txn
                .open_table(DELTA_INDEX_TABLE)
                .map_err(RedbStore::db_err)?;
            let mut out = Vec::new();
            for entry in index.iter().map_err(RedbStore::db_err)? {
                let (k, _) = entry.map_err(RedbStore::db_err)?;
                if let Ok(cid) = k.value().parse::<Cid>() {
                    out.push(cid);
                }
            }
            Ok(out)
        })
        .await
        .map_err(|e| StorageError::IoError(std::io::Error::other(e.to_string())))
        .and_then(|r| r)
        .unwrap_or_else(|e| {
            warn!("Failed to list CIDs from deltastore {:?}: {}", self.root, e);
            Vec::new()
        })
    }

    async fn stats(&self) -> BlockStoreStats {
        let db = Arc::clone(&self.db);
        tokio::task::spawn_blocking(move || -> Result<BlockStoreStats, StorageError> {
            let read_txn = db.begin_read().map_err(RedbStore::db_err)?;
            let index = read_txn
                .open_table(DELTA_INDEX_TABLE)
                .map_err(RedbStore::db_err)?;
            let mut block_count = 0usize;
            let mut total_size = 0usize;
            for entry in index.iter().map_err(RedbStore::db_err)? {
                let (_, value) = entry.map_err(RedbStore::db_err)?;
                let loc = Self::decode_location(value.value())?;
                block_count += 1;
                total_size += loc.len as usize;
            }
            Ok(BlockStoreStats {
                block_count,
                total_size,
            })
        })
        .await
        .map_err(|e| StorageError::IoError(std::io::Error::other(e.to_string())))
        .and_then(|r| r)
        .unwrap_or_else(|e| {
            warn!("Failed to compute deltastore stats from {:?}: {}", self.root, e);
            BlockStoreStats {
                block_count: 0,
                total_size: 0,
            }
        })
    }

    async fn clear(&self) {
        let db = Arc::clone(&self.db);
        let blocks_root = self.blocks_root.clone();
        let root = self.root.clone();
        let res = tokio::task::spawn_blocking(move || -> Result<(), StorageError> {
            if blocks_root.exists() {
                fs::remove_dir_all(&blocks_root)?;
            }
            fs::create_dir_all(&blocks_root)?;

            let write_txn = db.begin_write().map_err(RedbStore::db_err)?;
            {
                let mut index = write_txn
                    .open_table(DELTA_INDEX_TABLE)
                    .map_err(RedbStore::db_err)?;
                let mut keys = Vec::new();
                for entry in index.iter().map_err(RedbStore::db_err)? {
                    let (k, _) = entry.map_err(RedbStore::db_err)?;
                    keys.push(k.value().to_string());
                }
                for k in keys {
                    let _ = index.remove(k.as_str()).map_err(RedbStore::db_err)?;
                }
            }
            {
                let mut class_state = write_txn
                    .open_table(DELTA_CLASS_STATE_TABLE)
                    .map_err(RedbStore::db_err)?;
                let mut keys = Vec::new();
                for entry in class_state.iter().map_err(RedbStore::db_err)? {
                    let (k, _) = entry.map_err(RedbStore::db_err)?;
                    keys.push(k.value().to_string());
                }
                for k in keys {
                    let _ = class_state
                        .remove(k.as_str())
                        .map_err(RedbStore::db_err)?;
                }
            }
            write_txn.commit().map_err(RedbStore::db_err)?;
            Ok(())
        })
        .await
        .map_err(|e| StorageError::IoError(std::io::Error::other(e.to_string())))
        .and_then(|r| r);

        match res {
            Ok(()) => info!("Cleared all blocks from deltastore at {:?}", root),
            Err(e) => warn!("Failed to clear deltastore at {:?}: {}", root, e),
        }
    }
}

impl GeomTreeStore {
    const FILE_EXT: &'static str = ".data";

    fn to_hex(bytes: &[u8]) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut out = String::with_capacity(bytes.len() * 2);
        for b in bytes {
            out.push(HEX[(b >> 4) as usize] as char);
            out.push(HEX[(b & 0x0f) as usize] as char);
        }
        out
    }

    fn cid_entropy32(cid: &Cid) -> [u8; 32] {
        let digest = cid.hash().digest();
        let mut out = [0u8; 32];
        if digest.len() >= 32 {
            out.copy_from_slice(&digest[..32]);
        } else {
            out.copy_from_slice(blake3::hash(&cid.to_bytes()).as_bytes());
        }
        out
    }

    /// Geometry split inspired by projection-fiber decomposition:
    /// - basepoint: even-byte rail
    /// - fiber/kernel: odd-byte rail
    ///
    /// Together these preserve the full 32-byte entropy.
    fn split_geometry(cid: &Cid) -> ([u8; 16], [u8; 16]) {
        let h = Self::cid_entropy32(cid);

        let mut base = [0u8; 16];
        let mut fiber = [0u8; 16];
        for i in 0..16 {
            base[i] = h[i * 2];
            fiber[i] = h[i * 2 + 1];
        }
        (base, fiber)
    }

    fn block_path(&self, cid: &Cid) -> PathBuf {
        let (base, _fiber) = Self::split_geometry(cid);
        let mut p = self.root.clone();

        // FlatFS-style: shard directory from geometric base rail, full CID in filename.
        // Keeping full CID in filename preserves injective identity independent of sharder.
        for level in 0..self.shard_levels {
            let start = level * self.bytes_per_level;
            if start >= base.len() {
                break;
            }
            let end = (start + self.bytes_per_level).min(base.len());
            p.push(Self::to_hex(&base[start..end]));
        }

        let file_name = format!("{}{}", cid, Self::FILE_EXT);
        p.push(file_name);
        p
    }

    fn walk_block_files(root: &Path) -> Result<Vec<PathBuf>, StorageError> {
        if !root.exists() {
            return Ok(Vec::new());
        }

        let mut stack = vec![root.to_path_buf()];
        let mut files = Vec::new();

        while let Some(dir) = stack.pop() {
            for entry in fs::read_dir(&dir)? {
                let entry = entry?;
                let path = entry.path();
                let ft = entry.file_type()?;
                if ft.is_dir() {
                    stack.push(path);
                } else if ft.is_file() && path.to_string_lossy().ends_with(Self::FILE_EXT) {
                    files.push(path);
                }
            }
        }

        Ok(files)
    }

    async fn put_many(&self, blocks: Vec<Block>) -> Result<(), StorageError> {
        if blocks.is_empty() {
            return Ok(());
        }

        for block in &blocks {
            if block.cid.codec() == 0xcd02 {
                verify_blake3(&block.data, &block.cid)?;
            }
        }

        let store = self.clone();
        tokio::task::spawn_blocking(move || -> Result<(), StorageError> {
            for block in blocks {
                let path = store.block_path(&block.cid);
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent)?;
                }

                let open_res = fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path);

                let mut file = match open_res {
                    Ok(file) => file,
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                        continue;
                    }
                    Err(e) => return Err(StorageError::IoError(e)),
                };

                file.write_all(&block.data)?;
                if store.fsync_writes {
                    file.sync_data()?;
                }
            }

            Ok(())
        })
        .await
        .map_err(|e| StorageError::IoError(std::io::Error::other(e.to_string())))?
    }

    async fn get(&self, cid: &Cid) -> Result<Block, StorageError> {
        let path = self.block_path(cid);
        let cid_copy = *cid;
        tokio::task::spawn_blocking(move || {
            let data = fs::read(&path).map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    StorageError::BlockNotFound(cid_copy.to_string())
                } else {
                    StorageError::IoError(e)
                }
            })?;

            Ok(Block {
                cid: cid_copy,
                data,
            })
        })
        .await
        .map_err(|e| StorageError::IoError(std::io::Error::other(e.to_string())))?
    }

    async fn has(&self, cid: &Cid) -> bool {
        let path = self.block_path(cid);
        tokio::task::spawn_blocking(move || path.exists())
            .await
            .unwrap_or(false)
    }

    async fn delete(&self, cid: &Cid) -> Result<(), StorageError> {
        let path = self.block_path(cid);
        let cid_str = cid.to_string();
        tokio::task::spawn_blocking(move || {
            fs::remove_file(&path).map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    StorageError::BlockNotFound(cid_str)
                } else {
                    StorageError::IoError(e)
                }
            })
        })
        .await
        .map_err(|e| StorageError::IoError(std::io::Error::other(e.to_string())))?
    }

    async fn list_cids(&self) -> Vec<Cid> {
        let root = self.root.clone();
        tokio::task::spawn_blocking(move || -> Result<Vec<Cid>, StorageError> {
            let files = Self::walk_block_files(&root)?;
            let mut cids = Vec::new();

            for path in files {
                let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                    continue;
                };
                let Some(cid_str) = name.strip_suffix(Self::FILE_EXT) else {
                    continue;
                };
                if let Ok(cid) = cid_str.parse::<Cid>() {
                    cids.push(cid);
                }
            }

            Ok(cids)
        })
        .await
        .map_err(|e| StorageError::IoError(std::io::Error::other(e.to_string())))
        .and_then(|r| r)
        .unwrap_or_else(|e| {
            warn!("Failed to list CIDs from geomtree {:?}: {}", self.root, e);
            Vec::new()
        })
    }

    async fn stats(&self) -> BlockStoreStats {
        let root = self.root.clone();
        tokio::task::spawn_blocking(move || -> Result<BlockStoreStats, StorageError> {
            let files = Self::walk_block_files(&root)?;
            let mut block_count = 0usize;
            let mut total_size = 0usize;
            for path in files {
                let meta = fs::metadata(path)?;
                block_count += 1;
                total_size += meta.len() as usize;
            }
            Ok(BlockStoreStats {
                block_count,
                total_size,
            })
        })
        .await
        .map_err(|e| StorageError::IoError(std::io::Error::other(e.to_string())))
        .and_then(|r| r)
        .unwrap_or_else(|e| {
            warn!("Failed to compute geomtree stats from {:?}: {}", self.root, e);
            BlockStoreStats {
                block_count: 0,
                total_size: 0,
            }
        })
    }

    async fn clear(&self) {
        let root = self.root.clone();
        let res = tokio::task::spawn_blocking(move || -> Result<(), StorageError> {
            if root.exists() {
                fs::remove_dir_all(&root)?;
            }
            fs::create_dir_all(&root)?;
            Ok(())
        })
        .await
        .map_err(|e| StorageError::IoError(std::io::Error::other(e.to_string())))
        .and_then(|r| r);

        match res {
            Ok(()) => info!("Cleared all blocks from geomtree store"),
            Err(e) => warn!("Failed to clear geomtree store at {:?}: {}", self.root, e),
        }
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
    use std::path::Path;

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

    #[tokio::test]
    async fn test_geomtree_backend_put_get() {
        let temp_dir =
            std::env::temp_dir().join(format!("neverust-geomtree-test-{}", rand::random::<u64>()));
        let store = BlockStore::new_with_backend(Path::new(&temp_dir), "geomtree").unwrap();

        let data = b"geomtree block".to_vec();
        let block = Block::new(data.clone()).unwrap();
        let cid = block.cid;

        store.put(block.clone()).await.unwrap();
        store.put(block).await.unwrap(); // idempotent

        let out = store.get(&cid).await.unwrap();
        assert_eq!(out.data, data);
        assert!(store.has(&cid).await);

        let listed = store.list_cids().await;
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0], cid);

        store.delete(&cid).await.unwrap();
        assert!(!store.has(&cid).await);
    }

    #[tokio::test]
    async fn test_deltastore_backend_put_get() {
        let temp_dir = std::env::temp_dir()
            .join(format!("neverust-deltastore-test-{}", rand::random::<u64>()));
        let store = BlockStore::new_with_backend(Path::new(&temp_dir), "deltastore").unwrap();

        let data = vec![0xAB; 1024 * 768];
        let block = Block::new(data.clone()).unwrap();
        let cid = block.cid;

        store.put(block.clone()).await.unwrap();
        store.put(block).await.unwrap(); // idempotent

        let out = store.get(&cid).await.unwrap();
        assert_eq!(out.data, data);
        assert!(store.has(&cid).await);

        let listed = store.list_cids().await;
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0], cid);

        let stats = store.stats().await;
        assert_eq!(stats.block_count, 1);
        assert_eq!(stats.total_size, data.len());

        store.delete(&cid).await.unwrap();
        assert!(!store.has(&cid).await);
    }

    #[test]
    fn test_split_geometry_is_lossless_over_32_byte_entropy() {
        let data = b"geometry entropy lane".to_vec();
        let cid = Block::new(data).unwrap().cid;

        let entropy = GeomTreeStore::cid_entropy32(&cid);
        let (base, fiber) = GeomTreeStore::split_geometry(&cid);

        let mut merged = [0u8; 32];
        for i in 0..16 {
            merged[i * 2] = base[i];
            merged[i * 2 + 1] = fiber[i];
        }
        assert_eq!(merged, entropy);
    }
}
