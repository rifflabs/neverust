//! Async runtime and event loop for the node
//!
//! Handles the main event loop, processing Swarm events and managing
//! the lifecycle of the P2P node.

use crate::{config::Config, p2p::{create_swarm, BehaviourEvent, P2PError}};
use futures::StreamExt;
use libp2p::{
    swarm::SwarmEvent, Multiaddr,
};
use tokio::signal;
use tracing::{error, info, warn};

/// Run the Archivist node with the given configuration
pub async fn run_node(config: Config) -> Result<(), P2PError> {
    // Create swarm
    let mut swarm = create_swarm().await?;

    // Start listening on configured port
    let listen_addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", config.listen_port)
        .parse()
        .map_err(|e| P2PError::Transport(format!("Invalid listen address: {}", e)))?;

    swarm
        .listen_on(listen_addr)
        .map_err(|e| P2PError::Transport(format!("Failed to listen: {}", e)))?;

    info!("Node started with peer ID: {}", swarm.local_peer_id());
    info!("Listening on TCP port {}", config.listen_port);
    info!("Discovery on UDP port {}", config.disc_port);

    // Add bootstrap nodes to Kademlia
    let bootstrap_addrs = if config.bootstrap_nodes.is_empty() {
        info!("No bootstrap nodes configured, fetching from testnet...");
        Config::fetch_testnet_bootstrap_nodes().await
            .map_err(|e| P2PError::Transport(format!("Failed to fetch bootstrap nodes: {}", e)))?
    } else {
        config.bootstrap_nodes.clone()
    };

    for node_addr in &bootstrap_addrs {
        info!("Processing bootstrap: {}", node_addr);
        if let Ok(addr) = node_addr.parse::<Multiaddr>() {
            // Extract PeerId from multiaddr if present
            if let Some(libp2p::multiaddr::Protocol::P2p(peer_id)) = addr.iter().last() {
                swarm.behaviour_mut().kademlia.add_address(&peer_id, addr.clone());
                info!("Added bootstrap peer: {} at {}", peer_id, addr);
            } else {
                warn!("Bootstrap address missing peer ID: {}", node_addr);
            }
        } else {
            warn!("Invalid bootstrap address (not a valid multiaddr): {}", node_addr);
        }
    }

    // Subscribe to Archivist Gossipsub topics
    let blocks_topic = libp2p::gossipsub::IdentTopic::new("blocks");
    let transactions_topic = libp2p::gossipsub::IdentTopic::new("transactions");

    swarm.behaviour_mut().gossipsub.subscribe(&blocks_topic)
        .map_err(|e| P2PError::Swarm(format!("Failed to subscribe to blocks topic: {}", e)))?;
    swarm.behaviour_mut().gossipsub.subscribe(&transactions_topic)
        .map_err(|e| P2PError::Swarm(format!("Failed to subscribe to transactions topic: {}", e)))?;

    info!("Subscribed to Gossipsub topics: blocks, transactions");

    // Bootstrap the Kademlia DHT
    if let Err(e) = swarm.behaviour_mut().kademlia.bootstrap() {
        warn!("Kademlia bootstrap failed: {:?}", e);
    } else {
        info!("Kademlia bootstrap initiated");
    }

    // Main event loop
    loop {
        tokio::select! {
            event = swarm.select_next_some() => {
                match event {
                    SwarmEvent::NewListenAddr { address, .. } => {
                        info!("Listening on {}", address);
                    }
                    SwarmEvent::ConnectionEstablished {
                        peer_id,
                        endpoint,
                        ..
                    } => {
                        info!(
                            "Connected to peer: {} at {}",
                            peer_id,
                            endpoint.get_remote_address()
                        );
                    }
                    SwarmEvent::ConnectionClosed {
                        peer_id,
                        cause,
                        ..
                    } => {
                        warn!("Connection closed with {}: {:?}", peer_id, cause);
                    }
                    SwarmEvent::Behaviour(event) => {
                        match event {
                            BehaviourEvent::Ping(ping_event) => {
                                info!("Ping event: {:?}", ping_event);
                            }
                            BehaviourEvent::Identify(identify_event) => {
                                info!("Identify event: {:?}", identify_event);
                            }
                            BehaviourEvent::Kademlia(kad_event) => {
                                info!("Kademlia event: {:?}", kad_event);
                            }
                            BehaviourEvent::Gossipsub(gossipsub_event) => {
                                info!("Gossipsub event: {:?}", gossipsub_event);
                            }
                        }
                    }
                    SwarmEvent::IncomingConnection { local_addr, send_back_addr, .. } => {
                        info!("Incoming connection from {} on {}", send_back_addr, local_addr);
                    }
                    SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                        error!("Outgoing connection error to {:?}: {}", peer_id, error);
                    }
                    SwarmEvent::IncomingConnectionError { local_addr, send_back_addr, error, .. } => {
                        error!("Incoming connection error from {} on {}: {}", send_back_addr, local_addr, error);
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
