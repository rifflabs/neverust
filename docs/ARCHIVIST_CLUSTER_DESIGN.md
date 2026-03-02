# Archivist-Cluster Design (Neverust + Archivist Compatible)

This document captures the current `archivist-cluster` direction:

- **IPFS Cluster compatible model** (pins, allocations, status, recovery).
- **Citadel-style failover** (deterministic, self-healing, low-coordination).
- **Dual integration paths**:
  - Remote via HTTP API (Archivist Node and Neverust).
  - Local direct integration via Neverust crate (`BlockStore`).

## 1. Component Mapping

IPFS Cluster core concepts map to Neverust as:

- `Cluster` -> `ArchivistCluster` coordinator.
- `Consensus` -> pluggable state backend (CRDT-first, Raft optional).
- `PinAllocator` -> deterministic rendezvous placement (`select_replicas`).
- `PinTracker` -> per-node pin state machine (`queued/pinning/pinned/failed` target model).
- `IPFSConnector` -> backend adapters:
  - HTTP adapter for Archivist/Neverust nodes.
  - Local Neverust adapter for in-process pins.

## 2. Citadel-Inspired Failover

Instead of leader-heavy repin loops, failover is:

1. Compute deterministic replica order for CID.
2. Attempt primary allocation set.
3. On failure, advance to next deterministic candidates.
4. Keep convergence deterministic across coordinators.

This keeps failover fast, idempotent, and coordination-light while preserving predictable placement.

## 3. Implemented Foundations

## 3.1 Placement + Upload Mode

`neverust-core/src/cluster.rs`:

- `ClusterNode`
- `select_replicas()` highest-random-weight placement
- `upload_path_for_cid_str()` auto-selects:
  - `/api/archivist/v1/data` for manifest CIDs (`0xcd01`)
  - `/api/archivist/v1/data/raw` for non-manifest blocks

## 3.2 Cluster Core

`neverust-core/src/archivist_cluster.rs`:

- `ArchivistCluster`
- `ClusterMember` with `MemberBackend`:
  - `HttpApi`
  - `LocalNeverust(Arc<BlockStore>)`
- `pin_from_source()` and `pin_payload()` with deterministic failover.
- `PinOutcome` for reconciliation reporting.

## 3.3 Orchestrator Tool

`examples/cluster_pin_orchestrator.rs`:

- Downloads CID payload once from source.
- Selects replica targets with rendezvous.
- Fans out pins in parallel.
- Verifies returned CID matches expected.
- Supports node lists by inline spec or file.

## 3.4 API Primitives

`neverust-core/src/api.rs`:

- Added fast raw upload:
  - `POST /api/archivist/v1/data/raw`
- Added cheap presence probe:
  - `GET /api/archivist/v1/data/:cid/exists`

## 4. Compatibility Targets

To reach practical IPFS Cluster parity for `archivist-cluster`:

1. Add cluster pin APIs (REST):
   - `POST /cluster/pins/:cid`
   - `GET /cluster/pins`
   - `GET /cluster/pins/:cid/status`
   - `POST /cluster/pins/:cid/recover`
2. Persist pin intent + allocations (CRDT-first).
3. Add follower mode (read-only tracking peers).
4. Add health metrics feed and automatic reassignment.
5. Add reconciliation workers for continuous convergence.

## 5. Near-Term Build Order

1. Pin state store + reconciliation loop.
2. Cluster REST endpoints and client.
3. CRDT sync and membership feed.
4. Follower mode + large-scale soak tests.

