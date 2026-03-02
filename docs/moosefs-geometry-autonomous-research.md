# MooseFS Geometry Autonomous Research Plan

## Objective

Build a MooseFS-friendly blockstore driver that is:
- extremely high throughput on sequential ingest paths,
- multiwriter safe across hosts,
- collision-safe for every CID,
- scalable to effectively unlimited logical objects while keeping physical file count near an operational budget (for example ~10M files).

## Hard Constraints

1. MooseFS stripe preference: `64MiB`.
2. Payload class range: `512KiB..512MiB`.
3. Massive logical keyspace: billions to trillions of CIDs.
4. Physical file count must remain bounded.
5. No silent corruption, no CID aliasing, no retrieval ambiguity.

## Transferable Lessons

### From `grand-2026`

1. Projection split pattern (`base lane` + `fiber lane`):
   - use a reduced route key for placement,
   - preserve full identity separately to keep lossless recovery.
2. Linear-only lane limitations:
   - purely static routing tends to hard limits under pressure.
3. Topological/affine bypass:
   - add a controlled mutable signal (for example live split pointer/counter) to avoid linear dead lanes without losing determinism.
4. Recursive index tower:
   - small local fanout can still address massive global space when layers compose.

### From `go-ds-flatfs-mw`

1. Per-key multiwriter safety:
   - same-directory temp write + atomic rename.
2. Content-addressed semantics:
   - first-successful-writer-wins is valid when key derives from content.
3. Explicit tradeoff:
   - no strict global write-order guarantee.

## Driver Shape (Research Candidate)

1. `L0` Control Plane:
   - live mutable counters per shard/class (`level`, `split_ptr`, `item_count`, `next_file`, `next_offset`).
   - range-lock controlled updates (MooseFS distributed lock path).
2. `L1` Route Geometry:
   - CID split into route key and identity key.
   - route key chooses shard/class/bucket.
3. `L2` Index Geometry:
   - bounded bucket pages with deterministic probing and split growth.
   - affine rotation via live counter to avoid hot linear lanes.
4. `L3` Data Plane:
   - large blockfiles aligned to MooseFS-friendly write patterns.
   - classed placement for `512KiB..512MiB`.
5. `L4` Integrity Plane:
   - full CID bytes stored in index entry.
   - read path validates payload-to-CID.

## Non-Negotiable Invariants

1. Full CID equality check on lookup hit.
2. CID-to-record mapping injective at model level.
3. No silent success on corrupted payload.
4. Split growth always keeps bucket mapping in range.
5. First-free slot selection deterministic for fixed state.

## Autonomous Loop

1. Generate primitive compositions (routing, bucket layout, lock granularity, commit policy, fsync schedule).
2. Run in-memory composition harness.
3. Run real backend harness (single-node and multinode).
4. Record metrics into tradeoff matrix CSV.
5. Auto-ban repeated failure fingerprints.
6. Synthesize hybrid candidates:
   - one component from a good run,
   - one component from a failed run,
   - one novel parameter.
7. Re-run and update Pareto frontier.
8. Promote only candidates that pass integrity and ordering gates.

## VICE Adaptation

Adapted from `grand-2026` VICE rules:

1. Zero hand-wavy claims:
   - every performance claim must have command + numeric artifact.
2. No vacuous proof statements:
   - Lean theorems must encode real storage invariants (not tautologies).
3. Bridge-by-theorem:
   - every geometry transfer from `grand-2026` must appear as a concrete Lean theorem in `NeverustProofs`.
4. Shared primitive discipline:
   - routing/class/probe definitions are imported and reused, not duplicated ad-hoc.
5. Falsification-first scoreboards:
   - each run classified by explicit PASS/FAIL gates.

Scoreboard command:

```bash
python3 scripts/vice_scoreboard_from_csv.py /tmp/neverust_primitive_runs.csv
```

## Current Command Surface

In-memory:

```bash
cargo run --release --example primitive_pipeline_bench -- \
  inmem 300000 1048576 24 xor,blake3,index_mod:4194304 512 64
```

Real backend single-node:

```bash
cargo run --release --example primitive_pipeline_bench -- \
  real deltaflat /tmp/neverust-real 50000 1048576 16 256 \
  xor,blake3,index_mod:4194304 256 2048 true
```

Real backend multinode:

```bash
cargo run --release --example primitive_pipeline_bench -- \
  real-multinode deltaflat /tmp/neverust-real-multi 4 2 80000 524288 8 128 \
  xor,blake3,index_xorfold:4194304 256 2048 true
```

Tradeoff matrix / autoimprover:

```bash
cargo run --release --example primitive_tradeoff_autoimprover -- analyze /tmp/primitive_runs.csv 20 20
```

## Immediate Research Notes

1. Harness validation succeeded on `redb` with zero verification failures.
2. Real `deltaflat` runs showed non-zero verification failures in the new real-mode harness.
3. That makes retrieval-integrity investigation a top priority before aggressive optimization claims.
