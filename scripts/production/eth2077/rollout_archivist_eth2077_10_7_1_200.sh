#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ENV_FILE="${1:-$SCRIPT_DIR/archivist-eth2077.10-7-1-200.env}"

if [[ ! -f "$ENV_FILE" ]]; then
  echo "ERROR: env file not found: $ENV_FILE" >&2
  exit 1
fi

set -a
source "$ENV_FILE"
set +a

SERVICE_NAME="${SERVICE_NAME:-archivist-eth2077}"
START_AFTER_SETUP="${START_AFTER_SETUP:-true}"
EDGE_IP="${EDGE_IP:-10.7.1.200}"
API_PORT="${API_PORT:-33180}"
API_BINDADDR="${API_BINDADDR:-0.0.0.0}"

API_HOST="$API_BINDADDR"
if [[ "$API_HOST" == "0.0.0.0" ]]; then
  API_HOST="127.0.0.1"
fi

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "ERROR: missing command '$1'" >&2
    exit 1
  fi
}

need_cmd bash
need_cmd curl
need_cmd sudo

echo "[1/6] Shell syntax gate"
bash -n "$SCRIPT_DIR/setup_archivist_eth2077.sh"
bash -n "$SCRIPT_DIR/preflight_archivist_eth2077.sh"
echo "BASH_SYNTAX=PASS"

echo "[2/6] Setup + install service"
"$SCRIPT_DIR/setup_archivist_eth2077.sh" "$ENV_FILE"

if [[ "$START_AFTER_SETUP" == "true" ]]; then
  echo "[3/6] Restart service"
  sudo systemctl restart "${SERVICE_NAME}.service"
else
  echo "[3/6] Skip restart (START_AFTER_SETUP=${START_AFTER_SETUP})"
fi

echo "[4/6] Preflight gate"
"$SCRIPT_DIR/preflight_archivist_eth2077.sh" "$ENV_FILE"

echo "[5/6] API smoke"
curl -fsS "http://${API_HOST}:${API_PORT}/health" | sed -n '1,1p'
curl -fsS "http://${API_HOST}:${API_PORT}/api/archivist/v1/stats" | sed -n '1,1p'

echo "[6/6] TLS host checks"
for host in archivist.riff.cc neverust.riff.cc archcluster.riff.cc 2077.riff.cc explorer.riff.cc wallet.riff.cc market.riff.cc rpc.riff.cc; do
  if curl -kfsS --resolve "${host}:443:${EDGE_IP}" "https://${host}/" >/dev/null; then
    echo "HOST_OK ${host}"
  else
    echo "HOST_WARN ${host}"
  fi
done

echo
echo "ROLLOUT_STATUS=PASS"
echo "SERVICE=${SERVICE_NAME}.service"
echo "ENV_FILE=${ENV_FILE}"
