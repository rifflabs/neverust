//! Integration tests for manifest upload/download functionality
//!
//! These tests verify end-to-end manifest operations including:
//! - Data upload and manifest creation
//! - Block storage and retrieval
//! - Manifest encoding/decoding
//! - Data reconstruction from blocks

use neverust_core::{BlockStore, Chunker, Cid, Manifest, DEFAULT_BLOCK_SIZE};
use std::sync::Arc;

/// Test basic manifest creation and data reconstruction
#[tokio::test]
async fn test_manifest_upload_download_roundtrip() {
    // Create temporary storage (BlockStore::new() creates its own temp dir)
    let store = Arc::new(BlockStore::new());

    // Test data
    let test_data = b"Hello from Neverust manifest integration test!";

    // Step 1: Chunk the data and store blocks
    let mut chunker = Chunker::with_chunk_size(&test_data[..], DEFAULT_BLOCK_SIZE);
    let mut block_cids = Vec::new();

    while let Some(chunk) = chunker.next_chunk().await.expect("Chunking failed") {
        let cid = store.put_data(chunk).await.expect("Failed to store block");
        block_cids.push(cid);
    }

    assert!(
        !block_cids.is_empty(),
        "Should have created at least one block"
    );

    // Step 2: Verify all blocks are stored and retrievable
    for cid in &block_cids {
        let retrieved = store.get(cid).await.expect("Failed to retrieve block");
        assert_eq!(retrieved.cid, *cid, "CID mismatch");
    }

    // Step 3: Reconstruct data from blocks
    let mut reconstructed_data = Vec::new();
    for cid in &block_cids {
        let block = store.get(cid).await.expect("Failed to get block");
        reconstructed_data.extend_from_slice(&block.data);
    }

    assert_eq!(
        &reconstructed_data[..],
        test_data,
        "Reconstructed data should match original"
    );
}

/// Test manifest creation with metadata
#[tokio::test]
async fn test_manifest_with_metadata() {
    // Create temporary storage
    let store = Arc::new(BlockStore::new());

    // Test data
    let test_data = b"Test data for manifest with metadata";

    // Chunk and store
    let mut chunker = Chunker::with_chunk_size(&test_data[..], DEFAULT_BLOCK_SIZE);
    let mut block_cids = Vec::new();

    while let Some(chunk) = chunker.next_chunk().await.expect("Chunking failed") {
        let cid = store.put_data(chunk).await.expect("Failed to store block");
        block_cids.push(cid);
    }

    // Create manifest with metadata
    let tree_cid = block_cids[0]; // Use first block as tree root for testing
    let manifest = Manifest::new(
        tree_cid,
        DEFAULT_BLOCK_SIZE as u64,
        test_data.len() as u64,
        Some(0xcd02),                   // codex-block codec
        Some(0x12),                     // sha2-256 codec
        Some(1),                        // version
        None,                           // filename
        Some(mime::TEXT_PLAIN), // mimetype
    );

    assert_eq!(manifest.blocks_count(), 1);
    assert_eq!(manifest.dataset_size, test_data.len() as u64);
    assert_eq!(manifest.block_size, DEFAULT_BLOCK_SIZE as u64);
}

/// Test large data chunking and storage
#[tokio::test]
async fn test_large_data_manifest() {
    // Create temporary storage
    let store = Arc::new(BlockStore::new());

    // Create larger test data (multiple blocks)
    let test_data = vec![0u8; DEFAULT_BLOCK_SIZE * 3 + 1000]; // 3+ blocks

    // Chunk and store
    let mut chunker = Chunker::new(&test_data[..]);
    let mut block_cids = Vec::new();
    let mut total_stored = 0;

    while let Some(chunk) = chunker.next_chunk().await.expect("Chunking failed") {
        let chunk_len = chunk.len();
        let cid = store.put_data(chunk).await.expect("Failed to store block");
        block_cids.push(cid);
        total_stored += chunk_len;
    }

    assert_eq!(block_cids.len(), 4, "Should create 4 blocks");
    assert_eq!(total_stored, test_data.len(), "All data should be stored");

    // Verify all blocks retrievable
    for cid in &block_cids {
        let block = store.get(cid).await;
        assert!(block.is_ok(), "Block should be retrievable: {}", cid);
    }

    // Verify data reconstruction
    let mut reconstructed = Vec::new();
    for cid in &block_cids {
        let block = store.get(cid).await.expect("Failed to get block");
        reconstructed.extend_from_slice(&block.data);
    }

    assert_eq!(
        reconstructed, test_data,
        "Data should match after reconstruction"
    );
}

/// Test manifest encoding and decoding
#[tokio::test]
async fn test_manifest_encoding_decoding() {
    let tree_cid: Cid = "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi"
        .parse()
        .expect("Failed to parse CID");

    let original = Manifest::new(
        tree_cid,
        65536,
        1000000,
        Some(0xcd02),
        Some(0x12),
        Some(1),
        None,                                         // filename
        Some(mime::APPLICATION_OCTET_STREAM), // mimetype
    );

    // Encode
    let encoded = original.encode().expect("Failed to encode manifest");

    // Decode
    let decoded = Manifest::decode(&encoded).expect("Failed to decode manifest");

    // Verify
    assert_eq!(decoded.tree_cid, original.tree_cid);
    assert_eq!(decoded.block_size, original.block_size);
    assert_eq!(decoded.dataset_size, original.dataset_size);
    assert_eq!(decoded.blocks_count(), original.blocks_count());
}

/// Test empty data handling
#[tokio::test]
async fn test_empty_data_manifest() {
    let test_data = b"";
    let mut chunker = Chunker::new(&test_data[..]);

    let mut block_count = 0;
    while let Some(_chunk) = chunker.next_chunk().await.expect("Chunking failed") {
        block_count += 1;
    }

    assert_eq!(block_count, 0, "Empty data should produce no blocks");
}
