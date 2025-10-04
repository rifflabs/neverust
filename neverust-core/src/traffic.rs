//! Autonomous traffic generator for P2P testing
//!
//! Each node acts independently:
//! - Generates random blocks at a configurable rate
//! - Requests blocks from random peers (gossip-style)
//! - Uses BlockExc/BoTG for actual P2P block exchange
//! - No centralized coordination - truly peer-to-peer
//!
//! Enable with: ENABLE_TRAFFIC_GEN=true

use crate::storage::{Block, BlockStore};
use crate::botg::BoTgProtocol;
use rand::Rng;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tokio::sync::{mpsc, RwLock};
use tracing::{info, warn};
use std::collections::HashSet;

/// Traffic generator configuration
#[derive(Debug, Clone)]
pub struct TrafficConfig {
    /// Node identifier (for logging)
    pub node_id: String,
    /// Blocks to generate per minute
    pub upload_rate: u32,
    /// Block requests per minute
    pub request_rate: u32,
    /// Block size in bytes
    pub block_size: usize,
    /// API port for local node
    pub api_port: u16,
}

impl Default for TrafficConfig {
    fn default() -> Self {
        Self {
            node_id: "unknown".to_string(),
            upload_rate: 10,      // 10 blocks/min
            request_rate: 20,     // 20 requests/min
            block_size: 1024 * 1024,  // 1 MiB blocks
            api_port: 8080,
        }
    }
}

/// P2P block exchange command
#[derive(Debug, Clone)]
pub enum P2PCommand {
    /// Request block from network
    RequestBlock(cid::Cid),
    /// Advertise block availability
    AdvertiseBlock(cid::Cid),
}

/// Start autonomous traffic generator with P2P support
pub async fn start_traffic_generator(
    config: TrafficConfig,
    block_store: Arc<BlockStore>,
    botg: Arc<BoTgProtocol>,
    p2p_tx: mpsc::UnboundedSender<P2PCommand>,
) {
    info!(
        "Traffic generator starting for node {} (upload: {}/min, request: {}/min) - P2P MODE",
        config.node_id, config.upload_rate, config.request_rate
    );

    // Wait for node to be ready and peers to connect
    sleep(Duration::from_secs(10)).await;

    // Shared CID tracking for P2P discovery
    // All generated CIDs are added here so peers can discover and request them
    let known_cids: Arc<RwLock<HashSet<cid::Cid>>> = Arc::new(RwLock::new(HashSet::new()));

    // Spawn block upload + P2P advertise task
    let upload_config = config.clone();
    let upload_store = block_store.clone();
    let upload_tx = p2p_tx.clone();
    let upload_cids = known_cids.clone();
    tokio::spawn(async move {
        block_upload_loop_p2p(upload_config, upload_store, upload_tx, upload_cids).await;
    });

    // Spawn P2P block request task
    let request_config = config.clone();
    let request_store = block_store.clone();
    let request_tx = p2p_tx;
    let request_cids = known_cids.clone();
    tokio::spawn(async move {
        block_request_loop_p2p(request_config, request_store, request_tx, request_cids).await;
    });

    // Spawn CID discovery task to learn about blocks from other nodes
    // As blocks are received via BoTG and stored locally, this task discovers them
    let discovery_config = config.clone();
    let discovery_store = block_store.clone();
    let discovery_cids = known_cids.clone();
    tokio::spawn(async move {
        cid_discovery_loop(discovery_config, discovery_store, discovery_cids).await;
    });

    info!("Traffic generator running in P2P mode for node {}", config.node_id);
}

/// Generate blocks and advertise them via P2P
async fn block_upload_loop_p2p(
    config: TrafficConfig,
    block_store: Arc<BlockStore>,
    p2p_tx: mpsc::UnboundedSender<P2PCommand>,
    known_cids: Arc<RwLock<HashSet<cid::Cid>>>,
) {
    let base_interval = Duration::from_secs(60) / config.upload_rate;

    loop {
        // Generate random block data (1 MiB)
        let data: Vec<u8> = {
            let mut rng = rand::thread_rng();
            (0..config.block_size)
                .map(|_| rng.gen::<u8>())
                .collect()
        };

        // Create and store block
        match Block::new(data) {
            Ok(block) => {
                let cid = block.cid;
                match block_store.put(block).await {
                    Ok(_) => {
                        info!("[TRAFFIC-P2P] Node {} generated 1MiB block: {} - advertising to network", config.node_id, cid);

                        // Track this CID for P2P discovery
                        known_cids.write().await.insert(cid);

                        // Advertise block availability via P2P
                        if let Err(e) = p2p_tx.send(P2PCommand::AdvertiseBlock(cid)) {
                            warn!("[TRAFFIC-P2P] Failed to advertise block {}: {}", cid, e);
                        }
                    }
                    Err(e) => {
                        warn!("[TRAFFIC-P2P] Node {} failed to store block: {}", config.node_id, e);
                    }
                }
            }
            Err(e) => {
                warn!("[TRAFFIC-P2P] Node {} failed to create block: {}", config.node_id, e);
            }
        }

        // Add random jitter (0-50% of base interval)
        let jitter_ms = rand::random::<u64>() % (base_interval.as_millis() as u64 / 2);
        let jitter = Duration::from_millis(jitter_ms);
        sleep(base_interval + jitter).await;
    }
}

/// Request blocks from peers via P2P
async fn block_request_loop_p2p(
    config: TrafficConfig,
    block_store: Arc<BlockStore>,
    p2p_tx: mpsc::UnboundedSender<P2PCommand>,
    known_cids: Arc<RwLock<HashSet<cid::Cid>>>,
) {
    let base_interval = Duration::from_secs(60) / config.request_rate;

    loop {
        // Get snapshot of known CIDs from P2P discovery
        let cid_snapshot: Vec<cid::Cid> = {
            let cids = known_cids.read().await;
            cids.iter().copied().collect()
        };

        if !cid_snapshot.is_empty() {
            // Pick a random CID to request (use index-based selection to avoid Send issues)
            let random_index = (rand::random::<usize>()) % cid_snapshot.len();
            let random_cid = cid_snapshot[random_index];

            // Check if we already have this block
            if block_store.has(&random_cid).await {
                // Already have it, skip
            } else {
                // Don't have it, request from network
                info!("[TRAFFIC-P2P] Node {} requesting block {} from network", config.node_id, random_cid);
                if let Err(e) = p2p_tx.send(P2PCommand::RequestBlock(random_cid)) {
                    warn!("[TRAFFIC-P2P] Failed to request block {}: {}", random_cid, e);
                }
            }
        }

        // Add random jitter (0-50% of base interval)
        let jitter_ms = rand::random::<u64>() % (base_interval.as_millis() as u64 / 2);
        let jitter = Duration::from_millis(jitter_ms);
        sleep(base_interval + jitter).await;
    }
}

/// Discover CIDs from local block store (gossip-style discovery)
/// As blocks are received from other nodes via BoTG/P2P and stored locally,
/// this task discovers them and makes them available for re-requesting
async fn cid_discovery_loop(
    config: TrafficConfig,
    block_store: Arc<BlockStore>,
    known_cids: Arc<RwLock<HashSet<cid::Cid>>>,
) {
    // Discovery runs slower than generation to avoid overwhelming the network
    let discovery_interval = Duration::from_secs(30);

    loop {
        sleep(discovery_interval).await;

        // List all blocks in local store
        let cids = block_store.list_cids().await;
        let mut known = known_cids.write().await;
        let previous_count = known.len();

        // Add all discovered CIDs
        for cid in cids {
            known.insert(cid);
        }

        let new_count = known.len();
        if new_count > previous_count {
            info!(
                "[TRAFFIC-P2P] Node {} discovered {} new CIDs ({} total known)",
                config.node_id,
                new_count - previous_count,
                new_count
            );
        }
    }
}

/// Check if traffic generator should be enabled
pub fn is_enabled() -> bool {
    std::env::var("ENABLE_TRAFFIC_GEN")
        .map(|v| v.to_lowercase() == "true" || v == "1")
        .unwrap_or(false)
}

/// Get traffic config from environment variables
///
/// Environment variables:
/// - TRAFFIC_UPLOAD_RATE: Blocks per minute to generate (default: 10)
/// - TRAFFIC_REQUEST_RATE: Block requests per minute (default: 20)
/// - TRAFFIC_BLOCK_SIZE: Block size in bytes (default: 1048576 = 1MiB)
///   - Shortcuts: "1m" or "1M" = 1 MiB, "512k" = 512 KiB, "4k" = 4 KiB
pub fn config_from_env(node_id: String, api_port: u16) -> TrafficConfig {
    let block_size = std::env::var("TRAFFIC_BLOCK_SIZE")
        .ok()
        .and_then(|v| parse_size(&v))
        .unwrap_or(1024 * 1024); // Default 1 MiB

    TrafficConfig {
        node_id,
        upload_rate: std::env::var("TRAFFIC_UPLOAD_RATE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10),
        request_rate: std::env::var("TRAFFIC_REQUEST_RATE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(20),
        block_size,
        api_port,
    }
}

/// Parse human-readable size strings (1m, 512k, 4k, etc.)
fn parse_size(s: &str) -> Option<usize> {
    let s = s.trim().to_lowercase();

    // Try exact number first
    if let Ok(n) = s.parse::<usize>() {
        return Some(n);
    }

    // Parse with suffix
    if s.ends_with('m') {
        s[..s.len()-1].parse::<usize>().ok().map(|n| n * 1024 * 1024)
    } else if s.ends_with('k') {
        s[..s.len()-1].parse::<usize>().ok().map(|n| n * 1024)
    } else if s.ends_with("mb") {
        s[..s.len()-2].parse::<usize>().ok().map(|n| n * 1024 * 1024)
    } else if s.ends_with("kb") {
        s[..s.len()-2].parse::<usize>().ok().map(|n| n * 1024)
    } else {
        None
    }
}
