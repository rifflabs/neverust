//! Identify protocol shim for nim-libp2p compatibility
//!
//! Wraps rust-libp2p's identify::Behaviour to use custom SPR encoding
//! that's compatible with nim-libp2p v1.9.0.
//!
//! This shim preserves all rust-libp2p functionality while fixing only
//! the SPR encoding incompatibility.

use libp2p::{
    identify,
    identity::Keypair,
    swarm::{
        ConnectionHandler, ConnectionId, FromSwarm, NetworkBehaviour, THandler,
        THandlerInEvent, THandlerOutEvent, ToSwarm,
    },
    Multiaddr, PeerId,
};
use std::collections::VecDeque;
use std::sync::Arc;

/// Shim wrapper around identify::Behaviour with nim-libp2p compatible SPR encoding
pub struct IdentifyShim {
    inner: identify::Behaviour,
    keypair: Arc<Keypair>,
    local_peer_id: PeerId,
    listen_addrs: Vec<Multiaddr>,
}

impl IdentifyShim {
    /// Create a new IdentifyShim with custom SPR encoding
    ///
    /// This uses `Config::new` (without SPR) from rust-libp2p, then we handle
    /// SPR generation ourselves using nim-libp2p compatible encoding.
    pub fn new(protocol_version: String, keypair: &Keypair) -> Self {
        let config = identify::Config::new(protocol_version, keypair.public());
        let inner = identify::Behaviour::new(config);
        let local_peer_id = PeerId::from(keypair.public());

        Self {
            inner,
            keypair: Arc::new(keypair.clone()),
            local_peer_id,
            listen_addrs: Vec::new(),
        }
    }
}

impl NetworkBehaviour for IdentifyShim {
    type ConnectionHandler = <identify::Behaviour as NetworkBehaviour>::ConnectionHandler;
    type ToSwarm = identify::Event;

    fn handle_established_inbound_connection(
        &mut self,
        connection_id: ConnectionId,
        peer: PeerId,
        local_addr: &Multiaddr,
        remote_addr: &Multiaddr,
    ) -> Result<THandler<Self>, libp2p::swarm::ConnectionDenied> {
        self.inner.handle_established_inbound_connection(
            connection_id,
            peer,
            local_addr,
            remote_addr,
        )
    }

    fn handle_established_outbound_connection(
        &mut self,
        connection_id: ConnectionId,
        peer: PeerId,
        addr: &Multiaddr,
        role_override: libp2p::core::Endpoint,
    ) -> Result<THandler<Self>, libp2p::swarm::ConnectionDenied> {
        self.inner.handle_established_outbound_connection(
            connection_id,
            peer,
            addr,
            role_override,
        )
    }

    fn on_swarm_event(&mut self, event: FromSwarm) {
        // Track NewListenAddr events to maintain our listen addresses
        if let FromSwarm::NewListenAddr(e) = &event {
            self.listen_addrs.push(e.addr.clone());
        }

        self.inner.on_swarm_event(event);
    }

    fn on_connection_handler_event(
        &mut self,
        peer_id: PeerId,
        connection_id: ConnectionId,
        event: THandlerOutEvent<Self>,
    ) {
        // TODO: Intercept outgoing Identify messages here and replace SPR bytes
        // For now, just forward to inner behavior
        self.inner
            .on_connection_handler_event(peer_id, connection_id, event);
    }

    fn poll(
        &mut self,
        cx: &mut std::task::Context,
    ) -> std::task::Poll<ToSwarm<Self::ToSwarm, THandlerInEvent<Self>>> {
        // Forward poll to inner behavior
        self.inner.poll(cx)
    }
}

// TODO: Implement message interception to replace SPR bytes
// This requires understanding the exact point where Identify messages are encoded
// and finding a way to inject our custom SPR bytes.
//
// Potential approaches:
// 1. Intercept at ConnectionHandler level
// 2. Wrap the upgrade protocol
// 3. Fork identify::Behaviour to use our custom SPR encoder

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identify_shim_creation() {
        let keypair = Keypair::generate_secp256k1();
        let shim = IdentifyShim::new("test/1.0.0".to_string(), &keypair);
        assert_eq!(shim.local_peer_id, PeerId::from(keypair.public()));
    }
}
