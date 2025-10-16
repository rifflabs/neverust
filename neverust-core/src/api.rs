//! REST API for block operations and node management

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use base64::Engine;
use cid::{multibase::Base, Cid};
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::trace::TraceLayer;
use tracing::{error, info};

use crate::archivist_tree::ArchivistTree;
use crate::botg::BoTgProtocol;
use crate::chunker::Chunker;
use crate::manifest::Manifest;
use crate::metrics::Metrics;
use crate::storage::{Block, BlockStore, StorageError};
use libp2p::{identity::Keypair, Multiaddr};
use std::io::Cursor;
use std::sync::RwLock;

/// Convert CID to base58btc string (Archivist format with 'z' prefix)
fn cid_to_string(cid: &Cid) -> String {
    cid.to_string_of_base(Base::Base58Btc)
        .unwrap_or_else(|_| cid.to_string())
}

/// API state shared across handlers
#[derive(Clone)]
pub struct ApiState {
    pub block_store: Arc<BlockStore>,
    pub metrics: Metrics,
    pub peer_id: String,
    pub botg: Arc<BoTgProtocol>,
    pub keypair: Arc<Keypair>,
    pub listen_addrs: Arc<RwLock<Vec<Multiaddr>>>,
}

/// Response for storing a block
#[derive(Serialize, Deserialize)]
pub struct StoreBlockResponse {
    pub cid: String,
    pub size: usize,
}

/// Response for retrieving a block
#[derive(Serialize, Deserialize)]
pub struct GetBlockResponse {
    pub cid: String,
    pub data: String, // base64-encoded
    pub size: usize,
}

/// Health check response
#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub block_count: usize,
    pub total_bytes: usize,
}

/// Error response
#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

/// Create the REST API router
pub fn create_router(
    block_store: Arc<BlockStore>,
    metrics: Metrics,
    peer_id: String,
    botg: Arc<BoTgProtocol>,
    keypair: Arc<Keypair>,
    listen_addrs: Arc<RwLock<Vec<Multiaddr>>>,
) -> Router {
    let state = ApiState {
        block_store,
        metrics,
        peer_id,
        botg,
        keypair,
        listen_addrs,
    };

    Router::new()
        .route("/health", get(health_check))
        .route("/metrics", get(metrics_endpoint))
        .route("/api/v1/blocks", post(store_block))
        .route("/api/v1/blocks/:cid", get(get_block))
        // Archivist-compatible endpoints
        .route("/api/archivist/v1/data", post(archivist_upload))
        .route(
            "/api/archivist/v1/data/:cid/network/stream",
            get(archivist_download),
        )
        .route("/api/archivist/v1/peer-id", get(peer_id_endpoint))
        .route("/api/archivist/v1/stats", get(archivist_stats))
        .route("/api/archivist/v1/spr", get(spr_endpoint))
        .with_state(state)
        .layer(TraceLayer::new_for_http())
}

/// Health check endpoint
async fn health_check(State(state): State<ApiState>) -> impl IntoResponse {
    let stats = state.block_store.stats().await;

    Json(HealthResponse {
        status: "ok".to_string(),
        block_count: stats.block_count,
        total_bytes: stats.total_size,
    })
}

/// Prometheus metrics endpoint
async fn metrics_endpoint(State(state): State<ApiState>) -> impl IntoResponse {
    let stats = state.block_store.stats().await;

    // Generate Prometheus-compatible metrics using the Metrics module
    let metrics = state
        .metrics
        .to_prometheus(stats.block_count, stats.total_size);

    (
        StatusCode::OK,
        [("content-type", "text/plain; version=0.0.4")],
        metrics,
    )
}

/// Store a block (POST /api/v1/blocks)
async fn store_block(
    State(state): State<ApiState>,
    body: bytes::Bytes,
) -> Result<Json<StoreBlockResponse>, ApiError> {
    if body.is_empty() {
        return Err(ApiError::BadRequest("Empty block data".to_string()));
    }

    info!("API: Storing block ({} bytes)", body.len());

    // Create block from data
    let block = Block::new(body.to_vec())
        .map_err(|e| ApiError::Internal(format!("Failed to create block: {}", e)))?;

    let cid = block.cid;
    let size = block.size();

    // Store in BlockStore
    state
        .block_store
        .put(block)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to store block: {}", e)))?;

    info!("API: Stored block {} ({} bytes)", cid, size);

    Ok(Json(StoreBlockResponse {
        cid: cid_to_string(&cid),
        size,
    }))
}

/// Retrieve a block (GET /api/v1/blocks/:cid)
/// Supports HTTP Range headers for partial content retrieval
async fn get_block(
    State(state): State<ApiState>,
    Path(cid_str): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    info!("API: Retrieving block {}", cid_str);

    // Parse CID
    let cid = cid_str
        .parse()
        .map_err(|e| ApiError::BadRequest(format!("Invalid CID: {}", e)))?;

    // Get block from store
    let block = state.block_store.get(&cid).await.map_err(|e| match e {
        StorageError::BlockNotFound(_) => ApiError::NotFound(cid_str.clone()),
        _ => ApiError::Internal(format!("Failed to retrieve block: {}", e)),
    })?;

    let total_size = block.size();

    // Check for Range header (HTTP partial content request)
    if let Some(range_header) = headers.get("range") {
        if let Ok(range_str) = range_header.to_str() {
            if let Some(range) = parse_range_header(range_str, total_size) {
                let (start, end) = range;
                let range_data = &block.data[start..end];

                info!(
                    "API: Serving range [{}, {}) of block {} ({} bytes of {})",
                    start,
                    end,
                    cid_str,
                    range_data.len(),
                    total_size
                );

                // Return 206 Partial Content with Content-Range header
                let response = Json(GetBlockResponse {
                    cid: cid_str,
                    data: base64::prelude::BASE64_STANDARD.encode(range_data),
                    size: range_data.len(),
                });

                let mut resp = response.into_response();
                *resp.status_mut() = StatusCode::PARTIAL_CONTENT;
                resp.headers_mut().insert(
                    "content-range",
                    format!("bytes {}-{}/{}", start, end - 1, total_size)
                        .parse()
                        .unwrap(),
                );
                resp.headers_mut()
                    .insert("accept-ranges", "bytes".parse().unwrap());

                return Ok(resp);
            }
        }
    }

    // No range request - return full block
    info!(
        "API: Retrieved full block {} ({} bytes)",
        cid_str, total_size
    );

    let response = Json(GetBlockResponse {
        cid: cid_str,
        data: base64::prelude::BASE64_STANDARD.encode(&block.data),
        size: total_size,
    });

    let mut resp = response.into_response();
    resp.headers_mut()
        .insert("accept-ranges", "bytes".parse().unwrap());

    Ok(resp)
}

/// Parse HTTP Range header (e.g., "bytes=1024-2047")
/// Returns (start, end) where end is exclusive
fn parse_range_header(range_str: &str, total_size: usize) -> Option<(usize, usize)> {
    // Range header format: "bytes=start-end"
    let range_str = range_str.trim().strip_prefix("bytes=")?;

    // Split on '-'
    let parts: Vec<&str> = range_str.split('-').collect();
    if parts.len() != 2 {
        return None;
    }

    let start: usize = parts[0].parse().ok()?;
    let end: usize = if parts[1].is_empty() {
        total_size
    } else {
        // HTTP Range header end is inclusive, convert to exclusive
        parts[1].parse::<usize>().ok()? + 1
    };

    // Validate range
    if start >= total_size || start >= end {
        return None;
    }

    let end = std::cmp::min(end, total_size);

    Some((start, end))
}

/// Archivist-compatible upload endpoint (POST /api/archivist/v1/data)
/// Returns manifest CID as plain text
async fn archivist_upload(
    State(state): State<ApiState>,
    body: bytes::Bytes,
) -> Result<String, ApiError> {
    if body.is_empty() {
        return Err(ApiError::BadRequest("Empty data".to_string()));
    }

    let dataset_size = body.len();
    info!(
        "Archivist API: Uploading data ({} bytes) - will chunk and create manifest",
        dataset_size
    );

    // Step 1: Chunk the data and store blocks
    let cursor = Cursor::new(body.to_vec());
    let mut chunker = Chunker::new(cursor); // Uses default 64KB chunks
    let mut block_cids = Vec::new();

    while let Some(chunk) = chunker
        .next_chunk()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to read chunk: {}", e)))?
    {
        // Create block from chunk (uses codec 0xcd02)
        let block = Block::new(chunk)
            .map_err(|e| ApiError::Internal(format!("Failed to create block: {}", e)))?;

        info!(
            "Archivist API: Created block {} ({} bytes)",
            block.cid,
            block.size()
        );

        block_cids.push(block.cid);

        // Store block
        state
            .block_store
            .put(block)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to store block: {}", e)))?;
    }

    info!(
        "Archivist API: Stored {} blocks for dataset",
        block_cids.len()
    );

    // Step 2: Build Archivist tree from block CIDs
    let tree = ArchivistTree::new(block_cids)
        .map_err(|e| ApiError::Internal(format!("Failed to create tree: {}", e)))?;

    let tree_cid = tree
        .root_cid()
        .map_err(|e| ApiError::Internal(format!("Failed to compute tree CID: {}", e)))?;

    info!("Archivist API: Built tree with root CID {}", tree_cid);

    // Step 2.5: Store the tree's block list as a metadata block
    // We serialize the block CIDs and store them with a predictable key derived from tree_cid
    // This allows us to reconstruct the block CIDs during download
    let tree_block_list = tree.serialize_block_list();

    // Create a block from the serialized data
    let tree_metadata_block = Block::new(tree_block_list)
        .map_err(|e| ApiError::Internal(format!("Failed to create tree metadata block: {}", e)))?;

    let tree_metadata_cid = tree_metadata_block.cid;

    state
        .block_store
        .put(tree_metadata_block)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to store tree metadata: {}", e)))?;

    info!(
        "Archivist API: Stored tree metadata {} ({} block CIDs) for tree {}",
        tree_metadata_cid,
        tree.block_cids().len(),
        tree_cid
    );

    // Step 3: Create manifest
    // Store metadata CID in filename field for retrieval during download
    // Format: "metadata:<cid>"
    let manifest = Manifest::new(
        tree_cid,
        chunker.chunk_size() as u64,
        dataset_size as u64,
        None,                                            // codec (uses default 0xcd02)
        None,                                            // hcodec (uses default SHA-256)
        None,                                            // version (uses default 1)
        Some(format!("metadata:{}", tree_metadata_cid)), // filename (stores metadata CID)
        None,                                            // mimetype
    );

    info!(
        "Archivist API: Created manifest for tree {} ({} blocks, {} bytes)",
        tree_cid,
        manifest.blocks_count(),
        dataset_size
    );

    // Step 4: Encode manifest as block (uses codec 0xcd01)
    let manifest_block = manifest
        .to_block()
        .map_err(|e| ApiError::Internal(format!("Failed to encode manifest: {}", e)))?;

    let manifest_cid = manifest_block.cid;

    // Step 5: Store manifest block
    state
        .block_store
        .put(manifest_block)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to store manifest: {}", e)))?;

    info!(
        "Archivist API: Uploaded manifest {} (tree: {}, blocks: {}, size: {})",
        manifest_cid,
        tree_cid,
        manifest.blocks_count(),
        dataset_size
    );

    // Return manifest CID as plain text (Archivist format with base58btc encoding)
    Ok(cid_to_string(&manifest_cid))
}

/// Archivist-compatible download endpoint (GET /api/archivist/v1/data/:cid/network/stream)
/// Returns raw binary data
async fn archivist_download(
    State(state): State<ApiState>,
    Path(cid_str): Path<String>,
) -> Result<Vec<u8>, ApiError> {
    info!("Archivist API: Downloading {}", cid_str);

    // Parse CID
    let cid = cid_str
        .parse()
        .map_err(|e| ApiError::BadRequest(format!("Invalid CID: {}", e)))?;

    // Try to get block from local store first
    match state.block_store.get(&cid).await {
        Ok(block) => {
            // Check if this is a manifest (codec 0xcd01) or a data block (codec 0xcd02)
            if cid.codec() == 0xcd01 {
                // This is a manifest - decode it and fetch the actual data
                info!(
                    "Archivist API: {} is a manifest, decoding to get data blocks",
                    cid_str
                );

                let manifest = Manifest::from_block(&block)
                    .map_err(|e| ApiError::Internal(format!("Failed to decode manifest: {}", e)))?;

                info!(
                    "Archivist API: Manifest has {} blocks, dataset size: {} bytes",
                    manifest.blocks_count(),
                    manifest.dataset_size
                );

                // Step 1: Extract metadata CID from manifest filename field
                let metadata_cid_str = manifest.filename.as_ref().ok_or_else(|| {
                    ApiError::Internal(
                        "Manifest missing metadata CID (no filename field)".to_string(),
                    )
                })?;

                // Parse metadata CID from "metadata:<cid>" format
                let metadata_cid_str =
                    metadata_cid_str.strip_prefix("metadata:").ok_or_else(|| {
                        ApiError::Internal(format!("Invalid metadata format: {}", metadata_cid_str))
                    })?;

                let metadata_cid: Cid = metadata_cid_str.parse().map_err(|e| {
                    ApiError::Internal(format!("Failed to parse metadata CID: {}", e))
                })?;

                info!("Archivist API: Fetching metadata block {}", metadata_cid);

                // Step 2: Fetch the tree metadata block to get block CIDs
                let tree_metadata_block =
                    state.block_store.get(&metadata_cid).await.map_err(|e| {
                        ApiError::Internal(format!(
                            "Failed to fetch tree metadata {}: {}",
                            metadata_cid, e
                        ))
                    })?;

                // Step 3: Deserialize block CIDs from metadata
                let block_cids = ArchivistTree::deserialize_block_list(&tree_metadata_block.data)
                    .map_err(|e| {
                    ApiError::Internal(format!("Failed to deserialize tree metadata: {}", e))
                })?;

                info!(
                    "Archivist API: Retrieved {} block CIDs from metadata {}",
                    block_cids.len(),
                    metadata_cid
                );

                // Verify block count matches manifest
                if block_cids.len() != manifest.blocks_count() {
                    return Err(ApiError::Internal(format!(
                        "Block count mismatch: tree has {} blocks but manifest expects {}",
                        block_cids.len(),
                        manifest.blocks_count()
                    )));
                }

                // Step 4: Fetch all blocks and reassemble data
                let mut data: Vec<u8> = Vec::with_capacity(manifest.dataset_size as usize);

                for (idx, block_cid) in block_cids.iter().enumerate() {
                    info!(
                        "Archivist API: Fetching block {}/{}: {}",
                        idx + 1,
                        block_cids.len(),
                        block_cid
                    );

                    // Try to get block from local store first
                    let block = match state.block_store.get(block_cid).await {
                        Ok(b) => b,
                        Err(StorageError::BlockNotFound(_)) => {
                            // Block not found - this is an error for manifest downloads
                            // In production, would fetch from network via BlockExc
                            return Err(ApiError::Internal(format!(
                                "Block {} not found (block {}/{})",
                                block_cid,
                                idx + 1,
                                block_cids.len()
                            )));
                        }
                        Err(e) => {
                            return Err(ApiError::Internal(format!(
                                "Failed to fetch block {}: {}",
                                block_cid, e
                            )));
                        }
                    };

                    // Append block data
                    data.extend_from_slice(&block.data);

                    info!(
                        "Archivist API: Fetched block {}/{} ({} bytes, total: {} bytes)",
                        idx + 1,
                        block_cids.len(),
                        block.size(),
                        data.len()
                    );
                }

                // Verify final size matches manifest
                if data.len() != manifest.dataset_size as usize {
                    return Err(ApiError::Internal(format!(
                        "Data size mismatch: assembled {} bytes but manifest expects {} bytes",
                        data.len(),
                        manifest.dataset_size
                    )));
                }

                info!(
                    "Archivist API: Successfully assembled manifest {} ({} blocks, {} bytes)",
                    cid_str,
                    block_cids.len(),
                    data.len()
                );

                Ok(data)
            } else {
                // This is a data block - return it directly
                info!(
                    "Archivist API: Downloaded data block {} from local store ({} bytes)",
                    cid_str,
                    block.size()
                );
                Ok(block.data)
            }
        }
        Err(StorageError::BlockNotFound(_)) => {
            // Block not found locally - try fetching from known peers via HTTP
            // This is a temporary solution - in production would use BlockExc/BoTG
            info!(
                "Archivist API: Block {} not found locally, fetching from peers",
                cid_str
            );

            // Try all known peers in Docker network (Archivist-style peer discovery)
            // Generate peer list: bootstrap + node1..node49 (for 50 node cluster)
            let mut peer_urls = vec![];

            // Add Docker network peers
            peer_urls.push("http://bootstrap:8080".to_string());
            for i in 1..50 {
                peer_urls.push(format!("http://node{}:8080", i));
            }

            // Add known external Archivist testnet peers (try multiple ports)
            let external_peers = vec![
                "91.98.135.54",
                "10.7.1.200", // blackberry
            ];
            for peer in external_peers {
                // Try common Archivist API ports
                for port in [8080, 8070, 8000, 3000] {
                    peer_urls.push(format!("http://{}:{}", peer, port));
                }
            }

            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(2))
                .build()
                .map_err(|e| ApiError::Internal(format!("Failed to create HTTP client: {}", e)))?;

            // Shuffle peers for load distribution (Archivist-style)
            {
                let mut rng = rand::thread_rng();
                peer_urls.shuffle(&mut rng);
            }

            for base_url in peer_urls.iter().take(25) {
                // Try up to 25 random peers
                let url = format!(
                    "{}/api/archivist/v1/data/{}/network/stream",
                    base_url, cid_str
                );

                info!(
                    "Archivist API: Trying to fetch {} from {}",
                    cid_str, base_url
                );

                match client.get(&url).send().await {
                    Ok(resp) => {
                        if resp.status().is_success() {
                            match resp.bytes().await {
                                Ok(data) => {
                                    info!(
                                        "Archivist API: Fetched {} from {} ({} bytes)",
                                        cid_str,
                                        base_url,
                                        data.len()
                                    );

                                    // Store locally
                                    let block = Block::new(data.to_vec()).map_err(|e| {
                                        ApiError::Internal(format!("Failed to create block: {}", e))
                                    })?;

                                    state.block_store.put(block.clone()).await.map_err(|e| {
                                        ApiError::Internal(format!("Failed to store block: {}", e))
                                    })?;

                                    return Ok(block.data);
                                }
                                Err(e) => {
                                    info!(
                                        "Archivist API: Failed to read response from {}: {}",
                                        base_url, e
                                    );
                                }
                            }
                        } else {
                            info!(
                                "Archivist API: Got HTTP {} from {}",
                                resp.status(),
                                base_url
                            );
                        }
                    }
                    Err(e) => {
                        info!("Archivist API: Failed to fetch from {}: {}", base_url, e);
                    }
                }
            }

            info!(
                "Archivist API: Block {} not available from any peer",
                cid_str
            );
            Err(ApiError::NotFound(cid_str.clone()))
        }
        Err(e) => Err(ApiError::Internal(format!(
            "Failed to retrieve block: {}",
            e
        ))),
    }
}

/// Peer ID endpoint (GET /api/archivist/v1/peer-id)
async fn peer_id_endpoint(State(state): State<ApiState>) -> impl IntoResponse {
    Json(state.peer_id)
}

/// Stats endpoint (GET /api/archivist/v1/stats)
async fn archivist_stats(State(state): State<ApiState>) -> impl IntoResponse {
    let stats = state.block_store.stats().await;

    Json(serde_json::json!({
        "block_count": stats.block_count,
        "total_size": stats.total_size,
    }))
}

/// SPR endpoint (GET /api/archivist/v1/spr)
/// Returns the Signed Peer Record for this node
async fn spr_endpoint(State(state): State<ApiState>) -> Result<String, ApiError> {
    use crate::spr::generate_spr;
    use std::time::{SystemTime, UNIX_EPOCH};

    // Use current timestamp as sequence number
    let seq = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Read listen addresses from shared state
    let addrs_snapshot = state.listen_addrs.read().unwrap().clone();

    // Filter listen addresses to only include UDP addresses (Archivist format)
    // Archivist SPRs contain UDP addresses for discovery
    let udp_addrs: Vec<Multiaddr> = addrs_snapshot
        .iter()
        .filter_map(|addr| {
            let addr_str = addr.to_string();
            if addr_str.contains("/tcp/") {
                // Convert TCP to UDP for SPR (Archivist convention)
                let udp_str = addr_str.replace("/tcp/", "/udp/");
                udp_str.parse().ok()
            } else {
                None
            }
        })
        .collect();

    if udp_addrs.is_empty() {
        return Err(ApiError::Internal(
            "No listen addresses available".to_string(),
        ));
    }

    // Generate SPR
    let spr = generate_spr(&state.keypair, &udp_addrs, seq)
        .map_err(|e| ApiError::Internal(format!("Failed to generate SPR: {}", e)))?;

    info!("Generated SPR for peer {}", state.peer_id);

    Ok(spr)
}

/// API error type
#[derive(Debug)]
enum ApiError {
    BadRequest(String),
    NotFound(String),
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            ApiError::NotFound(cid) => (StatusCode::NOT_FOUND, format!("Block not found: {}", cid)),
            ApiError::Internal(msg) => {
                error!("API error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, msg)
            }
        };

        let body = Json(ErrorResponse { error: message });
        (status, body).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::util::ServiceExt;

    #[tokio::test]
    async fn test_health_check() {
        use crate::botg::BoTgConfig;
        use libp2p::identity::Keypair;

        let block_store = Arc::new(BlockStore::new());
        let metrics = Metrics::new();
        let peer_id = "12D3KooWTest123".to_string();
        let botg = Arc::new(BoTgProtocol::new(BoTgConfig::default()));
        let keypair = Arc::new(Keypair::generate_ed25519());
        let listen_addrs = Arc::new(RwLock::new(vec!["/ip4/127.0.0.1/tcp/8070"
            .parse()
            .unwrap()]));
        let app = create_router(block_store, metrics, peer_id, botg, keypair, listen_addrs);

        let request = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_store_and_get_block() {
        use crate::botg::BoTgConfig;
        use libp2p::identity::Keypair;

        let block_store = Arc::new(BlockStore::new());
        let metrics = Metrics::new();
        let botg = Arc::new(BoTgProtocol::new(BoTgConfig::default()));
        let keypair = Arc::new(Keypair::generate_ed25519());
        let listen_addrs = Arc::new(RwLock::new(vec!["/ip4/127.0.0.1/tcp/8070"
            .parse()
            .unwrap()]));
        let app = create_router(
            block_store,
            metrics,
            "12D3KooWTest123".to_string(),
            botg,
            keypair,
            listen_addrs,
        );

        // Store a block
        let test_data = b"Hello, REST API!";
        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/blocks")
            .header("content-type", "application/octet-stream")
            .body(Body::from(test_data.to_vec()))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let store_response: StoreBlockResponse = serde_json::from_slice(&body).unwrap();

        // Get the block back
        let request = Request::builder()
            .uri(format!("/api/v1/blocks/{}", store_response.cid))
            .body(Body::empty())
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let get_block_response: GetBlockResponse = serde_json::from_slice(&body).unwrap();

        // Verify data matches
        let decoded_data = base64::prelude::BASE64_STANDARD
            .decode(&get_block_response.data)
            .unwrap();
        assert_eq!(decoded_data, test_data);
    }

    #[tokio::test]
    async fn test_get_nonexistent_block() {
        use crate::botg::BoTgConfig;
        use libp2p::identity::Keypair;

        let block_store = Arc::new(BlockStore::new());
        let metrics = Metrics::new();
        let botg = Arc::new(BoTgProtocol::new(BoTgConfig::default()));
        let keypair = Arc::new(Keypair::generate_ed25519());
        let listen_addrs = Arc::new(RwLock::new(vec!["/ip4/127.0.0.1/tcp/8070"
            .parse()
            .unwrap()]));
        let app = create_router(
            block_store,
            metrics,
            "12D3KooWTest123".to_string(),
            botg,
            keypair,
            listen_addrs,
        );

        let request = Request::builder()
            .uri("/api/v1/blocks/bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi")
            .body(Body::empty())
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
