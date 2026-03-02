import Mathlib

/-!
Deployment safety invariants for production/testnet service configuration.

This module proves basic configuration properties we rely on in deployment
scripts:

1) all service ports are strictly positive,
2) p2p/api/discovery ports are pairwise distinct.
-/

namespace NeverustProofs.DeploymentSafety

structure ServicePorts where
  p2p : Nat
  api : Nat
  disc : Nat
deriving Repr

def Valid (ports : ServicePorts) : Prop :=
  0 < ports.p2p ∧
  0 < ports.api ∧
  0 < ports.disc ∧
  ports.p2p ≠ ports.api ∧
  ports.p2p ≠ ports.disc ∧
  ports.api ≠ ports.disc

theorem valid_pos_p2p {ports : ServicePorts} (h : Valid ports) : 0 < ports.p2p := by
  exact h.1

theorem valid_pos_api {ports : ServicePorts} (h : Valid ports) : 0 < ports.api := by
  exact h.2.1

theorem valid_pos_disc {ports : ServicePorts} (h : Valid ports) : 0 < ports.disc := by
  exact h.2.2.1

theorem valid_ne_p2p_api {ports : ServicePorts} (h : Valid ports) : ports.p2p ≠ ports.api := by
  exact h.2.2.2.1

theorem valid_ne_p2p_disc {ports : ServicePorts} (h : Valid ports) : ports.p2p ≠ ports.disc := by
  exact h.2.2.2.2.1

theorem valid_ne_api_disc {ports : ServicePorts} (h : Valid ports) : ports.api ≠ ports.disc := by
  exact h.2.2.2.2.2

def eth2077Ports : ServicePorts where
  p2p := 33070
  api := 33080
  disc := 33090

theorem eth2077Ports_valid : Valid eth2077Ports := by
  refine ⟨by decide, by decide, by decide, by decide, by decide, by decide⟩

end NeverustProofs.DeploymentSafety
