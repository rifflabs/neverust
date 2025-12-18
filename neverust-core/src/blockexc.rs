//! BlockExc protocol implementation
//!
//! Implements Archivist's custom BlockExc protocol for block exchange.
//! Protocol ID: /archivist/blockexc/1.0.0

use futures::AsyncReadExt;
use futures::AsyncWriteExt;
use libp2p::core::upgrade::ReadyUpgrade;
use libp2p::swarm::{
    handler::{ConnectionEvent, FullyNegotiatedInbound, FullyNegotiatedOutbound},
    ConnectionHandler, ConnectionHandlerEvent, StreamProtocol, SubstreamProtocol,
};
use libp2p::PeerId;
use serde::Deserialize;
use serde::Serialize;
use std::io;
use std::sync::Arc;
use tracing::{info, warn};

use crate::discovery::Discovery;
use crate::metrics::Metrics;
use crate::storage::BlockStore;

pub const PROTOCOL_ID: &str = "/archivist/blockexc/1.0.0";

/// Read a length-prefixed message from a stream
async fn read_length_prefixed<R: AsyncReadExt + Unpin>(
    reader: &mut R,
    max_size: usize,
) -> io::Result<Vec<u8>> {
    // Read the length prefix (unsigned varint)
    let mut length = 0u64;
    let mut shift = 0;
    loop {
        let mut buf = [0u8; 1];
        reader.read_exact(&mut buf).await?;
        let byte = buf[0];

        length |= ((byte & 0x7F) as u64) << shift;
        shift += 7;

        if byte & 0x80 == 0 {
            break;
        }

        if shift >= 64 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "varint too long",
            ));
        }
    }

    if length > max_size as u64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("message too large: {} > {}", length, max_size),
        ));
    }

    let mut data = vec![0u8; length as usize];
    reader.read_exact(&mut data).await?;
    Ok(data)
}

/// Write a length-prefixed message to a stream
async fn write_length_prefixed<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    data: &[u8],
) -> io::Result<()> {
    // Write length as unsigned varint
    let mut length = data.len() as u64;
    while length >= 0x80 {
        writer.write_all(&[(length as u8) | 0x80]).await?;
        length >>= 7;
    }
    writer.write_all(&[length as u8]).await?;

    // Write data
    writer.write_all(data).await?;
    writer.flush().await?;
    Ok(())
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub enum BlockExcMode {
    #[default]
    Altruistic,
    MarketPlace {
        price_per_byte: u64,
    },
}
impl std::str::FromStr for BlockExcMode {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "altruistic" => Ok(Self::Altruistic),
            "marketplace" => Ok(Self::MarketPlace { price_per_byte: 1 }),
            _ => Err("unrecognised mode (options: 'altruistic', 'marketplace')"),
        }
    }
}

impl BlockExcMode {
    pub fn mode_string(&self) -> String {
        match self {
            Self::Altruistic => "altruistic".to_string(),
            Self::MarketPlace { price_per_byte } => format!("Market @ {} per byte", price_per_byte),
        }
    }
    fn price_per_byte(&self) -> Option<u64> {
        if let Self::MarketPlace { price_per_byte } = self {
            Some(*price_per_byte)
        } else {
            None
        }
    }
}
/// BlockExc connection handler
pub struct BlockExcHandler {
    peer_id: PeerId,
    /// Whether we've requested an outbound stream
    outbound_requested: bool,
    /// Whether we have an active stream (inbound or outbound)
    has_active_stream: bool,
    /// Shared block store for reading/writing blocks
    block_store: Arc<BlockStore>,
    /// Node operating mode (altruistic or marketplace)
    mode: BlockExcMode,
    /// Metrics collector for tracking P2P traffic
    metrics: Metrics,
    /// Pending block request (if any)
    pending_request: Option<cid::Cid>,
}

impl BlockExcHandler {
    pub fn new(
        peer_id: PeerId,
        block_store: Arc<BlockStore>,
        mode: BlockExcMode,
        metrics: Metrics,
    ) -> Self {
        BlockExcHandler {
            peer_id,
            outbound_requested: false,
            has_active_stream: false,
            block_store,
            mode,
            metrics,
            pending_request: None,
        }
    }
}

/// Messages from BlockExcBehaviour to BlockExcHandler
#[derive(Debug, Clone)]
pub enum BlockExcFromBehaviour {
    /// Request a block from this peer
    RequestBlock { cid: cid::Cid },
}

/// Messages from BlockExcHandler to BlockExcBehaviour
#[derive(Debug, Clone)]
pub enum BlockExcToBehaviour {
    /// Block delivered from peer
    BlockReceived { cid: cid::Cid, data: Vec<u8> },
    /// Peer indicated they have this block
    BlockPresence { cid: cid::Cid, has_block: bool },
}

impl ConnectionHandler for BlockExcHandler {
    type FromBehaviour = BlockExcFromBehaviour;
    type ToBehaviour = BlockExcToBehaviour;
    type InboundProtocol = ReadyUpgrade<StreamProtocol>;
    type OutboundProtocol = ReadyUpgrade<StreamProtocol>;
    type InboundOpenInfo = ();
    type OutboundOpenInfo = cid::Cid;

    fn listen_protocol(&self) -> SubstreamProtocol<Self::InboundProtocol, Self::InboundOpenInfo> {
        SubstreamProtocol::new(ReadyUpgrade::new(StreamProtocol::new(PROTOCOL_ID)), ())
    }

    fn on_behaviour_event(&mut self, event: Self::FromBehaviour) {
        match event {
            BlockExcFromBehaviour::RequestBlock { cid } => {
                info!(
                    "BlockExc: Received request to fetch block {} from {}",
                    cid, self.peer_id
                );
                self.pending_request = Some(cid);
                self.outbound_requested = false; // Reset so poll() will create new stream
            }
        }
    }

    fn connection_keep_alive(&self) -> bool {
        // Keep connection alive if we have active streams or pending requests
        self.has_active_stream || self.pending_request.is_some()
    }

    fn poll(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<
        ConnectionHandlerEvent<Self::OutboundProtocol, Self::OutboundOpenInfo, Self::ToBehaviour>,
    > {
        // On-demand outbound stream creation: when we have a pending block request
        if let Some(cid) = self.pending_request.take() {
            if !self.outbound_requested {
                info!(
                    "BlockExc: Opening outbound stream to {} to request block {}",
                    self.peer_id, cid
                );
                self.outbound_requested = true;
                return std::task::Poll::Ready(ConnectionHandlerEvent::OutboundSubstreamRequest {
                    protocol: SubstreamProtocol::new(
                        ReadyUpgrade::new(StreamProtocol::new(PROTOCOL_ID)),
                        cid,
                    ),
                });
            }
        }

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
                let metrics = self.metrics.clone();
                info!(
                    "BlockExc: Fully negotiated inbound stream from {} (mode: {})",
                    peer_id,
                    mode.mode_string()
                );

                // Spawn task to handle the stream - read messages from remote peer
                tokio::spawn(async move {
                    use crate::messages::{decode_message, encode_message, BlockDelivery, Message};
                    use cid::Cid;

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

                                        // DEBUG: Log wantlist details
                                        if let Some(ref wl) = msg.wantlist {
                                            info!(
                                                "BlockExc: Wantlist has {} entries, full={}",
                                                wl.entries.len(),
                                                wl.full
                                            );
                                            for (i, entry) in wl.entries.iter().enumerate() {
                                                info!("BlockExc:   Entry[{}]: address={}, priority={}, cancel={}, want_type={}, send_dont_have={}",
                                                    i,
                                                    entry.address.is_some(),
                                                    entry.priority,
                                                    entry.cancel,
                                                    entry.want_type,
                                                    entry.send_dont_have
                                                );
                                                if let Some(addr) = &entry.address {
                                                    info!("BlockExc:   Entry[{}]: leaf={}, cid_len={}, tree_cid_len={}, index={}",
                                                        i,
                                                        addr.leaf,
                                                        addr.cid.len(),
                                                        addr.tree_cid.len(),
                                                        addr.index
                                                    );
                                                }
                                            }
                                        }

                                        // If they sent a wantlist, respond with blocks we have
                                        if let Some(wantlist) = msg.wantlist {
                                            use crate::messages::BlockPresence;

                                            if let BlockExcMode::Altruistic = mode {
                                                // ALTRUISTIC MODE: Serve blocks freely without payment
                                                info!("BlockExc: ALTRUISTIC MODE - serving blocks freely to {}", peer_id);
                                                let mut response_blocks = Vec::new();

                                                for entry in &wantlist.entries {
                                                    // Extract CID from BlockAddress
                                                    if let Some(cid_bytes) = entry.cid_bytes() {
                                                        info!("BlockExc: Extracted CID bytes ({} bytes)", cid_bytes.len());
                                                        if let Ok(cid) = Cid::try_from(cid_bytes) {
                                                            info!("BlockExc: Blackberry wants CID: {}", cid);
                                                            if let Ok(block) =
                                                                block_store.get(&cid).await
                                                            {
                                                                let total_size =
                                                                    block.data.len() as u64;

                                                                // Full block request (range retrieval removed per compatibility requirements)
                                                                info!("BlockExc: Serving full block {} to {} (altruistic) - {} bytes",
                                                                cid, peer_id, total_size);

                                                                metrics
                                                                    .block_sent(block.data.len()); // Track P2P traffic!
                                                                response_blocks.push(
                                                                    BlockDelivery::from_cid_and_data(
                                                                        cid.to_bytes(),
                                                                        block.data.clone(),
                                                                    )
                                                                );
                                                            } else {
                                                                warn!("BlockExc: Block {} NOT FOUND in local store", cid);
                                                            }
                                                        } else {
                                                            warn!("BlockExc: Failed to parse CID from {} bytes", cid_bytes.len());
                                                        }
                                                    } else {
                                                        warn!("BlockExc: No CID bytes in wantlist entry");
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
                                            } else if let BlockExcMode::MarketPlace {
                                                price_per_byte: _,
                                            } = mode
                                            {
                                                // MARKETPLACE MODE: Check payment before serving
                                                info!("BlockExc: MARKETPLACE MODE - checking payment from {}", peer_id);

                                                let has_payment = msg.payment.is_some();

                                                if has_payment {
                                                    info!("BlockExc: Payment received from {}, serving blocks", peer_id);
                                                    // Payment received - serve blocks
                                                    let mut response_blocks = Vec::new();

                                                    for entry in &wantlist.entries {
                                                        // Extract CID from BlockAddress
                                                        if let Some(cid_bytes) = entry.cid_bytes() {
                                                            if let Ok(cid) =
                                                                Cid::try_from(cid_bytes)
                                                            {
                                                                if let Ok(block) =
                                                                    block_store.get(&cid).await
                                                                {
                                                                    let total_size =
                                                                        block.data.len() as u64;

                                                                    // Full block request (range retrieval removed per compatibility requirements)
                                                                    info!("BlockExc: Serving full block {} to {} (paid) - {} bytes",
                                                                    cid, peer_id, total_size);

                                                                    metrics.block_sent(
                                                                        block.data.len(),
                                                                    ); // Track P2P traffic!
                                                                    response_blocks.push(
                                                                        BlockDelivery::from_cid_and_data(
                                                                            cid.to_bytes(),
                                                                            block.data.clone(),
                                                                        )
                                                                    );
                                                                }
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
                                                        // Extract CID from BlockAddress
                                                        if let Some(cid_bytes) = entry.cid_bytes() {
                                                            if let Ok(cid) =
                                                                Cid::try_from(cid_bytes)
                                                            {
                                                                if let Ok(block) =
                                                                    block_store.get(&cid).await
                                                                {
                                                                    let block_price =
                                                                        (block.data.len() as u64)
                                                                            * mode
                                                                                .price_per_byte()
                                                                                .unwrap_or_default(
                                                                                );
                                                                    info!("BlockExc: Block {} available for {} units", cid, block_price);

                                                                    block_presences.push(
                                                                        BlockPresence::from_cid(
                                                                            cid.to_bytes(),
                                                                            crate::messages::BlockPresenceType::PresenceHave,
                                                                            block_price.to_le_bytes().to_vec(),
                                                                        )
                                                                    );
                                                                }
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
                info: requested_cid,
            }) => {
                self.has_active_stream = true;
                let peer_id = self.peer_id;
                let block_store = self.block_store.clone();
                let metrics = self.metrics.clone();
                info!(
                    "BlockExc: Fully negotiated outbound stream to {} for block {}",
                    peer_id, requested_cid
                );

                // Spawn task to handle outbound stream - send WantList and receive blocks
                tokio::spawn(async move {
                    use crate::messages::{
                        decode_message, encode_message, Message, WantType, Wantlist, WantlistEntry,
                    };
                    use crate::storage::Block;

                    let mut stream = stream;

                    info!(
                        "BlockExc: Requesting block {} from {}",
                        requested_cid, peer_id
                    );

                    // Create WantList with requested CID using new BlockAddress structure
                    let wantlist = Wantlist {
                        entries: vec![WantlistEntry::from_cid(
                            requested_cid.to_bytes(),
                            WantType::WantBlock,
                        )],
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
                                            info!(
                                                "BlockExc: Received block! cid_len={}, data_len={}",
                                                msg_block.cid.len(),
                                                msg_block.data.len()
                                            );

                                            // Compute CID from data and verify it matches what we requested
                                            use crate::cid_blake3::blake3_cid;
                                            match blake3_cid(&msg_block.data) {
                                                Ok(computed_cid) => {
                                                    if computed_cid != requested_cid {
                                                        warn!("BlockExc: CID mismatch! Expected {}, got {}", requested_cid, computed_cid);
                                                        continue;
                                                    }

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
            _ => {}
        }
    }
}

use tokio::sync::mpsc;

/// Request to fetch a block from peers
#[derive(Debug, Clone)]
pub struct BlockRequest {
    pub cid: cid::Cid,
    pub response_tx:
        Arc<tokio::sync::Mutex<Option<tokio::sync::oneshot::Sender<crate::storage::Block>>>>,
}

/// BlockExc network behaviour
pub struct BlockExcBehaviour {
    block_store: Arc<BlockStore>,
    mode: BlockExcMode,
    metrics: Metrics,
    /// Channel for receiving block requests
    request_rx: mpsc::UnboundedReceiver<BlockRequest>,
    /// Pending block requests
    pending_requests: std::collections::HashMap<cid::Cid, BlockRequest>,
    /// Connected peers
    connected_peers: std::collections::HashSet<PeerId>,
    /// Pending events to send to handlers
    pending_events: std::collections::VecDeque<(PeerId, BlockExcFromBehaviour)>,
    /// Discovery engine for finding providers (optional)
    discovery: Option<Arc<Discovery>>,
    /// Blocks queued for discovery (CID -> retry count)
    discovery_queue: std::collections::HashMap<cid::Cid, u32>,
}

impl BlockExcBehaviour {
    pub fn new(
        block_store: Arc<BlockStore>,
        mode: BlockExcMode,
        metrics: Metrics,
    ) -> (Self, mpsc::UnboundedSender<BlockRequest>) {
        let (request_tx, request_rx) = mpsc::unbounded_channel();
        let behaviour = Self {
            block_store,
            mode,
            metrics,
            request_rx,
            pending_requests: std::collections::HashMap::new(),
            connected_peers: std::collections::HashSet::new(),
            pending_events: std::collections::VecDeque::new(),
            discovery: None,
            discovery_queue: std::collections::HashMap::new(),
        };
        (behaviour, request_tx)
    }

    /// Set the discovery engine for automatic provider discovery
    ///
    /// When a block is not found from connected peers, the discovery engine
    /// will be used to find providers for the block.
    pub fn set_discovery(&mut self, discovery: Arc<Discovery>) {
        info!("BlockExc: Discovery engine enabled");
        self.discovery = Some(discovery);
    }

    /// Request a specific block from a specific peer
    ///
    /// Sends a WantBlock message to the specified peer to request the given CID.
    /// The peer will respond with the block if they have it.
    ///
    /// # Arguments
    /// * `peer_id` - The peer to request the block from
    /// * `cid` - The CID of the block to request
    ///
    /// # Returns
    /// * `Ok(())` if the request was queued successfully
    /// * `Err(BlockExcError::NoPeers)` if the peer is not connected
    pub fn request_block(&mut self, peer_id: PeerId, cid: Cid) -> Result<(), BlockExcError> {
        if !self.connected_peers.contains(&peer_id) {
            return Err(BlockExcError::NoPeers);
        }

        info!(
            "BlockExc: Queueing request for block {} from peer {}",
            cid, peer_id
        );

        // Queue the RequestBlock event for this specific peer
        self.pending_events
            .push_back((peer_id, BlockExcFromBehaviour::RequestBlock { cid }));

        Ok(())
    }

    /// Broadcast a want for a block to all connected peers
    ///
    /// Sends WantBlock messages to all currently connected peers requesting the given CID.
    /// This is useful when you don't know which peer has the block.
    ///
    /// # Arguments
    /// * `cid` - The CID of the block to request
    ///
    /// # Returns
    /// * `Ok(usize)` - Number of peers the request was sent to
    /// * `Err(BlockExcError::NoPeers)` if no peers are connected
    pub fn broadcast_want(&mut self, cid: Cid) -> Result<usize, BlockExcError> {
        if self.connected_peers.is_empty() {
            return Err(BlockExcError::NoPeers);
        }

        let peer_count = self.connected_peers.len();
        info!(
            "BlockExc: Broadcasting want for block {} to {} peers",
            cid, peer_count
        );

        // Queue RequestBlock events for all connected peers
        for peer_id in &self.connected_peers {
            self.pending_events
                .push_back((*peer_id, BlockExcFromBehaviour::RequestBlock { cid }));
        }

        Ok(peer_count)
    }

    /// Get the number of currently connected peers
    pub fn connected_peer_count(&self) -> usize {
        self.connected_peers.len()
    }

    /// Get a list of all connected peer IDs
    pub fn connected_peers(&self) -> Vec<PeerId> {
        self.connected_peers.iter().copied().collect()
    }

    /// Queue blocks for discovery when not found via BlockExc
    ///
    /// This is called when a block request fails because no connected peers have it.
    /// The discovery engine will search the DHT for providers.
    ///
    /// # Arguments
    /// * `cids` - List of CIDs to discover providers for
    ///
    /// # Returns
    /// Number of blocks queued for discovery (0 if discovery disabled)
    pub fn queue_find_blocks(&mut self, cids: Vec<Cid>) -> usize {
        if self.discovery.is_none() {
            warn!("BlockExc: Discovery disabled, cannot queue blocks for discovery");
            return 0;
        }

        let mut queued = 0;
        for cid in cids {
            // Don't re-queue if already in discovery
            use std::collections::hash_map::Entry;
            if let Entry::Vacant(e) = self.discovery_queue.entry(cid) {
                info!("BlockExc: Queueing block {} for discovery", cid);
                e.insert(0); // 0 retries initially
                queued += 1;
            }
        }

        queued
    }

    /// Process discovery queue - find providers for queued blocks
    ///
    /// This should be called periodically from the poll() method to process
    /// blocks waiting for provider discovery.
    async fn _process_discovery_queue(&mut self) {
        if self.discovery.is_none() {
            return;
        }

        let discovery = self.discovery.as_ref().unwrap().clone();
        let mut completed = Vec::new();

        // Process each queued CID
        for (cid, retry_count) in &mut self.discovery_queue {
            const MAX_RETRIES: u32 = 3;

            if *retry_count >= MAX_RETRIES {
                warn!(
                    "BlockExc: Discovery for block {} exceeded max retries ({})",
                    cid, MAX_RETRIES
                );
                completed.push(*cid);
                continue;
            }

            info!(
                "BlockExc: Searching for providers of block {} (attempt {}/{})",
                cid,
                *retry_count + 1,
                MAX_RETRIES
            );

            // Track discovery query
            self.metrics.discovery_query();

            // Find providers via discovery engine
            match discovery.find(cid).await {
                Ok(providers) if !providers.is_empty() => {
                    info!(
                        "BlockExc: Found {} providers for block {} via discovery",
                        providers.len(),
                        cid
                    );

                    // Track successful discovery
                    self.metrics.discovery_success();

                    // Request block from discovered providers
                    for provider in providers {
                        if self.connected_peers.contains(&provider) {
                            // Already connected, request directly
                            self.pending_events.push_back((
                                provider,
                                BlockExcFromBehaviour::RequestBlock { cid: *cid },
                            ));
                        } else {
                            // TODO: Dial the provider first, then request
                            info!(
                                "BlockExc: Need to dial provider {} for block {}",
                                provider, cid
                            );
                        }
                    }

                    // Mark as completed (found providers)
                    completed.push(*cid);
                }
                Ok(_) => {
                    // No providers found yet, increment retry count
                    *retry_count += 1;
                    info!(
                        "BlockExc: No providers found for block {} (retry {}/{})",
                        cid, *retry_count, MAX_RETRIES
                    );

                    // Track failure if max retries reached
                    if *retry_count >= MAX_RETRIES {
                        self.metrics.discovery_failure();
                    }
                }
                Err(e) => {
                    warn!(
                        "BlockExc: Discovery error for block {}: {} (retry {}/{})",
                        cid,
                        e,
                        *retry_count + 1,
                        MAX_RETRIES
                    );
                    *retry_count += 1;

                    // Track failure if max retries reached
                    if *retry_count >= MAX_RETRIES {
                        self.metrics.discovery_failure();
                    }
                }
            }
        }

        // Remove completed CIDs from queue
        for cid in completed {
            self.discovery_queue.remove(&cid);
        }
    }
}

//
// BlockExc Client Implementation
//

use cid::Cid;

#[derive(Debug, thiserror::Error)]
pub enum BlockExcError {
    #[error("Block request failed: {0}")]
    RequestFailed(String),

    #[error("Block not found after retries")]
    NotFound,

    #[error("Timeout waiting for block")]
    Timeout,

    #[error("No peers available")]
    NoPeers,

    #[error("CID mismatch: expected {expected}, got {got}")]
    CidMismatch { expected: String, got: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Storage error: {0}")]
    Storage(#[from] crate::storage::StorageError),
}

/// BlockExc client for requesting blocks from peers
pub struct BlockExcClient {
    /// Channel to send block requests to the swarm
    request_tx: mpsc::UnboundedSender<BlockRequest>,
    /// Local block store
    block_store: Arc<BlockStore>,
    /// Metrics
    metrics: Metrics,
}

impl BlockExcClient {
    pub fn new(
        block_store: Arc<BlockStore>,
        metrics: Metrics,
        _max_retries: u32,
        request_tx: mpsc::UnboundedSender<BlockRequest>,
    ) -> Self {
        Self {
            request_tx,
            block_store,
            metrics,
        }
    }

    /// Request a block from the network via BlockExc protocol
    ///
    /// Sends a request to the swarm which broadcasts WantBlock messages to all connected peers
    pub async fn request_block(&self, cid: Cid) -> Result<crate::storage::Block, BlockExcError> {
        info!("BlockExc client: Requesting block {}", cid);

        // Check if block is already in local store
        if let Ok(block) = self.block_store.get(&cid).await {
            info!("BlockExc client: Block {} found in local store", cid);
            return Ok(block);
        }

        // Create a oneshot channel to receive the block
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        let response_tx = Arc::new(tokio::sync::Mutex::new(Some(response_tx)));

        // Send block request to swarm via channel
        let block_request = BlockRequest { cid, response_tx };

        if self.request_tx.send(block_request).is_err() {
            return Err(BlockExcError::RequestFailed(
                "Failed to send request to swarm".to_string(),
            ));
        }

        info!("BlockExc client: Sent request for block {} to swarm", cid);

        // Wait for block to arrive (with timeout)
        match tokio::time::timeout(std::time::Duration::from_secs(30), response_rx).await {
            Ok(Ok(block)) => {
                info!("BlockExc client: Successfully received block {}", cid);
                self.metrics.block_received(block.data.len());
                Ok(block)
            }
            Ok(Err(_)) => Err(BlockExcError::RequestFailed("Channel closed".to_string())),
            Err(_) => Err(BlockExcError::Timeout),
        }
    }
}

impl libp2p::swarm::NetworkBehaviour for BlockExcBehaviour {
    type ConnectionHandler = BlockExcHandler;
    type ToSwarm = BlockExcToBehaviour;

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
            self.metrics.clone(),
        ))
    }

    fn handle_established_outbound_connection(
        &mut self,
        _connection_id: libp2p::swarm::ConnectionId,
        peer: PeerId,
        _addr: &libp2p::Multiaddr,
        _role_override: libp2p::core::Endpoint,
        _port_use: libp2p::core::transport::PortUse,
    ) -> Result<libp2p::swarm::THandler<Self>, libp2p::swarm::ConnectionDenied> {
        Ok(BlockExcHandler::new(
            peer,
            self.block_store.clone(),
            self.mode.clone(),
            self.metrics.clone(),
        ))
    }

    fn on_swarm_event(&mut self, event: libp2p::swarm::FromSwarm) {
        match event {
            libp2p::swarm::FromSwarm::ConnectionEstablished(conn) => {
                info!("BlockExc: Connection established with {}", conn.peer_id);
                self.connected_peers.insert(conn.peer_id);
            }
            libp2p::swarm::FromSwarm::ConnectionClosed(conn) => {
                if conn.remaining_established == 0 {
                    info!("BlockExc: All connections closed with {}", conn.peer_id);
                    self.connected_peers.remove(&conn.peer_id);
                }
            }
            _ => {}
        }
    }

    fn on_connection_handler_event(
        &mut self,
        peer_id: PeerId,
        _connection_id: libp2p::swarm::ConnectionId,
        event: libp2p::swarm::THandlerOutEvent<Self>,
    ) {
        match event {
            BlockExcToBehaviour::BlockReceived { cid, data } => {
                info!(
                    "BlockExc behaviour: Received block {} from {} ({} bytes)",
                    cid,
                    peer_id,
                    data.len()
                );

                // Check if this block was retrieved via discovery
                let from_discovery = self.discovery_queue.contains_key(&cid);
                if from_discovery {
                    info!(
                        "BlockExc behaviour: Block {} was retrieved via discovery!",
                        cid
                    );
                    self.metrics.block_from_discovery();
                    // Remove from discovery queue since we got it
                    self.discovery_queue.remove(&cid);
                }

                // Store in block store
                let block = crate::storage::Block { cid, data };
                let block_store = self.block_store.clone();
                let metrics = self.metrics.clone();

                // Complete pending request if exists
                if let Some(request) = self.pending_requests.remove(&cid) {
                    let response_tx = request.response_tx.clone();
                    let block_clone = block.clone();
                    tokio::spawn(async move {
                        let mut tx_guard = response_tx.lock().await;
                        if let Some(tx) = tx_guard.take() {
                            let _ = tx.send(block_clone);
                        }
                    });
                }

                tokio::spawn(async move {
                    match block_store.put(block.clone()).await {
                        Ok(_) => {
                            metrics.block_received(block.data.len());
                        }
                        Err(e) => {
                            warn!("Failed to store received block: {}", e);
                        }
                    }
                });
            }
            BlockExcToBehaviour::BlockPresence { cid, has_block } => {
                info!(
                    "BlockExc behaviour: Peer {} {} block {}",
                    peer_id,
                    if has_block { "has" } else { "doesn't have" },
                    cid
                );
                // TODO: Track which peers have which blocks for smarter routing
            }
        }
    }

    fn poll(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<libp2p::swarm::ToSwarm<Self::ToSwarm, libp2p::swarm::THandlerInEvent<Self>>>
    {
        // Process pending handler events first
        if let Some((peer_id, event)) = self.pending_events.pop_front() {
            return std::task::Poll::Ready(libp2p::swarm::ToSwarm::NotifyHandler {
                peer_id,
                handler: libp2p::swarm::NotifyHandler::Any,
                event,
            });
        }

        // Process incoming block requests
        while let std::task::Poll::Ready(Some(request)) = self.request_rx.poll_recv(cx) {
            info!(
                "BlockExc behaviour: Received request for block {} from {} connected peers",
                request.cid,
                self.connected_peers.len()
            );

            // Store the pending request
            self.pending_requests.insert(request.cid, request.clone());

            // Queue RequestBlock events for all connected peers
            for peer_id in &self.connected_peers {
                self.pending_events.push_back((
                    *peer_id,
                    BlockExcFromBehaviour::RequestBlock { cid: request.cid },
                ));
            }

            // Process first pending event immediately
            if let Some((peer_id, event)) = self.pending_events.pop_front() {
                return std::task::Poll::Ready(libp2p::swarm::ToSwarm::NotifyHandler {
                    peer_id,
                    handler: libp2p::swarm::NotifyHandler::Any,
                    event,
                });
            }
        }

        std::task::Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cid_blake3::blake3_cid;
    use std::sync::Arc;

    fn create_test_behaviour() -> (BlockExcBehaviour, mpsc::UnboundedSender<BlockRequest>) {
        let block_store = Arc::new(BlockStore::new());
        let metrics = Metrics::new();
        BlockExcBehaviour::new(block_store, "altruistic".to_string(), 0, metrics)
    }

    #[test]
    fn test_request_block_no_peers() {
        let (mut behaviour, _tx) = create_test_behaviour();
        let test_cid = blake3_cid(b"test data").unwrap();

        // Should fail when no peers are connected
        let result = behaviour.request_block(PeerId::random(), test_cid);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), BlockExcError::NoPeers));
    }

    #[test]
    fn test_broadcast_want_no_peers() {
        let (mut behaviour, _tx) = create_test_behaviour();
        let test_cid = blake3_cid(b"test data").unwrap();

        // Should fail when no peers are connected
        let result = behaviour.broadcast_want(test_cid);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), BlockExcError::NoPeers));
    }

    #[test]
    fn test_request_block_with_peer() {
        let (mut behaviour, _tx) = create_test_behaviour();
        let test_cid = blake3_cid(b"test data").unwrap();
        let peer_id = PeerId::random();

        // Simulate peer connection
        behaviour.connected_peers.insert(peer_id);

        // Should succeed when peer is connected
        let result = behaviour.request_block(peer_id, test_cid);
        assert!(result.is_ok());

        // Should have queued a pending event
        assert_eq!(behaviour.pending_events.len(), 1);
        let (queued_peer, event) = behaviour.pending_events.front().unwrap();
        assert_eq!(*queued_peer, peer_id);
        match event {
            BlockExcFromBehaviour::RequestBlock { cid } => {
                assert_eq!(*cid, test_cid);
            }
        }
    }

    #[test]
    fn test_broadcast_want_with_peers() {
        let (mut behaviour, _tx) = create_test_behaviour();
        let test_cid = blake3_cid(b"test data").unwrap();
        let peer1 = PeerId::random();
        let peer2 = PeerId::random();
        let peer3 = PeerId::random();

        // Simulate multiple peer connections
        behaviour.connected_peers.insert(peer1);
        behaviour.connected_peers.insert(peer2);
        behaviour.connected_peers.insert(peer3);

        // Should succeed and return number of peers
        let result = behaviour.broadcast_want(test_cid);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 3);

        // Should have queued events for all peers
        assert_eq!(behaviour.pending_events.len(), 3);

        // Verify all events are for the correct CID
        for (_, event) in &behaviour.pending_events {
            match event {
                BlockExcFromBehaviour::RequestBlock { cid } => {
                    assert_eq!(*cid, test_cid);
                }
            }
        }
    }

    #[test]
    fn test_connected_peer_count() {
        let (mut behaviour, _tx) = create_test_behaviour();
        assert_eq!(behaviour.connected_peer_count(), 0);

        behaviour.connected_peers.insert(PeerId::random());
        assert_eq!(behaviour.connected_peer_count(), 1);

        behaviour.connected_peers.insert(PeerId::random());
        behaviour.connected_peers.insert(PeerId::random());
        assert_eq!(behaviour.connected_peer_count(), 3);
    }

    #[test]
    fn test_connected_peers() {
        let (mut behaviour, _tx) = create_test_behaviour();
        let peer1 = PeerId::random();
        let peer2 = PeerId::random();

        behaviour.connected_peers.insert(peer1);
        behaviour.connected_peers.insert(peer2);

        let peers = behaviour.connected_peers();
        assert_eq!(peers.len(), 2);
        assert!(peers.contains(&peer1));
        assert!(peers.contains(&peer2));
    }

    #[test]
    fn test_multiple_requests_queue_correctly() {
        let (mut behaviour, _tx) = create_test_behaviour();
        let test_cid1 = blake3_cid(b"test data 1").unwrap();
        let test_cid2 = blake3_cid(b"test data 2").unwrap();
        let peer1 = PeerId::random();
        let peer2 = PeerId::random();

        behaviour.connected_peers.insert(peer1);
        behaviour.connected_peers.insert(peer2);

        // Request different blocks from different peers
        behaviour.request_block(peer1, test_cid1).unwrap();
        behaviour.request_block(peer2, test_cid2).unwrap();

        // Should have two events queued
        assert_eq!(behaviour.pending_events.len(), 2);

        // Verify events are correctly queued
        let (p1, evt1) = &behaviour.pending_events[0];
        let (p2, evt2) = &behaviour.pending_events[1];

        assert_eq!(*p1, peer1);
        assert_eq!(*p2, peer2);

        match evt1 {
            BlockExcFromBehaviour::RequestBlock { cid } => assert_eq!(*cid, test_cid1),
        }
        match evt2 {
            BlockExcFromBehaviour::RequestBlock { cid } => assert_eq!(*cid, test_cid2),
        }
    }
}
