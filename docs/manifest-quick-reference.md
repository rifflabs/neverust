# Archivist Manifest Quick Reference Card

One-page reference for Archivist manifest implementation.

## Multicodec

```
codex-manifest = 0xCD01 (52481)
```

## Structure

```
DagPB → Header → ErasureInfo → VerificationInfo
        (req)    (optional)     (optional)
```

## Field Tags (Hex)

### Header
| Tag | Field | Type | Req |
|-----|-------|------|-----|
| 0A | treeCid | bytes | ✓ |
| 10 | blockSize | uint32 | ✓ |
| 18 | datasetSize | uint64 | ✓ |
| 20 | codec | uint32 | ✓ |
| 28 | hcodec | uint32 | ✓ |
| 30 | version | uint32 | ✓ |
| 3A | erasure | ErasureInfo | |
| 42 | filename | string | |
| 4A | mimetype | string | |

### ErasureInfo
| Tag | Field | Type | Req* |
|-----|-------|------|------|
| 08 | ecK | uint32 | ✓ |
| 10 | ecM | uint32 | ✓ |
| 1A | originalTreeCid | bytes | ✓ |
| 20 | originalDatasetSize | uint64 | ✓ |
| 28 | protectedStrategy | uint32 | ✓ |
| 32 | verification | VerificationInfo | |

### VerificationInfo
| Tag | Field | Type | Req* |
|-----|-------|------|------|
| 0A | verifyRoot | bytes | ✓ |
| 12 | slotRoots | repeated bytes | ✓ |
| 18 | cellSize | uint32 | ✓ |
| 20 | verifiableStrategy | uint32 | ✓ |

*If parent present

## Common Values

```rust
// Sizes
const BLOCK_SIZE: u32 = 65536;      // 64 KiB
const CELL_SIZE: u32 = 2048;        // 2 KiB

// Multicodecs
const SHA2_256: u32 = 0x12;         // Hash
const POSEIDON2: u32 = 0xCD10;      // ZK hash
const CODEX_BLOCK: u32 = 0xCD02;    // Data codec
const CODEX_MANIFEST: u32 = 0xCD01; // Manifest

// CID version
const CIDV1: u32 = 1;

// Strategies
const LINEAR: u32 = 0;
const STEPPED: u32 = 1;
```

## Protobuf Schema

```protobuf
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
```

## Rust Types (Prost)

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

## Encoding (Rust)

```rust
use prost::Message;

// 1. Create Header
let header = Header {
    tree_cid: cid.to_bytes(),
    block_size: 65536,
    dataset_size: 104857600,
    codec: 0xCD02,
    hcodec: 0x12,
    version: 1,
    erasure: None,
    filename: None,
    mimetype: None,
};

// 2. Encode Header
let mut buf = Vec::new();
header.encode(&mut buf)?;

// 3. Wrap in DagPB
let mut dag_pb = vec![0x0A]; // Field 1, wire type 2
prost::encoding::encode_varint(buf.len() as u64, &mut dag_pb);
dag_pb.extend(buf);

// dag_pb now contains the complete manifest
```

## Decoding (Rust)

```rust
// 1. Unwrap DagPB
if data[0] != 0x0A {
    return Err("Invalid DagPB wrapper");
}

let (len, offset) = decode_varint(&data[1..])?;
let header_bytes = &data[offset..offset + len];

// 2. Decode Header
let header = Header::decode(header_bytes)?;

// 3. Check optional fields
let protected = header.erasure.is_some();
let verifiable = header.erasure
    .as_ref()
    .and_then(|e| e.verification.as_ref())
    .is_some();
```

## Validation

```rust
// 1. Required fields
assert!(!header.tree_cid.is_empty());
assert!(header.block_size > 0);
assert!(header.dataset_size > 0);

// 2. CID version matches
assert_eq!(header.version, 1);

// 3. If protected: validate erasure
if let Some(erasure) = &header.erasure {
    let blocks = header.dataset_size / header.block_size as u64;
    let orig_blocks = erasure.original_dataset_size
        / header.block_size as u64;
    let steps = (orig_blocks + erasure.ec_k as u64 - 1)
        / erasure.ec_k as u64;
    let expected = steps * (erasure.ec_k + erasure.ec_m) as u64;
    assert_eq!(blocks, expected);
}

// 4. If verifiable: slot count
if let Some(verification) = &header.erasure
    .as_ref()
    .and_then(|e| e.verification.as_ref())
{
    let slots = erasure.ec_k + erasure.ec_m;
    assert_eq!(verification.slot_roots.len(), slots as usize);
}
```

## Wire Format Examples

### Simple Manifest
```hex
0A 5A           # DagPB field 1, length 90
  0A 26         # treeCid, length 38
    [38 bytes]
  10 80 80 04   # blockSize = 65536
  18 80 C8 AF 31 # datasetSize = 104857600
  20 82 9A 06   # codec = 0xCD02
  28 12         # hcodec = 0x12
  30 01         # version = 1
```

### Protected Manifest
```hex
0A 76           # DagPB field 1, length 118
  [header fields 1-6]
  3A 32         # erasure, length 50
    08 0A       # ecK = 10
    10 02       # ecM = 2
    1A 26       # originalTreeCid, length 38
      [38 bytes]
    20 80 C8 AF 31 # originalDatasetSize
    28 01       # protectedStrategy = 1
```

## Sizes

| Type | Size |
|------|------|
| Simple | ~100 B |
| Protected | ~150 B |
| Verifiable (12 slots) | ~650 B |

Formula: `70 + 55 + (40 × slots) + 15`

## Common Pitfalls

1. ❌ Missing DagPB wrapper
2. ❌ CID as string (should be bytes)
3. ❌ Wrong wire type (varint vs length-delimited)
4. ❌ Checking `== 0` instead of `.is_none()`
5. ❌ Wrong field numbers (field 6 of ErasureInfo is verification)

## Testing Checklist

- [ ] Simple manifest encode/decode
- [ ] Protected manifest encode/decode
- [ ] Verifiable manifest encode/decode
- [ ] Large dataset (>5 GiB)
- [ ] Empty optional fields
- [ ] Filename/mimetype present
- [ ] Invalid manifests (validation)
- [ ] Compatibility with Archivist node

## Dependencies

```toml
[dependencies]
prost = "0.12"
cid = "0.11"
unsigned-varint = "0.8"

[build-dependencies]
prost-build = "0.12"
```

## See Full Docs

- [Complete Format](./archivist-manifest-protobuf-format.md)
- [Implementation Guide](./manifest-implementation-guide.md)
- [Wire Format Examples](./manifest-wire-format-examples.md)
- [Research Summary](../MANIFEST_RESEARCH_SUMMARY.md)
