# Archivist Node Exploration - Complete Summary

## Overview

Successfully explored and documented the **Archivist Node** submodule at `/mnt/castle/workspace/neverust/archivist-node/`. This is a comprehensive decentralized storage system written in Nim that implements block exchange protocols, erasure coding, merkle tree verification, and smart contract marketplace integration.

## Key Findings

### 1. Language & Architecture
- **Implementation Language**: Nim 2.2.4 (NOT Rust - important!)
- **Architecture Pattern**: Modular microservice-like design with pluggable backends
- **Status**: Pre-alpha but architecturally mature
- **Dual Licensed**: Apache 2.0 and MIT

### 2. Core Protocol: Block Exchange (BlockExc)

The system implements an **extended IPFS BitSwap** protocol with:

**Protocol ID**: `/archivist/blockexc/1.0.0`

**Messages** (Protobuf-defined):
- **Wantlist**: Peer requests with priority and want-type (full block vs existence check)
- **Blocks**: CID-prefixed block data
- **BlockPresence**: Availability announcements with micropayment prices (UInt256)
- **AccountMessage**: Ethereum address for payment routing
- **StateChannelUpdate**: Nitro state channel for settlement

**Key Innovation**: Built-in micropayment support via Ethereum state channels

### 3. Content Addressing System

**CID-Based**:
- Version: CIDv1 (default)
- Codec: 0xcd02 for blocks, 0xc9 for manifests
- Multihash: SHA256 by default, supports Poseidon2 for proofs

**Block Addressing** (flexible):
- Direct: `BlockAddress(cid: Cid)` 
- Tree-indexed: `BlockAddress(treeCid: Cid, index: Natural)`

### 4. Manifest System

**Complex nested structure** supporting:
- Basic metadata (treeCid, datasetSize, blockSize, codec, version, filename, mimetype)
- **Erasure Coding** (optional): ecK data blocks, ecM parity blocks with strategy
- **Verifiable Proofs** (optional): verifyRoot, slotRoots for zk-proof generation

**Serialization**: Protobuf inside DAG-PB container

### 5. Storage Layer

**Abstract BlockStore Interface**:
```
getBlock(cid) -> Block
putBlock(block, ttl) -> void
hasBlock(cid) -> bool

# Tree-indexed operations
getBlock(treeCid, index) -> Block
getCidAndProof(treeCid, index) -> (Cid, Proof)

# Callbacks
onBlockStored: ?CidCallback
```

**Implementations**:
- RepoStore (FS, SQLite, LevelDB backends)
- NetworkStore (network fallback)
- CacheStore (in-memory)

### 6. Erasure Coding System

**Architecture**:
- Takes K blocks (data) -> encodes to K + M (parity)
- Systematic (original data first)
- Padded to square matrix for efficiency
- **Decoding**: Any K blocks or K columns (even with M missing blocks/column)

**Backend Abstraction**:
- EncoderProvider: Creates encoder for (size, blocks, parity)
- DecoderProvider: Creates decoder for (size, blocks, parity)
- Thread pool for parallel operations

### 7. Merkle Tree & Proof System

**Generic Implementation**:
```nim
MerkleTree[H, K]  # H = hash type, K = key type
  - Generic over hash function
  - Layers from leaf to root
  - Compression function for combining hashes

MerkleProof[H, K]
  - Index of leaf
  - Path of sibling hashes
  - Supports reconstruction verification
```

**Hash Algorithms**:
- SHA256 (standard)
- Poseidon2 (zk-proof compatible)

### 8. Block Chunking

**Chunker System**:
- Splits input into fixed-size blocks (default 64 KiB)
- Reader abstraction for different sources
- **Implementations**:
  - FileChunker (from file)
  - LPStreamChunker (from libp2p stream)
  - Optional padding for last block

### 9. Slot & Proof System

**Indexing Strategies**:
- **Linear**: Sequential blocks per iteration
- **Stepped**: Round-robin distribution across iterations

**Proof Structure**:
- Sample[H]: cellData + merklePaths
- PublicInputs[H]: slotIndex, datasetRoot, entropy
- ProofInputs[H]: Full proof with slots and cells

### 10. Marketplace Integration

**Smart Contract Layers**:
- **ClientInteractions**: Storage request initiation
- **HostInteractions**: Storage acceptance and proof submission
- **ValidatorInteractions**: Proof validation

**State Machines**:
- **SalesAgent**: Request fulfillment lifecycle
- **Purchase**: Storage purchase lifecycle

### 11. Discovery Layer

**DiscV5-based** (`discovery.nim`):
- Keccak256-based node ID mapping from CIDs/addresses
- Provider discovery: find peers storing content
- Peer lookup: resolve peer network addresses

**Operations**:
- `find(cid)` -> seq[SignedPeerRecord]
- `provide(cid)` -> announce availability
- `findPeer(peerId)` -> ?PeerRecord

### 12. REST API

**Endpoints**:
- POST `/api/v1/upload` - Upload content
- GET `/api/v1/download/:cid` - Download
- GET `/api/v1/manifests` - List manifests
- GET `/health` - Health check
- GET `/metrics` - Prometheus metrics

**OpenAPI 3.0.3 specification** provided

### 13. Configuration System

**Priority**: CLI > Environment > Config file

**Categories**:
- Network (listen addresses, NAT, discovery port)
- Storage (directory, backend, quota, TTL)
- Marketplace (optional - Ethereum RPC, wallet, contract)
- Performance (threads, metrics)

### 14. Concurrency Model

- **Runtime**: Chronos (async)
- **Scheduling**: AsyncHeapQueue for peer tasks
- **Task Pool**: For parallel operations
- **Default Limits**:
  - 10 concurrent peer tasks
  - 100 max inflight requests
  - 64 KiB block size
  - 2 KiB cell size

### 15. Error Handling

**Typed Error Hierarchy**:
```
ArchivistError
├── BlockNotFoundError
├── ErasureError
│   └── InsufficientBlocksError
└── IndexingError
    ├── IndexingWrongIndexError
    ├── IndexingWrongIterationsError
    ├── IndexingWrongGroupCountError
    └── IndexingWrongPadBlockCountError
```

**Pattern**: Result[T, E] with operators (?!, without, ?, success, failure)

## Architecture Highlights

### 1. Modular Design
- Clear separation: Protocol, Storage, Erasure, Proofs, Contracts, API
- Each component has well-defined interfaces
- Multiple implementations per abstraction

### 2. Protocol Extensions
- Base: Extended IPFS BitSwap
- Addition: Micropayment support (price fields)
- Addition: State channel settlement (Nitro)
- Addition: Proof system integration

### 3. Flexible Storage
- Abstract interface with multiple backends
- TTL-based expiry management
- Callback system for events

### 4. Proof-Ready Architecture
- Generic merkle trees
- Erasure-coded data layout
- Slot-based proof system
- Cell sampling for verification

### 5. Observable
- Prometheus metrics at protocol level
- Structured logging with topics
- Trace-level debugging support

## Files & Documentation Generated

### 1. `/mnt/castle/workspace/neverust/ARCHIVIST_NODE_ARCHITECTURE.md`
- **748 lines** of comprehensive architecture documentation
- All 16 major sections covered
- Protocol, components, data formats, error handling
- Implementation patterns and dependencies

### 2. `/mnt/castle/workspace/neverust/PROTOCOL_SPECIFICATION_QUICK_REF.md`
- **326 lines** of quick-reference specification
- Protocol details, message structures, state machines
- Configuration parameters, constants
- Implementation checklist for Rust port

## Critical Implementation Details for Rust Port

### Must Replicate Exactly

1. **Protocol Message Structure**
   - Wantlist with priority and want-type
   - BlockPresence with UInt256 prices
   - State channel updates as JSON

2. **CID System**
   - CIDv1 codec 0xcd02 for blocks
   - codec 0xc9 for manifests
   - SHA256 multihash default

3. **Manifest Format**
   - Protobuf serialization
   - DAG-PB wrapping
   - Nested erasure/verification structures

4. **Block Addressing**
   - Support both direct CID and tree+index
   - Flexible BlockAddress type

5. **Merkle Tree Operations**
   - Generic over hash type and key type
   - Path-based proof verification
   - Compression function pattern

6. **Erasure Encoding**
   - K data + M parity systematic encoding
   - Square matrix padding
   - Any K blocks decoding capability

7. **Indexing Strategies**
   - Linear: sequential blocks per iteration
   - Stepped: round-robin distribution

8. **Discovery Mapping**
   - Keccak256(CID.data) for content
   - Keccak256(address.bytes) for Eth addresses

### Technology Stack for Rust

- **P2P**: libp2p with RequestResponse protocol
- **Serialization**: protobuf-rs for messages, DAG-PB encoding
- **Async**: tokio runtime
- **Hashing**: sha2, poseidon2-rs or equivalent
- **Merkle Trees**: Generic trait-based implementation
- **Storage**: trait objects for BlockStore
- **REST**: actix-web or axum
- **Metrics**: prometheus crate
- **Contracts**: ethers-rs for Ethereum interaction

## Directory Structure

```
archivist-node/
├── archivist/
│   ├── blockexchange/          # Protocol implementation
│   │   ├── engine/             # Task scheduling
│   │   ├── network/            # Peer management
│   │   ├── peers/              # Peer context
│   │   └── protobuf/           # Message definitions
│   ├── stores/                 # Block storage
│   ├── erasure/                # Erasure coding
│   ├── merkletree/             # Merkle trees
│   ├── manifest/               # Manifest codec
│   ├── slots/                  # Proof system
│   ├── contracts/              # Smart contracts
│   ├── sales/                  # Sales agent
│   ├── purchasing/             # Purchase lifecycle
│   ├── rest/                   # REST API
│   ├── discovery.nim           # DiscV5
│   └── node.nim                # Main orchestration
├── openapi.yaml                # API specification
└── [tests, benchmarks, docker]
```

## Key Constants (for Reference)

- Block size: 64 KiB
- Cell size: 2 KiB
- Max slot depth: 32
- Max dataset depth: 8
- Block depth: 5
- Cell elements: 67
- Samples per proof: 5
- Block TTL: 7 days
- Default quota: 1 GiB
- Block interval: 256 blocks
- Default concurrent tasks: 10
- Max inflight requests: 100

## Next Steps for Rust Implementation

1. **Translate .proto files** to Rust protobuf definitions
2. **Implement libp2p handlers** for BlockExc protocol
3. **Create generic traits** for:
   - BlockStore (async storage)
   - MerkleTree (hash type generic)
   - IndexingStrategy (block selection)
4. **Implement manifest codec** with Protobuf + DAG-PB
5. **Build task scheduler** for peer management
6. **Integrate discovery** (DiscV5 via libp2p)
7. **REST API** with proper error handling
8. **Metrics & observability** (Prometheus)
9. **Configuration system** (TOML parsing)
10. **Test coverage** for all protocols

## References

- **Main Entry**: `/mnt/castle/workspace/neverust/archivist-node/archivist/archivist.nim`
- **Protocol Def**: `/mnt/castle/workspace/neverust/archivist-node/archivist/blockexchange/protobuf/message.proto`
- **REST API**: `/mnt/castle/workspace/neverust/archivist-node/archivist/rest/api.nim`
- **API Spec**: `/mnt/castle/workspace/neverust/archivist-node/openapi.yaml`
- **Node Orchestration**: `/mnt/castle/workspace/neverust/archivist-node/archivist/node.nim`

---

**Exploration Date**: November 13, 2025
**Total Documentation**: 1,074 lines across 2 comprehensive guides
**Status**: Ready for Rust implementation planning
