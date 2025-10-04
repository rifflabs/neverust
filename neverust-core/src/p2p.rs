//! P2P networking layer using rust-libp2p
//!
//! Implements the core P2P stack with TCP transport, Noise encryption,
//! Mplex multiplexing, and BlockExc protocol (matching Archivist exactly).

use libp2p::{
    noise, tcp, PeerId, Swarm, SwarmBuilder,
};
use libp2p_mplex as mplex;
use std::time::Duration;
use thiserror::Error;

use crate::blockexc::BlockExcBehaviour;

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
pub async fn create_swarm() -> Result<Swarm<Behaviour>, P2PError> {
    // Generate keypair for this node
    let keypair = libp2p::identity::Keypair::generate_ed25519();
    let peer_id = PeerId::from(keypair.public());

    tracing::info!("Local peer ID: {}", peer_id);

    // Create behavior: ONLY BlockExc (Archivist nodes don't use Ping or Identify)
    let behaviour = Behaviour {
        blockexc: BlockExcBehaviour,
    };

    // Build swarm with TCP transport, Noise security, and Mplex multiplexing
    // (matching Archivist's configuration)
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
        let swarm = create_swarm().await.unwrap();
        assert!(swarm.local_peer_id().to_string().len() > 0);
    }

    #[tokio::test]
    async fn test_swarm_can_listen() {
        let mut swarm = create_swarm().await.unwrap();
        let addr: Multiaddr = "/ip4/127.0.0.1/tcp/0".parse().unwrap();
        let result = swarm.listen_on(addr);
        assert!(result.is_ok());
    }
}
