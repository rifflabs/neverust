//! REST API for block operations and node management

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use base64::Engine;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::trace::TraceLayer;
use tracing::{error, info};

use crate::botg::BoTgProtocol;
use crate::metrics::Metrics;
use crate::storage::{Block, BlockStore, StorageError};

/// API state shared across handlers
#[derive(Clone)]
pub struct ApiState {
    pub block_store: Arc<BlockStore>,
    pub metrics: Metrics,
    pub peer_id: String,
    pub botg: Arc<BoTgProtocol>,
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
) -> Router {
    let state = ApiState {
        block_store,
        metrics,
        peer_id,
        botg,
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
        cid: cid.to_string(),
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
                resp.headers_mut().insert(
                    "accept-ranges",
                    "bytes".parse().unwrap(),
                );

                return Ok(resp);
            }
        }
    }

    // No range request - return full block
    info!("API: Retrieved full block {} ({} bytes)", cid_str, total_size);

    let response = Json(GetBlockResponse {
        cid: cid_str,
        data: base64::prelude::BASE64_STANDARD.encode(&block.data),
        size: total_size,
    });

    let mut resp = response.into_response();
    resp.headers_mut().insert(
        "accept-ranges",
        "bytes".parse().unwrap(),
    );

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
/// Returns CID as plain text
async fn archivist_upload(
    State(state): State<ApiState>,
    body: bytes::Bytes,
) -> Result<String, ApiError> {
    if body.is_empty() {
        return Err(ApiError::BadRequest("Empty data".to_string()));
    }

    info!("Archivist API: Uploading data ({} bytes)", body.len());

    // Create block from data
    let block = Block::new(body.to_vec())
        .map_err(|e| ApiError::Internal(format!("Failed to create block: {}", e)))?;

    let cid = block.cid;

    // Store in BlockStore
    state
        .block_store
        .put(block)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to store block: {}", e)))?;

    info!("Archivist API: Uploaded {} ({} bytes)", cid, body.len());

    // Return CID as plain text (Archivist format)
    Ok(cid.to_string())
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
            info!(
                "Archivist API: Downloaded {} from local store ({} bytes)",
                cid_str,
                block.size()
            );
            Ok(block.data)
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
            let mut peer_hostnames = vec!["bootstrap".to_string()];
            for i in 1..50 {
                peer_hostnames.push(format!("node{}", i));
            }

            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(2))
                .build()
                .map_err(|e| ApiError::Internal(format!("Failed to create HTTP client: {}", e)))?;

            // Shuffle peers for load distribution (Archivist-style)
            {
                let mut rng = rand::thread_rng();
                peer_hostnames.shuffle(&mut rng);
            }

            for hostname in peer_hostnames.iter().take(10) {
                // Try up to 10 random peers
                let url = format!(
                    "http://{}:8080/api/archivist/v1/data/{}/network/stream",
                    hostname, cid_str
                );

                info!(
                    "Archivist API: Trying to fetch {} from {}",
                    cid_str, hostname
                );

                match client.get(&url).send().await {
                    Ok(resp) => {
                        if resp.status().is_success() {
                            match resp.bytes().await {
                                Ok(data) => {
                                    info!(
                                        "Archivist API: Fetched {} from {} ({} bytes)",
                                        cid_str,
                                        hostname,
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
                                        hostname, e
                                    );
                                }
                            }
                        } else {
                            info!(
                                "Archivist API: Got HTTP {} from {}",
                                resp.status(),
                                hostname
                            );
                        }
                    }
                    Err(e) => {
                        info!("Archivist API: Failed to fetch from {}: {}", hostname, e);
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

        let block_store = Arc::new(BlockStore::new());
        let metrics = Metrics::new();
        let peer_id = "12D3KooWTest123".to_string();
        let botg = Arc::new(BoTgProtocol::new(BoTgConfig::default()));
        let app = create_router(block_store, metrics, peer_id, botg);

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

        let block_store = Arc::new(BlockStore::new());
        let metrics = Metrics::new();
        let botg = Arc::new(BoTgProtocol::new(BoTgConfig::default()));
        let app = create_router(block_store, metrics, "12D3KooWTest123".to_string(), botg);

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

        let block_store = Arc::new(BlockStore::new());
        let metrics = Metrics::new();
        let botg = Arc::new(BoTgProtocol::new(BoTgConfig::default()));
        let app = create_router(block_store, metrics, "12D3KooWTest123".to_string(), botg);

        let request = Request::builder()
            .uri("/api/v1/blocks/bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi")
            .body(Body::empty())
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
