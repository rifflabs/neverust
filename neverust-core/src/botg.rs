//! Block-over-TGP (BoTG) Protocol
//!
//! Two-layer fault-tolerant block exchange protocol:
//! - Layer 1: TGP (Temporal Graph Protocol) provides reliable high-speed transport (12-13x faster than TCP)
//! - Layer 2: BoTG provides rollup-based block exchange with instant convergence
//!
//! Design Philosophy:
//! - Replace WantList-based individual requests with batch rollups
//! - Leverage TGP's linear degradation under packet loss
//! - Achieve instant convergence even at 99% packet loss (TGP: 1+ Mbps)
//! - Optimize for Neverust-to-Neverust block exchange

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

// Re-export TGP types we'll use
pub use consensus_tgp::{TgpConfig, TgpHandle};
pub use consensus_common::types::StreamId;

/// Block identifier (CID-compatible)
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct BlockId {
    /// Raw CID bytes
    pub cid: Vec<u8>,
}

/// Block rollup - a batch of blocks being exchanged
#[derive(Debug, Clone)]
pub struct BlockRollup {
    /// Unique rollup ID
    pub id: u64,
    /// Block IDs in this rollup
    pub blocks: Vec<BlockId>,
    /// Total size in bytes
    pub total_size: u64,
    /// Priority (higher = more urgent)
    pub priority: u8,
}

/// BoTG protocol configuration
#[derive(Debug, Clone)]
pub struct BoTgConfig {
    /// Maximum rollup size (number of blocks)
    pub max_rollup_size: usize,
    /// Maximum rollup bytes
    pub max_rollup_bytes: u64,
    /// TGP MTU (from PROTOCOL_COMPARISON.md: 1200 bytes optimal)
    pub mtu: usize,
    /// Local peer ID
    pub local_peer_id: u64,
    /// TGP epoch
    pub epoch: u32,
}

impl Default for BoTgConfig {
    fn default() -> Self {
        Self {
            max_rollup_size: 1000,
            max_rollup_bytes: 100 * 1024 * 1024, // 100 MB per rollup
            mtu: 1200, // Optimal MTU from TGP benchmarks
            local_peer_id: rand::random(),
            epoch: 0,
        }
    }
}

/// BoTG protocol state machine
pub struct BoTgProtocol {
    config: BoTgConfig,
    /// Active TGP handles by peer
    handles: Arc<RwLock<HashMap<u64, TgpHandle>>>,
    /// Pending outbound rollups
    pending_rollups: Arc<RwLock<Vec<BlockRollup>>>,
    /// Blocks we have locally
    local_blocks: Arc<RwLock<HashSet<BlockId>>>,
    /// Blocks we want from peers
    want_blocks: Arc<RwLock<HashSet<BlockId>>>,
}

impl BoTgProtocol {
    /// Create a new BoTG protocol instance
    pub fn new(config: BoTgConfig) -> Self {
        Self {
            config,
            handles: Arc::new(RwLock::new(HashMap::new())),
            pending_rollups: Arc::new(RwLock::new(Vec::new())),
            local_blocks: Arc::new(RwLock::new(HashSet::new())),
            want_blocks: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    /// Create a TGP handle for a peer
    pub async fn connect_to_peer(&self, peer_id: u64) -> Result<(), BoTgError> {
        info!("BoTG: Setting up TGP handle for peer {}", peer_id);

        // Generate unique stream ID from epoch + local_id + peer_id
        // StreamId is u128, we'll pack: [epoch:32][local_id:64][peer_id:32]
        let stream_id: StreamId =
            ((self.config.epoch as u128) << 96) |
            ((self.config.local_peer_id as u128) << 32) |
            (peer_id as u128);

        let tgp_config = TgpConfig {
            stream_id,
            epoch: self.config.epoch,
            local_id: self.config.local_peer_id,
            peer_id,
            mtu: self.config.mtu,
        };

        // Create TGP handle
        let handle = TgpHandle { cfg: tgp_config };

        // Store handle
        self.handles.write().await.insert(peer_id, handle);

        info!("BoTG: Created TGP handle for peer {}", peer_id);
        Ok(())
    }

    /// Request blocks from a peer using rollup protocol
    pub async fn request_blocks(&self, peer_id: u64, blocks: Vec<BlockId>) -> Result<(), BoTgError> {
        debug!("BoTG: Requesting {} blocks from peer {}", blocks.len(), peer_id);

        // Create rollup from requested blocks
        let rollup = BlockRollup {
            id: rand::random(),
            blocks,
            total_size: 0, // Will be calculated when blocks arrive
            priority: 128, // Medium priority
        };

        // Add to pending rollups
        self.pending_rollups.write().await.push(rollup.clone());

        // Send rollup request over TGP
        let handles = self.handles.read().await;
        if handles.get(&peer_id).is_some() {
            // Serialize rollup request (TODO: implement proper encoding)
            let _request_bytes = self.encode_rollup_request(&rollup)?;

            // TODO: Use TGP handle's start_streaming to send data
            // For now, just track the rollup request
            debug!("BoTG: Queued rollup request {} for peer {}", rollup.id, peer_id);
            Ok(())
        } else {
            Err(BoTgError::NoPeerConnection(peer_id))
        }
    }

    /// Handle incoming rollup response
    pub async fn handle_rollup_response(&self, peer_id: u64, data: &[u8]) -> Result<(), BoTgError> {
        debug!("BoTG: Received {} bytes from peer {}", data.len(), peer_id);

        // Decode rollup response (TODO: implement proper decoding)
        let blocks = self.decode_rollup_response(data)?;

        // Store received blocks
        let mut local_blocks = self.local_blocks.write().await;
        for block in blocks {
            local_blocks.insert(block);
        }

        info!("BoTG: Received and stored blocks from peer {}", peer_id);
        Ok(())
    }

    /// Mark blocks as locally available
    pub async fn add_local_blocks(&self, blocks: Vec<BlockId>) {
        let mut local_blocks = self.local_blocks.write().await;
        for block in blocks {
            local_blocks.insert(block);
        }
    }

    /// Mark blocks as wanted
    pub async fn add_wanted_blocks(&self, blocks: Vec<BlockId>) {
        let mut want_blocks = self.want_blocks.write().await;
        for block in blocks {
            want_blocks.insert(block);
        }
    }

    // TODO: Implement proper protobuf encoding/decoding
    fn encode_rollup_request(&self, rollup: &BlockRollup) -> Result<Vec<u8>, BoTgError> {
        // Placeholder: simple binary encoding
        // Format: [rollup_id:8][num_blocks:4][block_cid_len:4][block_cid:N]...
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&rollup.id.to_le_bytes());
        bytes.extend_from_slice(&(rollup.blocks.len() as u32).to_le_bytes());

        for block in &rollup.blocks {
            bytes.extend_from_slice(&(block.cid.len() as u32).to_le_bytes());
            bytes.extend_from_slice(&block.cid);
        }

        Ok(bytes)
    }

    fn decode_rollup_response(&self, _data: &[u8]) -> Result<Vec<BlockId>, BoTgError> {
        // Placeholder: will implement proper decoding
        Ok(Vec::new())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BoTgError {
    #[error("TGP error: {0}")]
    TgpError(String),

    #[error("No connection to peer {0}")]
    NoPeerConnection(u64),

    #[error("Encoding error: {0}")]
    EncodingError(String),

    #[error("Decoding error: {0}")]
    DecodingError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_rollup_creation() {
        let blocks = vec![
            BlockId { cid: vec![1, 2, 3] },
            BlockId { cid: vec![4, 5, 6] },
        ];

        let rollup = BlockRollup {
            id: 42,
            blocks: blocks.clone(),
            total_size: 1024,
            priority: 255,
        };

        assert_eq!(rollup.id, 42);
        assert_eq!(rollup.blocks.len(), 2);
        assert_eq!(rollup.priority, 255);
    }

    #[tokio::test]
    async fn test_botg_protocol_creation() {
        let config = BoTgConfig::default();
        let protocol = BoTgProtocol::new(config);

        let blocks = vec![
            BlockId { cid: vec![1, 2, 3] },
        ];

        protocol.add_local_blocks(blocks.clone()).await;
        protocol.add_wanted_blocks(blocks).await;
    }

    #[test]
    fn test_encode_rollup_request() {
        let config = BoTgConfig::default();
        let protocol = BoTgProtocol::new(config);

        let rollup = BlockRollup {
            id: 123,
            blocks: vec![
                BlockId { cid: vec![1, 2, 3] },
            ],
            total_size: 100,
            priority: 128,
        };

        let encoded = protocol.encode_rollup_request(&rollup).unwrap();
        assert!(encoded.len() > 0);

        // Verify format: [rollup_id:8][num_blocks:4][block_cid_len:4][block_cid:3]
        assert_eq!(encoded.len(), 8 + 4 + 4 + 3);
    }
}
