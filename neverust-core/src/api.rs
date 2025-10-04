//! REST API for block operations and node management

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::trace::TraceLayer;
use tracing::{error, info};

use crate::storage::{Block, BlockStore, StorageError};

/// API state shared across handlers
#[derive(Clone)]
pub struct ApiState {
    pub block_store: Arc<BlockStore>,
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
pub fn create_router(block_store: Arc<BlockStore>) -> Router {
    let state = ApiState { block_store };

    Router::new()
        .route("/health", get(health_check))
        .route("/metrics", get(metrics))
        .route("/api/v1/blocks", post(store_block))
        .route("/api/v1/blocks/:cid", get(get_block))
        // Archivist-compatible endpoints
        .route("/api/archivist/v1/data", post(archivist_upload))
        .route("/api/archivist/v1/data/:cid/network/stream", get(archivist_download))
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
async fn metrics(State(state): State<ApiState>) -> impl IntoResponse {
    let stats = state.block_store.stats().await;

    // Generate Prometheus-compatible metrics in text format
    let metrics = format!(
        "# HELP neverust_block_count Total number of blocks stored\n\
         # TYPE neverust_block_count gauge\n\
         neverust_block_count {}\n\
         \n\
         # HELP neverust_block_bytes Total bytes of block data stored\n\
         # TYPE neverust_block_bytes gauge\n\
         neverust_block_bytes {}\n\
         \n\
         # HELP neverust_uptime_seconds Time since node started in seconds\n\
         # TYPE neverust_uptime_seconds counter\n\
         neverust_uptime_seconds {}\n",
        stats.block_count,
        stats.total_size,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );

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
async fn get_block(
    State(state): State<ApiState>,
    Path(cid_str): Path<String>,
) -> Result<Json<GetBlockResponse>, ApiError> {
    info!("API: Retrieving block {}", cid_str);

    // Parse CID
    let cid = cid_str
        .parse()
        .map_err(|e| ApiError::BadRequest(format!("Invalid CID: {}", e)))?;

    // Get block from store
    let block = state
        .block_store
        .get(&cid)
        .await
        .map_err(|e| match e {
            StorageError::BlockNotFound(_) => ApiError::NotFound(cid_str.clone()),
            _ => ApiError::Internal(format!("Failed to retrieve block: {}", e)),
        })?;

    info!("API: Retrieved block {} ({} bytes)", cid_str, block.size());

    Ok(Json(GetBlockResponse {
        cid: cid_str,
        data: base64::prelude::BASE64_STANDARD.encode(&block.data),
        size: block.size(),
    }))
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

    // Get block from store
    let block = state
        .block_store
        .get(&cid)
        .await
        .map_err(|e| match e {
            StorageError::BlockNotFound(_) => ApiError::NotFound(cid_str.clone()),
            _ => ApiError::Internal(format!("Failed to retrieve block: {}", e)),
        })?;

    info!("Archivist API: Downloaded {} ({} bytes)", cid_str, block.size());

    // Return raw bytes (Archivist format)
    Ok(block.data)
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
            ApiError::NotFound(cid) => (
                StatusCode::NOT_FOUND,
                format!("Block not found: {}", cid),
            ),
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
        let block_store = Arc::new(BlockStore::new());
        let app = create_router(block_store);

        let request = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_store_and_get_block() {
        let block_store = Arc::new(BlockStore::new());
        let app = create_router(block_store);

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
        let decoded_data = base64::prelude::BASE64_STANDARD.decode(&get_block_response.data).unwrap();
        assert_eq!(decoded_data, test_data);
    }

    #[tokio::test]
    async fn test_get_nonexistent_block() {
        let block_store = Arc::new(BlockStore::new());
        let app = create_router(block_store);

        let request = Request::builder()
            .uri("/api/v1/blocks/bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi")
            .body(Body::empty())
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
