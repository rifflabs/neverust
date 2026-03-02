#!/usr/bin/env bash
set -euo pipefail

# Comprehensive cross-cutting validation suite for:
# - Neverust
# - Archivist node (Nim)
# - FFI interop parity
# - proving-related and storage durability paths
#
# Usage:
#   scripts/production/comprehensive_crosscut_suite.sh
#
# Optional env:
#   ARCHIVIST_NODE_FFI_ROOT=/path/to/archivist-node
#   FULL=1  (run longer/optional suites)

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ARCHIVIST_FFI_ROOT_DEFAULT="$ROOT_DIR/.ffi/archivist-node-mempatch"
ARCHIVIST_NODE_FFI_ROOT="${ARCHIVIST_NODE_FFI_ROOT:-$ARCHIVIST_FFI_ROOT_DEFAULT}"
FULL="${FULL:-0}"
TMPDIR_DEFAULT="/mnt/riffcastle/archivist/tmp"
TMPDIR="${TMPDIR:-$TMPDIR_DEFAULT}"
LATEST_NIMBLE_DEFAULT="/tmp/nimble-latest/src/nimble"
if [[ -z "${NIMBLE_BIN:-}" ]]; then
  if [[ -x "$LATEST_NIMBLE_DEFAULT" ]]; then
    NIMBLE_BIN="$LATEST_NIMBLE_DEFAULT"
  else
    NIMBLE_BIN="nimble"
  fi
fi
NIMBLE_DIR_DEFAULT="$ROOT_DIR/.tmp/nimble-home"
NIMBLE_DIR="${NIMBLE_DIR:-$NIMBLE_DIR_DEFAULT}"
CARGO_HOME_DEFAULT="$ROOT_DIR/.tmp/cargo-home"
CARGO_HOME="${CARGO_HOME:-$CARGO_HOME_DEFAULT}"
CARGO_TARGET_DIR_DEFAULT="$ROOT_DIR/.tmp/cargo-target"
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$CARGO_TARGET_DIR_DEFAULT}"

if [[ ! -d "$ROOT_DIR" ]]; then
  echo "ERROR: repository root not found" >&2
  exit 1
fi
if [[ ! -d "$ARCHIVIST_NODE_FFI_ROOT" ]]; then
  echo "ERROR: ARCHIVIST_NODE_FFI_ROOT does not exist: $ARCHIVIST_NODE_FFI_ROOT" >&2
  exit 1
fi
mkdir -p "$TMPDIR" || true
tmp_probe=""
if [[ -d "$TMPDIR" ]]; then
  tmp_probe="$(mktemp -p "$TMPDIR" suite-probe.XXXXXX 2>/dev/null || true)"
fi
if [[ -z "$tmp_probe" ]]; then
  TMPDIR="$ROOT_DIR/.tmp/tmp"
  mkdir -p "$TMPDIR"
else
  rm -f "$tmp_probe"
fi
mkdir -p "$NIMBLE_DIR"
mkdir -p "$CARGO_HOME"
mkdir -p "$CARGO_TARGET_DIR"
export TMPDIR
export TMP="$TMPDIR"
export TEMP="$TMPDIR"
export NIMBLE_DIR
export CARGO_HOME
export CARGO_TARGET_DIR

run_step() {
  local name="$1"
  shift
  echo
  echo "==== $name ===="
  "$@"
}

cd "$ROOT_DIR"

run_step "Toolchain sanity" \
  bash -lc "nim --version && \"$NIMBLE_BIN\" --version && cargo --version && cmake --version | head -n 1 && rustc --version"

run_step "Neverust build (release)" \
  cargo build --release

run_step "Archivist build (nimble)" \
  bash -lc "cd \"$ARCHIVIST_NODE_FFI_ROOT\" && \"$NIMBLE_BIN\" build"

run_step "Neverust core targeted tests" \
  cargo test -p neverust-core --test archivist_parity_test -- --nocapture

run_step "Neverust blockstore integrity" \
  cargo test -p neverust-core --test blockstore_integrity_retrieval -- --nocapture

run_step "FFI interop parity (Archivist -> Neverust retrieval proof)" \
  bash -lc "ARCHIVIST_NODE_FFI_ROOT=\"$ARCHIVIST_NODE_FFI_ROOT\" cargo test -p neverust-core --test archivist_ffi_interop -- --nocapture"

run_step "Neverust API + transport integration tests" \
  cargo test --test integration_test -- --nocapture

run_step "Neverust block exchange tests" \
  cargo test --test block_exchange_test -- --nocapture

if [[ "$FULL" == "1" ]]; then
  run_step "Archivist node tests" \
    bash -lc "cd \"$ARCHIVIST_NODE_FFI_ROOT\" && nim c -r tests/testNode"

  run_step "Archivist contract tests" \
    bash -lc "cd \"$ARCHIVIST_NODE_FFI_ROOT\" && nim c -r tests/testContracts"

  run_step "Archivist integration tests (long)" \
    bash -lc "cd \"$ARCHIVIST_NODE_FFI_ROOT\" && nim c -r tests/testIntegration"

  run_step "Neverust performance tests" \
    cargo test --test performance_test -- --nocapture
fi

echo
echo "Comprehensive suite completed successfully."
