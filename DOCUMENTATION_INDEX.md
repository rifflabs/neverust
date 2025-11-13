# Archivist Node Exploration - Documentation Index

## Overview

Complete architectural and protocol exploration of the **Archivist Node** - a Nim-based decentralized durability engine implementing block exchange protocols, erasure coding, merkle trees, and smart contract marketplace integration.

## Document Guide

### 1. EXPLORATION_SUMMARY.md (Start Here!)
**360 lines | Quick Overview**

Best starting point for understanding what Archivist Node is and what was discovered.

**Contents**:
- Executive summary of findings
- All 15 key architectural components
- Technology stack recommendations for Rust port
- Directory structure overview
- Critical implementation details
- Next steps for Rust implementation

**Read this first to get the big picture.**

---

### 2. ARCHIVIST_NODE_ARCHITECTURE.md (Deep Dive)
**748 lines | Comprehensive Reference**

Complete architectural documentation covering every aspect of the system.

**Contents**:
1. Network Stack & Protocol Implementation
   - Block Exchange Protocol (extended BitSwap)
   - libp2p integration details
   - DiscV5 discovery layer
   - Network peer management

2. Main Components & Modules
   - Core node architecture
   - BlockExchange engine
   - BlockStore abstraction
   - Manifest system

3. Block Types & Addressing
   - CID system and codecs
   - Block and BlockAddress types

4. Data Formats & Encodings
   - Protobuf definitions
   - JSON/REST encoding
   - CID structure

5. Erasure Coding System
   - Module architecture
   - Encoding/decoding process
   - Backend abstraction

6. Merkle Tree Verification
   - Generic tree implementation
   - Poseidon2 hash integration

7. Block Chunking
   - Chunker system
   - Reader implementations

8. Slot & Proof System
   - Slot structure
   - Indexing strategies (Linear, Stepped)

9. Marketplace Integration
   - Smart contract interactions
   - Sales agent state machine
   - Purchase state machine

10. Configuration System
    - NodeConf structure
    - Block defaults
    - All configuration parameters

11. REST API
    - API endpoints
    - OpenAPI specification

12. Key Data Formats
    - Size units (NBytes)
    - Time handling (Clock interface)

13. Error Handling
    - Error hierarchy
    - Pattern usage

14. Concurrency Model
    - Async/await with Chronos
    - Message queue and scheduling

15. Logging & Monitoring
    - Structured logging
    - Prometheus metrics

16. Implementation Dependencies
    - Nim packages
    - Build requirements

17. File Organization
    - Complete directory structure

18. Architecture Patterns
    - Systematic design principles
    - Patterns for Rust implementation

**Use this for detailed implementation reference.**

---

### 3. PROTOCOL_SPECIFICATION_QUICK_REF.md (Implementation Guide)
**326 lines | Quick Reference**

Concise protocol specification and implementation checklist.

**Contents**:
- Block Exchange Protocol details (Protobuf structure)
- CID system specification
- Manifest format (storage, encoding, fields)
- Block exchange flow explanation
- Micropayment system
- Merkle tree operations
- Erasure coding layout
- Indexing strategies
- BlockStore interface
- Configuration parameters
- REST API endpoints
- Discovery mapping
- Block addressing types
- Key constants (sizes, parameters, timeouts)
- Error types
- Concurrency limits
- Marketplace state machines
- Rust implementation checklist

**Use this for quick lookup during implementation.**

---

## Quick Navigation by Topic

### Protocol & Networking
- **Block Exchange**: EXPLORATION_SUMMARY.md §2, ARCHIVIST_NODE_ARCHITECTURE.md §1.1, PROTOCOL_SPECIFICATION_QUICK_REF.md (start)
- **Discovery**: ARCHIVIST_NODE_ARCHITECTURE.md §1.3, PROTOCOL_SPECIFICATION_QUICK_REF.md (Discovery section)
- **Peer Management**: ARCHIVIST_NODE_ARCHITECTURE.md §1.4

### Storage & Data
- **BlockStore Interface**: ARCHIVIST_NODE_ARCHITECTURE.md §2.3, PROTOCOL_SPECIFICATION_QUICK_REF.md (BlockStore section)
- **Manifest System**: ARCHIVIST_NODE_ARCHITECTURE.md §2.4, PROTOCOL_SPECIFICATION_QUICK_REF.md (Manifest Format)
- **CID System**: ARCHIVIST_NODE_ARCHITECTURE.md §2.5 & §3.1, PROTOCOL_SPECIFICATION_QUICK_REF.md (CID section)

### Erasure Coding & Proofs
- **Erasure Coding**: ARCHIVIST_NODE_ARCHITECTURE.md §4, PROTOCOL_SPECIFICATION_QUICK_REF.md (Erasure Coding)
- **Merkle Trees**: ARCHIVIST_NODE_ARCHITECTURE.md §5, PROTOCOL_SPECIFICATION_QUICK_REF.md (Merkle Tree)
- **Slots & Proofs**: ARCHIVIST_NODE_ARCHITECTURE.md §7, PROTOCOL_SPECIFICATION_QUICK_REF.md (Indexing Strategies)

### Configuration & API
- **Configuration**: ARCHIVIST_NODE_ARCHITECTURE.md §9, PROTOCOL_SPECIFICATION_QUICK_REF.md (Configuration section)
- **REST API**: ARCHIVIST_NODE_ARCHITECTURE.md §10, PROTOCOL_SPECIFICATION_QUICK_REF.md (REST API section)

### Implementation
- **Error Handling**: ARCHIVIST_NODE_ARCHITECTURE.md §12
- **Concurrency**: ARCHIVIST_NODE_ARCHITECTURE.md §13
- **Architecture Patterns**: ARCHIVIST_NODE_ARCHITECTURE.md §16-17
- **Rust Port Checklist**: PROTOCOL_SPECIFICATION_QUICK_REF.md (end)

---

## Key Discoveries

### 1. Not Rust - It's Nim!
The Archivist Node is written in **Nim 2.2.4**, not Rust. This is important for understanding the architecture and patterns.

### 2. Extended BitSwap Protocol
Core protocol is an **extended IPFS BitSwap** with:
- Micropayment support (UInt256 prices per block)
- State channel settlement (Nitro)
- Proof system integration

### 3. Complex Data Structures
- **Manifest**: Nested structure supporting basic metadata, erasure coding, and verifiable proofs
- **BlockAddress**: Flexible addressing (direct CID or tree+index)
- **MerkleTree**: Generic over hash type and key type

### 4. Modular Architecture
- Abstract interfaces with multiple implementations
- Pluggable storage backends (FS, SQLite, LevelDB)
- Generic erasure coding backend support

### 5. Full Marketplace Integration
- Smart contract interactions (Client, Host, Validator)
- State machines for Sales and Purchase lifecycle
- Payment via Ethereum state channels

### 6. Discovery-Ready
- DiscV5-based peer discovery
- Keccak256 node ID mapping from CIDs and Ethereum addresses
- Provider records and peer lookup

---

## For Rust Implementation

### Essential Reads
1. Start: EXPLORATION_SUMMARY.md - Get the overview
2. Reference: ARCHIVIST_NODE_ARCHITECTURE.md - Detailed specifications
3. Checklist: PROTOCOL_SPECIFICATION_QUICK_REF.md - Implementation tasks

### Critical Implementation Details
- Protocol message structure (Protobuf)
- CID generation (CIDv1, codec 0xcd02, SHA256)
- Manifest format (Protobuf + DAG-PB)
- BlockStore trait design
- MerkleTree generics pattern
- Erasure encoding K+M systematic layout
- Discovery mapping (Keccak256)
- State machines for Sales/Purchase

### Technology Stack
- **P2P**: libp2p with RequestResponse
- **Serialization**: protobuf-rs, DAG-PB
- **Async**: tokio
- **Hashing**: sha2, poseidon2-rs
- **Storage**: trait objects
- **REST**: actix-web or axum
- **Metrics**: prometheus crate
- **Contracts**: ethers-rs

---

## File Locations in Repository

```
/mnt/castle/workspace/neverust/
├── EXPLORATION_SUMMARY.md                  # START HERE
├── ARCHIVIST_NODE_ARCHITECTURE.md         # Reference
├── PROTOCOL_SPECIFICATION_QUICK_REF.md    # Checklist
├── DOCUMENTATION_INDEX.md                 # This file
│
└── archivist-node/                        # Original source
    ├── archivist/
    │   ├── blockexchange/
    │   ├── stores/
    │   ├── erasure/
    │   ├── merkletree/
    │   ├── manifest/
    │   ├── slots/
    │   ├── contracts/
    │   ├── rest/
    │   └── discovery.nim
    ├── openapi.yaml
    └── [tests, benchmarks, docker]
```

---

## Documentation Statistics

| Document | Lines | Size | Focus |
|----------|-------|------|-------|
| EXPLORATION_SUMMARY.md | 360 | 11K | Overview & Big Picture |
| ARCHIVIST_NODE_ARCHITECTURE.md | 748 | 21K | Deep Dive & Reference |
| PROTOCOL_SPECIFICATION_QUICK_REF.md | 326 | 7.5K | Implementation Guide |
| **Total** | **1,434** | **39.5K** | Complete Coverage |

---

## Recommended Reading Order

1. **5 min**: Read EXPLORATION_SUMMARY.md for context
2. **20 min**: Skim PROTOCOL_SPECIFICATION_QUICK_REF.md for protocol overview
3. **30 min**: Deep dive into relevant sections of ARCHIVIST_NODE_ARCHITECTURE.md
4. **Reference**: Use PROTOCOL_SPECIFICATION_QUICK_REF.md during implementation

---

## Key Contacts & References

**Source Code**: `/mnt/castle/workspace/neverust/archivist-node/`

**Main Entry Point**: `archivist/archivist.nim`

**Protocol Definition**: `archivist/blockexchange/protobuf/message.proto`

**API Documentation**: `openapi.yaml`

**Node Orchestration**: `archivist/node.nim`

---

**Generated**: November 13, 2025
**Status**: Complete exploration and documentation
**Ready for**: Rust implementation planning

