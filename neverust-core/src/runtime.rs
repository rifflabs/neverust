//! Async runtime and event loop for the node
//!
//! Handles the main event loop, processing Swarm events and managing
//! the lifecycle of the P2P node.

use crate::{config::Config, p2p::{create_swarm, P2PError}};
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

                        // Once TCP is listening, dial bootstrap nodes
                        if tcp_listening && !bootstrapped {
                            info!("Listen addresses established, dialing bootstrap nodes...");

                            // Dial all bootstrap peers directly
                            // (Archivist doesn't use Kademlia - uses custom BlockExc protocol)
                            for node_addr in &bootstrap_addrs {
                                info!("Dialing bootstrap: {}", node_addr);
                                if let Ok(addr) = node_addr.parse::<Multiaddr>() {
                                    if let Err(e) = swarm.dial(addr.clone()) {
                                        error!("Failed to dial bootstrap peer {}: {:?}", node_addr, e);
                                    } else {
                                        info!("Dialing {}", node_addr);
                                    }
                                } else {
                                    warn!("Invalid bootstrap address: {}", node_addr);
                                }
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
                    SwarmEvent::Behaviour(_event) => {
                        // BlockExc events (currently just () as placeholder)
                        info!("BlockExc event");
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
