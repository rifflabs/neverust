import Mathlib

/-!
Probe allocation lane for sparse blockfiles.

This module formalizes deterministic probe sequences used for slot selection in
large sparse files. It proves:
- probe addresses are injective in step-index,
- for any finite occupied set, a free probe address exists,
- first-free probe choice is well-defined and unique.
-/

namespace NeverustProofs.ProbeAllocation

/-- Arithmetic probe sequence on natural-number slot space. -/
def probe (start step n : ℕ) : ℕ := start + step * n

/-- Probe sequence is monotone for positive step. -/
lemma probe_strictMono {start step : ℕ} (hstep : 0 < step) :
    StrictMono (probe start step) := by
  intro a b hab
  unfold probe
  nlinarith [Nat.mul_lt_mul_of_pos_left hab hstep]

/-- Probe sequence has no repeats for positive step. -/
theorem probe_injective {start step : ℕ} (hstep : 0 < step) :
    Function.Injective (probe start step) :=
  (probe_strictMono hstep).injective

/-- Any finite occupied slot set leaves infinitely many free probe slots. -/
theorem exists_probe_not_mem_finset
    (S : Finset ℕ) {start step : ℕ} (hstep : 0 < step) :
    ∃ n, probe start step n ∉ S := by
  classical
  by_cases hS : S.Nonempty
  · let m := S.max' hS
    refine ⟨m + 1, ?_⟩
    intro hmem
    have hle : probe start step (m + 1) ≤ m := by
      exact Finset.le_max' S (probe start step (m + 1)) hmem
    have hstep1 : 1 ≤ step := Nat.succ_le_of_lt hstep
    have hmul : m + 1 ≤ step * (m + 1) := by
      simpa using (Nat.mul_le_mul_right (m + 1) hstep1)
    have hgt : m < probe start step (m + 1) := by
      unfold probe
      omega
    omega
  · refine ⟨0, ?_⟩
    have hempty : S = ∅ := Finset.not_nonempty_iff_eq_empty.mp hS
    simpa [hempty]

/-- Existence of a first free probe index for finite occupancy. -/
theorem exists_first_free_probe
    (S : Finset ℕ) {start step : ℕ} (hstep : 0 < step) :
    ∃ n, probe start step n ∉ S ∧
      ∀ k < n, probe start step k ∈ S := by
  have hex : ∃ n, probe start step n ∉ S := exists_probe_not_mem_finset S hstep
  let n0 := Nat.find hex
  refine ⟨n0, Nat.find_spec hex, ?_⟩
  intro k hk
  by_contra hnot
  have hmin : Nat.find hex ≤ k := Nat.find_min' hex hnot
  have hk' : k < Nat.find hex := by simpa [n0] using hk
  omega

/-- First-free probe index is unique. -/
theorem first_free_probe_unique
    (S : Finset ℕ) {start step : ℕ} (_hstep : 0 < step)
    {n₁ n₂ : ℕ}
    (h₁ : probe start step n₁ ∉ S ∧ ∀ k < n₁, probe start step k ∈ S)
    (h₂ : probe start step n₂ ∉ S ∧ ∀ k < n₂, probe start step k ∈ S) :
    n₁ = n₂ := by
  by_cases hlt : n₁ < n₂
  · have : probe start step n₁ ∈ S := h₂.2 n₁ hlt
    exact False.elim (h₁.1 this)
  by_cases hgt : n₂ < n₁
  · have : probe start step n₂ ∈ S := h₁.2 n₂ hgt
    exact False.elim (h₂.1 this)
  exact Nat.le_antisymm (Nat.le_of_not_gt hgt) (Nat.le_of_not_gt hlt)

/-- Storage-level zero-collision statement for finite occupancy:
`first free` probe allocation produces a unique slot index. -/
theorem zero_collision_first_free
    (S : Finset ℕ) {start step : ℕ} (hstep : 0 < step) :
    ∃! n, probe start step n ∉ S ∧ ∀ k < n, probe start step k ∈ S := by
  rcases exists_first_free_probe S hstep with ⟨n, hn⟩
  refine ⟨n, hn, ?_⟩
  intro m hm
  exact first_free_probe_unique S hstep hm hn

end NeverustProofs.ProbeAllocation
