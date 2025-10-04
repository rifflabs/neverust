//! Async runtime and event loop for the node
//!
//! Handles the main event loop, processing Swarm events and managing
//! the lifecycle of the P2P node.

use crate::{
    api,
    botg::{BoTgConfig, BoTgProtocol},
    config::Config,
    metrics::Metrics,
    p2p::{create_swarm, P2PError},
    storage::BlockStore,
    traffic,
};
use futures::StreamExt;
use libp2p::{swarm::SwarmEvent, Multiaddr};
use std::sync::Arc;
use tokio::signal;
use tracing::{error, info, warn};

/// Run the Archivist node with the given configuration
pub async fn run_node(config: Config) -> Result<(), P2PError> {
    // Create block store
    let block_store = Arc::new(BlockStore::new());
    info!("Initialized block store");

    // Create metrics collector
    let metrics = Metrics::new();
    info!("Initialized metrics collector");

    // Create swarm first to get peer ID (pass metrics for P2P traffic tracking)
    let mut swarm = create_swarm(
        block_store.clone(),
        config.mode.clone(),
        config.price_per_byte,
        metrics.clone(),
    )
    .await?;
    let peer_id = swarm.local_peer_id().to_string();

    // Initialize BoTG (Block-over-TGP) protocol for high-speed block exchange
    info!(
        "Initializing BoTG protocol on UDP port {}",
        config.disc_port
    );
    let botg_config = BoTgConfig {
        local_peer_id: rand::random(), // Generate random peer ID for TGP
        epoch: 0,
        ..Default::default()
    };

    let bind_addr: std::net::SocketAddr = format!("0.0.0.0:{}", config.disc_port)
        .parse()
        .map_err(|e| P2PError::Transport(format!("Invalid bind address: {}", e)))?;

    // Create UDP socket for BoTG
    let udp_socket = Arc::new(
        tokio::net::UdpSocket::bind(bind_addr)
            .await
            .map_err(|e| P2PError::Transport(format!("Failed to bind UDP socket: {}", e)))?,
    );
    info!("BoTG: UDP socket bound to {}", bind_addr);

    // Create BoTG protocol and configure it
    let mut botg_protocol = BoTgProtocol::new(botg_config);
    botg_protocol.set_udp_socket(udp_socket);
    botg_protocol.set_block_store(block_store.clone());
    botg_protocol.set_metrics(metrics.clone());

    let botg = Arc::new(botg_protocol);

    // Start BoTG receive loop
    botg.clone().start_receive_loop();
    info!("BoTG ready for high-speed block exchange via UDP");

    // Add peers to BoTG for P2P communication (Docker network autodiscovery)
    tokio::spawn({
        let botg = botg.clone();
        async move {
            // Wait for nodes to start
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

            // Generate all 50 Docker network peers
            let mut docker_peers = vec![
                "172.25.0.10:8090".to_string(), // bootstrap
            ];

            // Add node1-49 (172.25.1.2 through 172.25.1.50)
            for i in 1..=49 {
                docker_peers.push(format!("172.25.1.{}:8090", i + 1));
            }

            let mut added = 0;
            for peer_str in &docker_peers {
                if let Ok(peer_addr) = peer_str.parse() {
                    botg.add_peer(peer_addr).await;
                    added += 1;
                }
            }
            info!(
                "BoTG: Added {} Docker network peers for P2P communication",
                added
            );
        }
    });

    // Start REST API server in background with peer ID and BoTG
    let api_block_store = block_store.clone();
    let api_metrics = metrics.clone();
    let api_peer_id = peer_id.clone();
    let api_botg = botg.clone();
    let api_port = config.api_port;
    tokio::spawn(async move {
        let app = api::create_router(api_block_store, api_metrics, api_peer_id, api_botg);
        let addr = format!("0.0.0.0:{}", api_port);
        info!("Starting REST API on {}", addr);

        let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
        axum::serve(listener, app).await.unwrap();
    });

    // Start autonomous traffic generator if enabled
    if traffic::is_enabled() {
        info!("Traffic generator enabled - starting autonomous P2P traffic");
        let traffic_config = traffic::config_from_env(peer_id.clone(), config.api_port);
        let traffic_store = block_store.clone();
        let traffic_botg = botg.clone();

        // Create P2P command channel for traffic generator
        let (p2p_tx, mut p2p_rx) = tokio::sync::mpsc::unbounded_channel();

        tokio::spawn(async move {
            traffic::start_traffic_generator(traffic_config, traffic_store, traffic_botg, p2p_tx)
                .await;
        });

        // Handle P2P commands from traffic generator
        let cmd_botg = botg.clone();
        tokio::spawn(async move {
            while let Some(cmd) = p2p_rx.recv().await {
                match cmd {
                    traffic::P2PCommand::RequestBlock(cid) => {
                        info!("[P2P-BoTG] Requesting block {} via BoTG/TGP", cid);
                        cmd_botg.request_blocks_by_cid(vec![cid]).await;
                    }
                    traffic::P2PCommand::AdvertiseBlock(cid) => {
                        info!("[P2P-BoTG] Advertising block {} via BoTG/TGP", cid);
                        cmd_botg.announce_blocks(vec![cid]).await;
                    }
                }
            }
        });
    }

    // Start listening on TCP (Archivist uses TCP+Noise+Mplex, NOT QUIC)
    let tcp_addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", config.listen_port)
        .parse()
        .map_err(|e| P2PError::Transport(format!("Invalid TCP address: {}", e)))?;

    swarm
        .listen_on(tcp_addr.clone())
        .map_err(|e| P2PError::Transport(format!("Failed to listen on TCP {}: {}", tcp_addr, e)))?;

    info!("Node started with peer ID: {}", swarm.local_peer_id());

    // Fetch bootstrap nodes early
    let bootstrap_addrs = if config.bootstrap_nodes.is_empty() {
        info!("No bootstrap nodes configured, fetching...");
        Config::fetch_bootstrap_nodes()
            .await
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
                        // Track transport types
                        if address.to_string().contains("/tcp/") {
                            info!("Listening on TCP: {}", address);
                            tcp_listening = true;
                        } else {
                            info!("Listening on {}", address);
                        }

                        // Once TCP is listening, dial bootstrap nodes
                        if tcp_listening && !bootstrapped {
                            info!("TCP transport ready, dialing bootstrap nodes...");

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
                        metrics.peer_connected();
                    }
                    SwarmEvent::ConnectionClosed {
                        peer_id,
                        cause,
                        ..
                    } => {
                        warn!("Connection closed with {}: {:?}", peer_id, cause);
                        metrics.peer_disconnected();
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
