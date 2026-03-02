#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ENV_FILE="${1:-$SCRIPT_DIR/archivist-eth2077.env}"

if [[ ! -f "$ENV_FILE" ]]; then
  echo "ERROR: env file not found: $ENV_FILE" >&2
  exit 1
fi

set -a
source "$ENV_FILE"
set +a

SERVICE_NAME="${SERVICE_NAME:-archivist-eth2077}"
LISTEN_ADDR="${LISTEN_ADDR:-/ip4/0.0.0.0/tcp/33070}"
DISC_PORT="${DISC_PORT:-33090}"
API_BINDADDR="${API_BINDADDR:-0.0.0.0}"
API_PORT="${API_PORT:-33080}"

P2P_PORT="$(awk -F/ '{print $NF}' <<<"$LISTEN_ADDR")"
API_HOST="${API_BINDADDR}"
if [[ "$API_HOST" == "0.0.0.0" ]]; then
  API_HOST="127.0.0.1"
fi
BASE_URL="http://${API_HOST}:${API_PORT}"

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "ERROR: missing command '$1'" >&2
    exit 1
  fi
}

need_cmd curl
need_cmd ss
need_cmd awk

echo "SERVICE=${SERVICE_NAME}.service"
if command -v systemctl >/dev/null 2>&1; then
  if systemctl is-active --quiet "${SERVICE_NAME}.service"; then
    echo "SERVICE_ACTIVE=yes"
  else
    echo "SERVICE_ACTIVE=no"
  fi
fi

echo "PORT_CHECK_BEGIN"
ss -lntu | awk 'NR==1 || /:('"$P2P_PORT"'|'"$DISC_PORT"'|'"$API_PORT"')\b/'
echo "PORT_CHECK_END"

echo "HEALTH_CHECK_BEGIN"
HEALTH_JSON="$(curl -fsS "${BASE_URL}/health")"
echo "$HEALTH_JSON"
echo "HEALTH_CHECK_END"

echo "PEER_ID_CHECK_BEGIN"
PEER_ID_RAW="$(curl -fsS "${BASE_URL}/api/archivist/v1/peer-id")"
echo "$PEER_ID_RAW"
echo "PEER_ID_CHECK_END"

CANARY_PAYLOAD="eth2077-canary-$(date +%s)-$$"
CANARY_CID="$(
  printf "%s" "$CANARY_PAYLOAD" | \
    curl -fsS -X POST "${BASE_URL}/api/archivist/v1/data" \
      -H "content-type: application/octet-stream" \
      --data-binary @-
)"

DOWNLOADED="$(
  curl -fsS "${BASE_URL}/api/archivist/v1/data/${CANARY_CID}/network/stream"
)"

if [[ "$DOWNLOADED" != "$CANARY_PAYLOAD" ]]; then
  echo "ERROR: canary mismatch" >&2
  echo "EXPECTED=${CANARY_PAYLOAD}" >&2
  echo "ACTUAL=${DOWNLOADED}" >&2
  exit 1
fi

echo "CANARY_CID=${CANARY_CID}"
echo "CANARY_UPLOAD_DOWNLOAD=pass"
echo "PREFLIGHT_STATUS=PASS"

