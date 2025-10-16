//! DiscV5-based peer discovery for Archivist network
//!
//! Implements UDP-based peer discovery using the Kademlia DHT protocol (DiscV5).
//! This enables automatic discovery of peers and content providers in the network.

use cid::Cid;
use discv5::handler::NodeContact;
use discv5::{enr, ConfigBuilder, Discv5, Event as Discv5Event, IpMode, ListenConfig, TalkRequest};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};
use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

// Re-export PeerId from libp2p for use in discovery
use libp2p::identity::PeerId;

#[derive(Debug, thiserror::Error)]
pub enum DiscoveryError {
    #[error("DiscV5 error: {0}")]
    Discv5Error(String),

    #[error("ENR error: {0}")]
    EnrError(String),

    #[error("Invalid peer ID")]
    InvalidPeerId,

    #[error("No providers found for CID: {0}")]
    NoProviders(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

type Result<T> = std::result::Result<T, DiscoveryError>;

// TALK protocol identifiers matching Archivist
const TALK_PROTOCOL_ADD_PROVIDER: &[u8] = b"add_provider";
const TALK_PROTOCOL_GET_PROVIDERS: &[u8] = b"get_providers";

/// Provider record for a CID
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderRecord {
    /// The CID being provided
    pub cid: String,
    /// The peer ID of the provider
    pub peer_id: Vec<u8>,
    /// Multiaddresses where the provider can be reached
    pub addrs: Vec<String>,
    /// Timestamp when this record was created
    pub timestamp: u64,
}

/// Request to add a provider record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddProviderRequest {
    pub record: ProviderRecord,
}

/// Response to add provider request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddProviderResponse {
    pub success: bool,
}

/// Request to get providers for a CID
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetProvidersRequest {
    pub cid: String,
}

/// Response to get providers request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetProvidersResponse {
    pub providers: Vec<ProviderRecord>,
    /// Closer peers that might have the providers
    pub closer_peers: Vec<Vec<u8>>, // NodeId bytes
}

/// Convert a CID to a DiscV5 NodeId using Keccak256
///
/// This matches the Archivist implementation: keccak256.digest(cid.data.buffer)
/// The resulting 256-bit hash is interpreted as a big-endian NodeId for DHT operations.
///
/// # Arguments
/// * `cid` - The CID to convert
///
/// # Returns
/// A DiscV5 NodeId derived from the Keccak256 hash of the CID bytes
pub fn cid_to_node_id(cid: &Cid) -> enr::NodeId {
    // Hash the CID bytes using Keccak256
    let mut hasher = Keccak256::new();
    hasher.update(cid.to_bytes());
    let hash = hasher.finalize();

    // Convert the 32-byte hash to a NodeId (256-bit big-endian)
    // NodeId::new expects a 32-byte array
    let hash_bytes: [u8; 32] = hash.into();
    enr::NodeId::new(&hash_bytes)
}

/// Provider storage and management
struct ProvidersManager {
    /// Local provider records: CID -> our record
    local_providers: HashMap<Cid, ProviderRecord>,
    /// Remote provider records: CID -> Vec<ProviderRecord>
    remote_providers: HashMap<Cid, Vec<ProviderRecord>>,
}

impl ProvidersManager {
    fn new() -> Self {
        Self {
            local_providers: HashMap::new(),
            remote_providers: HashMap::new(),
        }
    }

    /// Add a local provider record
    fn add_local(&mut self, cid: Cid, record: ProviderRecord) {
        self.local_providers.insert(cid, record);
    }

    /// Add a remote provider record
    fn add_remote(&mut self, cid: Cid, record: ProviderRecord) {
        self.remote_providers.entry(cid).or_default().push(record);
    }

    /// Get all providers for a CID (local + remote)
    fn get_providers(&self, cid: &Cid) -> Vec<ProviderRecord> {
        let mut providers = Vec::new();

        // Add local provider if we have it
        if let Some(local) = self.local_providers.get(cid) {
            providers.push(local.clone());
        }

        // Add remote providers
        if let Some(remote) = self.remote_providers.get(cid) {
            providers.extend(remote.iter().cloned());
        }

        providers
    }
}

/// Peer discovery service using DiscV5
pub struct Discovery {
    /// DiscV5 protocol instance
    discv5: Arc<Discv5>,

    /// Local peer ID
    peer_id: PeerId,

    /// Provider records manager
    providers: Arc<RwLock<ProvidersManager>>,

    /// Announced multiaddrs for this node
    announce_addrs: Vec<String>,
}

impl Discovery {
    /// Create a new Discovery instance
    pub async fn new(
        keypair: &libp2p::identity::Keypair,
        listen_addr: SocketAddr,
        announce_addrs: Vec<String>,
        bootstrap_peers: Vec<String>,
    ) -> Result<Self> {
        info!("Initializing DiscV5 peer discovery on {}", listen_addr);

        // Extract secp256k1 key bytes from libp2p keypair
        // libp2p v0.56+ uses identity module
        let _key_bytes = keypair
            .to_protobuf_encoding()
            .map_err(|e| DiscoveryError::EnrError(format!("Failed to encode keypair: {}", e)))?;

        // Try to create secp256k1 signing key from encoded bytes
        // For now, we'll generate a fresh key since libp2p keypair extraction is complex
        warn!(
            "Generating fresh secp256k1 key for DiscV5 (libp2p key extraction not yet implemented)"
        );
        let secret_key = enr::k256::ecdsa::SigningKey::random(&mut rand::thread_rng());

        let peer_id = keypair.public().to_peer_id();

        // Create ENR builder
        let enr_key = enr::CombinedKey::Secp256k1(secret_key);
        let mut builder = enr::Enr::builder();

        // Add IP and UDP port
        match listen_addr.ip() {
            IpAddr::V4(ip) => {
                builder.ip4(ip);
                builder.udp4(listen_addr.port());
            }
            IpAddr::V6(ip) => {
                builder.ip6(ip);
                builder.udp6(listen_addr.port());
            }
        }

        // Add libp2p peer ID as custom ENR entry
        builder.add_value("libp2p", &peer_id.to_bytes());

        let enr = builder
            .build(&enr_key)
            .map_err(|e| DiscoveryError::EnrError(e.to_string()))?;

        info!("Local ENR: {}", enr.to_base64());
        info!("Local Peer ID: {}", peer_id);

        // Create listen config
        let listen_config = ListenConfig::Ipv4 {
            ip: match listen_addr.ip() {
                IpAddr::V4(ip) => ip,
                IpAddr::V6(_) => {
                    return Err(DiscoveryError::Discv5Error("IPv6 not yet supported".into()))
                }
            },
            port: listen_addr.port(),
        };

        // Configure DiscV5 with listen config
        let config = ConfigBuilder::new(listen_config).build();

        // Initialize DiscV5
        let mut discv5 = Discv5::new(enr, enr_key, config)
            .map_err(|e| DiscoveryError::Discv5Error(e.to_string()))?;

        // Start listening (no arguments - uses config from constructor)
        discv5
            .start()
            .await
            .map_err(|e| DiscoveryError::Discv5Error(e.to_string()))?;

        info!("DiscV5 listening on {}", listen_addr);

        // Add bootstrap peers
        for peer_str in bootstrap_peers {
            match peer_str.parse::<enr::Enr<enr::CombinedKey>>() {
                Ok(bootstrap_enr) => match discv5.add_enr(bootstrap_enr.clone()) {
                    Ok(_) => info!("Added bootstrap peer: {}", bootstrap_enr.node_id()),
                    Err(e) => warn!("Failed to add bootstrap peer: {}", e),
                },
                Err(e) => warn!("Invalid bootstrap ENR {}: {}", peer_str, e),
            }
        }

        Ok(Self {
            discv5: Arc::new(discv5),
            peer_id,
            providers: Arc::new(RwLock::new(ProvidersManager::new())),
            announce_addrs,
        })
    }

    /// Get local peer ID
    pub fn local_peer_id(&self) -> &PeerId {
        &self.peer_id
    }

    /// Get local ENR
    pub fn local_enr(&self) -> enr::Enr<enr::CombinedKey> {
        self.discv5.local_enr()
    }

    /// Announce that we provide a specific CID (block)
    pub async fn provide(&self, cid: &Cid) -> Result<()> {
        debug!("Announcing provider record for CID: {}", cid);

        // Create provider record
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let record = ProviderRecord {
            cid: cid.to_string(),
            peer_id: self.peer_id.to_bytes(),
            addrs: self.announce_addrs.clone(),
            timestamp,
        };

        // Store locally
        let mut providers = self.providers.write().await;
        providers.add_local(*cid, record.clone());
        drop(providers);

        // Publish to DHT - find K closest nodes to CID and send ADD_PROVIDER
        let node_id = cid_to_node_id(cid);
        let closest_nodes = self
            .discv5
            .find_node(node_id)
            .await
            .map_err(|e| DiscoveryError::Discv5Error(e.to_string()))?;

        debug!("Found {} nodes close to CID {}", closest_nodes.len(), cid);

        // Send ADD_PROVIDER to closest nodes via TALK protocol
        let request = AddProviderRequest { record };
        let request_bytes = bincode::serialize(&request)
            .map_err(|e| DiscoveryError::SerializationError(e.to_string()))?;

        for node in closest_nodes.iter().take(3) {
            // Send to top 3 closest nodes
            // Convert ENR to NodeContact
            match NodeContact::try_from_enr(node.clone(), IpMode::default()) {
                Ok(node_contact) => {
                    match self
                        .discv5
                        .talk_req(
                            node_contact,
                            TALK_PROTOCOL_ADD_PROVIDER.to_vec(),
                            request_bytes.clone(),
                        )
                        .await
                    {
                        Ok(_response) => {
                            debug!("Sent ADD_PROVIDER to node {}", node.node_id());
                        }
                        Err(e) => {
                            warn!(
                                "Failed to send ADD_PROVIDER to node {}: {}",
                                node.node_id(),
                                e
                            );
                        }
                    }
                }
                Err(_) => {
                    debug!("Node {} is not contactable, skipping", node.node_id());
                }
            }
        }

        info!("Announced provider record for CID: {}", cid);
        Ok(())
    }

    /// Find providers for a specific CID
    pub async fn find(&self, cid: &Cid) -> Result<Vec<PeerId>> {
        debug!("Searching for providers of CID: {}", cid);

        // Check local cache first
        let providers = self.providers.read().await;
        let cached_providers = providers.get_providers(cid);
        if !cached_providers.is_empty() {
            debug!("Found {} providers in cache", cached_providers.len());
            let peer_ids = cached_providers
                .iter()
                .filter_map(|record| PeerId::from_bytes(&record.peer_id).ok())
                .collect();
            drop(providers);
            return Ok(peer_ids);
        }
        drop(providers);

        // Query DHT for providers - find K closest nodes to CID
        let node_id = cid_to_node_id(cid);
        let closest_nodes = self
            .discv5
            .find_node(node_id)
            .await
            .map_err(|e| DiscoveryError::Discv5Error(e.to_string()))?;

        debug!("Found {} nodes close to CID {}", closest_nodes.len(), cid);

        // Send GET_PROVIDERS to closest nodes via TALK protocol
        let request = GetProvidersRequest {
            cid: cid.to_string(),
        };
        let request_bytes = bincode::serialize(&request)
            .map_err(|e| DiscoveryError::SerializationError(e.to_string()))?;

        let mut all_providers = Vec::new();

        for node in closest_nodes.iter().take(3) {
            // Query top 3 closest nodes
            // Convert ENR to NodeContact
            match NodeContact::try_from_enr(node.clone(), IpMode::default()) {
                Ok(node_contact) => {
                    match self
                        .discv5
                        .talk_req(
                            node_contact,
                            TALK_PROTOCOL_GET_PROVIDERS.to_vec(),
                            request_bytes.clone(),
                        )
                        .await
                    {
                        Ok(response_bytes) => {
                            match bincode::deserialize::<GetProvidersResponse>(&response_bytes) {
                                Ok(response) => {
                                    debug!(
                                        "Received {} providers from node {}",
                                        response.providers.len(),
                                        node.node_id()
                                    );

                                    // Store received providers in cache
                                    let mut providers = self.providers.write().await;
                                    for provider_record in &response.providers {
                                        providers.add_remote(*cid, provider_record.clone());
                                    }
                                    drop(providers);

                                    all_providers.extend(response.providers);
                                }
                                Err(e) => {
                                    warn!("Failed to deserialize GET_PROVIDERS response: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            debug!(
                                "Failed to query node {} for providers: {}",
                                node.node_id(),
                                e
                            );
                        }
                    }
                }
                Err(_) => {
                    debug!("Node {} is not contactable, skipping", node.node_id());
                }
            }
        }

        if all_providers.is_empty() {
            return Err(DiscoveryError::NoProviders(cid.to_string()));
        }

        // Convert to PeerIds and deduplicate
        let peer_ids: Vec<PeerId> = all_providers
            .iter()
            .filter_map(|record| PeerId::from_bytes(&record.peer_id).ok())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        info!("Found {} providers for CID: {}", peer_ids.len(), cid);
        Ok(peer_ids)
    }

    /// Find a specific peer by ID
    pub async fn find_peer(&self, peer_id: &PeerId) -> Result<Vec<String>> {
        debug!("Searching for peer: {}", peer_id);

        // Query DHT for peer's ENR
        // Convert PeerId to NodeId for lookup
        // For now, return empty (not yet implemented)
        warn!("DHT peer lookups not yet fully implemented");

        Ok(vec![])
    }

    /// Get connected peer count
    pub fn connected_peers(&self) -> usize {
        self.discv5.connected_peers()
    }

    /// Run the discovery event loop
    pub async fn run(self: Arc<Self>) {
        info!("Starting DiscV5 event loop");

        let mut event_stream = self.discv5.event_stream().await.unwrap();

        loop {
            tokio::select! {
                Some(event) = event_stream.recv() => {
                    self.handle_event(event).await;
                }
            }
        }
    }

    /// Handle DiscV5 events
    async fn handle_event(&self, event: Discv5Event) {
        match event {
            Discv5Event::Discovered(enr) => {
                debug!("Discovered peer: {}", enr.node_id());

                // Extract libp2p peer ID if available
                if let Some(Ok(peer_id_bytes)) = enr.get_decodable::<Vec<u8>>("libp2p") {
                    match PeerId::from_bytes(&peer_id_bytes) {
                        Ok(peer_id) => {
                            info!(
                                "Discovered libp2p peer: {} (ENR: {})",
                                peer_id,
                                enr.node_id()
                            );
                        }
                        Err(e) => {
                            warn!("Invalid libp2p peer ID in ENR: {}", e);
                        }
                    }
                }
            }
            Discv5Event::NodeInserted { node_id, replaced } => {
                if let Some(old_node) = replaced {
                    debug!("Replaced node {} with {}", old_node, node_id);
                } else {
                    debug!("Inserted new node: {}", node_id);
                }
            }
            Discv5Event::SessionEstablished(enr, socket_addr) => {
                info!(
                    "Session established with {} at {}",
                    enr.node_id(),
                    socket_addr
                );
            }
            Discv5Event::TalkRequest(talk_request) => {
                self.handle_talk_request(talk_request).await;
            }
            _ => {
                // Other events
                debug!("DiscV5 event: {:?}", event);
            }
        }
    }

    /// Handle TALK protocol requests
    async fn handle_talk_request(&self, talk_request: TalkRequest) {
        let protocol = talk_request.protocol().to_vec();
        let request_body = talk_request.body().to_vec();

        match &protocol[..] {
            TALK_PROTOCOL_ADD_PROVIDER => {
                self.handle_add_provider(talk_request, &request_body).await;
            }
            TALK_PROTOCOL_GET_PROVIDERS => {
                self.handle_get_providers(talk_request, &request_body).await;
            }
            _ => {
                debug!(
                    "Unknown TALK protocol: {:?}",
                    String::from_utf8_lossy(&protocol)
                );
            }
        }
    }

    /// Handle ADD_PROVIDER request
    async fn handle_add_provider(&self, talk_request: TalkRequest, request_body: &[u8]) {
        match bincode::deserialize::<AddProviderRequest>(request_body) {
            Ok(request) => {
                let record = request.record;
                debug!("Received ADD_PROVIDER for CID: {}", record.cid);

                // Parse CID and store the provider record
                if let Ok(cid) = record.cid.parse::<Cid>() {
                    let mut providers = self.providers.write().await;
                    providers.add_remote(cid, record);
                    drop(providers);

                    // Send success response
                    let response = AddProviderResponse { success: true };
                    if let Ok(response_bytes) = bincode::serialize(&response) {
                        if let Err(e) = talk_request.respond(response_bytes) {
                            warn!("Failed to send ADD_PROVIDER response: {}", e);
                        }
                    }
                } else {
                    warn!("Invalid CID in ADD_PROVIDER request");
                    let response = AddProviderResponse { success: false };
                    if let Ok(response_bytes) = bincode::serialize(&response) {
                        let _ = talk_request.respond(response_bytes);
                    }
                }
            }
            Err(e) => {
                warn!("Failed to deserialize ADD_PROVIDER request: {}", e);
            }
        }
    }

    /// Handle GET_PROVIDERS request
    async fn handle_get_providers(&self, talk_request: TalkRequest, request_body: &[u8]) {
        match bincode::deserialize::<GetProvidersRequest>(request_body) {
            Ok(request) => {
                debug!("Received GET_PROVIDERS for CID: {}", request.cid);

                // Parse CID and lookup providers
                if let Ok(cid) = request.cid.parse::<Cid>() {
                    let providers = self.providers.read().await;
                    let provider_records = providers.get_providers(&cid);
                    drop(providers);

                    // Send response with providers
                    let response = GetProvidersResponse {
                        providers: provider_records,
                        closer_peers: Vec::new(), // TODO: implement closer peers lookup
                    };

                    if let Ok(response_bytes) = bincode::serialize(&response) {
                        if let Err(e) = talk_request.respond(response_bytes) {
                            warn!("Failed to send GET_PROVIDERS response: {}", e);
                        }
                    }
                } else {
                    warn!("Invalid CID in GET_PROVIDERS request");
                    let response = GetProvidersResponse {
                        providers: Vec::new(),
                        closer_peers: Vec::new(),
                    };
                    if let Ok(response_bytes) = bincode::serialize(&response) {
                        let _ = talk_request.respond(response_bytes);
                    }
                }
            }
            Err(e) => {
                warn!("Failed to deserialize GET_PROVIDERS request: {}", e);
            }
        }
    }

    /// Get statistics
    pub fn stats(&self) -> DiscoveryStats {
        DiscoveryStats {
            connected_peers: self.connected_peers(),
            local_peer_id: self.peer_id,
            local_enr: self.local_enr().to_base64(),
        }
    }
}

/// Discovery statistics
#[derive(Debug, Clone)]
pub struct DiscoveryStats {
    pub connected_peers: usize,
    pub local_peer_id: PeerId,
    pub local_enr: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use libp2p::identity::Keypair;

    #[test]
    fn test_cid_to_node_id_deterministic() {
        // Test that the same CID always produces the same NodeId
        let cid: Cid = "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi"
            .parse()
            .unwrap();

        let node_id1 = cid_to_node_id(&cid);
        let node_id2 = cid_to_node_id(&cid);

        assert_eq!(
            node_id1, node_id2,
            "CID to NodeId conversion must be deterministic"
        );
    }

    #[test]
    fn test_cid_to_node_id_different_cids() {
        // Test that different CIDs produce different NodeIds
        let cid1: Cid = "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi"
            .parse()
            .unwrap();
        let cid2: Cid = "bafybeihdwdcefgh4dqkjv67uzcmw7ojee6xedzdetojuzjevtenxquvyku"
            .parse()
            .unwrap();

        let node_id1 = cid_to_node_id(&cid1);
        let node_id2 = cid_to_node_id(&cid2);

        assert_ne!(
            node_id1, node_id2,
            "Different CIDs must produce different NodeIds"
        );
    }

    #[test]
    fn test_cid_to_node_id_keccak256_output() {
        // Test that the NodeId is a valid 256-bit value (32 bytes)
        let cid: Cid = "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi"
            .parse()
            .unwrap();

        let node_id = cid_to_node_id(&cid);

        // NodeId should be valid (not panic) and internally store 32 bytes
        // We can verify this by converting to raw bytes
        let raw_bytes = node_id.raw();
        assert_eq!(raw_bytes.len(), 32, "NodeId must be 32 bytes (256 bits)");
    }

    #[test]
    fn test_cid_to_node_id_matches_archivist_format() {
        // This test verifies the conversion uses Keccak256 properly
        // by ensuring the output format matches Archivist's expectations
        let cid: Cid = "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi"
            .parse()
            .unwrap();

        // Manually compute expected NodeId using Keccak256
        use sha3::{Digest, Keccak256};
        let mut hasher = Keccak256::new();
        hasher.update(cid.to_bytes());
        let expected_hash = hasher.finalize();

        let node_id = cid_to_node_id(&cid);
        let node_id_bytes = node_id.raw();

        assert_eq!(
            &expected_hash[..],
            node_id_bytes,
            "NodeId must match Keccak256 hash of CID bytes"
        );
    }

    #[test]
    fn test_cid_to_node_id_various_formats() {
        // Test CIDs with different formats (CIDv0, CIDv1, different codecs)
        let test_cases = vec![
            // CIDv1 with dag-pb codec (0x70)
            "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi",
            // CIDv1 with raw codec (0x55)
            "bafkreigh2akiscaildcqabsyg3dfr6chu3fgpregiymsck7e7aqa4s52zy",
            // Another CIDv1
            "bafybeihdwdcefgh4dqkjv67uzcmw7ojee6xedzdetojuzjevtenxquvyku",
        ];

        for cid_str in test_cases {
            let cid: Cid = cid_str.parse().unwrap();
            let node_id = cid_to_node_id(&cid);

            // Each should produce a valid 32-byte NodeId
            assert_eq!(
                node_id.raw().len(),
                32,
                "NodeId for {} must be 32 bytes",
                cid_str
            );
        }
    }

    #[tokio::test]
    async fn test_discovery_creation() {
        let keypair = Keypair::generate_secp256k1();
        let listen_addr = "127.0.0.1:9000".parse().unwrap();
        let announce_addrs = vec!["/ip4/127.0.0.1/tcp/8070".to_string()];

        let discovery = Discovery::new(&keypair, listen_addr, announce_addrs, vec![])
            .await
            .unwrap();

        assert_eq!(discovery.connected_peers(), 0);
        assert_eq!(discovery.local_peer_id(), &keypair.public().to_peer_id());
    }

    #[tokio::test]
    async fn test_provide_and_find() {
        let keypair = Keypair::generate_secp256k1();
        let listen_addr = "127.0.0.1:9001".parse().unwrap();
        let announce_addrs = vec!["/ip4/127.0.0.1/tcp/8070".to_string()];

        let discovery = Discovery::new(&keypair, listen_addr, announce_addrs, vec![])
            .await
            .unwrap();

        // Create a test CID
        let cid: Cid = "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi"
            .parse()
            .unwrap();

        // Announce we provide this CID
        // Note: This will try to publish to DHT but with no peers, it will just store locally
        discovery.provide(&cid).await.unwrap();

        // Should find ourselves as provider
        let providers = discovery.find(&cid).await.unwrap();
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0], keypair.public().to_peer_id());
    }

    #[test]
    fn test_providers_manager() {
        let mut manager = ProvidersManager::new();

        // Create test CID and record
        let cid: Cid = "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi"
            .parse()
            .unwrap();

        let record = ProviderRecord {
            cid: cid.to_string(),
            peer_id: vec![1, 2, 3, 4],
            addrs: vec!["/ip4/127.0.0.1/tcp/8070".to_string()],
            timestamp: 1234567890,
        };

        // Add local provider
        manager.add_local(cid, record.clone());

        // Retrieve providers
        let providers = manager.get_providers(&cid);
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].cid, cid.to_string());

        // Add remote provider with different peer ID
        let remote_record = ProviderRecord {
            cid: cid.to_string(),
            peer_id: vec![5, 6, 7, 8],
            addrs: vec!["/ip4/192.168.1.1/tcp/8070".to_string()],
            timestamp: 1234567891,
        };

        manager.add_remote(cid, remote_record.clone());

        // Should now have 2 providers
        let providers = manager.get_providers(&cid);
        assert_eq!(providers.len(), 2);
    }

    #[test]
    fn test_provider_record_serialization() {
        let record = ProviderRecord {
            cid: "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi".to_string(),
            peer_id: vec![1, 2, 3, 4],
            addrs: vec!["/ip4/127.0.0.1/tcp/8070".to_string()],
            timestamp: 1234567890,
        };

        // Test bincode serialization
        let serialized = bincode::serialize(&record).unwrap();
        let deserialized: ProviderRecord = bincode::deserialize(&serialized).unwrap();

        assert_eq!(record.cid, deserialized.cid);
        assert_eq!(record.peer_id, deserialized.peer_id);
        assert_eq!(record.addrs, deserialized.addrs);
        assert_eq!(record.timestamp, deserialized.timestamp);
    }

    #[test]
    fn test_add_provider_request_serialization() {
        let record = ProviderRecord {
            cid: "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi".to_string(),
            peer_id: vec![1, 2, 3, 4],
            addrs: vec!["/ip4/127.0.0.1/tcp/8070".to_string()],
            timestamp: 1234567890,
        };

        let request = AddProviderRequest { record };

        // Test bincode serialization
        let serialized = bincode::serialize(&request).unwrap();
        let deserialized: AddProviderRequest = bincode::deserialize(&serialized).unwrap();

        assert_eq!(request.record.cid, deserialized.record.cid);
        assert_eq!(request.record.peer_id, deserialized.record.peer_id);
    }

    #[test]
    fn test_get_providers_request_serialization() {
        let request = GetProvidersRequest {
            cid: "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi".to_string(),
        };

        // Test bincode serialization
        let serialized = bincode::serialize(&request).unwrap();
        let deserialized: GetProvidersRequest = bincode::deserialize(&serialized).unwrap();

        assert_eq!(request.cid, deserialized.cid);
    }

    #[test]
    fn test_get_providers_response_serialization() {
        let record = ProviderRecord {
            cid: "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi".to_string(),
            peer_id: vec![1, 2, 3, 4],
            addrs: vec!["/ip4/127.0.0.1/tcp/8070".to_string()],
            timestamp: 1234567890,
        };

        let response = GetProvidersResponse {
            providers: vec![record],
            closer_peers: vec![vec![9, 10, 11, 12]],
        };

        // Test bincode serialization
        let serialized = bincode::serialize(&response).unwrap();
        let deserialized: GetProvidersResponse = bincode::deserialize(&serialized).unwrap();

        assert_eq!(response.providers.len(), deserialized.providers.len());
        assert_eq!(response.closer_peers.len(), deserialized.closer_peers.len());
    }
}
