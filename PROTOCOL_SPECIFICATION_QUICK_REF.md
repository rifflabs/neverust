# Archivist Node - Protocol & Implementation Quick Reference

## Critical Implementation Details

### Block Exchange Protocol

**Protocol ID**: `/archivist/blockexc/1.0.0`

**Message Structure (Protobuf)**:
```protobuf
message Message {
  Wantlist wantlist;                    // Request blocks
  repeated Block payload;               // Send blocks
  repeated BlockPresence blockPresences; // Availability
  int32 pendingBytes;                   // Flow control
  AccountMessage account;               // Payment address
  StateChannelUpdate payment;           // State channel
}

message Wantlist.Entry {
  bytes block;                          // Block CID
  int32 priority;
  bool cancel;
  WantType wantType;                    // wantBlock | wantHave
  bool sendDontHave;
}

message Block {
  bytes prefix;                         // CID prefix
  bytes data;
}

message BlockPresence {
  bytes cid;
  BlockPresenceType type;               // presenceHave | presenceDontHave
  bytes price;                          // UInt256 micropayment price
}
```

### CID System

**Block CID Construction**:
- Version: CIDv1
- Codec: 0xcd02 (codex-block)
- Multihash: sha2-256

**Example**: `bafy2bzaced...` (CIDv1-encoded)

**Other Codecs**:
- Manifest: 0xc9 (codex-manifest)
- Dataset Root: 0xcd06 (codex-root)
- Slot Root: 0xcd08 (codex-slot-root)
- Proving Root: 0xcd09 (codex-proving-root)
- Slot Cell: 0xcd0a (codex-slot-cell)

### Manifest Format

**Storage Format**: Protobuf-serialized, wrapped in DAG-PB

**Core Fields**:
```
treeCid: Cid              # Merkle root
datasetSize: NBytes       # Total bytes
blockSize: NBytes         # Block size (64 KiB default)
codec: MultiCodec         # Dataset codec
hcodec: MultiCodec        # Hash codec
version: CidVersion       # CID version
filename: ?string
mimetype: ?string
```

**Erasure Encoding** (if `protected: true`):
```
ecK: int                  # Data blocks
ecM: int                  # Parity blocks
originalTreeCid: Cid
originalDatasetSize: NBytes
protectedStrategy: StrategyType  # Linear or Stepped
```

**Verification** (if `verifiable: true`):
```
verifyRoot: Cid
slotRoots: seq[Cid]       # Per-slot proof roots
cellSize: NBytes
verifiableStrategy: StrategyType
```

### Block Exchange Flow

**Peer A wants block X**:
1. Send `Wantlist` entry (X, priority, wantBlock)
2. Peer B responds with `Block` message (CID prefix + data)
3. Or `BlockPresence` (presenceDontHave if not available)

**Negotiation Features**:
- `wantHave`: Quick existence check before full block transfer
- `sendDontHave`: Explicit negative cache
- `priority`: Influence peer scheduling
- `cancel`: Revoke want entries

### Micropayments

**Per-Block Pricing**:
- `BlockPresence.price`: UInt256 (wei equivalent)
- Payment via Nitro state channels
- `AccountMessage`: Ethereum address for payments

### Merkle Tree Operations

**Generic Structure**:
```
MerkleTree[H, K]:
  - H: Hash type (Poseidon2 field elements or Sha256)
  - K: Key type (usually unused)
  - layers: seq[H] (tree layers)
  - compress: (H, H) -> H (compression function)

MerkleProof[H, K]:
  - index: Leaf position
  - path: seq[H] (sibling hashes from leaf to root)
  - nleaves: Tree size
```

**Proof Verification**:
```
reconstructRoot(proof, leaf):
  current = leaf
  for sibling in proof.path:
    current = compress(current, sibling)
  return current  // Should equal root
```

### Erasure Coding

**Encoding**: K data blocks -> K + M parity blocks

**Layout**:
- Systematic (data first, parity appended)
- Padded to square matrix
- Rows of B blocks each (N = K + M rows)

**Decoding**:
- Any K blocks (any combination of data/parity)
- Or K columns (with up to M blocks missing per column)
- Returns original K data blocks

### Indexing Strategies

**Linear Strategy**:
```
Iteration i -> blocks [i*step, i*step+step)
Example (3 blocks per iteration):
  i=0 -> [0, 1, 2]
  i=1 -> [3, 4, 5]
  i=2 -> [6, 7, 8]
```

**Stepped Strategy**:
```
Iteration i -> blocks [i, i+iterations, i+2*iterations, ...]
Example (3 iterations):
  i=0 -> [0, 3, 6]
  i=1 -> [1, 4, 7]
  i=2 -> [2, 5, 8]
```

### Block Storage Interface

**Core Operations**:
```
getBlock(cid: Cid) -> Block
putBlock(block: Block, ttl: Duration) -> void
hasBlock(cid: Cid) -> bool
delBlock(cid: Cid) -> void

# Tree-indexed
getBlock(treeCid: Cid, index: Natural) -> Block
putCidAndProof(treeCid: Cid, index: Natural, blockCid: Cid, proof) -> void
getCidAndProof(treeCid: Cid, index: Natural) -> (Cid, ArchivistProof)

# Callbacks
onBlockStored: ?CidCallback
```

### Configuration

**Network**:
- Listen addresses: MultiAddress (e.g., /ip4/0.0.0.0/tcp/8000)
- Discovery port: DiscV5 (typically 8001)
- NAT mode: None, UPnP, or manual

**Storage**:
- Data directory: Path
- Repository backend: fs|sqlite|leveldb
- Quota: Bytes (default 1 GiB)
- Block TTL: Duration (default 7 days)

**Marketplace** (optional):
- Ethereum RPC: URL
- Wallet: Account or private key file
- Marketplace contract: Address
- Validator mode: bool

**Performance**:
- Thread count: (0 = auto)
- Metrics enabled: bool
- Block size: 64 KiB
- Cell size: 2 KiB

### REST API

**Endpoints**:
- `POST /api/v1/upload` - Upload content
- `GET /api/v1/download/:cid` - Download
- `GET /api/v1/manifests` - List manifests
- `GET /health` - Health check
- `GET /metrics` - Prometheus metrics

**Response Format**:
- Success: JSON with manifest metadata
- Error: HTTP status + error message

### Discovery (DiscV5)

**Node ID Mapping**:
```
CID -> NodeId:     keccak256(cid.data)
Address -> NodeId: keccak256(address.bytes)
```

**Operations**:
- `find(cid)` -> seq[SignedPeerRecord]
- `provide(cid)` -> announce availability
- `findPeer(peerId)` -> ?PeerRecord

### Block Addressing

**Direct CID**:
```
BlockAddress(leaf: false, cid: cid)
```

**Tree + Index**:
```
BlockAddress(leaf: true, treeCid: treeCid, index: index)
```

### Key Constants

**Sizes**:
- Default block size: 64 KiB
- Default cell size: 2 KiB

**Proof Parameters**:
- Max slot depth: 32
- Max dataset depth: 8
- Block depth: 5
- Cell elements: 67
- Samples per proof: 5

**Timeouts**:
- Block TTL: 7 days
- Block interval: 256 blocks
- Blocks per interval: 1

### Error Types

```
ArchivistError (base)
├── BlockNotFoundError
├── ErasureError
│   └── InsufficientBlocksError (minSize reported)
└── IndexingError
    ├── IndexingWrongIndexError
    ├── IndexingWrongIterationsError
    ├── IndexingWrongGroupCountError
    └── IndexingWrongPadBlockCountError
```

### Concurrency Model

**Async Runtime**: Chronos
**Task Scheduling**: AsyncHeapQueue per BlockExcEngine
**Concurrent Peers**: Default 10 simultaneous peer tasks
**Inflight Requests**: Default max 100 concurrent requests

### Marketplace State Machines

**Sales Agent**:
```
-> Available
-> Submitted
-> Accepted
-> Filled
-> Finished
(or -> Cancelled)
```

**Purchase**:
```
Pending -> Submitted -> Started -> Finished
       \              /
        -> Cancelled

Unknown (on reload from chain)
     -> Failed
```

## Implementation Checklist for Rust

- [ ] Protobuf message definitions (message.proto)
- [ ] libp2p RequestResponse protocol handler
- [ ] Manifest encode/decode (Protobuf + DAG-PB)
- [ ] CID generation and validation
- [ ] BlockStore trait (async methods)
- [ ] Merkle tree generic implementation
- [ ] Erasure coding backend integration
- [ ] DiscV5 discovery client
- [ ] Block exchange engine (task scheduling)
- [ ] REST API with Actix/Axum
- [ ] Chunk streaming and buffering
- [ ] Configuration parsing (TOML)
- [ ] Prometheus metrics
- [ ] Smart contract interactions (ethers-rs)

