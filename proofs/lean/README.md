# Neverust Lean Proofs

This folder contains Lean proofs for storage geometry and correctness constraints.

## Current modules

- `NeverustProofs.StorageGeometry`
- `NeverustProofs.ProbeAllocation`
- `NeverustProofs.StripeClasses`

## What is proven

1. `geometryKey` split (base rail + fiber rail) is bijective over 32-byte key entropy:
   - `merge_geometryKey`
   - `geometryKey_merge`
   - `geometryKey_injective`
   - `geometryKey_bijective`
2. FlatFS-style encode remains injective for **any** shard function if filename keeps full key:
   - `flatfsEncode_injective`
   - `zero_collision_for_every_cid`
3. Deterministic probe allocation on sparse slot space has unique first-free selection:
   - `probe_injective`
   - `exists_first_free_probe`
   - `first_free_probe_unique`
   - `zero_collision_first_free`
4. MooseFS class lane covers shard sizes `512KiB..512MiB` and remains stripe-compatible:
   - `classOfSize_mem`
   - `class_lane_covers_target_range`
   - `classOfSize_stripe_compatible`

## Build

```bash
cd proofs/lean
lake build NeverustProofs.StorageGeometry
lake build NeverustProofs.ProbeAllocation
lake build NeverustProofs.StripeClasses
```
