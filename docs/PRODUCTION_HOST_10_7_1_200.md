# Production Host Runbook (10.7.1.200)

This runbook configures and validates a dual-node host:

- `Archivist` at `/opt/archivist`
- `Neverust` at `/opt/neverust`
- MooseFS-backed storage roots:
  - `/mnt/riffcastle/archivist/blockstores/archivist`
  - `/mnt/riffcastle/archivist/blockstores/neverust`

## 1) Host Layout

- `/mnt/riffcastle` is the canonical MooseFS path used by services and tests.
- `/opt/archivist` points to the patched Archivist checkout.
- `/opt/neverust` points to this Neverust checkout.

## 2) One-shot Setup

```bash
cd /opt/neverust
./scripts/production/setup_host_10_7_1_200.sh
```

The setup script:

1. Creates MooseFS storage roots and temp dir.
2. Builds Archivist (`nimble build`).
3. Builds Neverust (`cargo build --release`).
4. Installs systemd units:
   - `archivist.service`
   - `neverust.service`
5. Enables services.

## 3) Service Ports

Configured to avoid local conflicts:

- Neverust:
  - `TCP 8070` libp2p
  - `UDP 8090` discovery
  - `UDP 8091` BoTG (implicit `disc_port + 1`)
  - `TCP 8081` HTTP API
- Archivist:
  - `TCP 9070` libp2p (`--listen-addrs`)
  - `UDP 9090` discovery (`--disc-port`)
  - `TCP 9080` HTTP API (`--api-port`)
  - `TCP 8008` metrics (if enabled)

### Port Forwarding Matrix (to `10.7.1.200`)

Forward these public ports to the same port on `10.7.1.200`:

- Archivist:
  - `TCP 9070 -> 10.7.1.200:9070`
  - `UDP 9090 -> 10.7.1.200:9090`
  - `TCP 9080 -> 10.7.1.200:9080`
  - `TCP 8008 -> 10.7.1.200:8008` (optional metrics)
- Neverust:
  - `TCP 8070 -> 10.7.1.200:8070`
  - `UDP 8090 -> 10.7.1.200:8090`
  - `UDP 8091 -> 10.7.1.200:8091`
  - `TCP 8081 -> 10.7.1.200:8081`

Note: `TCP 8080` is in use by `lagoon-web` on this host and is intentionally not used by Neverust.

## 4) Start and Inspect

```bash
sudo systemctl start archivist.service
sudo systemctl start neverust.service
systemctl status archivist.service neverust.service
journalctl -u archivist.service -f
journalctl -u neverust.service -f
```

## 5) Comprehensive Cross-cutting Suite

Fast/default:

```bash
cd /opt/neverust
ARCHIVIST_NODE_FFI_ROOT=/opt/neverust/.ffi/archivist-node-mempatch \
./scripts/production/comprehensive_crosscut_suite.sh
```

Extended/long:

```bash
cd /opt/neverust
FULL=1 ARCHIVIST_NODE_FFI_ROOT=/opt/neverust/.ffi/archivist-node-mempatch \
./scripts/production/comprehensive_crosscut_suite.sh
```

Coverage includes:

- Neverust release build
- Archivist build
- Neverust API/parity tests
- Blockstore integrity tests
- FFI interop proof (Archivist upload -> Neverust retrieve)
- Neverust integration + block exchange tests
- Optional full Archivist test suites and contract/integration flows (`FULL=1`)

## 6) DeepWiki + Local Test Mapping

DeepWiki pages used for architecture scoping:

- `Overview`
- `Core Architecture`
- `Node Architecture`
- `Testing Framework`
- `CI/CD and DevOps`

For exact executable test paths, use the local repository trees (authoritative):

- Archivist: `/opt/neverust/.ffi/archivist-node-mempatch/tests/...`
- Neverust: `/opt/neverust/tests/...` and `/opt/neverust/neverust-core/tests/...`
