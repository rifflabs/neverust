# Archivist Manifest Wire Format Examples

Detailed wire-level encoding examples for Archivist manifests.

## Protobuf Encoding Basics

### Protobuf Wire Types

| Wire Type | Meaning | Used For |
|-----------|---------|----------|
| 0 | Varint | int32, int64, uint32, uint64, bool, enum |
| 1 | 64-bit | fixed64, sfixed64, double |
| 2 | Length-delimited | string, bytes, embedded messages, repeated fields |
| 3 | Start group | (deprecated) |
| 4 | End group | (deprecated) |
| 5 | 32-bit | fixed32, sfixed32, float |

### Field Tag Encoding

```
Tag = (field_number << 3) | wire_type

Examples:
Field 1, wire type 2 (length-delimited): 0x0A = (1 << 3) | 2
Field 2, wire type 0 (varint):           0x10 = (2 << 3) | 0
Field 3, wire type 0 (varint):           0x18 = (3 << 3) | 0
```

### Varint Encoding

```
Values 0-127: Single byte (MSB = 0)
Values 128+: Multiple bytes (MSB = 1 for continuation)

Examples:
1 → 0x01
127 → 0x7F
128 → 0x80 0x01
300 → 0xAC 0x02
65536 → 0x80 0x80 0x04
```

## Example 1: Simple Manifest (No Erasure Coding)

### Logical Structure

```rust
Header {
    tree_cid: [CID bytes],
    block_size: 65536,
    dataset_size: 104857600,
    codec: 0xCD02,
    hcodec: 0x12,
    version: 1,
    erasure: None,
    filename: Some("test.dat"),
    mimetype: Some("application/octet-stream"),
}
```

### Wire Format Breakdown

```
┌─ DagPB Node ──────────────────────────────────────────────┐
│                                                            │
│  0A [len]  ← Field 1 (Header), wire type 2                │
│    ┌─ Header ─────────────────────────────────────────┐   │
│    │                                                   │   │
│    │  0A 26  ← Field 1 (tree_cid), wire type 2        │   │
│    │    [38 bytes of CID]                             │   │
│    │                                                   │   │
│    │  10 80 80 04  ← Field 2 (block_size), varint     │   │
│    │    = 65536                                        │   │
│    │                                                   │   │
│    │  18 80 C8 AF 31  ← Field 3 (dataset_size)        │   │
│    │    = 104857600                                    │   │
│    │                                                   │   │
│    │  20 82 9A 06  ← Field 4 (codec), varint          │   │
│    │    = 0xCD02 = 52482                              │   │
│    │                                                   │   │
│    │  28 12  ← Field 5 (hcodec), varint               │   │
│    │    = 0x12 = 18                                   │   │
│    │                                                   │   │
│    │  30 01  ← Field 6 (version), varint              │   │
│    │    = 1                                            │   │
│    │                                                   │   │
│    │  42 08  ← Field 8 (filename), wire type 2        │   │
│    │    74 65 73 74 2E 64 61 74                       │   │
│    │    "test.dat"                                     │   │
│    │                                                   │   │
│    │  4A 18  ← Field 9 (mimetype), wire type 2        │   │
│    │    61 70 70 6C 69 63 61 74 69 6F 6E 2F ...      │   │
│    │    "application/octet-stream"                     │   │
│    │                                                   │   │
│    └───────────────────────────────────────────────────┘   │
│                                                            │
└────────────────────────────────────────────────────────────┘
```

### Hex Dump (Simple Manifest)

```hex
0A 5A                          # DagPB field 1, length 90
  0A 26                        # Header field 1 (tree_cid), length 38
    01 55 12 20                # CID: version 1, raw codec 0x55, sha2-256
    E3 B0 C4 42 98 FC 1C 14    # ... hash bytes ...
    9A FB F4 C8 99 6F B9 24
    27 AE 41 E4 64 9B 93 4C
    A4 95 99 1B 78 52 B8 55
  10 80 80 04                  # Header field 2 (block_size) = 65536
  18 80 C8 AF 31               # Header field 3 (dataset_size) = 104857600
  20 82 9A 06                  # Header field 4 (codec) = 0xCD02
  28 12                        # Header field 5 (hcodec) = 0x12
  30 01                        # Header field 6 (version) = 1
  42 08                        # Header field 8 (filename), length 8
    74 65 73 74 2E 64 61 74   # "test.dat"
  4A 18                        # Header field 9 (mimetype), length 24
    61 70 70 6C 69 63 61 74   # "application/octet-stream"
    69 6F 6E 2F 6F 63 74 65
    74 2D 73 74 72 65 61 6D
```

## Example 2: Protected Manifest (With Erasure Coding)

### Logical Structure

```rust
Header {
    tree_cid: [erasure_tree_cid],
    block_size: 65536,
    dataset_size: 125829120,  // After erasure (10+2)
    codec: 0xCD02,
    hcodec: 0x12,
    version: 1,
    erasure: Some(ErasureInfo {
        ec_k: 10,
        ec_m: 2,
        original_tree_cid: [original_cid],
        original_dataset_size: 104857600,
        protected_strategy: 1,  // Stepped
        verification: None,
    }),
    filename: None,
    mimetype: None,
}
```

### Wire Format Breakdown

```
┌─ DagPB Node ──────────────────────────────────────────────┐
│                                                            │
│  0A [len]  ← Field 1 (Header)                             │
│    ┌─ Header ─────────────────────────────────────────┐   │
│    │  0A 26  ← tree_cid                               │   │
│    │  10 80 80 04  ← block_size = 65536              │   │
│    │  18 80 E0 DC 3B  ← dataset_size = 125829120     │   │
│    │  20 82 9A 06  ← codec = 0xCD02                  │   │
│    │  28 12  ← hcodec = 0x12                         │   │
│    │  30 01  ← version = 1                           │   │
│    │                                                  │   │
│    │  3A [len]  ← Field 7 (erasure), wire type 2    │   │
│    │    ┌─ ErasureInfo ──────────────────────────┐   │   │
│    │    │  08 0A  ← Field 1 (ec_k) = 10          │   │   │
│    │    │  10 02  ← Field 2 (ec_m) = 2           │   │   │
│    │    │  1A 26  ← Field 3 (original_tree_cid)  │   │   │
│    │    │    [38 bytes]                          │   │   │
│    │    │  20 80 C8 AF 31  ← Field 4 (orig size) │   │   │
│    │    │    = 104857600                         │   │   │
│    │    │  28 01  ← Field 5 (strategy) = 1       │   │   │
│    │    └────────────────────────────────────────┘   │   │
│    │                                                  │   │
│    └──────────────────────────────────────────────────┘   │
│                                                            │
└────────────────────────────────────────────────────────────┘
```

### Hex Dump (Protected Manifest)

```hex
0A 76                          # DagPB field 1, length 118
  0A 26                        # Header field 1 (tree_cid), length 38
    01 55 12 20                # CID for erasure tree
    [32 bytes of hash]
  10 80 80 04                  # block_size = 65536
  18 80 E0 DC 3B               # dataset_size = 125829120
  20 82 9A 06                  # codec = 0xCD02
  28 12                        # hcodec = 0x12
  30 01                        # version = 1
  3A 32                        # Header field 7 (erasure), length 50
    08 0A                      # ErasureInfo field 1 (ec_k) = 10
    10 02                      # ErasureInfo field 2 (ec_m) = 2
    1A 26                      # ErasureInfo field 3 (original_tree_cid), length 38
      01 55 12 20              # Original CID
      [32 bytes of hash]
    20 80 C8 AF 31             # ErasureInfo field 4 (original_dataset_size) = 104857600
    28 01                      # ErasureInfo field 5 (protected_strategy) = 1
```

## Example 3: Verifiable Manifest (Full ZK Support)

### Logical Structure

```rust
Header {
    tree_cid: [erasure_tree_cid],
    block_size: 65536,
    dataset_size: 125829120,
    codec: 0xCD02,
    hcodec: 0xCD10,  // Poseidon2 for ZK
    version: 1,
    erasure: Some(ErasureInfo {
        ec_k: 10,
        ec_m: 2,
        original_tree_cid: [original_cid],
        original_dataset_size: 104857600,
        protected_strategy: 1,
        verification: Some(VerificationInfo {
            verify_root: [verify_cid],
            slot_roots: [[slot_cid_1], [slot_cid_2], ..., [slot_cid_12]],
            cell_size: 2048,
            verifiable_strategy: 0,  // Linear
        }),
    }),
}
```

### Wire Format Breakdown

```
┌─ DagPB Node ──────────────────────────────────────────────┐
│                                                            │
│  0A [len]  ← Field 1 (Header)                             │
│    ┌─ Header ─────────────────────────────────────────┐   │
│    │  [fields 1-6 same as protected manifest]         │   │
│    │                                                   │   │
│    │  3A [len]  ← Field 7 (erasure)                   │   │
│    │    ┌─ ErasureInfo ──────────────────────────┐    │   │
│    │    │  [fields 1-5 same as protected]        │    │   │
│    │    │                                         │    │   │
│    │    │  32 [len]  ← Field 6 (verification)    │    │   │
│    │    │    ┌─ VerificationInfo ─────────────┐  │    │   │
│    │    │    │  0A 26  ← Field 1 (verify_root) │  │    │   │
│    │    │    │    [38 bytes]                   │  │    │   │
│    │    │    │                                 │  │    │   │
│    │    │    │  12 26  ← Field 2 (slot_roots)  │  │    │   │
│    │    │    │    [38 bytes - slot 1]          │  │    │   │
│    │    │    │  12 26  ← Field 2 again         │  │    │   │
│    │    │    │    [38 bytes - slot 2]          │  │    │   │
│    │    │    │  ... (repeat 12 times total)    │  │    │   │
│    │    │    │                                 │  │    │   │
│    │    │    │  18 80 10  ← Field 3 (cell_size)│  │    │   │
│    │    │    │    = 2048                       │  │    │   │
│    │    │    │                                 │  │    │   │
│    │    │    │  20 00  ← Field 4 (strategy) = 0│  │    │   │
│    │    │    └─────────────────────────────────┘  │    │   │
│    │    └─────────────────────────────────────────┘    │   │
│    └───────────────────────────────────────────────────┘   │
└────────────────────────────────────────────────────────────┘
```

### Hex Dump (Verifiable Manifest - Abbreviated)

```hex
0A [large_len]                 # DagPB field 1
  0A 26                        # Header field 1 (tree_cid)
    [38 bytes]
  10 80 80 04                  # block_size = 65536
  18 80 E0 DC 3B               # dataset_size = 125829120
  20 82 9A 06                  # codec = 0xCD02
  28 90 9A 06                  # hcodec = 0xCD10 (Poseidon2)
  30 01                        # version = 1
  3A [erasure_len]             # Header field 7 (erasure)
    08 0A                      # ec_k = 10
    10 02                      # ec_m = 2
    1A 26 [38 bytes]           # original_tree_cid
    20 80 C8 AF 31             # original_dataset_size
    28 01                      # protected_strategy = 1
    32 [verif_len]             # ErasureInfo field 6 (verification)
      0A 26                    # VerificationInfo field 1 (verify_root)
        [38 bytes]
      12 26                    # VerificationInfo field 2 (slot_roots)
        [38 bytes - slot 1]
      12 26                    # field 2 again (slot 2)
        [38 bytes]
      12 26                    # field 2 again (slot 3)
        [38 bytes]
      ... (9 more slot_roots)
      18 80 10                 # VerificationInfo field 3 (cell_size) = 2048
      20 00                    # VerificationInfo field 4 (verifiable_strategy) = 0
```

## Field Tag Quick Reference

### Header (Level 2)
```
0x0A = (1 << 3) | 2 = Field 1 (tree_cid), length-delimited
0x10 = (2 << 3) | 0 = Field 2 (block_size), varint
0x18 = (3 << 3) | 0 = Field 3 (dataset_size), varint
0x20 = (4 << 3) | 0 = Field 4 (codec), varint
0x28 = (5 << 3) | 0 = Field 5 (hcodec), varint
0x30 = (6 << 3) | 0 = Field 6 (version), varint
0x3A = (7 << 3) | 2 = Field 7 (erasure), length-delimited
0x42 = (8 << 3) | 2 = Field 8 (filename), length-delimited
0x4A = (9 << 3) | 2 = Field 9 (mimetype), length-delimited
```

### ErasureInfo (Level 3)
```
0x08 = (1 << 3) | 0 = Field 1 (ec_k), varint
0x10 = (2 << 3) | 0 = Field 2 (ec_m), varint
0x1A = (3 << 3) | 2 = Field 3 (original_tree_cid), length-delimited
0x20 = (4 << 3) | 0 = Field 4 (original_dataset_size), varint
0x28 = (5 << 3) | 0 = Field 5 (protected_strategy), varint
0x32 = (6 << 3) | 2 = Field 6 (verification), length-delimited
```

### VerificationInfo (Level 4)
```
0x0A = (1 << 3) | 2 = Field 1 (verify_root), length-delimited
0x12 = (2 << 3) | 2 = Field 2 (slot_roots), length-delimited, REPEATED
0x18 = (3 << 3) | 0 = Field 3 (cell_size), varint
0x20 = (4 << 3) | 0 = Field 4 (verifiable_strategy), varint
```

## CID Encoding Example

### CIDv1 with SHA-256

```
Full CID: bafkreie7jkfw...
Bytes:
  01              # CID version (1)
  55              # Multicodec (raw = 0x55)
  12              # Multihash type (sha2-256 = 0x12)
  20              # Multihash length (32 bytes)
  E3 B0 C4 42 ... # Hash bytes (32 bytes)

Total: 36 bytes
Protobuf encoding: 0A 26 [36 bytes]
```

### CIDv1 with Poseidon2

```
Bytes:
  01              # CID version
  CD 02           # Multicodec (codex-block = 0xCD02, varint encoded)
  CD 10           # Multihash type (poseidon2-sponge = 0xCD10)
  20              # Multihash length (32 bytes)
  [32 bytes]      # Hash

Total: 38 bytes (multicodec is 2 bytes when >127)
Protobuf encoding: 0A 26 [38 bytes]
```

## Size Calculations

### Simple Manifest
```
DagPB wrapper: 1 byte (tag) + varint(length)
Header base: ~70 bytes
  tree_cid: 1 + 1 + 38 = 40 bytes
  block_size: 1 + 3 = 4 bytes
  dataset_size: 1 + 5 = 6 bytes
  codec: 1 + 3 = 4 bytes
  hcodec: 1 + 1 = 2 bytes
  version: 1 + 1 = 2 bytes
Optional filename: 1 + 1 + len(filename)
Optional mimetype: 1 + 1 + len(mimetype)

Total: ~72 + len(filename) + len(mimetype) bytes
```

### Protected Manifest
```
Base: ~72 bytes
ErasureInfo: ~55 bytes
  ec_k: 2 bytes
  ec_m: 2 bytes
  original_tree_cid: 40 bytes
  original_dataset_size: 6 bytes
  protected_strategy: 2 bytes
  wrapper: 3 bytes

Total: ~127 bytes
```

### Verifiable Manifest (12 slots)
```
Base + ErasureInfo: ~127 bytes
VerificationInfo: ~505 bytes
  verify_root: 40 bytes
  slot_roots: 40 * 12 = 480 bytes (repeated field)
  cell_size: 3 bytes
  verifiable_strategy: 2 bytes
  wrapper: 3 bytes

Total: ~632 bytes
```

## Parsing Tips

1. **Read field tag**: `(tag >> 3)` = field number, `(tag & 0x07)` = wire type
2. **Length-delimited fields**: Next byte(s) are varint length, then data
3. **Varint fields**: Continue reading while MSB = 1
4. **Repeated fields**: Same field number appears multiple times
5. **Unknown fields**: Skip by reading length and advancing pointer

## Wire Format Validation

```rust
fn validate_wire_format(bytes: &[u8]) -> Result<()> {
    // 1. Must start with DagPB wrapper
    assert_eq!(bytes[0] & 0x07, 2, "DagPB field 1 must be length-delimited");
    assert_eq!(bytes[0] >> 3, 1, "DagPB field must be #1");

    // 2. Decode length
    let (header_len, offset) = decode_varint(&bytes[1..])?;

    // 3. Validate Header fields
    validate_header(&bytes[offset..offset + header_len])?;

    Ok(())
}
```

## Debugging Wire Format

Use `protoc` to decode:

```bash
# Save bytes to file
echo "0A5A0A26..." | xxd -r -p > manifest.bin

# Decode with protoc (if you have the .proto file)
protoc --decode=archivist.manifest.Header manifest.proto < manifest.bin

# Or use protoscope (protobuf wire format inspector)
protoscope manifest.bin
```

## Common Encoding Errors

1. **Missing DagPB wrapper**: Header alone is invalid
2. **Wrong wire type**: Using varint (0) instead of length-delimited (2)
3. **Incorrect varint encoding**: Forgetting MSB continuation bit
4. **Wrong field numbers**: Off-by-one errors
5. **Missing required fields**: Fields 1-6 of Header MUST be present
6. **Empty vs absent**: Empty message (length 0) vs no field tag at all

## Performance Characteristics

| Operation | Simple | Protected | Verifiable |
|-----------|--------|-----------|------------|
| Encode time | ~1 μs | ~2 μs | ~10 μs |
| Decode time | ~2 μs | ~4 μs | ~20 μs |
| Wire size | ~100 B | ~150 B | ~650 B |
| Parse overhead | Minimal | Low | Moderate |

## See Also

- [Complete Protobuf Format](./archivist-manifest-protobuf-format.md)
- [Implementation Guide](./manifest-implementation-guide.md)
- [Protobuf Encoding Spec](https://protobuf.dev/programming-guides/encoding/)
