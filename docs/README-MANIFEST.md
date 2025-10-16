# Archivist Manifest Documentation Index

Complete documentation for implementing Archivist-compatible manifest encoding/decoding in Neverust.

## Quick Start

**New to manifests?** Start here:
1. Read [Quick Reference](./manifest-quick-reference.md) - One-page overview
2. Review [Implementation Guide](./manifest-implementation-guide.md) - Practical implementation steps
3. Consult [Complete Format](./archivist-manifest-protobuf-format.md) when you need details

**Ready to implement?**
1. Copy protobuf schema from [Implementation Guide](./manifest-implementation-guide.md)
2. Follow the Rust examples for encoding/decoding
3. Use [Wire Format Examples](./manifest-wire-format-examples.md) for debugging

## Documentation Files

### 1. Quick Reference Card
**File**: [manifest-quick-reference.md](./manifest-quick-reference.md)
**Size**: One page
**Use When**: Quick lookup of field tags, types, or common values

Contains:
- Field tag table (hex values)
- Common multicodec values
- Minimal protobuf schema
- Encoding/decoding snippets
- Common pitfalls
- Testing checklist

### 2. Implementation Guide
**File**: [manifest-implementation-guide.md](./manifest-implementation-guide.md)
**Size**: ~35 KB
**Use When**: Starting implementation or need practical examples

Contains:
- Field mapping summary (all levels)
- Encoding/decoding flow diagrams
- Complete Rust examples (simple, protected, verifiable)
- Validation rules with code
- Common multicodec reference
- Performance tips
- Common pitfalls
- Testing strategy
- Next steps for Neverust

### 3. Complete Protobuf Format
**File**: [archivist-manifest-protobuf-format.md](./archivist-manifest-protobuf-format.md)
**Size**: ~57 KB
**Use When**: Need comprehensive technical reference

Contains:
- Complete protobuf schema (all 3 levels)
- Field-by-field documentation
- Multicodec reference
- Default values
- Three manifest types explained (simple, protected, verifiable)
- Nim reference implementation snippets
- Rust implementation considerations
- Testing guidance

### 4. Wire Format Examples
**File**: [manifest-wire-format-examples.md](./manifest-wire-format-examples.md)
**Size**: ~76 KB
**Use When**: Debugging encoding issues or understanding binary format

Contains:
- Protobuf encoding basics
- Wire type reference
- Varint encoding explained
- Complete hex dumps of all 3 manifest types
- ASCII diagram breakdowns
- Field tag calculations
- Size calculations
- Debugging techniques
- Common encoding errors
- Performance characteristics

### 5. Research Summary
**File**: [../MANIFEST_RESEARCH_SUMMARY.md](../MANIFEST_RESEARCH_SUMMARY.md)
**Size**: ~65 KB
**Use When**: Understanding research methodology or verifying against source

Contains:
- Research methodology
- Key findings (10 major points)
- Source code references (archivist-node)
- Extracted schemas with line numbers
- Test case analysis
- Implementation recommendations
- Validation rules
- Critical implementation notes
- Next steps for Neverust

## Learning Path

### Path 1: For Implementers (Fast Track)
```
Quick Reference → Implementation Guide → Start Coding
                                       ↓
                          (Use Complete Format for details)
```

### Path 2: For Deep Understanding
```
Research Summary → Complete Format → Wire Format Examples
                                   ↓
                        Implementation Guide → Start Coding
```

### Path 3: For Debugging
```
Wire Format Examples → Implementation Guide → Quick Reference
         ↓
    (Compare hex dumps to identify issues)
```

## Key Concepts

### Three-Level Nesting

```
Level 1: DagPB Node (wrapper)
           ├─ Field 1: Header
Level 2:   │    ├─ Fields 1-6: Core metadata (required)
           │    ├─ Field 7: ErasureInfo (optional)
Level 3:   │    │    ├─ Fields 1-5: Erasure parameters (required if present)
           │    │    └─ Field 6: VerificationInfo (optional)
Level 4:   │    │         └─ Fields 1-4: ZK proof metadata (required if present)
           │    ├─ Field 8: filename (optional)
           │    └─ Field 9: mimetype (optional)
```

### Manifest Types

1. **Simple** - No erasure coding (smallest, ~100 bytes)
2. **Protected** - With erasure coding, no ZK proofs (~150 bytes)
3. **Verifiable** - Full ZK support (largest, ~650 bytes for 12 slots)

### Critical Implementation Points

1. **DagPB wrapper is required** - Don't skip it
2. **CIDs are bytes, not strings** - Use `cid.to_bytes()`
3. **Empty buffer means absent** - Check `is_none()`, not `== 0`
4. **Field 6 of ErasureInfo is verification** - Not field 7
5. **Repeated fields use same tag** - `slotRoots` repeats tag 0x12

## Code Examples

All documentation includes Rust code examples using:
- `prost` for protobuf encoding/decoding
- `cid` crate for CID handling
- Standard library for basic operations

Examples cover:
- Simple manifest creation and encoding
- Protected manifest with erasure coding
- Verifiable manifest with ZK proofs
- Validation logic
- Error handling
- Round-trip testing

## Testing

Comprehensive testing guidance includes:
- Unit tests (simple, protected, verifiable)
- Integration tests (compatibility with Archivist)
- Edge cases (large datasets, empty fields)
- Validation tests (invalid manifests)
- Performance benchmarks

## Dependencies

Required Rust crates:
```toml
[dependencies]
prost = "0.12"
cid = "0.11"
unsigned-varint = "0.8"
bytes = "1.5"

[build-dependencies]
prost-build = "0.12"
```

## Source References

All documentation is based on analysis of:
- **Repository**: `/tmp/archivist-node` (nim-codex)
- **Key Files**:
  - `archivist/manifest/coders.nim` - Encoding/decoding logic
  - `archivist/manifest/manifest.nim` - Data structures
  - `archivist/archivisttypes.nim` - Multicodec definitions
  - `archivist/indexingstrategy.nim` - Strategy enum
  - `tests/archivist/testmanifest.nim` - Test cases
  - `vendor/nim-libp2p/libp2p/multicodec.nim` - Multicodec table

## Next Steps for Neverust

1. **Set up protobuf generation**
   - Create `protos/manifest.proto`
   - Add `build.rs` with prost-build
   - Generate Rust types

2. **Implement validation**
   - Required field checks
   - Erasure block count validation
   - Slot root count validation
   - CID parsing validation

3. **Write tests**
   - Unit tests for all 3 manifest types
   - Round-trip encode/decode
   - Invalid manifest detection
   - Integration tests with real Archivist node

4. **Integrate with blockexc**
   - Manifest exchange protocol
   - CID-based manifest lookup
   - Caching and validation

5. **Add observability**
   - Metrics (encode/decode time, size distribution)
   - Tracing (manifest operations)
   - Logging (validation failures)

## FAQ

**Q: Why is the DagPB wrapper needed?**
A: Archivist stores manifests as IPLD blocks, which require the DAG-PB format. The Header alone is not a valid manifest.

**Q: What's the difference between `protectedStrategy` and `verifiableStrategy`?**
A: `protectedStrategy` defines how original blocks are grouped for erasure coding. `verifiableStrategy` defines how slot blocks are indexed for ZK proofs. They can be different (typically protected=Stepped, verifiable=Linear).

**Q: How many bytes is a CID?**
A: For CIDv1 with SHA-256: 36 bytes (1 version + 1 codec + 1 hash type + 1 hash length + 32 hash). With multicodecs >127 (like Poseidon2), it's 38 bytes.

**Q: Can I skip validation?**
A: Not recommended. Invalid manifests can cause errors during content retrieval. Always validate after decoding.

**Q: How do I debug encoding issues?**
A: Use the hex dumps in Wire Format Examples to compare your output. Common issues: missing DagPB wrapper, wrong wire type, incorrect varint encoding.

## Changelog

- **2025-10-08**: Initial documentation created from archivist-node analysis
  - Complete protobuf schema extracted
  - Wire format documented with hex examples
  - Implementation guide with Rust examples
  - Quick reference card for developers

## Contributing

To update this documentation:
1. Verify changes against archivist-node source code
2. Update all 5 documentation files for consistency
3. Test examples in Rust before committing
4. Update changelog in this README

## License

Documentation content: CC0 1.0 Universal (Public Domain)
Code examples: Same as Neverust project (MIT/Apache-2.0)

Based on Archivist (nim-codex) codebase:
- Apache License 2.0
- MIT License
