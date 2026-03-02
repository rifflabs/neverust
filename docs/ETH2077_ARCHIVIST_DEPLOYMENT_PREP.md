# ETH2077 Archivist Deployment Prep (Garage)

This runbook prepares an Archivist node deployment for ETH2077 testnet infrastructure in garage-managed hosts.

## Scope

- Deploy **Archivist node** from a garage checkout.
- Store data under MooseFS-backed paths.
- Install and manage a dedicated `systemd` unit.
- Verify readiness with a local upload/download canary.

## Files Added

- `scripts/production/eth2077/archivist-eth2077.env.example`
- `scripts/production/eth2077/setup_archivist_eth2077.sh`
- `scripts/production/eth2077/preflight_archivist_eth2077.sh`

## 1) Create Environment File

```bash
cd /mnt/riffcastle/castle/workspace/neverust
cp scripts/production/eth2077/archivist-eth2077.env.example \
   scripts/production/eth2077/archivist-eth2077.env
```

Edit `archivist-eth2077.env`:

- `ARCHIVIST_SRC` to your garage checkout (default is `/mnt/riffcastle/castle/garage/archivist-node`).
- `DATA_DIR` and `TMP_DIR` to MooseFS paths for ETH2077.
- `LISTEN_ADDR`, `DISC_PORT`, `API_PORT` to host-specific non-conflicting ports.

## 2) Install Service and Build Node

```bash
cd /mnt/riffcastle/castle/workspace/neverust
./scripts/production/eth2077/setup_archivist_eth2077.sh \
  ./scripts/production/eth2077/archivist-eth2077.env
```

The setup script:

1. Creates data/tmp directories and ownership.
2. Links the source to `ARCHIVIST_LINK` (default `/opt/archivist-eth2077`).
3. Builds Archivist with `nimble build`.
4. Renders and installs `/etc/systemd/system/archivist-eth2077.service`.
5. Reloads daemon and enables service.

## 3) Start and Inspect

```bash
sudo systemctl restart archivist-eth2077.service
sudo systemctl status archivist-eth2077.service
journalctl -u archivist-eth2077.service -f
```

## 4) Run Preflight Gate

```bash
cd /mnt/riffcastle/castle/workspace/neverust
./scripts/production/eth2077/preflight_archivist_eth2077.sh \
  ./scripts/production/eth2077/archivist-eth2077.env
```

Preflight checks:

- service activity status
- p2p/discovery/api port bindings
- `/health` response
- `/api/archivist/v1/peer-id` response
- upload/download canary roundtrip

Expected terminal result:

```text
PREFLIGHT_STATUS=PASS
```

## 5) External Connectivity Checklist

- Forward `LISTEN_ADDR` TCP port externally for P2P reachability.
- Forward `DISC_PORT` UDP if discovery is required externally.
- Optionally forward `API_PORT` TCP for remote API usage.
- Verify host firewall allows these ports.

## 6) Operational Notes

- Keep `LISTEN_ADDR`, `DISC_PORT`, and `API_PORT` distinct.
- Default fsync flags are enabled for safer persistence:
  - `FS_FSYNC_FILE=true`
  - `FS_FSYNC_DIR=true`
- Use a dedicated data root per testnet deployment to avoid accidental overlap.


## 7) Host-Specific Garage Rollout (10.7.1.200)

Prebaked env profile:

- `scripts/production/eth2077/archivist-eth2077.10-7-1-200.env`

One-command rollout + gate check:

```bash
cd /mnt/riffcastle/castle/workspace/neverust
./scripts/production/eth2077/rollout_archivist_eth2077_10_7_1_200.sh
```

This sequence runs:

1. Shell syntax gate for setup/preflight scripts.
2. Setup/install flow.
3. Service restart (`START_AFTER_SETUP=true`).
4. Preflight canary gate.
5. Local API smoke checks.
6. TLS host checks for:
   - `archivist.riff.cc`
   - `neverust.riff.cc`
   - `archcluster.riff.cc`
   - `2077.riff.cc`
   - `explorer.riff.cc`
   - `wallet.riff.cc`
   - `market.riff.cc`
   - `rpc.riff.cc`
