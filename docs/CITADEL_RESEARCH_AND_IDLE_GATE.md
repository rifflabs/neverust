# Citadel Research Notes + Idle Bandwidth Gate

## Scope
This note captures the concrete constraints extracted from Citadel papers + implementation and how they are encoded in Neverust Citadel mode.

## Key inputs reviewed
- `citadel/papers/01-15-*.md` (TGP, SPORE, SPIRAL, convergence, CVDF, PoL, split-brain recovery).
- `citadel/docs/MESH_PROTOCOL.md`
- `citadel/docs/TGP_NATIVE.md`
- `citadel/docs/CONSTITUTIONAL_P2P.md`
- `citadel/crates/citadel-protocols/src/coordinator.rs`
- `citadel/crates/citadel-lens/src/mesh/{state.rs,service.rs,flood.rs}`
- `lagoon/proofs/LagoonMesh/{BootstrapProofs,Defederation,PerformanceBounds}.lean`
- `lagoon/crates/anymesh/src/{mesh,repair}.rs`
- `lagoon/crates/lagoon-server/src/irc/{gossip,peer_addr_gossip,liveness_gossip}.rs`

## Constraints extracted
1. Coordination identity should be proof-based (QuadProof/frontier), not socket-state-based.
2. Anti-entropy should be event-driven and differential (`O(|A xor B|)` behavior), not full-state blast.
3. Admission must be bounded per origin and per host domain to resist spam/sybil floods.
4. Idle behavior should decay to lightweight control-plane beacons.
5. Large swarms should scale sublinearly in control-plane fanout.

## Neverust implementation choices
1. Deterministic defederation state:
   - LWW CRDT for follows/content.
   - Ordered op frontier + pending reorder buffer to prevent drift.
2. Admission guard:
   - PoW gate (`base_pow_bits` / `trusted_pow_bits`).
   - Per-origin rate cap per round.
   - Per-host new-origin cap per round (sybil pressure limiter).
3. Idle gate (new):
   - `IdleBandwidthGateConfig.max_idle_bytes_per_sec` defaults to `100 * 1024`.
   - Probe fanout is `O(log N)` clamped (`min_repair_peers..max_repair_peers`).
   - Beacons are fixed-size (`beacon_bytes`), deltas are budgeted (`op_bytes_estimate`).
   - For very large swarms only, idle cap relaxes by `O(log N)` above threshold.
4. Lagoon carry-over primitives (applied conceptually now, code lift next pass):
   - Bounded anti-entropy repair fanout from `anymesh::repair` style.
   - Gossip/liveness split from `lagoon-server` (control-plane vs data-plane).
   - Lean proof direction aligned with LagoonMesh convergence/bounds theorems.

## Property target
- For non-huge swarms: per-node idle egress remains <= `100 KiB/s`.
- For huge swarms: control-plane growth remains sublinear (`O(log N)` fanout and budget relaxation).

## Validation hooks
- `neverust-core/src/citadel.rs` simulation reports:
  - `max_idle_bytes_per_sec_observed`
  - `avg_idle_bytes_per_sec_observed`
  - `idle_budget_bytes_per_sec`
- Tests:
  - `idle_gate_stays_under_100kib_for_normal_swarm`
  - `ten_k_two_host_defederation_converges`
