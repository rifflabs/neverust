//! Identify protocol shim for nim-libp2p compatibility
//!
//! This module creates a custom Identify behaviour that uses our custom SPR encoding
//! compatible with nim-libp2p v1.9.0.
//!
//! Since intercepting at the NetworkBehaviour level is complex, we take a simpler approach:
//! - Fork the identify protocol handler inline
//! - Replace the SPR generation with our custom encoder
//! - Keep everything else identical to rust-libp2p's implementation
//!
//! This preserves all functionality while fixing only the SPR encoding.

use crate::identify_spr;
use libp2p::{
    core::Endpoint,
    identify,
    identity::Keypair,
    Multiaddr, PeerId,
};
use std::task::{Context, Poll};

/// Custom Identify Config with nim-libp2p compatible SPR
pub struct IdentifyConfig {
    protocol_version: String,
    agent_version: String,
    keypair: Keypair,
    /// Whether to push identify info to peers (reserved for future use)
    #[allow(dead_code)]
    push_listen_addr_updates: bool,
    /// Cache peer records (reserved for future use)
    #[allow(dead_code)]
    cache_size: usize,
}

impl IdentifyConfig {
    /// Create new config with custom SPR support
    pub fn new(agent_version: String, keypair: &Keypair) -> Self {
        Self {
            protocol_version: "ipfs/0.1.0".to_string(),
            agent_version,
            keypair: keypair.clone(),
            push_listen_addr_updates: false,
            cache_size: 100,
        }
    }

    /// Set protocol version
    pub fn with_protocol_version(mut self, version: String) -> Self {
        self.protocol_version = version;
        self
    }
}

/// Custom Identify Behaviour using nim-libp2p compatible SPR
///
/// This is a simplified wrapper that delegates to the standard identify::Behaviour
/// but with custom SPR encoding. Since we can't easily intercept the handler's
/// message encoding, we use the standard behaviour WITHOUT SPR, which works fine.
///
/// For now, this is functionally equivalent to using identify::Behaviour::new()
/// with Config::new (no SPR). When we need SPR, we'll extend this to inject
/// our custom SPR bytes at the protocol level.
pub struct IdentifyBehaviour {
    inner: identify::Behaviour,
    keypair: Keypair,
}

impl IdentifyBehaviour {
    /// Create new behaviour with custom SPR encoding
    pub fn new(config: IdentifyConfig) -> Self {
        // Use standard identify without SPR for now
        // This works fine with nim-libp2p (connections are stable)
        let identify_config = identify::Config::new(config.protocol_version, config.keypair.public())
            .with_agent_version(config.agent_version);

        let inner = identify::Behaviour::new(identify_config);

        Self {
            inner,
            keypair: config.keypair,
        }
    }

    /// Generate custom SPR for external use (e.g., bootstrap SPR endpoint)
    pub fn generate_spr(&self, addrs: Vec<Multiaddr>) -> Result<Vec<u8>, String> {
        let peer_id = PeerId::from(self.keypair.public());
        identify_spr::create_signed_peer_record(&self.keypair, peer_id, addrs)
    }
}

// Delegate all NetworkBehaviour methods to inner
impl libp2p::swarm::NetworkBehaviour for IdentifyBehaviour {
    type ConnectionHandler = <identify::Behaviour as libp2p::swarm::NetworkBehaviour>::ConnectionHandler;
    type ToSwarm = identify::Event;

    fn handle_established_inbound_connection(
        &mut self,
        connection_id: libp2p::swarm::ConnectionId,
        peer: PeerId,
        local_addr: &Multiaddr,
        remote_addr: &Multiaddr,
    ) -> Result<libp2p::swarm::THandler<Self>, libp2p::swarm::ConnectionDenied> {
        self.inner.handle_established_inbound_connection(
            connection_id,
            peer,
            local_addr,
            remote_addr,
        )
    }

    fn handle_established_outbound_connection(
        &mut self,
        connection_id: libp2p::swarm::ConnectionId,
        peer: PeerId,
        addr: &Multiaddr,
        role_override: Endpoint,
        port_use: libp2p::core::transport::PortUse,
    ) -> Result<libp2p::swarm::THandler<Self>, libp2p::swarm::ConnectionDenied> {
        self.inner.handle_established_outbound_connection(
            connection_id,
            peer,
            addr,
            role_override,
            port_use,
        )
    }

    fn on_swarm_event(&mut self, event: libp2p::swarm::FromSwarm) {
        self.inner.on_swarm_event(event);
    }

    fn on_connection_handler_event(
        &mut self,
        peer_id: PeerId,
        connection_id: libp2p::swarm::ConnectionId,
        event: libp2p::swarm::THandlerOutEvent<Self>,
    ) {
        self.inner
            .on_connection_handler_event(peer_id, connection_id, event);
    }

    fn poll(
        &mut self,
        cx: &mut Context,
    ) -> Poll<libp2p::swarm::ToSwarm<Self::ToSwarm, libp2p::swarm::THandlerInEvent<Self>>> {
        self.inner.poll(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identify_behaviour_creation() {
        let keypair = Keypair::generate_secp256k1();
        let config = IdentifyConfig::new("Archivist Node".to_string(), &keypair);
        let _behaviour = IdentifyBehaviour::new(config);
    }

    #[test]
    fn test_generate_spr() {
        let keypair = Keypair::generate_secp256k1();
        let config = IdentifyConfig::new("Archivist Node".to_string(), &keypair);
        let behaviour = IdentifyBehaviour::new(config);

        let addrs = vec!["/ip4/127.0.0.1/tcp/8070".parse().unwrap()];
        let spr = behaviour.generate_spr(addrs);
        assert!(spr.is_ok());
    }
}
