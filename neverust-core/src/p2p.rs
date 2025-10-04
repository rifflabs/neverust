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

/// Network behavior with ONLY BlockExc protocol (matching Archivist)
#[derive(libp2p::swarm::NetworkBehaviour)]
#[behaviour(to_swarm = "()")]
pub struct Behaviour {
    pub blockexc: BlockExcBehaviour,
}

/// Create a new P2P swarm with default configuration
pub async fn create_swarm(
    block_store: Arc<BlockStore>,
    mode: String,
    price_per_byte: u64,
    metrics: crate::metrics::Metrics,
) -> Result<Swarm<Behaviour>, P2PError> {
    // Generate keypair for this node
    let keypair = libp2p::identity::Keypair::generate_ed25519();
    let peer_id = PeerId::from(keypair.public());

    tracing::info!("Local peer ID: {} (mode: {})", peer_id, mode);

    // Create behavior: ONLY BlockExc (Archivist nodes don't use Ping or Identify)
    let behaviour = Behaviour {
        blockexc: BlockExcBehaviour::new(block_store, mode, price_per_byte, metrics),
    };

    // Build swarm with TCP transport to match Archivist testnet nodes
    // Archivist uses TCP+Noise+Mplex (NOT QUIC)
    let swarm = SwarmBuilder::with_existing_identity(keypair)
        .with_tokio()
        .with_tcp(
            tcp::Config::default().nodelay(true).port_reuse(true),
            noise::Config::new,
            mplex::MplexConfig::default,
        )
        .map_err(|e| P2PError::Transport(e.to_string()))?
        .with_behaviour(|_| behaviour)
        .map_err(|e| P2PError::Swarm(e.to_string()))?
        .with_swarm_config(|c| {
            // Match Archivist's 5-minute idle timeout
            c.with_idle_connection_timeout(Duration::from_secs(300))
        })
        .build();

    Ok(swarm)
}

#[cfg(test)]
mod tests {
    use super::*;
    use libp2p::Multiaddr;

    #[tokio::test]
    async fn test_create_swarm() {
        let block_store = Arc::new(BlockStore::new());
        let swarm = create_swarm(block_store, "altruistic".to_string(), 1)
            .await
            .unwrap();
        assert!(swarm.local_peer_id().to_string().len() > 0);
    }

    #[tokio::test]
    async fn test_swarm_can_listen() {
        let block_store = Arc::new(BlockStore::new());
        let mut swarm = create_swarm(block_store, "altruistic".to_string(), 1)
            .await
            .unwrap();
        let addr: Multiaddr = "/ip4/127.0.0.1/tcp/0".parse().unwrap();
        let result = swarm.listen_on(addr);
        assert!(result.is_ok());
    }
}
