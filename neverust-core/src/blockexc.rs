//! BlockExc protocol implementation
//!
//! Implements Archivist's custom BlockExc protocol for block exchange.
//! Protocol ID: /archivist/blockexc/1.0.0

use libp2p::core::upgrade::ReadyUpgrade;
use libp2p::swarm::{
    handler::{ConnectionEvent, FullyNegotiatedInbound, FullyNegotiatedOutbound},
    ConnectionHandler, ConnectionHandlerEvent, KeepAlive, StreamProtocol, SubstreamProtocol,
};
use libp2p::PeerId;
use std::io;
use std::sync::Arc;
use tracing::{info, warn};

use crate::metrics::Metrics;
use crate::storage::BlockStore;

pub const PROTOCOL_ID: &str = "/archivist/blockexc/1.0.0";

/// BlockExc connection handler
pub struct BlockExcHandler {
    peer_id: PeerId,
    keep_alive: KeepAlive,
    /// Whether we've requested an outbound stream
    outbound_requested: bool,
    /// Whether we have an active stream (inbound or outbound)
    has_active_stream: bool,
    /// Shared block store for reading/writing blocks
    block_store: Arc<BlockStore>,
    /// Node operating mode (altruistic or marketplace)
    mode: String,
    /// Price per byte in marketplace mode
    price_per_byte: u64,
    /// Metrics collector for tracking P2P traffic
    metrics: Metrics,
}

impl BlockExcHandler {
    pub fn new(
        peer_id: PeerId,
        block_store: Arc<BlockStore>,
        mode: String,
        price_per_byte: u64,
        metrics: Metrics,
    ) -> Self {
        BlockExcHandler {
            peer_id,
            keep_alive: KeepAlive::Yes,
            outbound_requested: false,
            has_active_stream: false,
            block_store,
            mode,
            price_per_byte,
            metrics,
        }
    }
}

impl ConnectionHandler for BlockExcHandler {
    type FromBehaviour = ();
    type ToBehaviour = ();
    #[allow(deprecated)]
    type Error = io::Error;
    type InboundProtocol = ReadyUpgrade<StreamProtocol>;
    type OutboundProtocol = ReadyUpgrade<StreamProtocol>;
    type InboundOpenInfo = ();
    type OutboundOpenInfo = ();

    fn listen_protocol(&self) -> SubstreamProtocol<Self::InboundProtocol, Self::InboundOpenInfo> {
        SubstreamProtocol::new(ReadyUpgrade::new(StreamProtocol::new(PROTOCOL_ID)), ())
    }

    fn on_behaviour_event(&mut self, _event: Self::FromBehaviour) {}

    fn connection_keep_alive(&self) -> KeepAlive {
        self.keep_alive
    }

    #[allow(deprecated)]
    fn poll(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<
        ConnectionHandlerEvent<
            Self::OutboundProtocol,
            Self::OutboundOpenInfo,
            Self::ToBehaviour,
            Self::Error,
        >,
    > {
        // DISABLED: Automatic outbound streams cause DialUpgradeErrors with Archivist testnet nodes
        // Testnet bootstrap nodes only accept inbound requests (server-side only)
        // For peer-to-peer Neverust communication, we'll implement demand-driven outbound streams
        // when we actually need to request blocks (not automatically on every connection)

        // TODO: Implement on-demand outbound stream requests:
        // - When we want to request specific blocks via WantList
        // - Detect peer capabilities (Neverust vs flagship) via identify protocol
        // - Only auto-dial for Neverust-to-Neverust communication

        std::task::Poll::Pending
    }

    fn on_connection_event(
        &mut self,
        event: ConnectionEvent<
            Self::InboundProtocol,
            Self::OutboundProtocol,
            Self::InboundOpenInfo,
            Self::OutboundOpenInfo,
        >,
    ) {
        match event {
            ConnectionEvent::FullyNegotiatedInbound(FullyNegotiatedInbound {
                protocol: stream,
                ..
            }) => {
                self.has_active_stream = true;
                let peer_id = self.peer_id;
                let block_store = self.block_store.clone();
                let mode = self.mode.clone();
                let price_per_byte = self.price_per_byte;
                let metrics = self.metrics.clone();
                info!("BlockExc: Fully negotiated inbound stream from {} (mode: {}, price: {} per byte)", peer_id, mode, price_per_byte);

                // Spawn task to handle the stream - read messages from remote peer
                tokio::spawn(async move {
                    use crate::messages::{
                        decode_message, encode_message, Block as MsgBlock, Message,
                    };
                    use cid::Cid;
                    use libp2p::core::upgrade::{read_length_prefixed, write_length_prefixed};

                    let mut stream = stream;
                    info!("BlockExc: Started reading from {}", peer_id);

                    loop {
                        // Try to read a length-prefixed message
                        match read_length_prefixed(&mut stream, 100 * 1024 * 1024).await {
                            Ok(data) => {
                                info!("BlockExc: Received {} bytes from {}", data.len(), peer_id);

                                // Try to decode the message
                                match decode_message(&data) {
                                    Ok(msg) => {
                                        info!("BlockExc: Decoded message from {}: wantlist={}, blocks={}, presences={}",
                                            peer_id,
                                            msg.wantlist.is_some(),
                                            msg.payload.len(),
                                            msg.block_presences.len()
                                        );

                                        // If they sent a wantlist, respond with blocks we have
                                        if let Some(wantlist) = msg.wantlist {
                                            use crate::messages::BlockPresence;

                                            if mode == "altruistic" {
                                                // ALTRUISTIC MODE: Serve blocks freely without payment
                                                info!("BlockExc: ALTRUISTIC MODE - serving blocks freely to {}", peer_id);
                                                let mut response_blocks = Vec::new();

                                                for entry in &wantlist.entries {
                                                    if let Ok(cid) = Cid::try_from(&entry.block[..])
                                                    {
                                                        if let Ok(block) =
                                                            block_store.get(&cid).await
                                                        {
                                                            let total_size = block.data.len() as u64;

                                                            // Check if this is a range request (Neverust extension)
                                                            let is_range_request = entry.start_byte != 0 || entry.end_byte != 0;

                                                            let (data, range_start, range_end) = if is_range_request {
                                                                // Range request - extract requested byte range
                                                                let start = entry.start_byte as usize;
                                                                let end = if entry.end_byte == 0 {
                                                                    total_size as usize
                                                                } else {
                                                                    std::cmp::min(entry.end_byte as usize, total_size as usize)
                                                                };

                                                                if start < block.data.len() && start < end {
                                                                    let range_data = block.data[start..end].to_vec();
                                                                    info!("BlockExc: Serving range [{}, {}) of block {} to {} (altruistic) - {} bytes of {}",
                                                                        start, end, cid, peer_id, range_data.len(), total_size);
                                                                    (range_data, start as u64, end as u64)
                                                                } else {
                                                                    warn!("BlockExc: Invalid range [{}, {}) for block {} (size: {})",
                                                                        start, end, cid, total_size);
                                                                    continue;
                                                                }
                                                            } else {
                                                                // Full block request (backward compatible)
                                                                info!("BlockExc: Serving full block {} to {} (altruistic) - {} bytes",
                                                                    cid, peer_id, total_size);
                                                                (block.data.clone(), 0, 0)
                                                            };

                                                            metrics.block_sent(data.len()); // Track P2P traffic!
                                                            response_blocks.push(MsgBlock {
                                                                prefix: cid.to_bytes()[0..4].to_vec(),
                                                                data,
                                                                range_start,
                                                                range_end,
                                                                total_size,
                                                            });
                                                        }
                                                    }
                                                }

                                                let response = Message {
                                                    wantlist: None,
                                                    payload: response_blocks,
                                                    block_presences: vec![],
                                                    pending_bytes: 0,
                                                    account: None,
                                                    payment: None,
                                                };

                                                if let Ok(response_bytes) =
                                                    encode_message(&response)
                                                {
                                                    if let Err(e) = write_length_prefixed(
                                                        &mut stream,
                                                        &response_bytes,
                                                    )
                                                    .await
                                                    {
                                                        warn!("BlockExc: Failed to send response to {}: {}", peer_id, e);
                                                        break;
                                                    }
                                                }
                                            } else if mode == "marketplace" {
                                                // MARKETPLACE MODE: Check payment before serving
                                                info!("BlockExc: MARKETPLACE MODE - checking payment from {}", peer_id);

                                                let has_payment = msg.payment.is_some();

                                                if has_payment {
                                                    info!("BlockExc: Payment received from {}, serving blocks", peer_id);
                                                    // Payment received - serve blocks
                                                    let mut response_blocks = Vec::new();

                                                    for entry in &wantlist.entries {
                                                        if let Ok(cid) =
                                                            Cid::try_from(&entry.block[..])
                                                        {
                                                            if let Ok(block) =
                                                                block_store.get(&cid).await
                                                            {
                                                                let total_size = block.data.len() as u64;

                                                                // Check if this is a range request (Neverust extension)
                                                                let is_range_request = entry.start_byte != 0 || entry.end_byte != 0;

                                                                let (data, range_start, range_end) = if is_range_request {
                                                                    // Range request - extract requested byte range
                                                                    let start = entry.start_byte as usize;
                                                                    let end = if entry.end_byte == 0 {
                                                                        total_size as usize
                                                                    } else {
                                                                        std::cmp::min(entry.end_byte as usize, total_size as usize)
                                                                    };

                                                                    if start < block.data.len() && start < end {
                                                                        let range_data = block.data[start..end].to_vec();
                                                                        info!("BlockExc: Serving range [{}, {}) of block {} to {} (paid) - {} bytes of {}",
                                                                            start, end, cid, peer_id, range_data.len(), total_size);
                                                                        (range_data, start as u64, end as u64)
                                                                    } else {
                                                                        warn!("BlockExc: Invalid range [{}, {}) for block {} (size: {})",
                                                                            start, end, cid, total_size);
                                                                        continue;
                                                                    }
                                                                } else {
                                                                    // Full block request (backward compatible)
                                                                    info!("BlockExc: Serving full block {} to {} (paid) - {} bytes",
                                                                        cid, peer_id, total_size);
                                                                    (block.data.clone(), 0, 0)
                                                                };

                                                                metrics.block_sent(data.len()); // Track P2P traffic!
                                                                response_blocks.push(MsgBlock {
                                                                    prefix: cid.to_bytes()[0..4].to_vec(),
                                                                    data,
                                                                    range_start,
                                                                    range_end,
                                                                    total_size,
                                                                });
                                                            }
                                                        }
                                                    }

                                                    let response = Message {
                                                        wantlist: None,
                                                        payload: response_blocks,
                                                        block_presences: vec![],
                                                        pending_bytes: 0,
                                                        account: None,
                                                        payment: None,
                                                    };

                                                    if let Ok(response_bytes) =
                                                        encode_message(&response)
                                                    {
                                                        if let Err(e) = write_length_prefixed(
                                                            &mut stream,
                                                            &response_bytes,
                                                        )
                                                        .await
                                                        {
                                                            warn!("BlockExc: Failed to send response to {}: {}", peer_id, e);
                                                            break;
                                                        }
                                                    }
                                                } else {
                                                    // No payment - send block presences with prices
                                                    info!("BlockExc: No payment from {}, sending presences with prices", peer_id);
                                                    let mut block_presences = Vec::new();

                                                    for entry in &wantlist.entries {
                                                        if let Ok(cid) =
                                                            Cid::try_from(&entry.block[..])
                                                        {
                                                            if let Ok(block) =
                                                                block_store.get(&cid).await
                                                            {
                                                                let block_price = (block.data.len()
                                                                    as u64)
                                                                    * price_per_byte;
                                                                info!("BlockExc: Block {} available for {} units", cid, block_price);

                                                                block_presences.push(
                                                                    BlockPresence {
                                                                        cid: cid.to_bytes(),
                                                                        r#type: 0, // Have
                                                                        price: block_price
                                                                            .to_le_bytes()
                                                                            .to_vec(),
                                                                    },
                                                                );
                                                            }
                                                        }
                                                    }

                                                    let response = Message {
                                                        wantlist: None,
                                                        payload: vec![],
                                                        block_presences,
                                                        pending_bytes: 0,
                                                        account: None,
                                                        payment: None,
                                                    };

                                                    if let Ok(response_bytes) =
                                                        encode_message(&response)
                                                    {
                                                        if let Err(e) = write_length_prefixed(
                                                            &mut stream,
                                                            &response_bytes,
                                                        )
                                                        .await
                                                        {
                                                            warn!("BlockExc: Failed to send response to {}: {}", peer_id, e);
                                                            break;
                                                        }
                                                    }
                                                }
                                            } else {
                                                warn!("BlockExc: Unknown mode '{}', defaulting to altruistic", mode);
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        warn!(
                                            "BlockExc: Failed to decode message from {}: {}",
                                            peer_id, e
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                if e.kind() != io::ErrorKind::UnexpectedEof {
                                    warn!("BlockExc: Error reading from {}: {}", peer_id, e);
                                }
                                break;
                            }
                        }
                    }

                    info!("BlockExc: Finished reading from {}", peer_id);
                });
            }
            ConnectionEvent::FullyNegotiatedOutbound(FullyNegotiatedOutbound {
                protocol: stream,
                ..
            }) => {
                self.has_active_stream = true;
                let peer_id = self.peer_id;
                let block_store = self.block_store.clone();
                let metrics = self.metrics.clone();
                info!("BlockExc: Fully negotiated outbound stream to {}", peer_id);

                // Spawn task to handle outbound stream - send WantList and receive blocks
                tokio::spawn(async move {
                    use crate::cid_blake3::blake3_cid;
                    use crate::messages::{
                        decode_message, encode_message, Message, WantType, Wantlist, WantlistEntry,
                    };
                    use crate::storage::Block;
                    use libp2p::core::upgrade::{read_length_prefixed, write_length_prefixed};

                    let mut stream = stream;

                    // Create a test CID - let's hash "Hello, Archivist!" and request that block
                    let test_data = b"Hello, Archivist!";
                    let test_cid = blake3_cid(test_data).expect("Failed to create test CID");

                    info!(
                        "BlockExc: Requesting test block {} from {}",
                        test_cid, peer_id
                    );

                    // Create WantList with test CID
                    let wantlist = Wantlist {
                        entries: vec![WantlistEntry {
                            block: test_cid.to_bytes(),
                            priority: 1,
                            cancel: false,
                            want_type: WantType::WantBlock as i32,
                            send_dont_have: true,
                            start_byte: 0, // Full block (backward compatible)
                            end_byte: 0,   // Full block (backward compatible)
                        }],
                        full: true,
                    };

                    let msg = Message {
                        wantlist: Some(wantlist),
                        payload: vec![],
                        block_presences: vec![],
                        pending_bytes: 0,
                        account: None,
                        payment: None,
                    };

                    let msg_bytes = match encode_message(&msg) {
                        Ok(bytes) => bytes,
                        Err(e) => {
                            warn!("BlockExc: Failed to encode WantList: {}", e);
                            return;
                        }
                    };

                    info!(
                        "BlockExc: Sending WantList ({} bytes) to {}",
                        msg_bytes.len(),
                        peer_id
                    );
                    if let Err(e) = write_length_prefixed(&mut stream, &msg_bytes).await {
                        warn!("BlockExc: Failed to send WantList to {}: {}", peer_id, e);
                        return;
                    }

                    // Listen for responses (blocks or presences)
                    loop {
                        match read_length_prefixed(&mut stream, 100 * 1024 * 1024).await {
                            Ok(data) => {
                                info!(
                                    "BlockExc: Received {} bytes from {} on outbound stream",
                                    data.len(),
                                    peer_id
                                );

                                match decode_message(&data) {
                                    Ok(response) => {
                                        info!(
                                            "BlockExc: Response from {}: blocks={}, presences={}",
                                            peer_id,
                                            response.payload.len(),
                                            response.block_presences.len()
                                        );

                                        // Store received blocks
                                        for msg_block in &response.payload {
                                            info!("BlockExc: Received block! prefix_len={}, data_len={}",
                                                msg_block.prefix.len(), msg_block.data.len());

                                            // Compute CID from data and verify
                                            match blake3_cid(&msg_block.data) {
                                                Ok(computed_cid) => {
                                                    // Create Block and store it
                                                    let block = Block {
                                                        cid: computed_cid,
                                                        data: msg_block.data.clone(),
                                                    };

                                                    let block_size = msg_block.data.len();
                                                    match block_store.put(block).await {
                                                        Ok(_) => {
                                                            info!("BlockExc: Stored block {} from {} - {} bytes", computed_cid, peer_id, block_size);
                                                            metrics.block_received(block_size);
                                                            // Track P2P traffic!
                                                        }
                                                        Err(e) => {
                                                            warn!("BlockExc: Failed to store block: {}", e);
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    warn!("BlockExc: Failed to compute CID for received block: {}", e);
                                                }
                                            }
                                        }

                                        // Log block presences
                                        for presence in &response.block_presences {
                                            info!(
                                                "BlockExc: Block presence type={:?}",
                                                presence.r#type
                                            );
                                        }
                                    }
                                    Err(e) => {
                                        warn!(
                                            "BlockExc: Failed to decode response from {}: {}",
                                            peer_id, e
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                if e.kind() != io::ErrorKind::UnexpectedEof {
                                    warn!(
                                        "BlockExc: Error reading from {} on outbound: {}",
                                        peer_id, e
                                    );
                                }
                                break;
                            }
                        }
                    }

                    info!("BlockExc: Finished outbound stream to {}", peer_id);
                });
            }
            ConnectionEvent::DialUpgradeError(err) => {
                warn!(
                    "BlockExc: Dial upgrade error to {}: {:?}",
                    self.peer_id, err
                );
            }
            ConnectionEvent::AddressChange(_)
            | ConnectionEvent::ListenUpgradeError(_)
            | ConnectionEvent::LocalProtocolsChange(_)
            | ConnectionEvent::RemoteProtocolsChange(_) => {}
        }
    }
}

/// BlockExc network behaviour
pub struct BlockExcBehaviour {
    block_store: Arc<BlockStore>,
    mode: String,
    price_per_byte: u64,
    metrics: Metrics,
}

impl BlockExcBehaviour {
    pub fn new(
        block_store: Arc<BlockStore>,
        mode: String,
        price_per_byte: u64,
        metrics: Metrics,
    ) -> Self {
        Self {
            block_store,
            mode,
            price_per_byte,
            metrics,
        }
    }
}

impl libp2p::swarm::NetworkBehaviour for BlockExcBehaviour {
    type ConnectionHandler = BlockExcHandler;
    type ToSwarm = ();

    fn handle_established_inbound_connection(
        &mut self,
        _connection_id: libp2p::swarm::ConnectionId,
        peer: PeerId,
        _local_addr: &libp2p::Multiaddr,
        _remote_addr: &libp2p::Multiaddr,
    ) -> Result<libp2p::swarm::THandler<Self>, libp2p::swarm::ConnectionDenied> {
        Ok(BlockExcHandler::new(
            peer,
            self.block_store.clone(),
            self.mode.clone(),
            self.price_per_byte,
            self.metrics.clone(),
        ))
    }

    fn handle_established_outbound_connection(
        &mut self,
        _connection_id: libp2p::swarm::ConnectionId,
        peer: PeerId,
        _addr: &libp2p::Multiaddr,
        _role_override: libp2p::core::Endpoint,
    ) -> Result<libp2p::swarm::THandler<Self>, libp2p::swarm::ConnectionDenied> {
        Ok(BlockExcHandler::new(
            peer,
            self.block_store.clone(),
            self.mode.clone(),
            self.price_per_byte,
            self.metrics.clone(),
        ))
    }

    fn on_swarm_event(&mut self, _event: libp2p::swarm::FromSwarm<Self::ConnectionHandler>) {}

    fn on_connection_handler_event(
        &mut self,
        _peer_id: PeerId,
        _connection_id: libp2p::swarm::ConnectionId,
        _event: libp2p::swarm::THandlerOutEvent<Self>,
    ) {
    }

    fn poll(
        &mut self,
        _cx: &mut std::task::Context<'_>,
        _params: &mut impl libp2p::swarm::PollParameters,
    ) -> std::task::Poll<libp2p::swarm::ToSwarm<Self::ToSwarm, libp2p::swarm::THandlerInEvent<Self>>>
    {
        std::task::Poll::Pending
    }
}
