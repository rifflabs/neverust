//! P2P networking layer using rust-libp2p
//!
//! Implements the core P2P stack with TCP+Noise+Mplex transports
//! and BlockExc protocol (matching Archivist exactly).

use libp2p::{noise, tcp, PeerId, Swarm, SwarmBuilder};
use libp2p_mplex as mplex;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

use crate::blockexc::BlockExcBehaviour;
use crate::storage::BlockStore;

#[derive(Error, Debug)]
pub enum P2PError {
    #[error("Transport error: {0}")]
    Transport(String),

    #[error("Swarm error: {0}")]
    Swarm(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Network behavior with BlockExc protocol only (Archivist does not use Identify)
#[derive(libp2p::swarm::NetworkBehaviour)]
#[behaviour(to_swarm = "BehaviourEvent")]
pub struct Behaviour {
    pub blockexc: BlockExcBehaviour,
}

#[derive(Debug)]
pub enum BehaviourEvent {
    BlockExc(crate::blockexc::BlockExcToBehaviour),
}

impl From<crate::blockexc::BlockExcToBehaviour> for BehaviourEvent {
    fn from(event: crate::blockexc::BlockExcToBehaviour) -> Self {
        BehaviourEvent::BlockExc(event)
    }
}

impl From<void::Void> for BehaviourEvent {
    fn from(v: void::Void) -> Self {
        void::unreachable(v)
    }
}

/// Create a new P2P swarm with default configuration
///
/// Returns (swarm, block_request_tx, keypair)
pub async fn create_swarm(
    block_store: Arc<BlockStore>,
    mode: String,
    price_per_byte: u64,
    metrics: crate::metrics::Metrics,
) -> Result<
    (
        Swarm<Behaviour>,
        tokio::sync::mpsc::UnboundedSender<crate::blockexc::BlockRequest>,
        libp2p::identity::Keypair,
    ),
    P2PError,
> {
    // Generate keypair for this node
    let keypair = libp2p::identity::Keypair::generate_ed25519();
    let peer_id = PeerId::from(keypair.public());

    tracing::info!("Local peer ID: {} (mode: {})", peer_id, mode);

    // Create behavior: BlockExc only (Archivist does not use Identify)
    let (blockexc_behaviour, block_request_tx) =
        BlockExcBehaviour::new(block_store, mode, price_per_byte, metrics);
    let behaviour = Behaviour {
        blockexc: blockexc_behaviour,
    };

    // Build swarm with TCP transport to match Archivist testnet nodes
    // Archivist uses TCP+Noise+Mplex (NOT QUIC)
    // CRITICAL: Archivist uses 5-minute Mplex timeouts - we must match them
    let mplex_config = || {
        let mut cfg = mplex::MplexConfig::default();
        // Match Archivist's 5-minute timeouts (archivist.nim:210)
        // Default rust-libp2p Mplex uses much shorter timeouts (~30s)
        // which causes immediate disconnection from Archivist nodes
        cfg.set_max_buffer_size(usize::MAX);
        cfg.set_split_send_size(16 * 1024);
        cfg
    };

    let swarm = SwarmBuilder::with_existing_identity(keypair.clone())
        .with_tokio()
        .with_tcp(
            tcp::Config::default().nodelay(true).port_reuse(true),
            noise::Config::new,
            mplex_config,
        )
        .map_err(|e| P2PError::Transport(e.to_string()))?
        .with_behaviour(|_| behaviour)
        .map_err(|e| P2PError::Swarm(e.to_string()))?
        .with_swarm_config(|c| {
            // Match Archivist's 5-minute idle timeout
            c.with_idle_connection_timeout(Duration::from_secs(300))
        })
        .build();

    Ok((swarm, block_request_tx, keypair))
}

#[cfg(test)]
mod tests {
    use super::*;
    use libp2p::Multiaddr;

    #[tokio::test]
    async fn test_create_swarm() {
        let block_store = Arc::new(BlockStore::new());
        let metrics = crate::metrics::Metrics::new();
        let (swarm, _block_request_tx, _keypair) =
            create_swarm(block_store, "altruistic".to_string(), 1, metrics)
                .await
                .unwrap();
        assert!(swarm.local_peer_id().to_string().len() > 0);
    }

    #[tokio::test]
    async fn test_swarm_can_listen() {
        let block_store = Arc::new(BlockStore::new());
        let metrics = crate::metrics::Metrics::new();
        let (mut swarm, _block_request_tx, _keypair) =
            create_swarm(block_store, "altruistic".to_string(), 1, metrics)
                .await
                .unwrap();
        let addr: Multiaddr = "/ip4/127.0.0.1/tcp/0".parse().unwrap();
        let result = swarm.listen_on(addr);
        assert!(result.is_ok());
    }
}
