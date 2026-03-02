#!/usr/bin/env bash
set -euo pipefail

# Production host setup for 10.7.1.200
# - Canonical app paths:
#   /opt/archivist -> archivist-node checkout
#   /opt/neverust  -> neverust checkout
# - Storage roots:
#   /mnt/riffcastle/archivist/blockstores/archivist
#   /mnt/riffcastle/archivist/blockstores/neverust

ARCHIVIST_SRC_DEFAULT="/mnt/riffcastle/castle/garage/archivist-node-mempatch"
NEVERUST_SRC_DEFAULT="/mnt/riffcastle/castle/workspace/neverust"

ARCHIVIST_SRC="${ARCHIVIST_SRC:-$ARCHIVIST_SRC_DEFAULT}"
NEVERUST_SRC="${NEVERUST_SRC:-$NEVERUST_SRC_DEFAULT}"
LATEST_NIMBLE_DEFAULT="/tmp/nimble-latest/src/nimble"
if [[ -z "${NIMBLE_BIN:-}" ]]; then
  if [[ -x "$LATEST_NIMBLE_DEFAULT" ]]; then
    NIMBLE_BIN="$LATEST_NIMBLE_DEFAULT"
  else
    NIMBLE_BIN="nimble"
  fi
fi
NIMBLE_DIR="${NIMBLE_DIR:-/tmp/nimble-home}"

if [[ ! -d "$ARCHIVIST_SRC" ]]; then
  echo "ERROR: ARCHIVIST_SRC does not exist: $ARCHIVIST_SRC" >&2
  exit 1
fi
if [[ ! -d "$NEVERUST_SRC" ]]; then
  echo "ERROR: NEVERUST_SRC does not exist: $NEVERUST_SRC" >&2
  exit 1
fi

echo "[1/6] Creating /mnt/riffcastle storage roots"
sudo mkdir -p /mnt/riffcastle/archivist/blockstores/archivist
sudo mkdir -p /mnt/riffcastle/archivist/blockstores/neverust
sudo mkdir -p /mnt/riffcastle/archivist/tmp
sudo chown -R wings:wings /mnt/riffcastle/archivist
export TMPDIR="/mnt/riffcastle/archivist/tmp"
export TMP="$TMPDIR"
export TEMP="$TMPDIR"
mkdir -p "$NIMBLE_DIR"
export NIMBLE_DIR

echo "[2/6] Wiring /opt app roots"
sudo ln -sfn "$ARCHIVIST_SRC" /opt/archivist
sudo ln -sfn "$NEVERUST_SRC" /opt/neverust

echo "[3/6] Building Archivist binary"
(
  cd /opt/archivist
  export TMPDIR="${TMPDIR}"
  export TMP="${TMPDIR}"
  export TEMP="${TMPDIR}"
  "$NIMBLE_BIN" build
)

echo "[4/6] Building Neverust release binary"
(
  cd /opt/neverust
  export TMPDIR="${TMPDIR}"
  export TMP="${TMPDIR}"
  export TEMP="${TMPDIR}"
  cargo build --release
)

echo "[5/6] Installing systemd units"
sudo install -m 0644 "$NEVERUST_SRC/scripts/production/systemd/archivist.service" /etc/systemd/system/archivist.service
sudo install -m 0644 "$NEVERUST_SRC/scripts/production/systemd/neverust.service" /etc/systemd/system/neverust.service
sudo systemctl daemon-reload
sudo systemctl enable archivist.service
sudo systemctl enable neverust.service

echo "[6/6] Setup complete (services not auto-started by this script)"
echo "To start:"
echo "  sudo systemctl start archivist.service"
echo "  sudo systemctl start neverust.service"
echo
echo "To inspect:"
echo "  systemctl status archivist.service neverust.service"
echo "  journalctl -u archivist.service -f"
echo "  journalctl -u neverust.service -f"
