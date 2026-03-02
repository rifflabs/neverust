#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ENV_FILE="${1:-$SCRIPT_DIR/archivist-eth2077.env}"

if [[ ! -f "$ENV_FILE" ]]; then
  echo "ERROR: env file not found: $ENV_FILE" >&2
  echo "Hint: copy $SCRIPT_DIR/archivist-eth2077.env.example to $SCRIPT_DIR/archivist-eth2077.env" >&2
  exit 1
fi

set -a
source "$ENV_FILE"
set +a

SERVICE_NAME="${SERVICE_NAME:-archivist-eth2077}"
SERVICE_USER="${SERVICE_USER:-wings}"
SERVICE_GROUP="${SERVICE_GROUP:-wings}"
ARCHIVIST_SRC="${ARCHIVIST_SRC:-/mnt/riffcastle/castle/garage/archivist-node}"
ARCHIVIST_LINK="${ARCHIVIST_LINK:-/opt/archivist-eth2077}"
DATA_DIR="${DATA_DIR:-/mnt/mfs/eth2077/archivist}"
TMP_DIR="${TMP_DIR:-/mnt/mfs/eth2077/tmp}"
LISTEN_ADDR="${LISTEN_ADDR:-/ip4/0.0.0.0/tcp/33070}"
DISC_PORT="${DISC_PORT:-33090}"
API_BINDADDR="${API_BINDADDR:-0.0.0.0}"
API_PORT="${API_PORT:-33080}"
LOG_LEVEL="${LOG_LEVEL:-info}"
REPO_KIND="${REPO_KIND:-fs}"
FS_FSYNC_FILE="${FS_FSYNC_FILE:-true}"
FS_FSYNC_DIR="${FS_FSYNC_DIR:-true}"
NIMBLE_BIN="${NIMBLE_BIN:-nimble}"
BUILD_ARCHIVIST="${BUILD_ARCHIVIST:-true}"
ENABLE_SERVICE="${ENABLE_SERVICE:-true}"
START_SERVICE="${START_SERVICE:-false}"
SUDO_BIN="${SUDO_BIN:-sudo}"

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "ERROR: missing command '$1'" >&2
    exit 1
  fi
}

need_cmd "$NIMBLE_BIN"
need_cmd systemctl
need_cmd install
need_cmd awk

if [[ ! -d "$ARCHIVIST_SRC" ]]; then
  echo "ERROR: ARCHIVIST_SRC does not exist: $ARCHIVIST_SRC" >&2
  exit 1
fi

P2P_PORT="$(awk -F/ '{print $NF}' <<<"$LISTEN_ADDR")"
if [[ -z "$P2P_PORT" || "$P2P_PORT" == "$LISTEN_ADDR" ]]; then
  echo "ERROR: LISTEN_ADDR must end with /tcp/<port>, got: $LISTEN_ADDR" >&2
  exit 1
fi

if [[ "$P2P_PORT" == "$API_PORT" || "$P2P_PORT" == "$DISC_PORT" || "$API_PORT" == "$DISC_PORT" ]]; then
  echo "ERROR: P2P/API/DISC ports must be unique. Got p2p=$P2P_PORT api=$API_PORT disc=$DISC_PORT" >&2
  exit 1
fi

echo "[1/7] Creating ETH2077 storage and temp directories"
"$SUDO_BIN" mkdir -p "$DATA_DIR" "$TMP_DIR"
"$SUDO_BIN" chown -R "${SERVICE_USER}:${SERVICE_GROUP}" "$DATA_DIR" "$TMP_DIR"

echo "[2/7] Wiring Archivist source to ${ARCHIVIST_LINK}"
"$SUDO_BIN" ln -sfn "$ARCHIVIST_SRC" "$ARCHIVIST_LINK"

if [[ "$BUILD_ARCHIVIST" == "true" ]]; then
  echo "[3/7] Building Archivist via ${NIMBLE_BIN}"
  (
    cd "$ARCHIVIST_LINK"
    export TMPDIR="$TMP_DIR"
    export TMP="$TMP_DIR"
    export TEMP="$TMP_DIR"
    "$NIMBLE_BIN" build
  )
else
  echo "[3/7] Skipping Archivist build (BUILD_ARCHIVIST=${BUILD_ARCHIVIST})"
fi

echo "[4/7] Rendering systemd unit"
UNIT_TMP="$(mktemp "/tmp/${SERVICE_NAME}.XXXXXX.service")"
cat >"$UNIT_TMP" <<EOF
[Unit]
Description=Archivist Node (ETH2077)
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=${SERVICE_USER}
Group=${SERVICE_GROUP}
WorkingDirectory=${ARCHIVIST_LINK}
Environment=TMPDIR=${TMP_DIR}
Environment=TMP=${TMP_DIR}
Environment=TEMP=${TMP_DIR}
ExecStart=${ARCHIVIST_LINK}/build/archivist \\
  --data-dir=${DATA_DIR} \\
  --listen-addrs=${LISTEN_ADDR} \\
  --disc-port=${DISC_PORT} \\
  --api-bindaddr=${API_BINDADDR} \\
  --api-port=${API_PORT} \\
  --log-level=${LOG_LEVEL} \\
  --repo-kind=${REPO_KIND} \\
  --fs-fsync-file=${FS_FSYNC_FILE} \\
  --fs-fsync-dir=${FS_FSYNC_DIR}
Restart=always
RestartSec=3
LimitNOFILE=65535
NoNewPrivileges=true
ProtectHome=true
PrivateTmp=true

[Install]
WantedBy=multi-user.target
EOF

echo "[5/7] Installing ${SERVICE_NAME}.service"
"$SUDO_BIN" install -m 0644 "$UNIT_TMP" "/etc/systemd/system/${SERVICE_NAME}.service"
rm -f "$UNIT_TMP"

echo "[6/7] Reloading systemd daemon"
"$SUDO_BIN" systemctl daemon-reload

if [[ "$ENABLE_SERVICE" == "true" ]]; then
  echo "[7/7] Enabling ${SERVICE_NAME}.service"
  "$SUDO_BIN" systemctl enable "${SERVICE_NAME}.service"
else
  echo "[7/7] Skipping enable (ENABLE_SERVICE=${ENABLE_SERVICE})"
fi

if [[ "$START_SERVICE" == "true" ]]; then
  echo "Starting ${SERVICE_NAME}.service"
  "$SUDO_BIN" systemctl restart "${SERVICE_NAME}.service"
fi

echo
echo "ETH2077 setup complete."
echo "Service: ${SERVICE_NAME}.service"
echo "Data dir: ${DATA_DIR}"
echo "API: http://127.0.0.1:${API_PORT}"
echo
echo "Next:"
echo "  sudo systemctl status ${SERVICE_NAME}.service"
echo "  ./scripts/production/eth2077/preflight_archivist_eth2077.sh ${ENV_FILE}"

