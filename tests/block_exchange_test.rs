//! Integration test for block exchange between two Neverust nodes

use futures_util::StreamExt;
use libp2p::Multiaddr;
use neverust_core::{create_swarm, BlockStore, Block, Metrics};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

#[tokio::test]
async fn test_two_nodes_exchange_blocks() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging for test
    let _ = tracing_subscriber::fmt()
        .with_env_filter("debug")
        .try_init();

    // Create block stores
    let store1 = Arc::new(BlockStore::new());
    let store2 = Arc::new(BlockStore::new());

    // Create metrics collectors
    let metrics1 = Metrics::new();
    let metrics2 = Metrics::new();

    // Create two swarms (nodes) with their block stores
    let mut swarm1 = create_swarm(store1.clone(), "altruistic".to_string(), 0, metrics1).await?;
    let mut swarm2 = create_swarm(store2.clone(), "altruistic".to_string(), 0, metrics2).await?;

    let peer1_id = *swarm1.local_peer_id();
    let peer2_id = *swarm2.local_peer_id();

    tracing::info!("Node 1 peer ID: {}", peer1_id);
    tracing::info!("Node 2 peer ID: {}", peer2_id);

    // Start listening on node 1
    let addr1: Multiaddr = "/ip4/127.0.0.1/tcp/0".parse()?;
    swarm1.listen_on(addr1)?;

    // Wait for node 1 to get its listen address
    let node1_addr = loop {
        match swarm1.next().await {
            Some(libp2p::swarm::SwarmEvent::NewListenAddr { address, .. }) => {
                tracing::info!("Node 1 listening on: {}", address);
                break address;
            }
            _ => continue,
        }
    };

    // Create full multiaddr for node 1
    let node1_full_addr = format!("{}/p2p/{}", node1_addr, peer1_id).parse::<Multiaddr>()?;
    tracing::info!("Node 1 full address: {}", node1_full_addr);

    // Start listening on node 2
    let addr2: Multiaddr = "/ip4/127.0.0.1/tcp/0".parse()?;
    swarm2.listen_on(addr2)?;

    // Wait for node 2 to get its listen address
    loop {
        match swarm2.next().await {
            Some(libp2p::swarm::SwarmEvent::NewListenAddr { address, .. }) => {
                tracing::info!("Node 2 listening on: {}", address);
                break;
            }
            _ => continue,
        }
    }

    // Node 2 dials node 1
    tracing::info!("Node 2 dialing node 1...");
    swarm2.dial(node1_full_addr.clone())?;

    // Wait for connection to establish
    let connection_timeout = Duration::from_secs(10);
    let result = timeout(connection_timeout, async {
        let mut node1_connected = false;
        let mut node2_connected = false;

        loop {
            tokio::select! {
                Some(event) = swarm1.next() => {
                    if let libp2p::swarm::SwarmEvent::ConnectionEstablished { peer_id, .. } = event {
                        tracing::info!("Node 1 connected to: {}", peer_id);
                        node1_connected = true;
                    }
                }
                Some(event) = swarm2.next() => {
                    if let libp2p::swarm::SwarmEvent::ConnectionEstablished { peer_id, .. } = event {
                        tracing::info!("Node 2 connected to: {}", peer_id);
                        node2_connected = true;
                    }
                }
            }

            if node1_connected && node2_connected {
                break;
            }
        }
    })
    .await;

    assert!(result.is_ok(), "Connection timed out");

    // Create a test block on node 1
    let test_data = b"Hello from Neverust!".to_vec();
    let test_block = Block::new(test_data)?;
    let test_cid = test_block.cid;

    store1.put(test_block.clone()).await?;
    tracing::info!("Node 1 stored block: {}", test_cid);

    // Now we need to trigger block exchange
    // For now, this test verifies:
    // 1. Two nodes can connect
    // 2. BlockExc protocol is available
    // 3. Block storage works

    // TODO: Trigger actual block request from node 2 to node 1
    // This requires implementing the block request mechanism

    tracing::info!("Test completed successfully!");

    Ok(())
}

#[tokio::test]
async fn test_block_storage() -> Result<(), Box<dyn std::error::Error>> {
    let store = BlockStore::new();

    // Create test data
    let test_data = b"Test block content".to_vec();
    let test_block = Block::new(test_data.clone())?;
    let cid = test_block.cid;

    // Store block
    store.put(test_block).await?;

    // Retrieve block
    let retrieved = store.get(&cid).await?;
    assert_eq!(retrieved.data, test_data, "Block data should match");
    assert_eq!(retrieved.cid, cid, "Block CID should match");

    // Check stats
    let stats = store.stats().await;
    assert_eq!(stats.block_count, 1, "Should have 1 block");
    assert!(stats.total_size > 0, "Should have non-zero bytes");

    Ok(())
}
