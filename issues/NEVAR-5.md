# NEVAR-5: Create Event Loop + Async Runtime

**Phase**: 0 | **Status**: Todo | **Priority**: Critical

## Description
Implement the main event loop using tokio that processes Swarm events. Handle connection establishment, peer discovery, protocol events (Ping, Identify), and log all significant events using structured logging.

## Acceptance Criteria
- [ ] Test: Event loop processes SwarmEvent::NewListenAddr
- [ ] Test: Event loop processes SwarmEvent::ConnectionEstablished
- [ ] Test: Event loop logs peer ID on startup
- [ ] Test: Event loop can be gracefully shutdown
- [ ] Implement: Tokio runtime setup
- [ ] Implement: Swarm event loop with match statement
- [ ] Implement: Event handlers for Ping and Identify
- [ ] Implement: Graceful shutdown on Ctrl+C
- [ ] Implementation complete and all tests pass
- [ ] Committed atomically

## Relationships
- **Blocked by**: NEVAR-3 (needs config), NEVAR-4 (needs Swarm)
- **Blocking**: NEVAR-6 (integration test needs running node)
- **Relates to**: NEVAR-7 (uses structured logging)
- **Start after**: NEVAR-4
- **Finish before**: NEVAR-6

## Technical Notes

**Test First** (TDD):
```rust
// neverust-core/src/runtime.rs tests
#[tokio::test]
async fn test_event_loop_starts() {
    let config = Config::default();
    let handle = tokio::spawn(async move {
        run_node(config).await
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    handle.abort();
    // Should not panic
}
```

**Implementation**:
```rust
use libp2p::swarm::SwarmEvent;
use tokio::signal;
use tracing::{info, warn, error};

pub async fn run_node(config: Config) -> Result<(), P2PError> {
    // Create swarm
    let mut swarm = create_swarm().await?;

    // Start listening
    let listen_addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", config.listen_port)
        .parse()
        .map_err(|e| P2PError::Transport(format!("Invalid address: {}", e)))?;

    swarm.listen_on(listen_addr)?;

    info!("Node started with peer ID: {}", swarm.local_peer_id());
    info!("Listening on port {}", config.listen_port);

    // Event loop
    loop {
        tokio::select! {
            event = swarm.select_next_some() => {
                match event {
                    SwarmEvent::NewListenAddr { address, .. } => {
                        info!("Listening on {}", address);
                    }
                    SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                        info!("Connected to peer: {} at {}", peer_id, endpoint.get_remote_address());
                    }
                    SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                        warn!("Connection closed with {}: {:?}", peer_id, cause);
                    }
                    SwarmEvent::Behaviour(BehaviourEvent::Ping(event)) => {
                        info!("Ping event: {:?}", event);
                    }
                    SwarmEvent::Behaviour(BehaviourEvent::Identify(event)) => {
                        info!("Identify event: {:?}", event);
                    }
                    _ => {}
                }
            }
            _ = signal::ctrl_c() => {
                info!("Received Ctrl+C, shutting down...");
                break;
            }
        }
    }

    info!("Node stopped");
    Ok(())
}
```

**Main Binary** (src/main.rs):
```rust
use neverust_core::{Config, run_node};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Parse CLI
    let config = Config::from_cli()?;

    // Run node
    run_node(config).await?;

    Ok(())
}
```

**Crate Versions**:
- tokio = "1" with "full" feature
- tracing = "0.1"

## Time Estimate
30 minutes
