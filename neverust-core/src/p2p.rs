//! P2P networking layer using rust-libp2p
//!
//! Implements the core P2P stack with TCP transport, Noise encryption,
//! Mplex multiplexing, and Ping + Identify behaviors.

use libp2p::{
    noise, ping, tcp, PeerId, Swarm, SwarmBuilder,
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

/// Network behavior with Ping and BlockExc protocols
#[derive(libp2p::swarm::NetworkBehaviour)]
#[behaviour(to_swarm = "BehaviourEvent")]
pub struct Behaviour {
    pub ping: ping::Behaviour,
    pub blockexc: BlockExcBehaviour,
}

#[derive(Debug)]
pub enum BehaviourEvent {
    Ping(ping::Event),
    BlockExc(()),
}

impl From<ping::Event> for BehaviourEvent {
    fn from(event: ping::Event) -> Self {
        BehaviourEvent::Ping(event)
    }
}

impl From<()> for BehaviourEvent {
    fn from(_: ()) -> Self {
        BehaviourEvent::BlockExc(())
    }
}

/// Create a new P2P swarm with default configuration
pub async fn create_swarm() -> Result<Swarm<Behaviour>, P2PError> {
    // Generate keypair for this node
    let keypair = libp2p::identity::Keypair::generate_ed25519();
    let peer_id = PeerId::from(keypair.public());

    tracing::info!("Local peer ID: {}", peer_id);

    // Create behaviors: Ping for keep-alive and BlockExc for block exchange
    let behaviour = Behaviour {
        ping: ping::Behaviour::new(ping::Config::new()),
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
            c.with_idle_connection_timeout(Duration::from_secs(60))
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
