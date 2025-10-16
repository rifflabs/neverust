# Archivist Manifest Protobuf Research Summary

## Overview

Complete reverse-engineering of the Archivist manifest protobuf format from the nim-codex (archivist-node) reference implementation.

**Research Date**: 2025-10-08
**Source Repository**: `/tmp/archivist-node` (nim-codex codebase)
**Key Files Analyzed**:
- `/tmp/archivist-node/archivist/manifest/coders.nim` (encoding/decoding logic)
- `/tmp/archivist-node/archivist/manifest/manifest.nim` (data structures)
- `/tmp/archivist-node/archivist/archivisttypes.nim` (multicodec definitions)
- `/tmp/archivist-node/tests/archivist/testmanifest.nim` (test cases)

## Key Findings

### 1. Three-Level Nested Protobuf Structure

The manifest uses a **3-level nested protobuf** wrapped in DAG-PB:

```
DagPB Node (wrapper)
  └─ Field 1: Header (protobuf message)
      ├─ Fields 1-6: Required core fields
      ├─ Field 7: ErasureInfo (optional, protobuf message)
      │   ├─ Fields 1-5: Erasure coding parameters
      │   └─ Field 6: VerificationInfo (optional, protobuf message)
      │       └─ Fields 1-4: Zero-knowledge proof metadata
      ├─ Field 8: filename (optional string)
      └─ Field 9: mimetype (optional string)
```

### 2. Multicodec: `codex-manifest` = `0xCD01` (52481)

Found in `/tmp/archivist-node/vendor/nim-libp2p/libp2p/multicodec.nim:434`:

```nim
("codex-manifest", 0xCD01),
```

Related codecs:
- `codex-block`: `0xCD02` (data blocks)
- `codex-root`: `0xCD03` (merkle tree root)
- `codex-slot-root`: `0xCD04` (erasure slot root)
- `poseidon2-alt_bn_128-sponge-r2`: `0xCD10` (ZK hash)

### 3. Complete Protobuf Schema

Extracted from `coders.nim` comments (lines 40-67):

```protobuf
syntax = "proto3";

message VerificationInfo {
  bytes verifyRoot = 1;
  repeated bytes slotRoots = 2;
  uint32 cellSize = 3;
  uint32 verifiableStrategy = 4;
}

message ErasureInfo {
  uint32 ecK = 1;
  uint32 ecM = 2;
  bytes originalTreeCid = 3;
  uint64 originalDatasetSize = 4;
  uint32 protectedStrategy = 5;
  VerificationInfo verification = 6;
}

message Header {
  bytes treeCid = 1;
  uint32 blockSize = 2;
  uint64 datasetSize = 3;
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

### 4. Field Mapping Complete

| Level | Field # | Name | Type | Required | Notes |
|-------|---------|------|------|----------|-------|
| **DagPB** | 1 | header | Header | Yes | Outer wrapper |
| **Header** | 1 | treeCid | bytes | Yes | Merkle tree root CID |
| | 2 | blockSize | uint32 | Yes | Typically 65536 (64 KiB) |
| | 3 | datasetSize | uint64 | Yes | Total size in bytes |
| | 4 | codec | uint32 | Yes | Data codec (0xCD02) |
| | 5 | hcodec | uint32 | Yes | Hash codec (0x12 or 0xCD10) |
| | 6 | version | uint32 | Yes | CID version (1) |
| | 7 | erasure | ErasureInfo | No | Present if erasure coded |
| | 8 | filename | string | No | Original filename |
| | 9 | mimetype | string | No | MIME type |
| **ErasureInfo** | 1 | ecK | uint32 | Yes* | Data block count |
| | 2 | ecM | uint32 | Yes* | Parity block count |
| | 3 | originalTreeCid | bytes | Yes* | Pre-erasure CID |
| | 4 | originalDatasetSize | uint64 | Yes* | Pre-erasure size |
| | 5 | protectedStrategy | uint32 | Yes* | 0=Linear, 1=Stepped |
| | 6 | verification | VerificationInfo | No | ZK proof metadata |
| **VerificationInfo** | 1 | verifyRoot | bytes | Yes* | Top-level proof CID |
| | 2 | slotRoots | repeated bytes | Yes* | Per-slot CIDs |
| | 3 | cellSize | uint32 | Yes* | Slot cell size (2048) |
| | 4 | verifiableStrategy | uint32 | Yes* | 0=Linear, 1=Stepped |

*Required if parent message is present

### 5. Encoding Logic (from `coders.nim`)

```nim
# Core encoding logic (lines 70-107)
var header = initProtoBuffer()
header.write(1, manifest.treeCid.data.buffer)
header.write(2, manifest.blockSize.uint32)
header.write(3, manifest.datasetSize.uint64)
header.write(4, manifest.codec.uint32)
header.write(5, manifest.hcodec.uint32)
header.write(6, manifest.version.uint32)

if manifest.protected:
  var erasureInfo = initProtoBuffer()
  erasureInfo.write(1, manifest.ecK.uint32)
  erasureInfo.write(2, manifest.ecM.uint32)
  erasureInfo.write(3, manifest.originalTreeCid.data.buffer)
  erasureInfo.write(4, manifest.originalDatasetSize.uint64)
  erasureInfo.write(5, manifest.protectedStrategy.uint32)

  if manifest.verifiable:
    var verificationInfo = initProtoBuffer()
    verificationInfo.write(1, manifest.verifyRoot.data.buffer)
    for slotRoot in manifest.slotRoots:
      verificationInfo.write(2, slotRoot.data.buffer)  # Repeated field
    verificationInfo.write(3, manifest.cellSize.uint32)
    verificationInfo.write(4, manifest.verifiableStrategy.uint32)
    erasureInfo.write(6, verificationInfo)

  erasureInfo.finish()
  header.write(7, erasureInfo)

if manifest.filename.isSome:
  header.write(8, manifest.filename.get())
if manifest.mimetype.isSome:
  header.write(9, manifest.mimetype.get())

pbNode.write(1, header)
pbNode.finish()
```

### 6. Decoding Logic (from `coders.nim`)

```nim
# Detection of optional nested messages (lines 167-188)
let protected = pbErasureInfo.buffer.len > 0
var verifiable = false

if protected:
  pbErasureInfo.getField(1, ecK)
  pbErasureInfo.getField(2, ecM)
  pbErasureInfo.getField(3, originalTreeCid)
  pbErasureInfo.getField(4, originalDatasetSize)
  pbErasureInfo.getField(5, protectedStrategy)
  pbErasureInfo.getField(6, pbVerificationInfo)

  verifiable = pbVerificationInfo.buffer.len > 0
  if verifiable:
    pbVerificationInfo.getField(1, verifyRoot)
    pbVerificationInfo.getRequiredRepeatedField(2, slotRoots)
    pbVerificationInfo.getField(3, cellSize)
    pbVerificationInfo.getField(4, verifiableStrategy)
```

**Key insight**: Empty buffer length (`buffer.len > 0`) is used to detect presence of nested messages.

### 7. Indexing Strategy Enum

From `indexingstrategy.nim:8-19`:

```nim
type StrategyType* = enum
  LinearStrategy   # 0 => blocks [0,1,2], 1 => blocks [3,4,5], ...
  SteppedStrategy  # 0 => blocks [0,3,6], 1 => blocks [1,4,7], ...
```

Values: `0 = Linear`, `1 = Stepped`

### 8. Test Cases (from `testmanifest.nim`)

```nim
# Simple manifest
let manifest = Manifest.new(
  treeCid = Cid.example,
  blockSize = 1.MiBs,
  datasetSize = 100.MiBs
)

# Protected manifest
let protectedManifest = Manifest.new(
  manifest = manifest,
  treeCid = Cid.example,
  datasetSize = 200.MiBs,
  ecK = 2,
  ecM = 2,
  strategy = SteppedStrategy
)

# Verifiable manifest
let verifiableManifest = Manifest.new(
  manifest = protectedManifest,
  verifyRoot = verifyCid,
  slotRoots = slotLeavesCids
)
```

### 9. Wire Format Tag Calculations

```
Protobuf tag = (field_number << 3) | wire_type

Examples:
Header field 1 (treeCid):     0x0A = (1 << 3) | 2 (length-delimited)
Header field 2 (blockSize):   0x10 = (2 << 3) | 0 (varint)
Header field 7 (erasure):     0x3A = (7 << 3) | 2 (length-delimited)
Header field 8 (filename):    0x42 = (8 << 3) | 2 (length-delimited)
```

### 10. Size Estimates

| Manifest Type | Approximate Wire Size |
|---------------|----------------------|
| Simple | ~100 bytes |
| Protected (no verification) | ~150 bytes |
| Verifiable (12 slots) | ~650 bytes |

Formula for verifiable manifest:
```
size ≈ 70 + 55 + (40 * num_slots) + 15
     ≈ 70 + 55 + (40 * (ecK + ecM)) + 15
```

## Implementation Recommendations for Neverust

### 1. Use Prost for Protobuf

```toml
[dependencies]
prost = "0.12"
prost-types = "0.12"

[build-dependencies]
prost-build = "0.12"
```

### 2. Protobuf Definition File

Create `protos/manifest.proto` with the schema above, then use `prost-build` in `build.rs`:

```rust
fn main() {
    prost_build::compile_protos(&["protos/manifest.proto"], &["protos/"])
        .unwrap();
}
```

### 3. Rust Type Mapping

```rust
#[derive(Clone, PartialEq, prost::Message)]
pub struct Header {
    #[prost(bytes = "vec", tag = "1")]
    pub tree_cid: Vec<u8>,
    #[prost(uint32, tag = "2")]
    pub block_size: u32,
    #[prost(uint64, tag = "3")]
    pub dataset_size: u64,
    #[prost(uint32, tag = "4")]
    pub codec: u32,
    #[prost(uint32, tag = "5")]
    pub hcodec: u32,
    #[prost(uint32, tag = "6")]
    pub version: u32,
    #[prost(message, optional, tag = "7")]
    pub erasure: Option<ErasureInfo>,
    #[prost(string, optional, tag = "8")]
    pub filename: Option<String>,
    #[prost(string, optional, tag = "9")]
    pub mimetype: Option<String>,
}
```

### 4. Validation Rules

```rust
impl Header {
    pub fn validate(&self) -> Result<(), ManifestError> {
        // 1. Required fields present
        if self.tree_cid.is_empty() {
            return Err(ManifestError::MissingTreeCid);
        }

        // 2. Block size divides dataset size
        if self.dataset_size % self.block_size as u64 != 0 {
            // Allow padding in last block
        }

        // 3. If protected: validate erasure
        if let Some(erasure) = &self.erasure {
            erasure.validate()?;

            // Check blocksCount == steps * (ecK + ecM)
            let blocks_count = (self.dataset_size + self.block_size as u64 - 1)
                / self.block_size as u64;
            let original_blocks = (erasure.original_dataset_size
                + self.block_size as u64 - 1) / self.block_size as u64;
            let steps = (original_blocks + erasure.ec_k as u64 - 1)
                / erasure.ec_k as u64;
            let expected = steps * (erasure.ec_k + erasure.ec_m) as u64;

            if blocks_count != expected {
                return Err(ManifestError::InvalidErasureBlockCount);
            }
        }

        // 4. If verifiable: validate verification
        if let Some(erasure) = &self.erasure {
            if let Some(verification) = &erasure.verification {
                verification.validate()?;

                // slot_roots.len() == ecK + ecM
                let expected_slots = erasure.ec_k + erasure.ec_m;
                if verification.slot_roots.len() != expected_slots as usize {
                    return Err(ManifestError::InvalidSlotRootCount);
                }
            }
        }

        Ok(())
    }
}
```

### 5. Encoding/Decoding

```rust
use prost::Message;

impl Header {
    pub fn encode(&self) -> Result<Vec<u8>, ManifestError> {
        self.validate()?;

        let mut buf = Vec::new();
        self.encode(&mut buf)
            .map_err(|e| ManifestError::EncodingError(e.to_string()))?;

        // Wrap in DagPB
        let mut dag_pb = Vec::new();
        dag_pb.push(0x0A); // Field 1, wire type 2
        prost::encoding::encode_varint(buf.len() as u64, &mut dag_pb);
        dag_pb.extend_from_slice(&buf);

        Ok(dag_pb)
    }

    pub fn decode(data: &[u8]) -> Result<Self, ManifestError> {
        // Unwrap DagPB
        if data[0] != 0x0A {
            return Err(ManifestError::InvalidDagPbWrapper);
        }

        let (len, offset) = decode_varint(&data[1..])?;
        let header_bytes = &data[offset..offset + len];

        // Decode Header
        let header = Self::decode(header_bytes)
            .map_err(|e| ManifestError::DecodingError(e.to_string()))?;

        header.validate()?;
        Ok(header)
    }
}
```

## Documentation Outputs

Created comprehensive documentation in `/opt/castle/workspace/neverust/docs/`:

1. **archivist-manifest-protobuf-format.md** - Complete protobuf specification
   - Full schema with all nested messages
   - Field-by-field documentation
   - Encoding/decoding patterns
   - Multicodec reference
   - Common patterns and error handling

2. **manifest-implementation-guide.md** - Quick implementation reference
   - Field mapping summary
   - Encoding/decoding flow diagrams
   - Rust code examples
   - Validation rules
   - Common pitfalls
   - Testing checklist

3. **manifest-wire-format-examples.md** - Wire-level encoding examples
   - Protobuf wire type reference
   - Field tag calculations
   - Hex dumps of encoded manifests
   - Size calculations
   - Debugging techniques

## Critical Implementation Notes

### 1. DagPB Wrapper is Required

The manifest is **NOT** just the Header protobuf. It must be wrapped:

```rust
// WRONG
let bytes = header.encode_to_vec();

// CORRECT
let mut header_bytes = header.encode_to_vec();
let mut dag_pb = vec![0x0A]; // Field 1, wire type 2
prost::encoding::encode_varint(header_bytes.len() as u64, &mut dag_pb);
dag_pb.extend(header_bytes);
```

### 2. Optional Field Detection

Use `.is_none()` or check buffer length, NOT default values:

```rust
// Detect if manifest is protected
let protected = header.erasure.is_some();

// Detect if manifest is verifiable
let verifiable = header.erasure
    .as_ref()
    .and_then(|e| e.verification.as_ref())
    .is_some();
```

### 3. CID Encoding

CIDs are stored as **raw bytes**, not base58/base32 strings:

```rust
// Convert CID to bytes
let cid_bytes = cid.to_bytes();  // Includes multibase prefix

// Parse CID from bytes
let cid = Cid::try_from(cid_bytes.as_slice())?;
```

### 4. Repeated Fields

`slot_roots` uses the same field tag multiple times:

```rust
// Encoding (prost does this automatically for Vec)
#[prost(bytes = "vec", repeated, tag = "2")]
pub slot_roots: Vec<Vec<u8>>,

// Wire format: 0x12 [len] [data] 0x12 [len] [data] ...
```

### 5. Strategy Type Values

MUST be exactly 0 or 1:

```rust
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum StrategyType {
    Linear = 0,
    Stepped = 1,
}

impl From<u32> for StrategyType {
    fn from(value: u32) -> Self {
        match value {
            0 => Self::Linear,
            1 => Self::Stepped,
            _ => panic!("Invalid strategy type: {}", value),
        }
    }
}
```

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_manifest_roundtrip() {
        let manifest = Header {
            tree_cid: vec![1, 2, 3],
            block_size: 65536,
            dataset_size: 104857600,
            codec: 0xCD02,
            hcodec: 0x12,
            version: 1,
            erasure: None,
            filename: Some("test.dat".to_string()),
            mimetype: None,
        };

        let encoded = manifest.encode().unwrap();
        let decoded = Header::decode(&encoded).unwrap();

        assert_eq!(manifest, decoded);
    }

    #[test]
    fn test_protected_manifest() {
        // Test with ErasureInfo but no VerificationInfo
    }

    #[test]
    fn test_verifiable_manifest() {
        // Test with full VerificationInfo
    }

    #[test]
    fn test_invalid_manifests() {
        // Test validation failures
    }
}
```

### Integration Tests

Test against real Archivist node:
1. Upload file to Archivist
2. Fetch manifest CID
3. Download manifest block
4. Decode and verify structure matches

## Next Steps for Neverust

1. **Create protobuf definitions** in `protos/manifest.proto`
2. **Set up build.rs** with prost-build
3. **Implement validation** logic
4. **Write comprehensive tests** (unit + integration)
5. **Integrate with blockexc** protocol for manifest exchange
6. **Add metrics** (encode/decode time, manifest size distribution)
7. **Test compatibility** with real Archivist nodes

## References

- **Source code**: `/tmp/archivist-node/archivist/manifest/`
- **Tests**: `/tmp/archivist-node/tests/archivist/testmanifest.nim`
- **Multicodec table**: `/tmp/archivist-node/vendor/nim-libp2p/libp2p/multicodec.nim`
- **Protobuf spec**: https://protobuf.dev/programming-guides/encoding/
- **CID spec**: https://github.com/multiformats/cid

## Conclusion

The Archivist manifest format is a **well-structured, nested protobuf** with clear separation of concerns:

- **Header**: Core metadata (always present)
- **ErasureInfo**: Erasure coding parameters (optional)
- **VerificationInfo**: Zero-knowledge proof metadata (optional)

Implementation in Rust using `prost` should be straightforward, with the main complexity being:
1. DagPB wrapper handling
2. Optional nested message detection
3. Validation of erasure coding constraints

All necessary information has been extracted and documented for full Archivist compatibility.
