//! BlockExc protocol implementation
//!
//! Implements Archivist's custom BlockExc protocol for block exchange.
//! Protocol ID: /archivist/blockexc/1.0.0

use libp2p::core::upgrade::ReadyUpgrade;
use libp2p::swarm::{
    ConnectionHandler, ConnectionHandlerEvent, KeepAlive, SubstreamProtocol,
    StreamProtocol, handler::{ConnectionEvent, FullyNegotiatedInbound, FullyNegotiatedOutbound},
};
use libp2p::PeerId;
use std::io;
use tracing::{info, warn};

pub const PROTOCOL_ID: &str = "/archivist/blockexc/1.0.0";

/// BlockExc connection handler
pub struct BlockExcHandler {
    peer_id: PeerId,
    keep_alive: KeepAlive,
    /// Whether we've requested an outbound stream
    outbound_requested: bool,
    /// Whether we have an active stream (inbound or outbound)
    has_active_stream: bool,
}

impl BlockExcHandler {
    pub fn new(peer_id: PeerId) -> Self {
        BlockExcHandler {
            peer_id,
            keep_alive: KeepAlive::Yes,
            outbound_requested: false,
            has_active_stream: false,
        }
    }
}

impl ConnectionHandler for BlockExcHandler {
    type FromBehaviour = ();
    type ToBehaviour = ();
    type Error = io::Error;
    type InboundProtocol = ReadyUpgrade<StreamProtocol>;
    type OutboundProtocol = ReadyUpgrade<StreamProtocol>;
    type InboundOpenInfo = ();
    type OutboundOpenInfo = ();

    fn listen_protocol(&self) -> SubstreamProtocol<Self::InboundProtocol, Self::InboundOpenInfo> {
        SubstreamProtocol::new(
            ReadyUpgrade::new(StreamProtocol::new(PROTOCOL_ID)),
            ()
        )
    }

    fn on_behaviour_event(&mut self, _event: Self::FromBehaviour) {}

    fn connection_keep_alive(&self) -> KeepAlive {
        self.keep_alive
    }

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
        // Don't automatically request outbound streams
        // Wait for the remote peer to dial us, or for an explicit send request
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
                info!("BlockExc: Fully negotiated inbound stream from {}", peer_id);

                // Spawn task to handle the stream - keep it alive and read any messages
                tokio::spawn(async move {
                    use libp2p::core::upgrade::{read_length_prefixed, write_length_prefixed};

                    let mut stream = stream;
                    info!("BlockExc: Started reading from {}", peer_id);

                    loop {
                        // Try to read a length-prefixed message
                        match read_length_prefixed(&mut stream, 100 * 1024 * 1024).await {
                            Ok(data) => {
                                info!("BlockExc: Received {} bytes from {}", data.len(), peer_id);

                                // Send empty response to acknowledge
                                let response: Vec<u8> = vec![];
                                if let Err(e) = write_length_prefixed(&mut stream, &response).await {
                                    warn!("BlockExc: Failed to send response to {}: {}", peer_id, e);
                                    break;
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
                info!("BlockExc: Fully negotiated outbound stream to {}", peer_id);

                // Spawn task to handle outbound stream - send initial hello message
                tokio::spawn(async move {
                    use libp2p::core::upgrade::{read_length_prefixed, write_length_prefixed};
                    use crate::messages::{Message, encode_message};

                    let mut stream = stream;

                    // Send empty protobuf Message to initiate protocol
                    // The Archivist node expects a valid protobuf Message, not an empty Vec
                    let hello = Message::default();
                    let hello_bytes = match encode_message(&hello) {
                        Ok(bytes) => bytes,
                        Err(e) => {
                            warn!("BlockExc: Failed to encode hello message: {}", e);
                            return;
                        }
                    };

                    info!("BlockExc: Sending {} byte hello message to {}", hello_bytes.len(), peer_id);
                    if let Err(e) = write_length_prefixed(&mut stream, &hello_bytes).await {
                        warn!("BlockExc: Failed to send hello to {}: {}", peer_id, e);
                        return;
                    }
                    info!("BlockExc: Successfully sent hello to {}", peer_id);

                    // Listen for responses
                    loop {
                        match read_length_prefixed(&mut stream, 100 * 1024 * 1024).await {
                            Ok(data) => {
                                info!("BlockExc: Received {} bytes from {} on outbound stream", data.len(), peer_id);
                            }
                            Err(e) => {
                                if e.kind() != io::ErrorKind::UnexpectedEof {
                                    warn!("BlockExc: Error reading from {} on outbound: {}", peer_id, e);
                                }
                                break;
                            }
                        }
                    }

                    info!("BlockExc: Finished outbound stream to {}", peer_id);
                });
            }
            ConnectionEvent::DialUpgradeError(err) => {
                warn!("BlockExc: Dial upgrade error to {}: {:?}", self.peer_id, err);
            }
            ConnectionEvent::AddressChange(_)
            | ConnectionEvent::ListenUpgradeError(_)
            | ConnectionEvent::LocalProtocolsChange(_)
            | ConnectionEvent::RemoteProtocolsChange(_) => {}
        }
    }
}

/// BlockExc network behaviour
pub struct BlockExcBehaviour;

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
        Ok(BlockExcHandler::new(peer))
    }

    fn handle_established_outbound_connection(
        &mut self,
        _connection_id: libp2p::swarm::ConnectionId,
        peer: PeerId,
        _addr: &libp2p::Multiaddr,
        _role_override: libp2p::core::Endpoint,
    ) -> Result<libp2p::swarm::THandler<Self>, libp2p::swarm::ConnectionDenied> {
        Ok(BlockExcHandler::new(peer))
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
    ) -> std::task::Poll<libp2p::swarm::ToSwarm<Self::ToSwarm, libp2p::swarm::THandlerInEvent<Self>>> {
        std::task::Poll::Pending
    }
}
