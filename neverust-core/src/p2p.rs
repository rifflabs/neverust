//! P2P networking layer using rust-libp2p
//!
//! Implements the core P2P stack with TCP transport, Noise encryption,
//! Yamux multiplexing, and Ping + Identify behaviors.

use libp2p::{
    gossipsub, identify, kad, noise, ping, tcp, yamux, PeerId, Swarm, SwarmBuilder,
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

/// Combined network behavior with Ping, Identify, Kademlia, and Gossipsub protocols
#[derive(libp2p::swarm::NetworkBehaviour)]
#[behaviour(to_swarm = "BehaviourEvent")]
pub struct Behaviour {
    pub ping: ping::Behaviour,
    pub identify: identify::Behaviour,
    pub kademlia: kad::Behaviour<kad::store::MemoryStore>,
    pub gossipsub: gossipsub::Behaviour,
}

#[derive(Debug)]
pub enum BehaviourEvent {
    Ping(ping::Event),
    Identify(identify::Event),
    Kademlia(kad::Event),
    Gossipsub(gossipsub::Event),
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

impl From<kad::Event> for BehaviourEvent {
    fn from(event: kad::Event) -> Self {
        BehaviourEvent::Kademlia(event)
    }
}

impl From<gossipsub::Event> for BehaviourEvent {
    fn from(event: gossipsub::Event) -> Self {
        BehaviourEvent::Gossipsub(event)
    }
}

/// Create a new P2P swarm with default configuration
pub async fn create_swarm() -> Result<Swarm<Behaviour>, P2PError> {
    // Generate keypair for this node
    let keypair = libp2p::identity::Keypair::generate_ed25519();
    let peer_id = PeerId::from(keypair.public());

    tracing::info!("Local peer ID: {}", peer_id);

    // Create Kademlia with in-memory store
    let store = kad::store::MemoryStore::new(peer_id);
    let kademlia = kad::Behaviour::new(peer_id, store);

    // Create Gossipsub with message signing
    let gossipsub_config = gossipsub::ConfigBuilder::default()
        .heartbeat_interval(Duration::from_secs(10))
        .validation_mode(gossipsub::ValidationMode::Strict)
        .build()
        .map_err(|e| P2PError::Swarm(format!("Gossipsub config error: {}", e)))?;

    let gossipsub = gossipsub::Behaviour::new(
        gossipsub::MessageAuthenticity::Signed(keypair.clone()),
        gossipsub_config,
    )
    .map_err(|e| P2PError::Swarm(format!("Gossipsub creation error: {}", e)))?;

    // Create behaviors
    let behaviour = Behaviour {
        ping: ping::Behaviour::new(ping::Config::new()),
        identify: identify::Behaviour::new(identify::Config::new(
            "/archivist/1.0.0".to_string(),
            keypair.public(),
        )),
        kademlia,
        gossipsub,
    };

    // Build swarm with TCP transport, Noise security, and Yamux multiplexing
    // (matching Archivist's configuration)
    let swarm = SwarmBuilder::with_existing_identity(keypair)
        .with_tokio()
        .with_tcp(
            tcp::Config::default().nodelay(true).port_reuse(true),
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
