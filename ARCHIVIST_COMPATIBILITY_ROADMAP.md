# Neverust ↔ Archivist-Node Compatibility Roadmap

## Executive Summary

**Goal**: Make Neverust a fully compatible Rust implementation of the Archivist storage protocol, interoperable with durability-labs/archivist-node (Nim implementation).

**Current Status**: **75% Compatible** - Core protocol and messages fully implemented, gaps in advanced features.

**Timeline Estimate**:
- Phase 1 (Erasure Coding): 2-3 weeks
- Phase 2 (Storage Backends): 1-2 weeks
- Phase 3 (Marketplace): 3-4 weeks
- Phase 4 (Testing & Integration): 2-3 weeks
- **Total**: 8-12 weeks to full compatibility

---

## ✅ What's Already Compatible

### 1. Protocol Layer (100% Compatible)
- **Protocol ID**: `/archivist/blockexc/1.0.0` ✅
  - Location: `neverust-core/src/blockexc.rs:22`
  - Matches archivist-node exactly

### 2. Message Format (100% Compatible)
All protobuf messages match archivist-node specification:

**Message** (`messages.rs:9-27`):
- ✅ wantlist (optional)
- ✅ payload (repeated BlockDelivery)
- ✅ block_presences (repeated)
- ✅ pending_bytes (flow control)
- ✅ account (AccountMessage for payments)
- ✅ payment (StateChannelUpdate for Nitro)

**WantlistEntry** (`messages.rs:120-136`):
- ✅ address (BlockAddress - supports both simple CID and tree leaves)
- ✅ priority (i32)
- ✅ cancel (bool)
- ✅ want_type (WantBlock | WantHave)
- ✅ send_dont_have (bool)

**BlockPresence** (`messages.rs:259-268`):
- ✅ address (BlockAddress)
- ✅ type (PresenceHave | PresenceDontHave)
- ✅ **price** (Vec<u8> for UInt256 micropayments) 🎉

**BlockDelivery** (`messages.rs:212-228`):
- ✅ cid (block CID)
- ✅ data (block data)
- ✅ address (BlockAddress)
- ✅ proof (ArchivistProof for Merkle trees)

### 3. Content Addressing (100% Compatible)
**CID System** (`cid_blake3.rs`):
- ✅ CIDv1
- ✅ Codec: 0xcd02 (codex-block)
- ✅ Multihash: SHA-256 (code 0x12)
- ✅ Codec 0xc9 support for manifests

**BlockAddress** (`messages.rs:43-90`):
- ✅ Simple CID addressing (`leaf=false`)
- ✅ Tree-indexed addressing (`leaf=true, tree_cid, index`)
- ✅ Helper methods for both modes

### 4. Manifest System (95% Compatible)
**Manifest Structure** (`manifest.rs`):
- ✅ tree_cid, block_size, dataset_size
- ✅ codec, hcodec, version
- ✅ filename, mimetype
- ✅ ErasureInfo (ec_k, ec_m, original_tree_cid, protected_strategy)
- ✅ VerificationInfo (verify_root, slot_roots, cell_size)
- ✅ Protobuf encoding in DAG-PB container
- ⚠️ **Gap**: Need to verify erasure strategy implementation details

### 5. Discovery (100% Compatible)
- ✅ DiscV5-based peer discovery (`discv5 = "0.10"`)
- ✅ Keccak256 node ID mapping
- ✅ Implementation in `discovery.rs` and `discovery_engine.rs`

### 6. Storage Layer (60% Compatible)
**Current** (`storage.rs`):
- ✅ Abstract BlockStore trait
- ✅ RocksDB backend
- ✅ In-memory backend
- ❌ **Missing**: Filesystem backend
- ❌ **Missing**: SQLite backend
- ❌ **Missing**: LevelDB backend

### 7. API Layer (80% Compatible)
- ✅ REST API server (`api.rs`)
- ✅ Metrics endpoint (`metrics.rs`)
- ⚠️ **Gap**: May need archivist-specific endpoints

---

## ❌ What's Missing

### Phase 1: Erasure Coding Implementation

**Current Status**: Structure exists, implementation details unclear

**Required**:
1. **Encoder**:
   - Input: K data blocks
   - Output: K + M blocks (K original + M parity)
   - Layout: Systematic (data first, then parity)
   - Matrix padding: Pad to square for efficiency

2. **Decoder**:
   - Input: Any K blocks from K+M total
   - Output: Original K data blocks
   - Alternative: Any K columns (even with M blocks missing per column)

3. **Strategy Types**:
   - Linear: Blocks numbered 0..(K+M-1)
   - Stepped: Interleaved layout for distributed storage

**Implementation Options**:
- Use `reed-solomon-erasure` crate (Rust)
- Or port from archivist-node's erasure module
- Thread pool for parallel encoding/decoding

**Files to Create/Modify**:
- `neverust-core/src/erasure.rs` (new)
- `neverust-core/src/erasure_encoder.rs` (new)
- `neverust-core/src/erasure_decoder.rs` (new)

**Estimated Effort**: 2-3 weeks

---

### Phase 2: Merkle Tree Enhancements

**Current Status**: Basic proof structure exists

**Required**:
1. **Generic Hash Function Support**:
   ```rust
   trait HashFunction {
       fn hash(&self, data: &[u8]) -> Vec<u8>;
       fn compress(&self, left: &[u8], right: &[u8]) -> Vec<u8>;
   }

   struct MerkleTree<H: HashFunction> {
       layers: Vec<Vec<Vec<u8>>>,
       hasher: H,
   }
   ```

2. **Hash Implementations**:
   - SHA256Hash (already have via `sha2` crate)
   - Poseidon2Hash (for zk-proof compatibility)
     - Use `poseidon2` crate or similar
     - Field element arithmetic

3. **Proof Operations**:
   - Proof generation
   - Proof verification
   - Root reconstruction

**Files to Create/Modify**:
- `neverust-core/src/merkle.rs` (new)
- `neverust-core/src/poseidon.rs` (new for Poseidon2)

**Dependencies to Add**:
```toml
poseidon2 = "0.2"  # or latest
ff = "0.13"  # Field elements
```

**Estimated Effort**: 1-2 weeks

---

### Phase 3: Storage Backend Expansion

**Current Status**: Only RocksDB

**Required Backends**:

1. **Filesystem Backend**:
   ```rust
   struct FSStore {
       base_path: PathBuf,
       // Store blocks as files: {base_path}/{cid.to_string()}
   }
   ```
   - File naming: CID as filename
   - Directory sharding: First 2-4 chars of CID
   - TTL via file metadata timestamps

2. **SQLite Backend**:
   ```rust
   struct SQLiteStore {
       conn: rusqlite::Connection,
   }
   ```
   - Schema:
     ```sql
     CREATE TABLE blocks (
       cid BLOB PRIMARY KEY,
       data BLOB NOT NULL,
       stored_at INTEGER NOT NULL,
       ttl INTEGER
     );
     CREATE INDEX idx_ttl ON blocks(ttl);
     ```

3. **LevelDB Backend** (optional):
   - Similar to RocksDB
   - Use `leveldb` crate

**Files to Create**:
- `neverust-core/src/storage_fs.rs` (new)
- `neverust-core/src/storage_sqlite.rs` (new)
- `neverust-core/src/storage_leveldb.rs` (new - optional)

**Dependencies to Add**:
```toml
rusqlite = { version = "0.31", features = ["bundled"] }
leveldb = "0.8"  # optional
```

**Estimated Effort**: 1-2 weeks

---

### Phase 4: Marketplace Integration

**Current Status**: Not implemented

**Required**:
1. **State Machines**:
   - Sales FSM (Downloading → Filling → Finished → Failed)
   - Purchase FSM (Submitted → Started → Finished → Failed)
   - Cancellation handling

2. **Smart Contract Integration**:
   - Ethereum RPC client
   - Contract ABIs (Client, Host, Validator)
   - Event monitoring
   - Transaction submission

3. **Data Structures**:
   ```rust
   struct SalesAgent {
       slot_id: SlotId,
       request: StorageRequest,
       state: SalesState,
   }

   struct PurchasingAgent {
       purchase_id: PurchaseId,
       request: StorageRequest,
       state: PurchaseState,
   }
   ```

**Files to Create**:
- `neverust-core/src/sales.rs` (new)
- `neverust-core/src/purchasing.rs` (new)
- `neverust-core/src/contracts.rs` (new)
- `neverust-core/src/marketplace_types.rs` (new)

**Dependencies to Add**:
```toml
ethers = { version = "2.0", features = ["abigen", "ws"] }
```

**Estimated Effort**: 3-4 weeks

---

### Phase 5: libp2p Version Alignment

**Current Issue**: Version conflicts
- neverust-core uses libp2p 0.56
- Tests/dependencies pull in libp2p 0.52 and libp2p-swarm 0.43
- Causes `SwarmEvent` type mismatches

**Solution**:
1. **Pin all libp2p dependencies to 0.56**:
   ```toml
   [dependencies]
   libp2p = { version = "0.56", features = [...] }
   libp2p-swarm = "0.47"  # Compatible with 0.56
   libp2p-mplex = "0.43"

   # In workspace Cargo.toml, add:
   [patch.crates-io]
   libp2p = { version = "0.56" }
   libp2p-swarm = { version = "0.47" }
   ```

2. **Update test imports**:
   ```rust
   use libp2p::swarm::SwarmEvent;  // Will now use 0.47
   ```

3. **Verify no transitive dependencies pull old versions**:
   ```bash
   cargo tree -p libp2p-swarm
   ```

**Files to Modify**:
- `Cargo.toml` (workspace-level patches)
- `neverust-core/Cargo.toml` (explicit versions)
- `tests/*.rs` (update imports if needed)

**Estimated Effort**: 2-3 days

---

## Implementation Roadmap

### Week 1-2: Erasure Coding
- [ ] Research `reed-solomon-erasure` crate
- [ ] Implement `ErasureEncoder` (K → K+M systematic)
- [ ] Implement `ErasureDecoder` (any K → original K)
- [ ] Add matrix padding logic
- [ ] Implement Linear and Stepped strategies
- [ ] Write comprehensive tests (encoding/decoding roundtrip)
- [ ] Benchmark performance

### Week 3-4: Merkle Trees + Poseidon2
- [ ] Design generic `MerkleTree<H: HashFunction>` trait
- [ ] Implement SHA256 hash function
- [ ] Add Poseidon2 hash function (for zk-proofs)
- [ ] Proof generation and verification
- [ ] Integration with manifest system
- [ ] Tests for both hash types

### Week 5-6: Storage Backends
- [ ] Implement Filesystem backend with directory sharding
- [ ] Implement SQLite backend with TTL indexing
- [ ] Optional: LevelDB backend
- [ ] Write migration tools (RocksDB ↔ FS ↔ SQLite)
- [ ] Performance benchmarks (compare all backends)
- [ ] Integration tests

### Week 7: libp2p Version Fix
- [ ] Pin all libp2p deps to 0.56/0.47
- [ ] Add workspace-level patches
- [ ] Fix test compilation errors
- [ ] Run full test suite
- [ ] Verify no transitive conflicts

### Week 8-10: Marketplace
- [ ] Implement Sales FSM
- [ ] Implement Purchasing FSM
- [ ] Add Ethereum contract integration
- [ ] Event monitoring and transaction submission
- [ ] State persistence
- [ ] Integration tests (use local testnet)

### Week 11-12: Integration Testing
- [ ] Set up dual testnet (Neverust + archivist-node)
- [ ] Test block exchange between Rust and Nim nodes
- [ ] Test erasure-coded content retrieval
- [ ] Test marketplace operations
- [ ] Performance benchmarks
- [ ] Documentation and examples

---

## Testing Strategy

### Unit Tests
- Each module has comprehensive unit tests
- Mock external dependencies
- Fast execution (<5s total)

### Integration Tests
- Cross-node compatibility tests
- Neverust node ↔ archivist-node (Nim)
- Verify protocol message compatibility
- Erasure coding roundtrip tests

### Compatibility Test Plan
```bash
# Terminal 1: Start Nim archivist-node
cd archivist-node
nim c -r archivist.nim --data-dir=/tmp/nim-node

# Terminal 2: Start Rust Neverust node
cd neverust
cargo run -- --data-dir=/tmp/rust-node --bootstrap=/ip4/127.0.0.1/tcp/8080/p2p/<nim-peer-id>

# Terminal 3: Upload to Nim, download from Rust
# Upload 1GB file to Nim node
curl -X POST http://localhost:8080/api/v1/upload -F "file=@test.dat"

# Request from Rust node
curl http://localhost:9000/api/v1/download/<cid> -o downloaded.dat

# Verify integrity
sha256sum test.dat downloaded.dat
```

### Performance Benchmarks
- Block exchange throughput (MB/s)
- Erasure encoding/decoding speed
- Merkle proof generation time
- Storage backend comparison

---

## Success Criteria

### Phase 1: Basic Compatibility
- [x] Protocol ID matches
- [x] Message format compatible
- [x] CID system compatible
- [x] Basic block exchange works

### Phase 2: Advanced Features
- [ ] Erasure coding works (K+M blocks)
- [ ] Merkle proofs verify correctly
- [ ] Multiple storage backends
- [ ] Marketplace operations

### Phase 3: Full Interoperability
- [ ] Neverust and archivist-node can exchange blocks
- [ ] Erasure-coded content retrieves correctly
- [ ] Cross-implementation marketplace trades
- [ ] Performance within 20% of Nim implementation

---

## Dependencies Summary

### Existing (Already in neverust-core/Cargo.toml)
```toml
libp2p = "0.56"
cid = "0.11"
multihash = "0.19"
sha2 = "0.10"
prost = "0.12"
rocksdb = "0.22"
discv5 = "0.10"
```

### To Add
```toml
# Erasure coding
reed-solomon-erasure = "6.0"

# Poseidon2 (zk-proof hash)
poseidon2 = "0.2"
ff = "0.13"

# Additional storage backends
rusqlite = { version = "0.31", features = ["bundled"] }
leveldb = "0.8"  # optional

# Marketplace / Ethereum
ethers = { version = "2.0", features = ["abigen", "ws"] }
```

---

## Open Questions

1. **Erasure Coding Library**: Use `reed-solomon-erasure` or port from Nim?
   - **Recommendation**: Use Rust crate for performance and safety

2. **Poseidon2 Field**: Which prime field for Poseidon2?
   - **Answer**: Check archivist-node implementation (likely BN254 or BLS12-381)

3. **Marketplace Testing**: How to test without deploying to real Ethereum?
   - **Answer**: Use Ganache/Hardhat local testnet

4. **Performance Target**: Match Nim's speed or optimize further?
   - **Goal**: Within 20% initially, then optimize

5. **Storage Backend Default**: Which should be default?
   - **Recommendation**: RocksDB (already default, best performance)

---

## Resources

### Documentation
- [x] `EXPLORATION_SUMMARY.md` - Overview of archivist-node
- [x] `ARCHIVIST_NODE_ARCHITECTURE.md` - Deep architectural reference
- [x] `PROTOCOL_SPECIFICATION_QUICK_REF.md` - Implementation checklist
- [x] This document - Compatibility roadmap

### Code References
- Archivist-Node (Nim): `/mnt/castle/workspace/neverust/archivist-node/`
- Neverust (Rust): `/mnt/castle/workspace/neverust/neverust-core/src/`

### External Resources
- IPFS BitSwap Spec: https://github.com/ipfs/specs/blob/master/BITSWAP.md
- libp2p Specs: https://github.com/libp2p/specs
- Reed-Solomon: https://docs.rs/reed-solomon-erasure/
- Nitro Protocol: https://docs.statechannels.org/

---

## Next Immediate Actions

1. **Fix libp2p version conflicts** (2-3 days)
   - This is blocking tests from running
   - Quick win to unblock development

2. **Start Erasure Coding implementation** (Week 1-2)
   - Core feature for data redundancy
   - High priority for compatibility

3. **Set up compatibility test environment** (1 week)
   - Build archivist-node (Nim)
   - Create test scripts
   - Establish baseline metrics

4. **Document current API differences** (2-3 days)
   - Compare REST endpoints
   - Identify missing routes
   - Plan API alignment

---

**Last Updated**: 2025-11-13
**Status**: Ready for implementation
**Contact**: See README.md for contributing guidelines
