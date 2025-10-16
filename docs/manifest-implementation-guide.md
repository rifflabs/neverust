# Archivist Manifest Implementation Guide for Neverust

Quick reference for implementing Archivist-compatible manifest encoding/decoding in Rust.

## Quick Facts

- **Format**: Protobuf3 nested inside DAG-PB
- **Multicodec**: `codex-manifest` = `0xCD01` (52481)
- **Structure**: 3-level nesting (DagPB → Header → ErasureInfo → VerificationInfo)
- **All integers**: Protobuf varint encoding
- **All CIDs**: Raw bytes (multibase-encoded CID)

## Field Mapping Summary

### Level 1: DagPB Node
```
Field 1: Header (message)
```

### Level 2: Header
```
Field 1: tree_cid (bytes)           - Required - Merkle tree root CID
Field 2: block_size (uint32)        - Required - Block size in bytes (typically 65536)
Field 3: dataset_size (uint64)      - Required - Total dataset size
Field 4: codec (uint32)             - Required - Data codec (0xCD02 = codex-block)
Field 5: hcodec (uint32)            - Required - Hash codec (0x12 = sha2-256)
Field 6: version (uint32)           - Required - CID version (1 = CIDv1)
Field 7: erasure (ErasureInfo)      - Optional - Erasure coding metadata
Field 8: filename (string)          - Optional - Original filename
Field 9: mimetype (string)          - Optional - MIME type
```

### Level 3: ErasureInfo (Field 7 of Header)
```
Field 1: ec_k (uint32)              - Required* - Number of data blocks
Field 2: ec_m (uint32)              - Required* - Number of parity blocks
Field 3: original_tree_cid (bytes)  - Required* - Pre-erasure tree CID
Field 4: original_dataset_size (u64)- Required* - Pre-erasure size
Field 5: protected_strategy (u32)   - Required* - 0=Linear, 1=Stepped
Field 6: verification (VerifInfo)   - Optional  - ZK proof metadata
```

### Level 4: VerificationInfo (Field 6 of ErasureInfo)
```
Field 1: verify_root (bytes)        - Required* - Top-level proof root CID
Field 2: slot_roots (repeated bytes)- Required* - Per-slot CIDs (ecK+ecM entries)
Field 3: cell_size (uint32)         - Required* - Slot cell size (default 2048)
Field 4: verifiable_strategy (u32)  - Required* - 0=Linear, 1=Stepped
```

*Required if parent message is present

## Encoding Flow

```
1. Create Header with required fields (1-6)
2. If erasure coded:
   a. Create ErasureInfo with fields 1-5
   b. If verifiable:
      - Create VerificationInfo with fields 1-4
      - Add to ErasureInfo field 6
   c. Add ErasureInfo to Header field 7
3. Add optional filename/mimetype (fields 8-9) if present
4. Encode Header as protobuf
5. Wrap in DagPB node at field 1
6. Encode DagPB
```

## Decoding Flow

```
1. Decode DagPB node
2. Extract Header from field 1
3. Extract required fields 1-6
4. Check if field 7 (ErasureInfo) exists:
   - If empty buffer: simple manifest (no erasure)
   - If present: decode fields 1-5
5. Check if field 6 of ErasureInfo exists:
   - If empty buffer: protected but not verifiable
   - If present: decode fields 1-4
6. Extract optional filename/mimetype (fields 8-9)
```

## Common Multicodec Values

```rust
// Hash codecs
const SHA2_256: u32 = 0x12;          // Standard SHA-256
const POSEIDON2_SPONGE: u32 = 0xCD10;  // Poseidon2 for ZK
const POSEIDON2_MERKLE: u32 = 0xCD11;  // Poseidon2 merkle

// Data codecs
const CODEX_MANIFEST: u32 = 0xCD01;
const CODEX_BLOCK: u32 = 0xCD02;
const CODEX_ROOT: u32 = 0xCD03;
const CODEX_SLOT_ROOT: u32 = 0xCD04;

// CID version
const CIDV1: u32 = 1;
```

## Strategy Types

```rust
pub enum StrategyType {
    Linear = 0,    // Sequential: slot0=[0,1,2], slot1=[3,4,5]
    Stepped = 1,   // Interleaved: slot0=[0,3,6], slot1=[1,4,7]
}
```

## Example: Simple Manifest

```rust
use prost::Message;

let manifest = Header {
    tree_cid: cid.to_bytes(),
    block_size: 65536,
    dataset_size: 104857600,
    codec: 0xCD02,
    hcodec: 0x12,
    version: 1,
    erasure: None,
    filename: Some("example.dat".to_string()),
    mimetype: Some("application/octet-stream".to_string()),
};

let mut buf = Vec::new();
manifest.encode(&mut buf)?;

// Wrap in DagPB
let dag_pb = DagPbNode { header: buf };
let final_bytes = dag_pb.encode_to_vec();
```

## Example: Protected Manifest

```rust
let erasure = ErasureInfo {
    ec_k: 10,
    ec_m: 2,
    original_tree_cid: original_cid.to_bytes(),
    original_dataset_size: 104857600,
    protected_strategy: 1,  // Stepped
    verification: None,
};

let manifest = Header {
    tree_cid: erasure_cid.to_bytes(),
    block_size: 65536,
    dataset_size: 125829120,  // Larger after erasure
    codec: 0xCD02,
    hcodec: 0x12,
    version: 1,
    erasure: Some(erasure),
    filename: None,
    mimetype: None,
};
```

## Example: Verifiable Manifest

```rust
let verification = VerificationInfo {
    verify_root: verify_cid.to_bytes(),
    slot_roots: slot_cids.iter().map(|c| c.to_bytes()).collect(),
    cell_size: 2048,
    verifiable_strategy: 0,  // Linear
};

let erasure = ErasureInfo {
    ec_k: 10,
    ec_m: 2,
    original_tree_cid: original_cid.to_bytes(),
    original_dataset_size: 104857600,
    protected_strategy: 1,
    verification: Some(verification),
};

let manifest = Header {
    tree_cid: erasure_cid.to_bytes(),
    block_size: 65536,
    dataset_size: 125829120,
    codec: 0xCD02,
    hcodec: 0xCD10,  // Poseidon2 for ZK
    version: 1,
    erasure: Some(erasure),
    filename: None,
    mimetype: None,
};
```

## Validation Rules

1. **blockSize must divide datasetSize evenly** (or pad last block)
2. **If protected**: `blocksCount == steps * (ecK + ecM)` where `steps = ceil(originalBlocksCount / ecK)`
3. **If verifiable**: `slot_roots.len() == ecK + ecM`
4. **CID versions must match** between tree_cid and version field
5. **All CIDs must be valid** (parse without error)
6. **Strategies**: Must be 0 or 1

## Testing Checklist

- [ ] Simple manifest (no erasure)
- [ ] Simple manifest with filename/mimetype
- [ ] Protected manifest (erasure but no verification)
- [ ] Verifiable manifest (full ZK support)
- [ ] Large datasets (>5 GiB)
- [ ] Edge cases (empty optional fields)
- [ ] Round-trip encode/decode equality
- [ ] Invalid manifests (validation errors)

## Rust Dependencies

```toml
[dependencies]
prost = "0.12"
cid = "0.11"
unsigned-varint = "0.8"
bytes = "1.5"

[build-dependencies]
prost-build = "0.12"
```

## Protobuf Definition File

Create `protos/manifest.proto`:

```protobuf
syntax = "proto3";
package archivist.manifest;

message VerificationInfo {
  bytes verify_root = 1;
  repeated bytes slot_roots = 2;
  uint32 cell_size = 3;
  uint32 verifiable_strategy = 4;
}

message ErasureInfo {
  uint32 ec_k = 1;
  uint32 ec_m = 2;
  bytes original_tree_cid = 3;
  uint64 original_dataset_size = 4;
  uint32 protected_strategy = 5;
  VerificationInfo verification = 6;
}

message Header {
  bytes tree_cid = 1;
  uint32 block_size = 2;
  uint64 dataset_size = 3;
  uint32 codec = 4;
  uint32 hcodec = 5;
  uint32 version = 6;
  ErasureInfo erasure = 7;
  string filename = 8;
  string mimetype = 9;
}

message DagPbNode {
  Header header = 1;
}
```

Build script `build.rs`:

```rust
fn main() {
    prost_build::compile_protos(&["protos/manifest.proto"], &["protos/"])
        .unwrap();
}
```

## Compatibility Notes

1. **Field 7 absence**: Check `erasure.is_none()` OR buffer length == 0
2. **Repeated fields**: `slot_roots` uses same field tag multiple times
3. **String encoding**: UTF-8 (protobuf standard)
4. **CID bytes**: Include full multibase encoding (typically starts with 0x01 for CIDv1)
5. **Varint overflow**: Use u64 for large sizes, validate against platform limits

## Performance Tips

1. **Pre-allocate buffers**: Estimate size = 100 + (num_slots * 40) bytes
2. **Reuse CID parsing**: Cache parsed CIDs to avoid repeated parsing
3. **Lazy verification**: Don't validate erasure/verification fields unless needed
4. **Streaming decode**: For large manifests, decode Header first, then conditionally decode erasure
5. **Zero-copy**: Use `bytes::Bytes` to avoid allocations

## Common Pitfalls

1. **Forgetting to wrap in DagPB**: Header alone is NOT a valid manifest
2. **Wrong field numbers**: Field 6 of ErasureInfo is verification, NOT field 7
3. **Empty vs None**: Empty buffer ≠ absent field in some protobuf implementations
4. **CID format**: Store as bytes, not base58/base32 string
5. **Strategy validation**: Must be exactly 0 or 1, not arbitrary uint32

## Next Steps for Neverust

1. Generate Rust types from `manifest.proto` using prost
2. Implement `encode()` and `decode()` methods
3. Add validation logic (manifest.verify())
4. Write comprehensive tests (see Testing Checklist)
5. Integrate with blockexc protocol for manifest exchange
6. Add metrics (encode/decode time, manifest size)

## See Also

- [Complete Protobuf Format Documentation](./archivist-manifest-protobuf-format.md)
- [Archivist Node Reference Implementation](https://github.com/codex-storage/nim-codex)
