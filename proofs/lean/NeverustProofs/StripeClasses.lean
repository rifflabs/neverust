import Mathlib

/-!
Neverust storage class lane for MooseFS-oriented blockfiles.

This module locks three facts used by the planned on-disk layout:
1. shard classes cover the target range `512KiB .. 512MiB`,
2. class selection is deterministic and always returns one class value,
3. chosen classes are stripe-compatible with `64MiB` MooseFS stripes.
-/

namespace NeverustProofs.StripeClasses

abbrev KiB : Nat := 1024
abbrev MiB : Nat := 1024 * KiB

abbrev stripeBytes : Nat := 64 * MiB
abbrev minShardBytes : Nat := 512 * KiB
abbrev maxShardBytes : Nat := 512 * MiB

abbrev C0 : Nat := 524288
abbrev C1 : Nat := 1048576
abbrev C2 : Nat := 2097152
abbrev C3 : Nat := 4194304
abbrev C4 : Nat := 8388608
abbrev C5 : Nat := 16777216
abbrev C6 : Nat := 33554432
abbrev C7 : Nat := 67108864
abbrev C8 : Nat := 134217728
abbrev C9 : Nat := 268435456
abbrev C10 : Nat := 536870912

def classList : List Nat := [C0, C1, C2, C3, C4, C5, C6, C7, C8, C9, C10]

/-- Deterministic size-class selector for shard payload size (bytes). -/
def classOfSize (n : Nat) : Nat :=
  if n ≤ C0 then C0 else
  if n ≤ C1 then C1 else
  if n ≤ C2 then C2 else
  if n ≤ C3 then C3 else
  if n ≤ C4 then C4 else
  if n ≤ C5 then C5 else
  if n ≤ C6 then C6 else
  if n ≤ C7 then C7 else
  if n ≤ C8 then C8 else
  if n ≤ C9 then C9 else
  C10

theorem classOfSize_mem (n : Nat) : classOfSize n ∈ classList := by
  unfold classOfSize classList
  split_ifs <;> simp

theorem classOfSize_ge_input
    {n : Nat}
    (hmin : minShardBytes ≤ n)
    (hmax : n ≤ maxShardBytes) :
    n ≤ classOfSize n := by
  have _hmin_use : minShardBytes ≤ n := hmin
  unfold classOfSize
  split_ifs <;> omega

theorem classOfSize_le_max (n : Nat) : classOfSize n ≤ maxShardBytes := by
  let c := classOfSize n
  have hmem : c ∈ classList := by
    simpa [c] using classOfSize_mem n
  simp [classList] at hmem
  rcases hmem with h0 | h1 | h2 | h3 | h4 | h5 | h6 | h7 | h8 | h9 | h10
  · have hc : classOfSize n = C0 := by simpa [c] using h0
    rw [hc]
    norm_num [maxShardBytes, C0]
  · have hc : classOfSize n = C1 := by simpa [c] using h1
    rw [hc]
    norm_num [maxShardBytes, C1]
  · have hc : classOfSize n = C2 := by simpa [c] using h2
    rw [hc]
    norm_num [maxShardBytes, C2]
  · have hc : classOfSize n = C3 := by simpa [c] using h3
    rw [hc]
    norm_num [maxShardBytes, C3]
  · have hc : classOfSize n = C4 := by simpa [c] using h4
    rw [hc]
    norm_num [maxShardBytes, C4]
  · have hc : classOfSize n = C5 := by simpa [c] using h5
    rw [hc]
    norm_num [maxShardBytes, C5]
  · have hc : classOfSize n = C6 := by simpa [c] using h6
    rw [hc]
    norm_num [maxShardBytes, C6]
  · have hc : classOfSize n = C7 := by simpa [c] using h7
    rw [hc]
    norm_num [maxShardBytes, C7]
  · have hc : classOfSize n = C8 := by simpa [c] using h8
    rw [hc]
    norm_num [maxShardBytes, C8]
  · have hc : classOfSize n = C9 := by simpa [c] using h9
    rw [hc]
    norm_num [maxShardBytes, C9]
  · have hc : classOfSize n = C10 := by simpa [c] using h10
    rw [hc]
    norm_num [maxShardBytes, C10]

/-- Every selected class is stripe-compatible:
small classes divide one `64MiB` stripe; large classes are stripe multiples. -/
theorem classOfSize_stripe_compatible (n : Nat) :
    (classOfSize n ≤ stripeBytes ∧ stripeBytes % classOfSize n = 0) ∨
    (stripeBytes ≤ classOfSize n ∧ classOfSize n % stripeBytes = 0) := by
  let c := classOfSize n
  have hmem : c ∈ classList := by
    simpa [c] using classOfSize_mem n
  simp [classList] at hmem
  rcases hmem with h0 | h1 | h2 | h3 | h4 | h5 | h6 | h7 | h8 | h9 | h10
  · left
    have hc : classOfSize n = C0 := by simpa [c] using h0
    rw [hc]
    norm_num [stripeBytes, C0]
  · left
    have hc : classOfSize n = C1 := by simpa [c] using h1
    rw [hc]
    norm_num [stripeBytes, C1]
  · left
    have hc : classOfSize n = C2 := by simpa [c] using h2
    rw [hc]
    norm_num [stripeBytes, C2]
  · left
    have hc : classOfSize n = C3 := by simpa [c] using h3
    rw [hc]
    norm_num [stripeBytes, C3]
  · left
    have hc : classOfSize n = C4 := by simpa [c] using h4
    rw [hc]
    norm_num [stripeBytes, C4]
  · left
    have hc : classOfSize n = C5 := by simpa [c] using h5
    rw [hc]
    norm_num [stripeBytes, C5]
  · left
    have hc : classOfSize n = C6 := by simpa [c] using h6
    rw [hc]
    norm_num [stripeBytes, C6]
  · left
    have hc : classOfSize n = C7 := by simpa [c] using h7
    rw [hc]
    norm_num [stripeBytes, C7]
  · right
    have hc : classOfSize n = C8 := by simpa [c] using h8
    rw [hc]
    norm_num [stripeBytes, C8]
  · right
    have hc : classOfSize n = C9 := by simpa [c] using h9
    rw [hc]
    norm_num [stripeBytes, C9]
  · right
    have hc : classOfSize n = C10 := by simpa [c] using h10
    rw [hc]
    norm_num [stripeBytes, C10]

theorem class_lane_covers_target_range
    {n : Nat}
    (hmin : minShardBytes ≤ n)
    (hmax : n ≤ maxShardBytes) :
    n ≤ classOfSize n ∧ classOfSize n ≤ maxShardBytes := by
  exact ⟨classOfSize_ge_input hmin hmax, classOfSize_le_max n⟩

end NeverustProofs.StripeClasses
