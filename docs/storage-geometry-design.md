# Neverust Storage Geometry Design (FlatFS Lane)

## Goal

Define a FlatFS-style on-disk mapping that:
1. Preserves every CID exactly.
2. Proves zero collisions in path identity at the model level.
3. Uses geometric sharding for directory locality/distribution.

## FlatFS Semantics Anchor

From `go-ds-flatfs-mw`:
- Directory is shard-derived from key (`dir = shard(key)`).
- Filename keeps full key (`file = key + ".data"`).

Consequence: if full key is in filename, path identity is injective in key regardless of shard function.

## Proposed Geometry Key

Model CID entropy as 32-byte vector.

Split into two 16-byte rails:
- `basepoint rail`: even-byte positions (0,2,4,...,30)
- `fiber rail`: odd-byte positions (1,3,5,...,31)

Use:
- `basepoint rail` for shard derivation (directory geometry).
- full CID string in filename for exact identity.

## Proof Obligations

1. `split` and `merge` are inverse (lossless).
2. `geometryKey` is injective/bijective over 32-byte domain.
3. FlatFS encode `(shard(base), full_key)` is injective for any shard.

## Lean Status

Implemented in:
- `proofs/lean/NeverustProofs/StorageGeometry.lean`

Built successfully with:

```bash
cd proofs/lean
lake build NeverustProofs.StorageGeometry
```

Key theorems:
- `merge_geometryKey`
- `geometryKey_merge`
- `geometryKey_injective`
- `geometryKey_bijective`
- `flatfsEncode_injective`
- `zero_collision_for_every_cid`

## Next Implementation Step

Move from file-per-CID `geomtree` to geometric blockfiles:

1. keep geometric shard derivation from `basepoint rail`,
2. keep full CID as identity payload in slot header,
3. allocate within classed blockfiles using deterministic probe mapping.

Detailed design and MooseFS constraints are captured in:

- `docs/delta-store-design.md`
