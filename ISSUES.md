# Neverust Development Issues

**Total Issues**: 150 (NEVAR-1 through NEVAR-150)
**Status**: Generated on 2025-10-03
**Project**: Archivist Storage Node in Rust

---

## Plane.so Relationship Types Reference

Based on makeplane/plane source code:

**Primary Types (stored in database)**:
- `duplicate` - Issue is a duplicate of another
- `relates_to` - General relationship, shared context
- `blocked_by` - Cannot proceed until other issue is complete
- `start_before` - Must start before another issue starts
- `finish_before` - Must finish before another issue finishes

**Inverse Types (computed for display)**:
- `blocking` - Inverse of `blocked_by`
- `start_after` - Inverse of `start_before`
- `finish_after` - Inverse of `finish_before`

---

## Issue Tree - ASCII Dependency Visualization

```
PHASE 0: HELLO P2P (Tonight - 2-4 hours)
=========================================

NEVAR-1 [Generate Issue Tracking]
  |
  +--blocked_by--> (none - starting point)
  |
  +--blocking--> NEVAR-2, NEVAR-3, NEVAR-4, NEVAR-7, NEVAR-8, NEVAR-9, NEVAR-10
  |
  v
NEVAR-2 [Cargo Workspace Setup]
  |
  +--blocked_by--> NEVAR-1
  +--blocking--> NEVAR-3, NEVAR-4
  +--start_before--> NEVAR-3
  |
  v
NEVAR-3 [CLI Framework + Config] ----+
  |                                  |
  +--blocked_by--> NEVAR-2           |
  +--blocking--> NEVAR-5             |
  +--relates_to--> NEVAR-8           |
  +--start_before--> NEVAR-4         |
  |                                  |
  v                                  |
NEVAR-4 [P2P Core - Swarm] <---------+
  |
  +--blocked_by--> NEVAR-2
  +--blocking--> NEVAR-5, NEVAR-6
  +--relates_to--> NEVAR-11 (Kademlia in Phase 1)
  +--finish_before--> NEVAR-5
  |
  v
NEVAR-5 [Event Loop + Async Runtime]
  |
  +--blocked_by--> NEVAR-3, NEVAR-4
  +--blocking--> NEVAR-6
  +--relates_to--> NEVAR-7
  +--finish_before--> NEVAR-6
  |
  v
NEVAR-6 [Integration Test - 2 Nodes Ping]
  |
  +--blocked_by--> NEVAR-4, NEVAR-5
  +--blocking--> Phase 0 Complete
  |
  v
NEVAR-7 [Structured Logging (tracing)]
  |
  +--blocked_by--> NEVAR-1
  +--relates_to--> NEVAR-5
  +--start_after--> NEVAR-2
  |
NEVAR-8 [Config File Loading (TOML)]
  |
  +--blocked_by--> NEVAR-1
  +--relates_to--> NEVAR-3
  +--start_after--> NEVAR-3
  |
NEVAR-9 [Error Handling (thiserror)]
  |
  +--blocked_by--> NEVAR-1
  +--relates_to--> ALL
  +--start_after--> NEVAR-2
  |
NEVAR-10 [Documentation + Examples]
  |
  +--blocked_by--> NEVAR-1
  +--blocked_by--> NEVAR-6 (Phase 0 complete)
  +--finish_after--> NEVAR-6


PHASE 1: STORAGE NODE (Days 1-2)
=================================

NEVAR-11 [Kademlia DHT Integration]
  |
  +--blocked_by--> NEVAR-6 (Phase 0 complete)
  +--blocking--> NEVAR-12, NEVAR-13
  +--relates_to--> NEVAR-4
  |
  v
NEVAR-12 [Content Addressing (CID)]
  |
  +--blocked_by--> NEVAR-11
  +--blocking--> NEVAR-13, NEVAR-14
  +--relates_to--> NEVAR-61 (erasure coding)
  |
  v
NEVAR-13 [In-Memory Block Storage]
  |
  +--blocked_by--> NEVAR-11, NEVAR-12
  +--blocking--> NEVAR-14, NEVAR-15
  +--finish_before--> NEVAR-31 (persistent storage)
  |
  v
NEVAR-14 [REST API Framework (axum)]
  |
  +--blocked_by--> NEVAR-12, NEVAR-13
  +--blocking--> NEVAR-15, NEVAR-16, NEVAR-17
  |
  v
NEVAR-15 [POST /api/v1/storage/store]
  |
  +--blocked_by--> NEVAR-13, NEVAR-14
  +--relates_to--> NEVAR-16
  |
NEVAR-16 [GET /api/v1/storage/retrieve/:cid]
  |
  +--blocked_by--> NEVAR-13, NEVAR-14
  +--relates_to--> NEVAR-15
  |
NEVAR-17 [GET /health Endpoint]
  |
  +--blocked_by--> NEVAR-14
  +--blocking--> NEVAR-18
  +--relates_to--> NEVAR-41 (Prometheus health)
  |
NEVAR-18 [Prometheus Metrics Setup]
  |
  +--blocked_by--> NEVAR-17
  +--blocking--> NEVAR-19, NEVAR-20
  +--relates_to--> NEVAR-41
  |
  v
NEVAR-19 [Metric: peer_count]
  |
  +--blocked_by--> NEVAR-18
  +--relates_to--> NEVAR-11
  |
NEVAR-20 [Metric: blocks_stored]
  |
  +--blocked_by--> NEVAR-18
  +--relates_to--> NEVAR-13
  |
NEVAR-21 to NEVAR-30 [Additional Storage Features]
  +-- Request validation, error responses, API tests, etc.


PHASE 2: NETWORK NODE (Days 3-7)
=================================

NEVAR-31 [Persistent Storage Backend]
  |
  +--blocked_by--> NEVAR-13 (in-memory working)
  +--blocking--> NEVAR-32
  +--relates_to--> NEVAR-33
  |
  v
NEVAR-32 [RocksDB Integration]
  |
  +--blocked_by--> NEVAR-31
  +--duplicate--> NEVAR-33 (choose one)
  |
NEVAR-33 [Sled Integration]
  |
  +--blocked_by--> NEVAR-31
  +--duplicate--> NEVAR-32 (choose one)
  |
  v
NEVAR-34 [Request-Response Protocol]
  |
  +--blocked_by--> NEVAR-31
  +--blocking--> NEVAR-35, NEVAR-36
  +--relates_to--> NEVAR-11
  |
  v
NEVAR-35 [Block Exchange Protocol]
  |
  +--blocked_by--> NEVAR-34
  +--blocking--> NEVAR-36
  |
  v
NEVAR-36 [Block Request/Response Handlers]
  |
  +--blocked_by--> NEVAR-35
  +--relates_to--> NEVAR-13, NEVAR-16
  |
NEVAR-37 [Bootstrap Node Configuration]
  |
  +--blocked_by--> NEVAR-11
  +--relates_to--> NEVAR-8
  |
NEVAR-38 [NAT Traversal (libp2p-relay)]
  |
  +--blocked_by--> NEVAR-11
  +--relates_to--> NEVAR-37
  |
NEVAR-39 [Docker Deployment]
  |
  +--blocked_by--> NEVAR-6 (working node)
  +--blocking--> NEVAR-40
  +--relates_to--> NEVAR-50
  |
  v
NEVAR-40 [docker-compose.yml]
  |
  +--blocked_by--> NEVAR-39
  +--relates_to--> NEVAR-41
  |
NEVAR-41 [GET /metrics Prometheus Endpoint]
  |
  +--blocked_by--> NEVAR-18
  +--relates_to--> NEVAR-42
  |
  v
NEVAR-42 [Grafana Dashboard JSON]
  |
  +--blocked_by--> NEVAR-41
  +--relates_to--> NEVAR-40
  |
NEVAR-43 to NEVAR-60 [Network Features]
  +-- Multi-node testing, performance metrics, monitoring


PHASE 3: DURABILITY NODE (Weeks 2-3)
=====================================

NEVAR-61 [Erasure Coding Setup]
  |
  +--blocked_by--> NEVAR-36 (block exchange working)
  +--blocking--> NEVAR-62, NEVAR-63
  +--relates_to--> NEVAR-12
  |
  v
NEVAR-62 [Reed-Solomon Encoding]
  |
  +--blocked_by--> NEVAR-61
  +--blocking--> NEVAR-64
  |
  v
NEVAR-63 [Reed-Solomon Decoding]
  |
  +--blocked_by--> NEVAR-61
  +--blocking--> NEVAR-64
  |
  v
NEVAR-64 [Slot-Based Storage]
  |
  +--blocked_by--> NEVAR-62, NEVAR-63
  +--blocking--> NEVAR-65
  |
  v
NEVAR-65 [Manifest Generation]
  |
  +--blocked_by--> NEVAR-64
  +--blocking--> NEVAR-66
  +--relates_to--> NEVAR-12
  |
  v
NEVAR-66 [Merkle Tree (SHA256 + Poseidon2)]
  |
  +--blocked_by--> NEVAR-65
  +--blocking--> NEVAR-67
  +--relates_to--> NEVAR-101 (ZK proofs)
  |
  v
NEVAR-67 [Content Advertisement (DHT)]
  |
  +--blocked_by--> NEVAR-66
  +--blocking--> NEVAR-68
  +--relates_to--> NEVAR-11
  |
  v
NEVAR-68 [Lazy Repair Mechanism]
  |
  +--blocked_by--> NEVAR-67
  +--blocking--> NEVAR-69
  |
  v
NEVAR-69 [Missing Slot Detection]
  |
  +--blocked_by--> NEVAR-68
  +--blocking--> NEVAR-70
  |
  v
NEVAR-70 [Slot Reconstruction]
  |
  +--blocked_by--> NEVAR-69
  +--relates_to--> NEVAR-63
  |
NEVAR-71 [Performance Benchmarks (criterion)]
  |
  +--blocked_by--> NEVAR-36
  +--relates_to--> NEVAR-72, NEVAR-73
  |
  v
NEVAR-72 [Benchmark: dial_ms (p50/p95/p99)]
  |
  +--blocked_by--> NEVAR-71
  |
NEVAR-73 [Benchmark: content_fetch_ms]
  |
  +--blocked_by--> NEVAR-71
  +--relates_to--> NEVAR-36
  |
NEVAR-74 to NEVAR-100 [Durability Features]
  +-- Multi-node integration tests, fault tolerance, data recovery


PHASE 4: MARKETPLACE NODE (Week 4+)
====================================

NEVAR-101 [ZK Proof Setup (ark-groth16)]
  |
  +--blocked_by--> NEVAR-66 (Merkle trees)
  +--blocking--> NEVAR-102, NEVAR-103
  |
  v
NEVAR-102 [Circom Circuit Integration]
  |
  +--blocked_by--> NEVAR-101
  +--blocking--> NEVAR-103
  |
  v
NEVAR-103 [Proof Generation Loop]
  |
  +--blocked_by--> NEVAR-102
  +--blocking--> NEVAR-104
  |
  v
NEVAR-104 [Proof Verification]
  |
  +--blocked_by--> NEVAR-103
  +--relates_to--> NEVAR-111
  |
NEVAR-105 [Smart Contract Integration (ethers-rs)]
  |
  +--blocked_by--> NEVAR-70 (durability working)
  +--blocking--> NEVAR-106, NEVAR-107
  |
  v
NEVAR-106 [Marketplace Contract Interaction]
  |
  +--blocked_by--> NEVAR-105
  +--blocking--> NEVAR-108, NEVAR-109
  |
  v
NEVAR-107 [Storage Contract Interaction]
  |
  +--blocked_by--> NEVAR-105
  +--blocking--> NEVAR-110
  |
  v
NEVAR-108 [Slot Purchase Logic]
  |
  +--blocked_by--> NEVAR-106
  +--relates_to--> NEVAR-64
  |
NEVAR-109 [Slot Sale Logic]
  |
  +--blocked_by--> NEVAR-106
  +--relates_to--> NEVAR-64
  |
NEVAR-110 [Proof Submission On-Chain]
  |
  +--blocked_by--> NEVAR-107
  +--relates_to--> NEVAR-103
  |
NEVAR-111 [Staking Collateral Management]
  |
  +--blocked_by--> NEVAR-106
  +--blocking--> NEVAR-112
  |
  v
NEVAR-112 [Slashing Logic]
  |
  +--blocked_by--> NEVAR-111
  +--relates_to--> NEVAR-104
  |
NEVAR-113 [Marketplace API Endpoints]
  |
  +--blocked_by--> NEVAR-106
  +--blocking--> NEVAR-114, NEVAR-115
  +--relates_to--> NEVAR-14
  |
  v
NEVAR-114 [POST /api/v1/marketplace/list]
  |
  +--blocked_by--> NEVAR-113
  |
NEVAR-115 [POST /api/v1/marketplace/buy]
  |
  +--blocked_by--> NEVAR-113
  |
NEVAR-116 to NEVAR-140 [Marketplace Features]
  +-- Economic incentives, collateral management, market discovery


PHASE 5: PHOENIX TESTING (Week 5+)
===================================

NEVAR-141 [Phoenix Phase 1: Evaluate]
  |
  +--blocked_by--> NEVAR-6 (basic node working)
  +--blocking--> NEVAR-142
  |
  v
NEVAR-142 [Phoenix Phase 2: Playthrough - Multi-Device Testing]
  |
  +--blocked_by--> NEVAR-141
  +--blocking--> NEVAR-143
  |
  v
NEVAR-143 [Playwright Test Suite - 12 Device Profiles]
  |
  +--blocked_by--> NEVAR-142
  +--blocking--> NEVAR-144
  |
  v
NEVAR-144 [Phoenix Phase 3: Record - Screen Recordings]
  |
  +--blocked_by--> NEVAR-143
  +--blocking--> NEVAR-145
  |
  v
NEVAR-145 [Director's Report (GPT-5 via Zen MCP)]
  |
  +--blocked_by--> NEVAR-144
  +--relates_to--> NEVAR-146
  |
NEVAR-146 [Features Report (GPT-5 via Zen MCP)]
  |
  +--blocked_by--> NEVAR-144
  +--relates_to--> NEVAR-145
  |
NEVAR-147 [Phoenix Phase 4: Suggest - UX Improvements]
  |
  +--blocked_by--> NEVAR-145, NEVAR-146
  +--blocking--> NEVAR-148
  |
  v
NEVAR-148 [Implementation Roadmap Report]
  |
  +--blocked_by--> NEVAR-147
  |
NEVAR-149 [Phoenix Phase 5: Build - Production Readiness]
  |
  +--blocked_by--> NEVAR-148
  +--blocking--> NEVAR-150
  |
  v
NEVAR-150 [Production Deployment + Launch]
  |
  +--blocked_by--> NEVAR-149
  +-- FINAL DELIVERABLE
```

---

## Issues by Phase

### Phase 0: Hello P2P (NEVAR-1 to NEVAR-10) - TONIGHT

| Issue | Title | Priority | Status | Blocked By | Blocking |
|-------|-------|----------|--------|------------|----------|
| NEVAR-1 | Generate Issue Tracking Structure | Critical | Todo | - | 2,3,4,7,8,9,10 |
| NEVAR-2 | Set up Cargo Workspace | Critical | Todo | 1 | 3,4 |
| NEVAR-3 | Implement CLI Framework (clap) | Critical | Todo | 2 | 5 |
| NEVAR-4 | Build P2P Swarm (libp2p) | Critical | Todo | 2 | 5,6 |
| NEVAR-5 | Create Event Loop + Async Runtime | Critical | Todo | 3,4 | 6 |
| NEVAR-6 | Write Integration Test (2 Nodes Ping) | Critical | Todo | 4,5 | Phase 1 |
| NEVAR-7 | Add Structured Logging (tracing) | High | Todo | 1 | - |
| NEVAR-8 | Implement Config File Loading (TOML) | High | Todo | 1 | - |
| NEVAR-9 | Add Error Handling (thiserror) | High | Todo | 1 | - |
| NEVAR-10 | Write Documentation + Examples | Medium | Todo | 1,6 | - |

### Phase 1: Storage Node (NEVAR-11 to NEVAR-30) - Days 1-2

| Issue | Title | Priority | Blocked By |
|-------|-------|----------|------------|
| NEVAR-11 | Integrate Kademlia DHT | Critical | 6 |
| NEVAR-12 | Implement Content Addressing (CID) | Critical | 11 |
| NEVAR-13 | Build In-Memory Block Storage | Critical | 11,12 |
| NEVAR-14 | Set up REST API Framework (axum) | Critical | 12,13 |
| NEVAR-15 | Implement POST /api/v1/storage/store | Critical | 13,14 |
| NEVAR-16 | Implement GET /api/v1/storage/retrieve/:cid | Critical | 13,14 |
| NEVAR-17 | Implement GET /health Endpoint | High | 14 |
| NEVAR-18 | Set up Prometheus Metrics | High | 17 |
| NEVAR-19 | Add Metric: peer_count | High | 18 |
| NEVAR-20 | Add Metric: blocks_stored | High | 18 |
| NEVAR-21-30 | Additional storage features, tests, validation | Medium-Low | Various |

### Phase 2: Network Node (NEVAR-31 to NEVAR-60) - Days 3-7

| Issue | Title | Priority | Blocked By |
|-------|-------|----------|------------|
| NEVAR-31 | Design Persistent Storage Backend | Critical | 13 |
| NEVAR-32 | Integrate RocksDB | Critical | 31 |
| NEVAR-33 | Integrate Sled (alternative) | Critical | 31 |
| NEVAR-34 | Implement Request-Response Protocol | Critical | 31 |
| NEVAR-35 | Build Block Exchange Protocol | Critical | 34 |
| NEVAR-36 | Implement Block Request/Response Handlers | Critical | 35 |
| NEVAR-37 | Configure Bootstrap Nodes | High | 11 |
| NEVAR-38 | Implement NAT Traversal | High | 11 |
| NEVAR-39 | Create Docker Deployment | High | 6 |
| NEVAR-40 | Write docker-compose.yml | High | 39 |
| NEVAR-41 | Implement GET /metrics Endpoint | High | 18 |
| NEVAR-42 | Create Grafana Dashboard | High | 41 |
| NEVAR-43-60 | Network features, multi-node tests, monitoring | Medium-Low | Various |

### Phase 3: Durability Node (NEVAR-61 to NEVAR-100) - Weeks 2-3

| Issue | Title | Priority | Blocked By |
|-------|-------|----------|------------|
| NEVAR-61 | Set up Erasure Coding Framework | Critical | 36 |
| NEVAR-62 | Implement Reed-Solomon Encoding | Critical | 61 |
| NEVAR-63 | Implement Reed-Solomon Decoding | Critical | 61 |
| NEVAR-64 | Build Slot-Based Storage System | Critical | 62,63 |
| NEVAR-65 | Implement Manifest Generation | Critical | 64 |
| NEVAR-66 | Build Merkle Trees (SHA256 + Poseidon2) | Critical | 65 |
| NEVAR-67 | Implement Content Advertisement (DHT) | Critical | 66 |
| NEVAR-68 | Design Lazy Repair Mechanism | High | 67 |
| NEVAR-69 | Implement Missing Slot Detection | High | 68 |
| NEVAR-70 | Implement Slot Reconstruction | High | 69 |
| NEVAR-71 | Set up Performance Benchmarks (criterion) | High | 36 |
| NEVAR-72 | Benchmark: dial_ms (p50/p95/p99) | High | 71 |
| NEVAR-73 | Benchmark: content_fetch_ms | High | 71 |
| NEVAR-74-100 | Durability features, fault tolerance, recovery | Medium-Low | Various |

### Phase 4: Marketplace Node (NEVAR-101 to NEVAR-140) - Week 4+

| Issue | Title | Priority | Blocked By |
|-------|-------|----------|------------|
| NEVAR-101 | Set up ZK Proof System (ark-groth16) | Critical | 66 |
| NEVAR-102 | Integrate Circom Circuits | Critical | 101 |
| NEVAR-103 | Implement Proof Generation Loop | Critical | 102 |
| NEVAR-104 | Implement Proof Verification | Critical | 103 |
| NEVAR-105 | Set up Smart Contract Integration (ethers-rs) | Critical | 70 |
| NEVAR-106 | Integrate Marketplace Contract | Critical | 105 |
| NEVAR-107 | Integrate Storage Contract | Critical | 105 |
| NEVAR-108 | Implement Slot Purchase Logic | High | 106 |
| NEVAR-109 | Implement Slot Sale Logic | High | 106 |
| NEVAR-110 | Implement Proof Submission On-Chain | High | 107 |
| NEVAR-111 | Implement Staking Collateral Management | High | 106 |
| NEVAR-112 | Implement Slashing Logic | High | 111 |
| NEVAR-113 | Set up Marketplace API Endpoints | High | 106 |
| NEVAR-114 | Implement POST /api/v1/marketplace/list | High | 113 |
| NEVAR-115 | Implement POST /api/v1/marketplace/buy | High | 113 |
| NEVAR-116-140 | Marketplace features, economics, discovery | Medium-Low | Various |

### Phase 5: Phoenix Testing (NEVAR-141 to NEVAR-150) - Week 5+

| Issue | Title | Priority | Blocked By |
|-------|-------|----------|------------|
| NEVAR-141 | Phoenix Phase 1: Evaluate | Critical | 6 |
| NEVAR-142 | Phoenix Phase 2: Playthrough - Multi-Device | Critical | 141 |
| NEVAR-143 | Playwright Test Suite (12 Device Profiles) | Critical | 142 |
| NEVAR-144 | Phoenix Phase 3: Record - Screen Recordings | High | 143 |
| NEVAR-145 | Generate Director's Report (GPT-5/Zen MCP) | High | 144 |
| NEVAR-146 | Generate Features Report (GPT-5/Zen MCP) | High | 144 |
| NEVAR-147 | Phoenix Phase 4: Suggest - UX Improvements | High | 145,146 |
| NEVAR-148 | Generate Implementation Roadmap | High | 147 |
| NEVAR-149 | Phoenix Phase 5: Build - Production Ready | Critical | 148 |
| NEVAR-150 | Production Deployment + Launch | Critical | 149 |

---

## Relationship Matrix

Full relationship mapping exported to: `issues/relationships.csv`

Quick Reference:
- **Critical Path**: NEVAR-1 → 2 → 3/4 → 5 → 6 → 11 → ... → 150
- **Parallel Tracks**: Logging (7), Config (8), Errors (9) run alongside core
- **Major Milestones**:
  - NEVAR-6: Phase 0 Complete (Tonight)
  - NEVAR-30: Phase 1 Complete (Storage working)
  - NEVAR-60: Phase 2 Complete (Network operational)
  - NEVAR-100: Phase 3 Complete (Durability guaranteed)
  - NEVAR-140: Phase 4 Complete (Marketplace live)
  - NEVAR-150: PRODUCTION LAUNCH

---

## Usage

1. **Tonight**: Focus on NEVAR-1 through NEVAR-6
2. **Track Progress**: Update `issues/NEVAR-N.md` status as you go
3. **Respect Dependencies**: Check `blocked_by` before starting
4. **Commit Atomically**: One logical change per commit
5. **Test First**: Write tests before implementation (TDD)

Individual issue files in: `issues/NEVAR-N.md`
