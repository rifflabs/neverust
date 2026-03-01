# Neverust Delta Store Design (MooseFS)

## Problem Constraints

1. MooseFS likes `64MiB` stripe-aligned access.
2. We need shard payload sizes from `512KiB` through `512MiB`.
3. Logical object count can be massive (billions+), so physical file count must stay low.
4. Writes must remain multiwriter-safe on a shared MooseFS mount.

## Inputs We Reused

### `go-ds-flatfs-mw` (operational semantics)

- FlatFS keying rule:
  - shard directory from key projection,
  - full key preserved in filename (`key + ".data"`).
- Multiwriter safety pattern:
  - write temp artifact in destination directory,
  - same-directory atomic rename,
  - first-successful-writer-wins for content-addressed keys.

### `grand-2026` (geometry/proof pattern)

From `Gutoe/ProjectionFibers.lean` and `Gutoe/ContainmentScope.lean`:

- Projection can be intentionally non-injective (`base` projection as locality lane).
- Identity is recovered by carrying full state/key separately.
- Fibers are translated copies of a shared kernel; we can reuse one geometry everywhere.

Storage translation:

- `base rail` drives placement geometry.
- full CID is retained as identity authority.
- repeated slot topology per directory is valid (same kernel shape, translated basepoint).

## On-Disk Geometry

Root:

`<data>/delta-store/v1/`

Directory sharding (FlatFS-style, geometric):

- derive `base rail` from CID entropy,
- use 2-3 hex levels from base rail bytes.

Example:

`<root>/a7/3f/1c/`

Inside each shard directory:

- class lane subdirs:
  - `c512k`, `c1m`, `c2m`, ..., `c512m`.
- blockfiles:
  - `blk-00000000.dat`, `blk-00000001.dat`, ...

## Size Class Lane

Fixed classes:

- `512KiB`, `1MiB`, `2MiB`, `4MiB`, `8MiB`, `16MiB`, `32MiB`, `64MiB`, `128MiB`, `256MiB`, `512MiB`.

Selection:

- `classOfSize(size)` = smallest class `>= size`.

Stripe relation:

- classes `<= 64MiB`: stripe holds integral slot count,
- classes `>= 64MiB`: slot spans integral stripe count.

Formalized in Lean:

- `NeverustProofs.StripeClasses`.

## Slot Addressing (Low File Count, Huge Logical Count)

We avoid one-file-per-CID. A CID maps to a virtual slot stream:

1. `start = H_fiber(cid, class)` (base probe origin),
2. `step = odd(H2_fiber(cid, class))`,
3. probe `slot_n = start + step * n`.

`slot_n` maps to `(blockfile_id, slot_index)` in that class lane.

This gives deterministic open addressing over a sparse slot space.

Formalized in Lean:

- `NeverustProofs.ProbeAllocation`.

## Octree Index Variant (Sparse In-Memory Walk)

This can be layered as a sparse index over the same blockfiles:

1. Use `8-bit` fanout per level (`0..255`) from CID bitstream.
2. Treat each level as 256-way partition; conceptually this is an octree-style
   geometric refinement of address space.
3. Keep only hot upper nodes in RAM; cold lower nodes stay on disk.
4. Leaves store compact `(file_id, slot_id)` references.

Result:

- we can walk from a small 256-way directory/index surface to an effectively
  massive logical address space (up to full CID entropy) without loading the
  whole index in memory.
- memory footprint scales with active working set, not total key cardinality.

## Multiwriter Protocol

Per-slot lock and two-phase write:

1. acquire lock file with `create_new` in same shard/class directory:
   - e.g. `blk-0001.dat.slot-0042.lock`.
2. inspect slot header:
   - if full + same CID: success (idempotent),
   - if full + different CID: probe next slot,
   - if empty/tombstone: write payload + header, fsync if enabled.
3. release lock by deleting lock file.

Crash handling:

- lock file carries timestamp + writer id,
- stale lock lease can be reclaimed conservatively.

Semantics:

- first writer to claim a free slot wins,
- repeated writes of same CID are safe idempotent no-ops.

## Slot Record Format (Per Slot)

Fixed header + payload area:

- `magic`, `version`, `state` (`empty|writing|full|tombstone`),
- `data_len`,
- `cid_len`,
- `cid_bytes` (binary CID),
- checksum.

Payload is class-sized slot body; `data_len` indicates used prefix.

## Why This Meets the Constraints

1. `64MiB` stripe compatibility is explicit in class lane.
2. `512KiB..512MiB` range has deterministic class mapping.
3. Low physical file count: many logical CIDs packed into blockfiles.
4. Multiwriter safety follows lock + same-dir atomic transition pattern adapted from FlatFS-MW.

## Implementation Order

1. Add `deltastore` backend beside `redb`/`geomtree`.
2. Implement class mapper + probe mapper + slot codec.
3. Implement lock protocol and stale-lock recovery.
4. Add compaction/rebalancing for tombstone-heavy files.
5. Benchmark against current upload path with progress + MiB/s reporting.
