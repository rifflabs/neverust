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

    // Start listening on TCP
    let tcp_addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", config.listen_port)
        .parse()
        .map_err(|e| P2PError::Transport(format!("Invalid TCP address: {}", e)))?;

    swarm
        .listen_on(tcp_addr.clone())
        .map_err(|e| P2PError::Transport(format!("Failed to listen on TCP {}: {}", tcp_addr, e)))?;

    info!("Node started with peer ID: {}", swarm.local_peer_id());

    // Fetch bootstrap nodes early
    let bootstrap_addrs = if config.bootstrap_nodes.is_empty() {
        info!("No bootstrap nodes configured, fetching from testnet...");
        Config::fetch_testnet_bootstrap_nodes().await
            .map_err(|e| P2PError::Transport(format!("Failed to fetch bootstrap nodes: {}", e)))?
    } else {
        config.bootstrap_nodes.clone()
    };

    // Track if we've established listen addresses
    let mut tcp_listening = false;
    let mut bootstrapped = false;

    // Main event loop
    loop {
        tokio::select! {
            event = swarm.select_next_some() => {
                match event {
                    SwarmEvent::NewListenAddr { address, .. } => {
                        info!("Listening on {}", address);

                        // Track TCP listening
                        if address.to_string().contains("/tcp/") {
                            tcp_listening = true;
                        }

                        // Once TCP is listening, add bootstrap nodes
                        if tcp_listening && !bootstrapped {
                            info!("Listen addresses established, adding bootstrap nodes...");

                            for node_addr in &bootstrap_addrs {
                                info!("Processing bootstrap: {}", node_addr);
                                if let Ok(addr) = node_addr.parse::<Multiaddr>() {
                                    if let Some(libp2p::multiaddr::Protocol::P2p(peer_id)) = addr.iter().last() {
                                        swarm.behaviour_mut().kademlia.add_address(&peer_id, addr.clone());
                                        info!("Added bootstrap peer: {} at {}", peer_id, addr);
                                    } else {
                                        warn!("Bootstrap address missing peer ID: {}", node_addr);
                                    }
                                } else {
                                    warn!("Invalid bootstrap address: {}", node_addr);
                                }
                            }

                            // Subscribe to Gossipsub topics
                            let blocks_topic = libp2p::gossipsub::IdentTopic::new("blocks");
                            let transactions_topic = libp2p::gossipsub::IdentTopic::new("transactions");

                            if let Err(e) = swarm.behaviour_mut().gossipsub.subscribe(&blocks_topic) {
                                warn!("Failed to subscribe to blocks topic: {}", e);
                            }
                            if let Err(e) = swarm.behaviour_mut().gossipsub.subscribe(&transactions_topic) {
                                warn!("Failed to subscribe to transactions topic: {}", e);
                            }
                            info!("Subscribed to Gossipsub topics: blocks, transactions");

                            // Explicitly dial first bootstrap peer to test connection
                            if let Some(first_bootstrap) = bootstrap_addrs.first() {
                                if let Ok(addr) = first_bootstrap.parse::<Multiaddr>() {
                                    info!("Explicitly dialing first bootstrap peer: {}", addr);
                                    if let Err(e) = swarm.dial(addr.clone()) {
                                        error!("Failed to dial bootstrap peer: {:?}", e);
                                    }
                                }
                            }

                            // Bootstrap Kademlia
                            if let Err(e) = swarm.behaviour_mut().kademlia.bootstrap() {
                                warn!("Kademlia bootstrap failed: {:?}", e);
                            } else {
                                info!("Kademlia bootstrap initiated");
                            }

                            bootstrapped = true;
                        }
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
