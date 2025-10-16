# Archivist Manifest Implementation

## Overview

This document describes the implementation of Archivist Manifest structures in neverust, including protobuf encoding/decoding and block creation with codec `0xcd01`.

## File Structure

- **Source**: `/opt/castle/workspace/neverust/neverust-core/src/manifest.rs`
- **Tests**: Comprehensive test suite included in the same file (11 tests)
- **Example**: `/opt/castle/workspace/neverust/neverust-core/examples/manifest_demo.rs`

## Key Components

### 1. Manifest Struct

```rust
pub struct Manifest {
    pub tree_cid: Cid,           // Root CID of the merkle tree
    pub block_size: u64,         // Size of each contained block
    pub dataset_size: u64,       // Total size of all blocks
    pub codec: u64,              // Dataset codec (default: 0xcd02)
    pub hcodec: u64,             // Multihash codec (default: 0x12 SHA-256)
    pub version: u32,            // CID version (default: 1)
    pub filename: Option<String>, // Original filename (optional)
    pub mimetype: Option<String>, // MIME type (optional)
    pub erasure: Option<ErasureInfo>, // Erasure coding info (if protected)
}
```

### 2. Erasure Coding Support

```rust
pub struct ErasureInfo {
    pub ec_k: u32,                          // Number of blocks to encode
    pub ec_m: u32,                          // Number of parity blocks
    pub original_tree_cid: Cid,             // Original root before EC
    pub original_dataset_size: u64,         // Original size before EC
    pub protected_strategy: StrategyType,   // Indexing strategy
    pub verification: Option<VerificationInfo>, // Verification info (if verifiable)
}
```

### 3. Verification Support

```rust
pub struct VerificationInfo {
    pub verify_root: Cid,                   // Verification tree root
    pub slot_roots: Vec<Cid>,               // Individual slot roots
    pub cell_size: u64,                     // Size of each slot cell
    pub verifiable_strategy: StrategyType,  // Indexing strategy
}
```

## Protobuf Schema

The manifest uses the following protobuf structure (compatible with Archivist):

```protobuf
message DagPbNode {
  bytes data = 1;  // Contains the Header message
}

message Header {
  bytes treeCid = 1;            // Tree root CID
  uint32 blockSize = 2;         // Block size
  uint64 datasetSize = 3;       // Dataset size
  uint32 codec = 4;             // Dataset codec
  uint32 hcodec = 5;            // Multihash codec
  uint32 version = 6;           // CID version
  ErasureInfo erasure = 7;      // Erasure info (optional)
  string filename = 8;          // Filename (optional)
  string mimetype = 9;          // MIME type (optional)
}

message ErasureInfo {
  uint32 ecK = 1;                          // Number of encoded blocks
  uint32 ecM = 2;                          // Number of parity blocks
  bytes originalTreeCid = 3;               // Original tree CID
  uint64 originalDatasetSize = 4;          // Original dataset size
  uint32 protectedStrategy = 5;            // Protected strategy
  VerificationInfo verification = 6;       // Verification info (optional)
}

message VerificationInfo {
  bytes verifyRoot = 1;                    // Verify root CID
  repeated bytes slotRoots = 2;            // Slot root CIDs
  uint32 cellSize = 3;                     // Cell size
  uint32 verifiableStrategy = 4;           // Verifiable strategy
}
```

## Codec Values

```rust
pub const MANIFEST_CODEC: u64 = 0xcd01;  // codex-manifest
pub const BLOCK_CODEC: u64 = 0xcd02;     // codex-block
pub const SHA256_CODEC: u64 = 0x12;      // sha2-256
pub const BLAKE3_CODEC: u64 = 0x1e;      // blake3
```

## Key Methods

### Creating a Manifest

```rust
// Simple unprotected manifest
let manifest = Manifest::new(
    tree_cid,
    65536,              // 64KB blocks
    1024 * 1024,        // 1MB dataset
    None,               // Use default codec
    None,               // Use default hash codec
    None,               // Use default version
    Some("file.bin".to_string()),
    Some("application/octet-stream".to_string()),
);

// Protected manifest with erasure coding
let manifest = Manifest::new_protected(
    tree_cid,
    block_size,
    dataset_size,
    BLOCK_CODEC,
    SHA256_CODEC,
    1,
    10,  // ec_k
    3,   // ec_m
    original_tree_cid,
    original_dataset_size,
    StrategyType::SteppedStrategy,
    Some("file.bin".to_string()),
    None,
);
```

### Encoding and Decoding

```rust
// Encode to protobuf bytes
let encoded: Vec<u8> = manifest.encode()?;

// Decode from protobuf bytes
let decoded: Manifest = Manifest::decode(&encoded)?;
```

### Block Creation

```rust
// Convert manifest to block (with codec 0xcd01)
let block: Block = manifest.to_block()?;

// Verify codec
assert_eq!(block.cid.codec(), MANIFEST_CODEC);

// Recover manifest from block
let recovered: Manifest = Manifest::from_block(&block)?;
```

## CID Structure

The manifest block CID is constructed as follows:

```
CID = <version><codec><multihash>

version:   1 (CIDv1)
codec:     0xcd01 (ManifestCodec)
multihash: <hash_code><hash_length><hash_bytes>
           - hash_code: 0x1e (BLAKE3)
           - hash_length: 32
           - hash_bytes: BLAKE3 hash of protobuf-encoded manifest
```

Example CID: `bagazuay6ea7s2z63tpuyiuntpeuunjidscztm6kf4paxn5c4xnjzsfqydwju6`

## Test Coverage

All 11 tests pass:

1. ✅ `test_manifest_creation` - Basic manifest creation
2. ✅ `test_manifest_encode_decode_roundtrip` - Full encode/decode cycle
3. ✅ `test_manifest_encode_decode_minimal` - Minimal manifest (no optional fields)
4. ✅ `test_manifest_to_block` - Block creation and recovery
5. ✅ `test_manifest_cid_computation` - CID determinism
6. ✅ `test_manifest_blocks_count` - Block count calculation
7. ✅ `test_manifest_protected` - Protected manifest with erasure coding
8. ✅ `test_manifest_protected_encode_decode` - Protected manifest roundtrip
9. ✅ `test_manifest_verifiable` - Verifiable manifest with slot roots
10. ✅ `test_manifest_from_block_wrong_codec` - Error handling
11. ✅ `test_strategy_type_conversion` - Strategy type enum conversion

## Compatibility

The implementation is fully compatible with Archivist's manifest format:

- **Protobuf Structure**: Matches Archivist's nested protobuf message structure
- **Codec Values**: Uses correct multicodec values (0xcd01 for manifest)
- **CID Format**: Follows Archivist's CID construction
- **Optional Fields**: Properly handles optional filename, mimetype, erasure, and verification fields
- **Erasure Coding**: Supports protected manifests with ec_k/ec_m parameters
- **Verification**: Supports verifiable manifests with slot roots

## Running Tests

```bash
# Run manifest tests only
cargo test --package neverust-core --lib manifest

# Run all tests
cargo test --package neverust-core

# Run the demo
cargo run --package neverust-core --example manifest_demo
```

## Example Output

```
=== Archivist Manifest Demo ===

1. Creating a simple unprotected manifest...
   Tree CID: bagbjuay6eaubnldzlxkwc63w47efvqpep2ztskf2uftje7t4mtkx45c4vpgu4
   Block size: 65536 bytes
   Dataset size: 10485760 bytes
   Number of blocks: 160
   Codec: 0xcd02
   Hash codec: 0x12
   Filename: Some("example.bin")
   MIME type: Some("application/octet-stream")
   Protected: false

2. Encoding manifest to protobuf...
   Encoded size: 98 bytes
   First 32 bytes (hex): 0a600a2601829a031e202816ac795dd5617b76e7c85ac1e47eb33928baa16692

3. Decoding manifest from protobuf...
   Tree CID matches: true
   Block size matches: true
   Dataset size matches: true
   Filename matches: true

4. Creating a block from the manifest...
   Block CID: bagazuay6ea7s2z63tpuyiuntpeuunjidscztm6kf4paxn5c4xnjzsfqydwju6
   Block codec: 0xcd01 (should be 0xcd01)
   Block data size: 98 bytes

5. Recovering manifest from block...
   Tree CID matches: true
   All fields match: true

=== Demo Complete ===
```

## Integration Points

The manifest module integrates with:

- **Block Store**: Manifests can be stored as blocks in the block store
- **BlockExc Protocol**: Manifests can be exchanged between peers
- **API Layer**: Manifests can be created and queried via HTTP API
- **Merkle Tree**: Tree CIDs reference Archivist merkle trees
- **Chunker**: Block size determines chunking strategy

## Future Enhancements

Potential future additions:

1. ✅ **Manifest Validation**: Verify consistency between fields
2. ⏳ **Manifest Caching**: Cache decoded manifests for performance
3. ⏳ **Manifest Indexing**: Index manifests by filename/mimetype
4. ⏳ **Manifest Search**: Search manifests by metadata
5. ⏳ **Manifest Visualization**: Display manifest structure
6. ⏳ **Manifest Diff**: Compare two manifests
7. ⏳ **Manifest Migration**: Convert between versions

## References

- **Archivist Reference**: `/tmp/archivist-node/archivist/manifest/`
- **Protobuf Encoding**: `/tmp/archivist-node/archivist/manifest/coders.nim`
- **Multicodec Table**: `/tmp/archivist-node/vendor/nim-libp2p/libp2p/multicodec.nim`
- **CID Spec**: https://github.com/multiformats/cid

## Summary

The Archivist Manifest implementation in neverust is:

- ✅ **Complete**: All fields from Archivist manifest.nim implemented
- ✅ **Tested**: 11 comprehensive tests covering all functionality
- ✅ **Compatible**: Protobuf encoding matches Archivist exactly
- ✅ **Documented**: Clear examples and API documentation
- ✅ **Integrated**: Exported from neverust-core for use throughout the project
- ✅ **Performant**: Zero-copy protobuf encoding where possible

The implementation successfully uses codec `0xcd01` for manifest blocks and provides full support for unprotected, protected, and verifiable manifests.
