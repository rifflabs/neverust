//! Neverust Core
//!
//! Core P2P networking and storage functionality for the Archivist node.

pub mod blockexc;
pub mod botg;
pub mod cid_blake3;
pub mod config;
pub mod messages;
pub mod p2p;
pub mod runtime;
pub mod spr;
pub mod storage;

pub use config::Config;
pub use p2p::{create_swarm, Behaviour, P2PError};
pub use runtime::run_node;
pub use spr::{parse_spr_records, SprError};
pub use botg::{BoTgProtocol, BoTgConfig, BlockId, BlockRollup, BoTgError};
pub use cid_blake3::{blake3_cid, blake3_hash, verify_blake3, StreamingVerifier, CidError};
pub use storage::{Block, BlockStore, BlockStoreStats, StorageError};
