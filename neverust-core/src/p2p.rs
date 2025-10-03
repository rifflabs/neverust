//! P2P networking layer using rust-libp2p
//!
//! Implements the core P2P stack with TCP transport, Noise encryption,
//! Yamux multiplexing, and Ping + Identify behaviors.

use libp2p::{
    identify, noise, ping, tcp, yamux, PeerId, Swarm, SwarmBuilder,
};
use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum P2PError {
    #[error("Transport error: {0}")]
    Transport(String),

    #[error("Swarm error: {0}")]
    Swarm(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Combined network behavior with Ping and Identify protocols
#[derive(libp2p::swarm::NetworkBehaviour)]
#[behaviour(to_swarm = "BehaviourEvent")]
pub struct Behaviour {
    pub ping: ping::Behaviour,
    pub identify: identify::Behaviour,
}

#[derive(Debug)]
pub enum BehaviourEvent {
    Ping(ping::Event),
    Identify(identify::Event),
}

impl From<ping::Event> for BehaviourEvent {
    fn from(event: ping::Event) -> Self {
        BehaviourEvent::Ping(event)
    }
}

impl From<identify::Event> for BehaviourEvent {
    fn from(event: identify::Event) -> Self {
        BehaviourEvent::Identify(event)
    }
}

/// Create a new P2P swarm with default configuration
pub async fn create_swarm() -> Result<Swarm<Behaviour>, P2PError> {
    // Generate keypair for this node
    let keypair = libp2p::identity::Keypair::generate_ed25519();
    let peer_id = PeerId::from(keypair.public());

    tracing::info!("Local peer ID: {}", peer_id);

    // Create behaviors
    let behaviour = Behaviour {
        ping: ping::Behaviour::new(ping::Config::new()),
        identify: identify::Behaviour::new(identify::Config::new(
            "/neverust/0.1.0".to_string(),
            keypair.public(),
        )),
    };

    // Build swarm with TCP + Noise + Yamux
    let swarm = SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
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
