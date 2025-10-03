# NEVAR-4: Build P2P Swarm (libp2p)

**Phase**: 0 | **Status**: Todo | **Priority**: Critical

## Description
Create the core P2P networking layer using rust-libp2p. Build a Swarm with TCP transport, Noise encryption, Yamux multiplexing, and Ping + Identify behaviors. This is the foundation of all P2P communication.

## Acceptance Criteria
- [ ] Test: Swarm can be created with valid peer ID
- [ ] Test: Swarm has TCP transport configured
- [ ] Test: Noise encryption is enabled
- [ ] Test: Yamux multiplexing is enabled
- [ ] Test: Ping behavior is registered
- [ ] Test: Identify behavior is registered
- [ ] Test: Swarm can listen on TCP address
- [ ] Implement: Transport builder with TCP + Noise + Yamux
- [ ] Implement: NetworkBehaviour with Ping + Identify
- [ ] Implement: Swarm creation function
- [ ] Implementation complete and all tests pass
- [ ] Committed atomically

## Relationships
- **Blocked by**: NEVAR-2 (workspace must exist)
- **Blocking**: NEVAR-5 (event loop needs Swarm), NEVAR-6 (integration test needs Swarm)
- **Relates to**: NEVAR-11 (Kademlia DHT in Phase 1)
- **Start after**: NEVAR-2
- **Finish before**: NEVAR-5

## Technical Notes

**Test First** (TDD):
```rust
// neverust-core/src/p2p.rs tests
#[tokio::test]
async fn test_create_swarm() {
    let swarm = create_swarm().await.unwrap();
    assert!(swarm.local_peer_id().to_string().len() > 0);
}

#[tokio::test]
async fn test_swarm_can_listen() {
    let mut swarm = create_swarm().await.unwrap();
    let addr = "/ip4/127.0.0.1/tcp/0".parse().unwrap();
    swarm.listen_on(addr).unwrap();
}
```

**Implementation**:
```rust
use libp2p::{
    Swarm, SwarmBuilder,
    identity::Keypair,
    tcp, noise, yamux, ping, identify,
    PeerId, Multiaddr,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum P2PError {
    #[error("Transport error: {0}")]
    Transport(String),
    #[error("Swarm error: {0}")]
    Swarm(String),
}

// NetworkBehaviour combining Ping + Identify
#[derive(libp2p::swarm::NetworkBehaviour)]
pub struct Behaviour {
    ping: ping::Behaviour,
    identify: identify::Behaviour,
}

pub async fn create_swarm() -> Result<Swarm<Behaviour>, P2PError> {
    // Generate or load keypair
    let keypair = Keypair::generate_ed25519();
    let peer_id = PeerId::from(keypair.public());

    // Build transport: TCP + Noise + Yamux
    let transport = tcp::tokio::Transport::default()
        .upgrade(libp2p::core::upgrade::Version::V1)
        .authenticate(noise::Config::new(&keypair).map_err(|e| P2PError::Transport(e.to_string()))?)
        .multiplex(yamux::Config::default())
        .boxed();

    // Create behaviors
    let behaviour = Behaviour {
        ping: ping::Behaviour::new(ping::Config::new()),
        identify: identify::Behaviour::new(identify::Config::new(
            "/neverust/0.1.0".to_string(),
            keypair.public(),
        )),
    };

    // Build swarm
    let swarm = SwarmBuilder::with_tokio_executor(transport, behaviour, peer_id)
        .build();

    Ok(swarm)
}
```

**Crate Versions**:
- libp2p = "0.53" with features: tcp, noise, yamux, ping, identify, kad

**References**:
- DeepWiki: libp2p/rust-libp2p
- Tutorial: rust-libp2p/tutorials/ping.rs

## Time Estimate
60 minutes (TDD cycle)
