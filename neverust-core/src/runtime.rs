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
