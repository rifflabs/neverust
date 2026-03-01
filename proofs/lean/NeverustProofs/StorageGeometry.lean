import Mathlib

/-!
Neverust Storage Geometry — FlatFS collision lane

This module formalizes a geometric keying scheme for CID-like 32-byte entropy:
- split into `(basepoint rail, fiber rail)` via even/odd byte lanes,
- merge reconstructs exactly,
- FlatFS path encoding remains injective when filename carries full key.
-/

namespace NeverustProofs.StorageGeometry

noncomputable section

abbrev Byte := Fin 256
abbrev CID32 := Fin 32 → Byte
abbrev Rail16 := Fin 16 → Byte

/-- Even-byte selector (basepoint rail). -/
def evenIndex (i : Fin 16) : Fin 32 :=
  ⟨2 * i.1, by
    have hi : i.1 ≤ 15 := Nat.le_pred_of_lt i.2
    omega⟩

/-- Odd-byte selector (fiber rail). -/
def oddIndex (i : Fin 16) : Fin 32 :=
  ⟨2 * i.1 + 1, by
    have hi : i.1 ≤ 15 := Nat.le_pred_of_lt i.2
    omega⟩

/-- Half-index map from 32 slots into 16 slots. -/
def halfIndex (j : Fin 32) : Fin 16 :=
  ⟨j.1 / 2, by
    have hj : j.1 ≤ 31 := Nat.le_pred_of_lt j.2
    omega⟩

/-- Basepoint projection rail. -/
def projectBase (c : CID32) : Rail16 := fun i => c (evenIndex i)

/-- Fiber projection rail. -/
def projectFiber (c : CID32) : Rail16 := fun i => c (oddIndex i)

/-- Geometry key used for path decomposition. -/
def geometryKey (c : CID32) : Rail16 × Rail16 := (projectBase c, projectFiber c)

/-- Merge rails back into full 32-byte key. -/
def merge (base fiber : Rail16) : CID32 :=
  fun j =>
    if j.1 % 2 = 0 then
      base (halfIndex j)
    else
      fiber (halfIndex j)

lemma evenIndex_mod_two (i : Fin 16) : (evenIndex i).1 % 2 = 0 := by
  simp [evenIndex]

lemma oddIndex_mod_two (i : Fin 16) : (oddIndex i).1 % 2 = 1 := by
  simp [oddIndex]

lemma halfIndex_evenIndex (i : Fin 16) : halfIndex (evenIndex i) = i := by
  ext
  simp [halfIndex, evenIndex]

lemma halfIndex_oddIndex (i : Fin 16) : halfIndex (oddIndex i) = i := by
  ext
  change (2 * i.1 + 1) / 2 = i.1
  omega

lemma even_reconstruct (j : Fin 32) (h : j.1 % 2 = 0) : 2 * (j.1 / 2) = j.1 := by
  have hmod := Nat.mod_add_div j.1 2
  rw [h, Nat.zero_add] at hmod
  exact hmod

lemma odd_reconstruct (j : Fin 32) (h : j.1 % 2 ≠ 0) : 2 * (j.1 / 2) + 1 = j.1 := by
  have h1 : j.1 % 2 = 1 := by
    have hlt : j.1 % 2 < 2 := Nat.mod_lt _ (by decide)
    omega
  have hmod := Nat.mod_add_div j.1 2
  rw [h1] at hmod
  have : 1 + 2 * (j.1 / 2) = j.1 := hmod
  omega

lemma evenIndex_of_even (j : Fin 32) (h : j.1 % 2 = 0) : evenIndex (halfIndex j) = j := by
  ext
  change 2 * (j.1 / 2) = j.1
  exact even_reconstruct j h

lemma oddIndex_of_odd (j : Fin 32) (h : j.1 % 2 ≠ 0) : oddIndex (halfIndex j) = j := by
  ext
  change 2 * (j.1 / 2) + 1 = j.1
  exact odd_reconstruct j h

/-- Merge after split reconstructs the original key exactly. -/
theorem merge_geometryKey (c : CID32) : merge (geometryKey c).1 (geometryKey c).2 = c := by
  funext j
  by_cases h : j.1 % 2 = 0
  · have hj : evenIndex (halfIndex j) = j := evenIndex_of_even j h
    simp [merge, geometryKey, projectBase, h, hj]
  · have hj : oddIndex (halfIndex j) = j := oddIndex_of_odd j h
    simp [merge, geometryKey, projectFiber, h, hj]

/-- Split after merge recovers both rails exactly. -/
theorem geometryKey_merge (base fiber : Rail16) :
    geometryKey (merge base fiber) = (base, fiber) := by
  apply Prod.ext
  · funext i
    simp [geometryKey, projectBase, merge, evenIndex_mod_two, halfIndex_evenIndex]
  · funext i
    have hne : (oddIndex i).1 % 2 ≠ 0 := by
      rw [oddIndex_mod_two]
      decide
    simp [geometryKey, projectFiber, merge, hne, halfIndex_oddIndex]

/-- No collisions in geometric keying over full 32-byte domain. -/
theorem geometryKey_injective : Function.Injective geometryKey := by
  intro c₁ c₂ h
  have h' := congrArg (fun k => merge k.1 k.2) h
  simpa [merge_geometryKey] using h'

/-- Geometry keying is bijective with explicit inverse `merge`. -/
theorem geometryKey_bijective : Function.Bijective geometryKey := by
  refine ⟨geometryKey_injective, ?_⟩
  intro k
  refine ⟨merge k.1 k.2, ?_⟩
  simpa [geometryKey_merge]

/-- Abstract FlatFS encode model: shard from basepoint + full key in filename. -/
def flatfsEncode {Dir : Type} (shard : Rail16 → Dir) (c : CID32) : Dir × CID32 :=
  (shard (geometryKey c).1, c)

/-- FlatFS encode remains injective regardless of shard function,
because full key is retained in filename component. -/
theorem flatfsEncode_injective {Dir : Type} (shard : Rail16 → Dir) :
    Function.Injective (flatfsEncode shard) := by
  intro c₁ c₂ h
  exact congrArg Prod.snd h

/-- Storage statement: every CID has a unique encoded path key. -/
theorem zero_collision_for_every_cid {Dir : Type} (shard : Rail16 → Dir)
    (c₁ c₂ : CID32) :
    flatfsEncode shard c₁ = flatfsEncode shard c₂ → c₁ = c₂ := by
  intro h
  exact flatfsEncode_injective shard h

end

end NeverustProofs.StorageGeometry
