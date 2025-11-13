# Archivist Node - Comprehensive Architecture Analysis

## Executive Summary

The **Archivist Node** is a Nim-based decentralized durability engine for P2P networks that enables storing files and data with predictable durability guarantees. It implements a sophisticated block exchange protocol, erasure coding, merkle tree verification, and on-chain marketplace integration.

**Key Implementation Language**: Nim 2.2.4 (NOT Rust)
**Status**: Pre-alpha, actively developed
**Dual Licensed**: Apache 2.0 and MIT

---

## 1. Network Stack & Protocol Implementation

### 1.1 Block Exchange Protocol

**Protocol Codec**: `/archivist/blockexc/1.0.0`

**Protobuf-based Message Format** (`message.proto`):
- **Wantlist**: Peer requests for blocks
  - Supports `WantType.wantBlock` (full block) and `WantType.wantHave` (existence check)
  - Priority-based entry system with cancel capability
  - `sendDontHave` flag for negative availability signals
  
- **Block Payload**: CID-prefixed block data
  - Prefix contains: CID version, multicodec, multihash type+length
  - Data field holds raw block bytes
  
- **BlockPresence**: Availability announcements
  - `presenceHave` / `presenceDontHave` indicators
  - **Price field**: UInt256 for micropayment pricing per block
  
- **Account Message**: Ethereum address for payment routing
  
- **StateChannelUpdate**: Nitro state channels for payment settlement
  - Signed state serialized as JSON

### 1.2 libp2p Integration

**Dependencies**:
- `pkg/libp2p` - Core P2P networking framework
- `pkg/libp2p/cid` - Content Identifier handling
- `pkg/libp2p/multicodec` - Codec specifications
- `pkg/libp2p/multihash` - Hash algorithm support
- `pkg/libp2p/switch` - Peer connection switching
- `pkg/libp2p/routing_record` - Routing metadata
- `pkg/libp2p/signed_envelope` - Cryptographic peer records

**Key Components**:
- **Switch**: Manages peer connections and protocol multiplexing
- **PeerId**: Unique peer identifier (libp2p basis)
- **MultiAddress**: Network addressing scheme (TCP/IP4, etc.)

### 1.3 Discovery Layer

**DiscV5-based Discovery** (`discovery.nim`):
- **DHT Integration**: Keccak256-based node ID hashing for content location
- **Provider Records**: Signed peer records with connection information
- **Find Protocol**: `find(cid: Cid)` returns list of providers storing content
- **Provide Protocol**: `provide(cid: Cid)` announces local content availability
- **Peer Lookup**: `findPeer(peerId: PeerId)` resolves peer network addresses

**Discovery Uses**:
```
CID -> NodeId conversion: keccak256(cid.data)
Eth Address -> NodeId: keccak256(address.bytes)
```

### 1.4 Network Peer Management

**BlockExcNetwork** type:
- Manages multiple concurrent peer connections
- Inflight request semaphore (default: 100 max requests)
- Async message sending with cancellation support
- Handlers for: WantList, BlockDelivery, BlockPresence, Account, Payment

**Connection Lifecycle**:
```
New Peer -> Establish libp2p stream
         -> Set up NetworkPeer wrapper
         -> Register in peers table
         -> Handle incoming messages
         -> Track active requests
```

---

## 2. Main Components & Modules

### 2.1 Core Node Architecture

```
ArchivistNode (node.nim):
├── switch: Switch                    # libp2p networking
├── networkId: PeerId                 # Local peer identity
├── networkStore: BlockStore          # Block storage interface
├── engine: BlockExcEngine           # Block exchange orchestration
├── prover: ?Prover                  # Optional storage proof system
├── discovery: Discovery             # DiscV5 provider discovery
├── contracts: (client, host, validator)  # Smart contract interactions
├── clock: Clock                     # On-chain or system time
├── taskpool: Taskpool               # Async task execution
└── trackedFutures: TrackedFutures   # Future lifecycle tracking
```

### 2.2 BlockExchange Engine

**BlockExcEngine** (`blockexchange/engine/engine.nim`):

Core responsibilities:
- **Peer Context Management**: Track peers, their blocks, and requests
- **Task Scheduling**: Priority queue of peer tasks
- **Want List Handling**: Request blocks from peers
- **Block Delivery**: Receive and validate incoming blocks
- **Concurrent Task Processing**: Default 10 concurrent tasks

**Constants**:
- `DefaultMaxPeersPerRequest = 10` (peers per block request)
- `DefaultTaskQueueSize = 100`
- `DefaultConcurrentTasks = 10`

**Metrics Tracked**:
- `archivist_block_exchange_want_have_lists_sent/received`
- `archivist_block_exchange_want_block_lists_sent/received`
- `archivist_block_exchange_blocks_sent/received`

**Submodules**:
- **Payments**: Micropayment processing via Nitro
- **Discovery**: Provider discovery and advertising
- **Advertiser**: Announcing local block availability
- **PendingBlocks**: Managing in-flight block requests

### 2.3 BlockStore (Storage Abstraction)

**BlockStore** interface methods:

```nim
# Block retrieval
getBlock(cid: Cid) -> Block
getBlock(treeCid: Cid, index: Natural) -> Block
getBlock(address: BlockAddress) -> Block

# Proof retrieval
getBlockAndProof(treeCid: Cid, index: Natural) -> (Block, ArchivistProof)
getCidAndProof(treeCid: Cid, index: Natural) -> (Cid, ArchivistProof)

# Storage operations
putBlock(blk: Block, ttl: Duration) -> void
putCidAndProof(treeCid: Cid, index: Natural, blockCid: Cid, proof: ArchivistProof) -> void

# Lifecycle
delBlock(cid: Cid) -> void
hasBlock(cid: Cid) -> bool

# Expiry management
ensureExpiry(cid: Cid, expiry: SecondsSince1970) -> void

# Callbacks
onBlockStored: ?CidCallback
```

**Implementation Variants**:
- **RepoStore**: FS/SQLite/LevelDB backend selection
- **NetworkStore**: Network-backed block fetching (fallback)
- **CacheStore**: In-memory caching layer

### 2.4 Manifest System

**Manifest** (`manifest/manifest.nim`):

Complex nested structure for content metadata:

```nim
Manifest = ref object
  treeCid: Cid              # Root of merkle tree
  datasetSize: NBytes       # Total data size
  blockSize: NBytes         # Block size (default 64 KiB)
  codec: MultiCodec         # Dataset codec
  hcodec: MultiCodec        # Multihash codec
  version: CidVersion       # CID version
  filename: ?string         # Original filename
  mimetype: ?string         # Content type
  
  # Protected (erasure coded) data:
  protected: bool
  ├─ ecK: int              # Data blocks
  ├─ ecM: int              # Parity blocks
  ├─ originalTreeCid: Cid
  ├─ originalDatasetSize: NBytes
  ├─ protectedStrategy: StrategyType
  │
  └─ verifiable: bool      # Proof-capable erasure code
     ├─ verifyRoot: Cid       # Top-level proof root
     ├─ slotRoots: seq[Cid]   # Per-slot proof roots
     ├─ cellSize: NBytes      # Proof cell size
     └─ verifiableStrategy: StrategyType
```

**Manifest Codec**: `codex-manifest` (multicodec)

**Serialization**: Protobuf inside DAG-PB container

**Encoding Parameters**:
```
Header (protobuf):
  1: treeCid (bytes)
  2: blockSize (uint32)
  3: datasetSize (uint64)
  4: codec (uint32)
  5: hcodec (uint32)
  6: version (uint32)
  7: erasureInfo (protobuf)
  8: filename (string)
  9: mimetype (string)

ErasureInfo (protobuf):
  1: ecK (uint32)
  2: ecM (uint32)
  3: originalTreeCid (bytes)
  4: originalDatasetSize (uint64)
  5: protectedStrategy (uint32)
  6: verificationInfo (protobuf)

VerificationInfo (protobuf):
  1: verifyRoot (bytes)
  2: slotRoots (repeated bytes)
  3: cellSize (uint32)
  4: verifiableStrategy (uint32)
```

### 2.5 Block Types & Addressing

**Block** (`blocktype.nim`):
```nim
Block = ref object of RootObj
  cid: Cid
  data: seq[byte]
```

**BlockAddress** - Flexible addressing:
```nim
BlockAddress = object
  case leaf: bool
  of true:
    treeCid: Cid      # For merkle tree lookups
    index: Natural    # Block index within tree
  else:
    cid: Cid          # Direct CID lookup
```

**CID Codecs Used**:
- `BlockCodec` (0xcd02 - "codex-block")
- `ManifestCodec` (0xc9 - "codex-manifest")
- `DatasetRootCodec` (0xcd06 - "codex-root")
- `SlotRootCodec` (0xcd08 - "codex-slot-root")
- `SlotProvingRootCodec` (0xcd09 - "codex-proving-root")
- `SlotCellCodec` (0xcd0a - "codex-slot-cell")

**Hash Codecs**:
- `Sha256HashCodec` (sha2-256)
- `Pos2Bn128SpngCodec` (poseidon2-alt_bn_128-sponge-r2)
- `Pos2Bn128MrklCodec` (poseidon2-alt_bn_128-merkle-2kb)

---

## 3. Data Formats & Encodings

### 3.1 Content Identifier (CID) System

**CID Structure**:
- **Version**: CIDv1 (default)
- **Multicodec**: Specifies block type (manifest, data, etc.)
- **Multihash**: Hash function + digest

**Block CID Creation**:
```nim
Block.new(data, version=CIDv1, mcodec=Sha256HashCodec, codec=BlockCodec)
  -> Computes sha256(data)
  -> Creates CID(CIDv1, BlockCodec, hash)
```

### 3.2 Protobuf Definitions

**Key .proto files**:
- `blockexchange/protobuf/message.proto` - BlockExc protocol messages
- Automatically transpiled to Nim protobuf stubs

### 3.3 JSON/REST Encoding

**REST Coders** (`rest/coders.nim`):
- JSON serialization of manifest metadata
- CID, block, and filesystem representations

**API Endpoints** (from OpenAPI spec):
- `POST /upload` - Upload content
- `GET /download/:cid` - Download by CID
- `GET /manifests` - List stored manifests
- `GET /health` - Health check
- Metrics endpoint at `/metrics`

---

## 4. Erasure Coding System

### 4.1 Erasure Module Architecture

**Erasure** (`erasure/erasure.nim`):

```nim
Erasure = ref object
  taskPool: Taskpool           # Thread pool for encoding/decoding
  encoderProvider: proc        # Create encoders
  decoderProvider: proc        # Create decoders
  store: BlockStore            # Underlying storage
```

**Encoding Process**:
1. Take K blocks (original data)
2. Encode into M parity blocks
3. Resulting N = K + M rows
4. Each row has B blocks (power of 2 for erasure math)
5. Rows padded with empty blocks to square shape
6. Result is systematic (original data first, parity appended)

**Decoding Capability**:
- With any K rows (partial or complete)
- Or any K columns (even with M missing blocks per column)
- Or any combination maintaining K total blocks

**EncodingParams**:
```nim
ecK: Natural             # Data blocks
ecM: Natural             # Parity blocks  
rounded: Natural         # Padded to square
steps: Natural           # Processing steps
blocksCount: Natural     # Total blocks
strategy: StrategyType   # Indexing strategy
```

**Backend Abstraction**:
- `EncoderProvider` proc creates encoder for (size, blocks, parity)
- `DecoderProvider` proc creates decoder for (size, blocks, parity)
- Allows pluggable codec backends

---

## 5. Merkle Tree Verification

### 5.1 Generic Merkle Tree

**MerkleTree[H, K]** (`merkletree/merkletree.nim`):

Generic over hash type H and key type K:

```nim
MerkleTree[H, K] = ref object
  layers: seq[seq[H]]        # Tree layers (bottom to top)
  compress: CompressFn[H, K] # Hash compression function
  zero: H                    # Zero/empty node value

MerkleProof[H, K] = ref object
  index: int                 # Leaf index
  path: seq[H]              # Path from leaf to root
  nleaves: int              # Tree size
  compress: CompressFn[H, K]
  zero: H
```

**Operations**:
- `getProof(index)` - Generate merkle proof for leaf
- `reconstructRoot(proof, leaf)` - Verify proof
- `root()` - Get merkle root

### 5.2 Poseidon2 Hash Integration

**Poseidon2** (`merkletree/poseidon2.nim`):
- Field-arithmetic hash function used in proofs
- Archivist-specific merkle tree implementation
- Supports zk-proof generation

---

## 6. Block Chunking

### 6.1 Chunker System

**Chunker** (`chunker.nim`):

```nim
Chunker = ref object
  reader: Reader              # Data source
  offset: int                 # Bytes read
  chunkSize: NBytes           # Block size (default 64 KiB)
  pad: bool                   # Pad last chunk?

Reader = proc(data: ChunkBuffer, len: int): Future[int]
```

**Reader Implementations**:
- **FileChunker**: Read from file handle
- **LPStreamChunker**: Read from libp2p stream
- Both support configurable chunk size and padding

---

## 7. Slot & Proof System

### 7.1 Slot Structure

**Slot Types** (`slots/types.nim`):

```nim
Sample[H] = object
  cellData: seq[H]
  merklePaths: seq[H]

PublicInputs[H] = object
  slotIndex: int
  datasetRoot: H
  entropy: H

ProofInputs[H] = object
  entropy: H
  datasetRoot: H
  slotIndex: Natural
  slotRoot: H
  nCellsPerSlot: Natural
  nSlotsPerDataSet: Natural
  slotProof: seq[H]          # Inclusion of slot in dataset
  samples: seq[Sample[H]]    # Cell inclusions in slot
```

### 7.2 Indexing Strategies

**StrategyType** (`indexingstrategy.nim`):

**LinearStrategy**:
```
Iteration 0 -> indices [0, 1, 2]
Iteration 1 -> indices [3, 4, 5]
Iteration 2 -> indices [6, 7, 8]
```

**SteppedStrategy**:
```
Iteration 0 -> indices [0, 3, 6]
Iteration 1 -> indices [1, 4, 7]
Iteration 2 -> indices [2, 5, 8]
```

Both support:
- `groupCount` - Number of groups
- `padBlockCount` - Padding per group
- Iterator-based index generation

---

## 8. Marketplace Integration

### 8.1 Smart Contract Interactions

**Contracts** module:
- **ClientInteractions**: Purchasing interface
  - Initiates storage requests
  - Monitors request fulfillment
  
- **HostInteractions**: Sales interface
  - Accepts storage requests
  - Manages slot allocation
  - Handles proof submissions
  
- **ValidatorInteractions**: Proof validation
  - Validates storage proofs
  - Challenges failed proofs

### 8.2 Sales Agent State Machine

**SalesAgent** (`sales/salesagent.nim`):

State transitions for storage request fulfillment:
```
Idle -> Available -> Submitted -> Accepted -> Filled -> Finished
                              -> Cancelled
```

**SalesData**:
```nim
requestId: RequestId
slotIndex: uint64
request: ?StorageRequest
slotQueueItem: ?SlotQueueItem
```

### 8.3 Purchase State Machine

**Purchase** (`purchasing/purchase.nim`):

```
Pending -> Submitted -> Started -> Finished
                   \              /
                    -> Cancelled
                             \
                       Unknown (load from chain)
                             /
                         -> Failed
```

---

## 9. Configuration System

### 9.1 NodeConf Structure

**Configuration Priority**: CLI args > Env vars > Config file

**Key Parameters** (`conf.nim`):

```nim
# Network
listenAddrs: seq[MultiAddress]        # Default: /ip4/0.0.0.0/tcp/0
nat: NatConfig                        # NAT traversal
discoveryPort: Port                   # DiscV5 port

# Storage
dataDir: OutDir                       # Data directory
repoKind: RepoKind                    # fs|sqlite|leveldb
quotaBytes: NBytes                    # Storage quota

# Marketplace (optional)
persistence: bool                     # Enable on-chain features
ethProvider: string                   # Ethereum RPC URL
ethAccount: ?EthAddress              # Account address
ethPrivateKey: ?InputFile             # Private key file
marketplaceAddress: ?EthAddress       # Marketplace contract
validator: bool                       # Enable proof validation

# Performance
threads: ThreadCount                  # Async worker threads
metricsEnabled: bool                  # Prometheus metrics
```

### 9.2 Block Defaults

```nim
DefaultBlockSize = 64 KiB
DefaultCellSize = 2 KiB

DefaultMaxSlotDepth = 32
DefaultMaxDatasetDepth = 8
DefaultBlockDepth = 5
DefaultCellElms = 67
DefaultSamplesNum = 5

DefaultQuotaBytes = 1 GiB
DefaultBlockTtl = 7 days
DefaultBlockInterval = 256 blocks
DefaultNumBlocksPerInterval = 1
```

---

## 10. REST API

### 10.1 API Endpoints

**Base paths**:
```
/api/v1/            - Main API
/metrics            - Prometheus metrics
/health             - Health check
```

**Core Operations**:
- **Upload**: `POST /upload` - Store content
- **Download**: `GET /download/:cid` - Retrieve by CID
- **Manifests**: `GET /manifests` - List stored manifests
- **Status**: `GET /status` - Node information

**Metrics**:
- `archivist_api_uploads` - Upload count
- `archivist_api_downloads` - Download count

### 10.2 OpenAPI Specification

Complete OpenAPI 3.0.3 spec at `openapi.yaml`:
- Schema definitions for CID, MultiAddress, PeerId, etc.
- Endpoint documentation
- Request/response models

---

## 11. Key Data Formats

### 11.1 Size Units

**NBytes** - Typed size representation:
```nim
KiB = 1,024 bytes
MiB = 1,048,576 bytes
GiB = 1,073,741,824 bytes

DefaultBlockSize = 64 KiB
DefaultCellSize = 2 KiB
```

### 11.2 Time Handling

**Clock Interface**:
- **SystemClock**: Real time
- **OnChainClock**: Ethereum block time
- `SecondsSince1970` timestamps

---

## 12. Error Handling

**Error Hierarchy**:
```nim
ArchivistError (base)
├── BlockNotFoundError
├── ErasureError
│   └── InsufficientBlocksError
├── IndexingError
│   ├── IndexingWrongIndexError
│   ├── IndexingWrongIterationsError
│   ├── IndexingWrongGroupCountError
│   └── IndexingWrongPadBlockCountError
└── [Protocol-specific errors]
```

**Error Handling Pattern**:
```nim
Result[T, E] type (using pkg/results)
Operators: ?!, without, ?, success, failure
```

---

## 13. Concurrency Model

### 13.1 Async/Await

- **Runtime**: Chronos (async framework)
- **Task Pool**: TaskPool for parallel encoding/decoding
- **Tracked Futures**: TrackedFutures for lifecycle management

### 13.2 Message Queue

**BlockExcEngine**:
- AsyncHeapQueue for peer task scheduling
- Concurrent task processing (default 10 workers)
- Semaphore-controlled inflight requests

---

## 14. Logging & Monitoring

### 14.1 Structured Logging

- **Framework**: Chronicles
- **Topics**: Topic-based filtering
- **Formats**: Text, JSON, structured

### 14.2 Metrics

**Prometheus Integration**:
- Block exchange counters
- API metrics
- Proof generation metrics

---

## 15. Implementation Dependencies

### 15.1 Nim Packages (key)
- `libp2p` - P2P networking
- `chronos` - Async runtime
- `taskpools` - Thread pool
- `chronicles` - Logging
- `presto` - REST server
- `ethers` - Ethereum client
- `nitro` - State channels
- `poseidon2` - Hash function
- `datastore` - Storage backends
- `questionable` - Result types
- `stint` - BigInt support
- `libp2p/protobuf` - Protobuf

### 15.2 Build Requirements
- Nim 2.2.4
- CMake 3.x
- Rust 1.79.0 (for some dependencies)
- Node.js 22.x (optional, for tests)

---

## 16. File Organization

```
archivist-node/
├── archivist/
│   ├── archivist.nim              # Entry point
│   ├── node.nim                   # Node orchestration
│   ├── conf.nim                   # Configuration
│   ├── blocktype.nim              # Block/CID definitions
│   ├── manifest/                  # Manifest encoding
│   ├── blockexchange/             # Block exchange engine
│   │   ├── engine/                # Main exchange logic
│   │   ├── network/               # Network layer
│   │   ├── peers/                 # Peer management
│   │   └── protobuf/              # Message definitions
│   ├── stores/                    # Block storage
│   ├── erasure/                   # Erasure coding
│   ├── merkletree/                # Merkle trees
│   ├── slots/                     # Slot system
│   ├── contracts/                 # Smart contract interactions
│   ├── sales/                     # Sales agent
│   ├── purchasing/                # Purchase state machine
│   ├── rest/                      # REST API
│   ├── discovery.nim              # DiscV5
│   └── [other modules]
├── openapi.yaml                   # API specification
├── tests/                         # Test suite
├── benchmarks/                    # Performance benchmarks
└── docker/                        # Docker configuration
```

---

## Summary: Architecture Patterns

**Systematic Design**:
1. **Modular Protocol**: Extended IPFS BitSwap with micropayment extensions
2. **Flexible Storage**: Abstract BlockStore with multiple backends
3. **Proof-Ready**: Merkle tree + erasure code for verifiable storage
4. **Market Integration**: Full smart contract lifecycle
5. **Async-First**: Chronos-based concurrent operations
6. **Observable**: Prometheus metrics + structured logging
7. **Generics**: Generic merkle trees and hash functions
8. **State Machines**: Explicit sale/purchase lifecycle

**For Rust Implementation**:
- Replicate BlockExc protocol with libp2p-request-response
- Implement manifest codec/decoder using protobuf-rs
- Use generic traits for merkle trees (similar pattern to Nim generics)
- Abstract storage via trait objects
- Use tokio for async/concurrency
- Follow same module organization
