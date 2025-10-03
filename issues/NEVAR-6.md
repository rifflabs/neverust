# NEVAR-6: Write Integration Test (2 Nodes Ping)

**Phase**: 0 | **Status**: Todo | **Priority**: Critical

## Description
Create integration test that spawns 2 independent nodes, has them discover each other, and verify they can successfully ping. This validates the entire Phase 0 P2P stack end-to-end.

## Acceptance Criteria
- [ ] Test: Spawn node 1 on port 8071
- [ ] Test: Spawn node 2 on port 8072
- [ ] Test: Node 2 dials node 1's multiaddr
- [ ] Test: Connection is established between nodes
- [ ] Test: Ping events are received by both nodes
- [ ] Test: Nodes can be gracefully shutdown
- [ ] Implement: Test helper to spawn node in background
- [ ] Implement: Test helper to wait for connection
- [ ] Implement: Test helper to verify ping events
- [ ] Implementation complete and test passes
- [ ] Committed atomically

## Relationships
- **Blocked by**: NEVAR-4 (needs Swarm), NEVAR-5 (needs event loop)
- **Blocking**: Phase 0 Complete, Phase 1 start, NEVAR-10 (docs reference working code)
- **Start after**: NEVAR-5
- **Finish after**: NEVAR-5

## Technical Notes

**Integration Test** (tests/integration_test.rs):
```rust
use neverust_core::{Config, create_swarm};
use libp2p::{Swarm, Multiaddr, PeerId};
use std::time::Duration;
use tokio::time::timeout;

#[tokio::test]
async fn test_two_nodes_can_ping() {
    // Spawn node 1
    let mut node1 = spawn_test_node(8071).await;
    let node1_peer_id = *node1.local_peer_id();
    let node1_addr = get_listen_addr(&mut node1).await;

    // Spawn node 2
    let mut node2 = spawn_test_node(8072).await;
    let node2_peer_id = *node2.local_peer_id();

    // Node 2 dials node 1
    node2.dial(node1_addr.clone()).unwrap();

    // Wait for connection
    let connected = timeout(Duration::from_secs(10), async {
        wait_for_connection(&mut node1, &node2_peer_id).await &&
        wait_for_connection(&mut node2, &node1_peer_id).await
    }).await;

    assert!(connected.is_ok(), "Nodes failed to connect within 10 seconds");

    // Wait for ping events
    let pinged = timeout(Duration::from_secs(5), async {
        wait_for_ping(&mut node1).await || wait_for_ping(&mut node2).await
    }).await;

    assert!(pinged.is_ok(), "No ping events received within 5 seconds");
}

async fn spawn_test_node(port: u16) -> Swarm<Behaviour> {
    let mut swarm = create_swarm().await.unwrap();
    let addr: Multiaddr = format!("/ip4/127.0.0.1/tcp/{}", port).parse().unwrap();
    swarm.listen_on(addr).unwrap();
    swarm
}

async fn get_listen_addr(swarm: &mut Swarm<Behaviour>) -> Multiaddr {
    loop {
        if let Some(SwarmEvent::NewListenAddr { address, .. }) = swarm.next().await {
            return address;
        }
    }
}

async fn wait_for_connection(swarm: &mut Swarm<Behaviour>, expected_peer: &PeerId) -> bool {
    while let Some(event) = swarm.next().await {
        if let SwarmEvent::ConnectionEstablished { peer_id, .. } = event {
            if peer_id == *expected_peer {
                return true;
            }
        }
    }
    false
}

async fn wait_for_ping(swarm: &mut Swarm<Behaviour>) -> bool {
    while let Some(event) = swarm.next().await {
        if let SwarmEvent::Behaviour(BehaviourEvent::Ping(_)) = event {
            return true;
        }
    }
    false
}
```

**Test Execution**:
```bash
cargo test --test integration_test -- --nocapture
```

**Success Criteria**:
- Test completes in <10 seconds
- Both nodes connect successfully
- Ping events are logged
- No panics or errors

## Time Estimate
20 minutes
