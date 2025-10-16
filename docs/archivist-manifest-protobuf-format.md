# Archivist Manifest Protobuf Format

Complete documentation of the Archivist manifest protobuf encoding format based on analysis of archivist-node codebase.

## Overview

Archivist manifests are encoded using Protocol Buffers (protobuf3) and stored with the multicodec `codex-manifest` (0xCD01). The manifest describes dataset metadata including:

- Tree structure (Merkle tree CID)
- Block and dataset sizes
- Codec information (hashing and data codecs)
- Optional erasure coding information
- Optional verification/proving information
- Optional metadata (filename, mimetype)

## Multicodec Information

- **Name**: `codex-manifest`
- **Code**: `0xCD01` (52481 decimal)
- **Related Codecs**:
  - `codex-block`: `0xCD02`
  - `codex-root`: `0xCD03`
  - `codex-slot-root`: `0xCD04`
  - `codex-proving-root`: `0xCD05`
  - `codex-slot-cell`: `0xCD06`

## Protobuf Schema

The manifest is encoded as a DAG-PB node where the `Data` field contains a nested protobuf `Header` message.

### Complete Schema

```protobuf
syntax = "proto3";

// Nested inside DAG-PB Data field
message VerificationInfo {
  bytes verifyRoot = 1;             // Decimal encoded field-element (CID bytes)
  repeated bytes slotRoots = 2;     // Decimal encoded field-elements (CID bytes, repeated)
  uint32 cellSize = 3;              // Size of each slot cell
  uint32 verifiableStrategy = 4;    // Indexing strategy enum (0=Linear, 1=Stepped)
}

message ErasureInfo {
  uint32 ecK = 1;                            // Number of encoded blocks
  uint32 ecM = 2;                            // Number of parity blocks
  bytes originalTreeCid = 3;                 // CID of the original dataset
  uint64 originalDatasetSize = 4;            // Size of the original dataset
  uint32 protectedStrategy = 5;              // Indexing strategy enum (0=Linear, 1=Stepped)
  VerificationInfo verification = 6;         // Verification information (optional)
}

message Header {
  bytes treeCid = 1;                // CID (root) of the tree
  uint32 blockSize = 2;             // Size of a single block (in bytes)
  uint64 datasetSize = 3;           // Size of the dataset (in bytes)
  uint32 codec = 4;                 // Dataset multicodec (e.g., 0xCD02 for codex-block)
  uint32 hcodec = 5;                // Multihash codec (e.g., 0x12 for sha2-256)
  uint32 version = 6;               // CID version (1 for CIDv1)
  ErasureInfo erasure = 7;          // Erasure coding info (optional)
  string filename = 8;              // Original filename (optional)
  string mimetype = 9;              // Original mimetype (optional)
}

// Top-level DAG-PB wrapper
message DagPBNode {
  Header header = 1;  // The manifest header is stored in field 1
}
```

## Field Details

### Header Message (Field 1 of DAG-PB)

| Field # | Name | Type | Required | Description |
|---------|------|------|----------|-------------|
| 1 | `treeCid` | bytes | Yes | Raw CID bytes (multibase encoded) of the merkle tree root |
| 2 | `blockSize` | uint32 | Yes | Size of each block in the dataset (typically 65536 = 64 KiB) |
| 3 | `datasetSize` | uint64 | Yes | Total size of the dataset in bytes |
| 4 | `codec` | uint32 | Yes | Multicodec for dataset blocks (0xCD02 = codex-block) |
| 5 | `hcodec` | uint32 | Yes | Multicodec for hash function (0x12 = sha2-256, 0xCD10 = poseidon2) |
| 6 | `version` | uint32 | Yes | CID version (1 = CIDv1) |
| 7 | `erasure` | ErasureInfo | No | Present if dataset is erasure coded |
| 8 | `filename` | string | No | Original filename of uploaded content |
| 9 | `mimetype` | string | No | MIME type of uploaded content |

### ErasureInfo Message (Field 7 of Header)

| Field # | Name | Type | Required | Description |
|---------|------|------|----------|-------------|
| 1 | `ecK` | uint32 | Yes* | Number of data blocks in erasure coding (e.g., 10) |
| 2 | `ecM` | uint32 | Yes* | Number of parity blocks (e.g., 2 for 10+2 Reed-Solomon) |
| 3 | `originalTreeCid` | bytes | Yes* | CID of the original (pre-erasure) merkle tree |
| 4 | `originalDatasetSize` | uint64 | Yes* | Size of original dataset before erasure coding |
| 5 | `protectedStrategy` | uint32 | Yes* | Indexing strategy: 0=LinearStrategy, 1=SteppedStrategy |
| 6 | `verification` | VerificationInfo | No | Present if dataset is verifiable (supports storage proofs) |

*Required if ErasureInfo is present

### VerificationInfo Message (Field 6 of ErasureInfo)

| Field # | Name | Type | Required | Description |
|---------|------|------|----------|-------------|
| 1 | `verifyRoot` | bytes | Yes* | CID of the top-level merkle tree built from slot roots |
| 2 | `slotRoots` | repeated bytes | Yes* | CIDs of individual slot roots (length = ecK + ecM) |
| 3 | `cellSize` | uint32 | Yes* | Size of each slot cell (default: 2048 bytes) |
| 4 | `verifiableStrategy` | uint32 | Yes* | Indexing strategy: 0=LinearStrategy, 1=SteppedStrategy |

*Required if VerificationInfo is present

## Encoding Details

### CID Encoding
- CIDs are stored as raw bytes (the full multibase-encoded CID)
- Use `Cid::try_from(bytes)` or equivalent to parse
- Example: A CID like `bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi` is stored as its raw byte representation

### Indexing Strategy Enum
```rust
enum StrategyType {
    LinearStrategy = 0,    // 0 => blocks [0,1,2], 1 => blocks [3,4,5], ...
    SteppedStrategy = 1,   // 0 => blocks [0,3,6], 1 => blocks [1,4,7], ...
}
```

### Multicodec Values
Common codecs used in Archivist:

| Name | Code (hex) | Code (dec) | Usage |
|------|------------|------------|-------|
| `sha2-256` | 0x12 | 18 | Standard SHA-256 hashing |
| `poseidon2-alt_bn_128-sponge-r2` | 0xCD10 | 52496 | Poseidon2 sponge (for ZK) |
| `poseidon2-alt_bn_128-merkle-2kb` | 0xCD11 | 52497 | Poseidon2 merkle |
| `codex-manifest` | 0xCD01 | 52481 | Manifest metadata |
| `codex-block` | 0xCD02 | 52482 | Dataset blocks |
| `codex-root` | 0xCD03 | 52483 | Dataset merkle root |

### Default Values
```rust
const DEFAULT_BLOCK_SIZE: u32 = 65536;  // 64 KiB
const DEFAULT_CELL_SIZE: u32 = 2048;    // 2 KiB
const DEFAULT_CID_VERSION: u32 = 1;     // CIDv1
```

## Manifest Types

### 1. Simple Manifest (No Erasure Coding)

Minimal manifest for unprotected datasets:

```
Header {
  treeCid: <CID bytes>
  blockSize: 65536
  datasetSize: 104857600  // 100 MiB
  codec: 0xCD02
  hcodec: 0x12
  version: 1
  // No erasure field
  // Optional filename/mimetype
}
```

### 2. Protected Manifest (With Erasure Coding)

Manifest with Reed-Solomon erasure coding:

```
Header {
  treeCid: <CID bytes>
  blockSize: 65536
  datasetSize: 209715200  // 200 MiB (after erasure)
  codec: 0xCD02
  hcodec: 0x12
  version: 1
  erasure: {
    ecK: 2
    ecM: 2
    originalTreeCid: <CID bytes>
    originalDatasetSize: 104857600  // 100 MiB (before erasure)
    protectedStrategy: 1  // SteppedStrategy
    // No verification field
  }
}
```

### 3. Verifiable Manifest (With Storage Proofs)

Manifest supporting zero-knowledge storage proofs:

```
Header {
  treeCid: <CID bytes>
  blockSize: 65536
  datasetSize: 209715200
  codec: 0xCD02
  hcodec: 0xCD10  // Poseidon2 for ZK compatibility
  version: 1
  erasure: {
    ecK: 2
    ecM: 2
    originalTreeCid: <CID bytes>
    originalDatasetSize: 104857600
    protectedStrategy: 1
    verification: {
      verifyRoot: <CID bytes>
      slotRoots: [<CID1>, <CID2>, <CID3>, <CID4>]  // ecK + ecM slots
      cellSize: 2048
      verifiableStrategy: 0  // LinearStrategy
    }
  }
}
```

## Encoding Implementation (Nim Reference)

```nim
proc encode*(manifest: Manifest): seq[byte] =
  var pbNode = initProtoBuffer()
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
        verificationInfo.write(2, slotRoot.data.buffer)
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
  return pbNode.buffer
```

## Decoding Implementation (Nim Reference)

```nim
proc decode*(data: seq[byte]): Manifest =
  var pbNode = initProtoBuffer(data)
  var pbHeader: ProtoBuffer

  pbNode.getField(1, pbHeader)

  var treeCidBuf, originalTreeCid, verifyRoot: seq[byte]
  var datasetSize, originalDatasetSize: uint64
  var codec, hcodec, version, blockSize: uint32
  var ecK, ecM, protectedStrategy, cellSize, verifiableStrategy: uint32
  var slotRoots: seq[seq[byte]]
  var filename, mimetype: string

  # Decode Header
  pbHeader.getField(1, treeCidBuf)
  pbHeader.getField(2, blockSize)
  pbHeader.getField(3, datasetSize)
  pbHeader.getField(4, codec)
  pbHeader.getField(5, hcodec)
  pbHeader.getField(6, version)

  var pbErasureInfo: ProtoBuffer
  pbHeader.getField(7, pbErasureInfo)
  pbHeader.getField(8, filename)
  pbHeader.getField(9, mimetype)

  let protected = pbErasureInfo.buffer.len > 0
  var verifiable = false

  if protected:
    pbErasureInfo.getField(1, ecK)
    pbErasureInfo.getField(2, ecM)
    pbErasureInfo.getField(3, originalTreeCid)
    pbErasureInfo.getField(4, originalDatasetSize)
    pbErasureInfo.getField(5, protectedStrategy)

    var pbVerificationInfo: ProtoBuffer
    pbErasureInfo.getField(6, pbVerificationInfo)

    verifiable = pbVerificationInfo.buffer.len > 0
    if verifiable:
      pbVerificationInfo.getField(1, verifyRoot)
      pbVerificationInfo.getRequiredRepeatedField(2, slotRoots)
      pbVerificationInfo.getField(3, cellSize)
      pbVerificationInfo.getField(4, verifiableStrategy)

  # Construct manifest from decoded fields
  # (See coders.nim for full implementation)
```

## Wire Format Example

For a simple manifest (no erasure coding):

```
Outer DAG-PB:
  Field 1 (Header):
    Field 1 (treeCid): 0x01551220<32 bytes of SHA-256 hash>
    Field 2 (blockSize): 0x10 0x00 0x01 0x00 (65536 as varint)
    Field 3 (datasetSize): 0x18 0x00 0x40 0x06 0x00 (104857600 as varint)
    Field 4 (codec): 0x20 0x01 0xCD (0xCD02 as varint)
    Field 5 (hcodec): 0x28 0x12 (0x12 as varint)
    Field 6 (version): 0x30 0x01 (1 as varint)
```

## Important Notes

1. **All field numbers are 1-indexed** (protobuf convention)
2. **Optional fields** are omitted entirely if not present (not set to default values)
3. **Empty buffers indicate absence** - e.g., `pbErasureInfo.buffer.len == 0` means not protected
4. **CIDs are stored as raw bytes** - include multicodec prefix
5. **Repeated fields** (like `slotRoots`) use the same field number multiple times
6. **Varint encoding** is used for all integer types (protobuf standard)

## Testing

From `testmanifest.nim`:

```nim
let manifest = Manifest.new(
  treeCid = Cid.example,
  blockSize = 1.MiBs,        // 1048576 bytes
  datasetSize = 100.MiBs,    // 104857600 bytes
)

let encoded = manifest.encode().tryGet()
let decoded = Manifest.decode(encoded).tryGet()

assert decoded == manifest
```

## References

- Source: `/tmp/archivist-node/archivist/manifest/coders.nim`
- Schema documentation: Lines 40-67 in `coders.nim`
- Multicodec table: `/tmp/archivist-node/vendor/nim-libp2p/libp2p/multicodec.nim`
- Test suite: `/tmp/archivist-node/tests/archivist/testmanifest.nim`

## Rust Implementation Considerations

When implementing in Rust:

1. Use `prost` or `quick-protobuf` for protobuf encoding/decoding
2. Use `cid` crate for CID handling
3. Use `unsigned-varint` for multicodec encoding
4. Handle optional fields with `Option<T>`
5. Use `bytes::Bytes` for efficient byte handling
6. Implement `From` traits for easy conversion between types

Example Rust types:

```rust
#[derive(Clone, PartialEq, Message)]
pub struct Header {
    #[prost(bytes, tag = "1")]
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

#[derive(Clone, PartialEq, Message)]
pub struct ErasureInfo {
    #[prost(uint32, tag = "1")]
    pub ec_k: u32,
    #[prost(uint32, tag = "2")]
    pub ec_m: u32,
    #[prost(bytes, tag = "3")]
    pub original_tree_cid: Vec<u8>,
    #[prost(uint64, tag = "4")]
    pub original_dataset_size: u64,
    #[prost(uint32, tag = "5")]
    pub protected_strategy: u32,
    #[prost(message, optional, tag = "6")]
    pub verification: Option<VerificationInfo>,
}

#[derive(Clone, PartialEq, Message)]
pub struct VerificationInfo {
    #[prost(bytes, tag = "1")]
    pub verify_root: Vec<u8>,
    #[prost(bytes, repeated, tag = "2")]
    pub slot_roots: Vec<Vec<u8>>,
    #[prost(uint32, tag = "3")]
    pub cell_size: u32,
    #[prost(uint32, tag = "4")]
    pub verifiable_strategy: u32,
}
```
