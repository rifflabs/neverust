#!/usr/bin/env bash
set -euo pipefail

RESULTS_CSV="${1:-/tmp/neverust_primitive_runs.csv}"
DATA_ROOT="${2:-/tmp/neverust-autonomous-research}"

BACKENDS="${BACKENDS:-redb deltaflat deltastore geomtree}"
PIPELINES="${PIPELINES:-xor,blake3,index_mod:1048576 xor,blake3,index_xorfold:1048576}"
SINGLE_BLOCKS="${SINGLE_BLOCKS:-20000}"
SINGLE_BLOCK_SIZE="${SINGLE_BLOCK_SIZE:-262144}"
SINGLE_WORKERS="${SINGLE_WORKERS:-8}"
SINGLE_BATCH="${SINGLE_BATCH:-128}"

MULTI_NODES="${MULTI_NODES:-3}"
MULTI_REPLICATION="${MULTI_REPLICATION:-2}"
MULTI_BLOCKS="${MULTI_BLOCKS:-24000}"
MULTI_BLOCK_SIZE="${MULTI_BLOCK_SIZE:-131072}"
MULTI_WORKERS_PER_NODE="${MULTI_WORKERS_PER_NODE:-4}"
MULTI_BATCH="${MULTI_BATCH:-128}"

VERIFY_STRIDE="${VERIFY_STRIDE:-128}"
VERIFY_SAMPLES="${VERIFY_SAMPLES:-512}"
CLEAR_EXISTING="${CLEAR_EXISTING:-true}"

mkdir -p "$(dirname "$RESULTS_CSV")"
mkdir -p "$DATA_ROOT"

if [ ! -f "$RESULTS_CSV" ]; then
  cat > "$RESULTS_CSV" <<'CSV'
run_id,p_mode,p_backend,p_pipeline,p_nodes,p_replication,p_block_size,p_workers,p_batch,throughput_mibps,p99_ms,cpu_pct,mem_mb,write_amp,read_amp,durability_score,correctness_failures,reorder_violations,gc_violations,notes
CSV
fi

extract_metric() {
  local key="$1"
  local file="$2"
  awk -F'=' -v k="$key" '$1==k {print $2}' "$file" | tail -n 1
}

append_row() {
  local run_id="$1"
  local mode="$2"
  local backend="$3"
  local pipeline="$4"
  local nodes="$5"
  local repl="$6"
  local block_size="$7"
  local workers="$8"
  local batch="$9"
  local out_file="${10}"

  local throughput verify logical physical elapsed
  throughput="$(extract_metric THROUGHPUT_MiBPS "$out_file")"
  verify="$(extract_metric VERIFICATION_FAILURES "$out_file")"
  logical="$(extract_metric LOGICAL_BYTES "$out_file")"
  physical="$(extract_metric PHYSICAL_BYTES "$out_file")"
  elapsed="$(extract_metric ELAPSED_SEC "$out_file")"

  [ -z "$throughput" ] && throughput="0"
  [ -z "$verify" ] && verify="9999"
  [ -z "$logical" ] && logical="0"
  [ -z "$physical" ] && physical="0"
  [ -z "$elapsed" ] && elapsed="0"

  local write_amp durability notes
  if [ "$logical" = "0" ]; then
    write_amp="0"
  else
    write_amp="$(awk -v p="$physical" -v l="$logical" 'BEGIN { if (l <= 0) print 0; else printf("%.6f", p/l) }')"
  fi

  if [ "$verify" = "0" ]; then
    durability="1.0"
    notes="ok"
  else
    durability="0.2"
    notes="verification_failures=${verify}"
  fi

  # p99/cpu/mem/read_amp are placeholders unless external profilers are wired.
  echo "${run_id},${mode},${backend},${pipeline},${nodes},${repl},${block_size},${workers},${batch},${throughput},0,0,0,${write_amp},1.0,${durability},${verify},0,0,${notes}" >> "$RESULTS_CSV"
}

run_single() {
  local backend="$1"
  local pipeline="$2"
  local run_id="single-${backend}-$(date +%s%N)"
  local out_file="/tmp/${run_id}.out"
  local dir="${DATA_ROOT}/${run_id}"

  cargo run --release --example primitive_pipeline_bench -- \
    real "$backend" "$dir" "$SINGLE_BLOCKS" "$SINGLE_BLOCK_SIZE" "$SINGLE_WORKERS" "$SINGLE_BATCH" \
    "$pipeline" "$VERIFY_STRIDE" "$VERIFY_SAMPLES" "$CLEAR_EXISTING" \
    | tee "$out_file"

  append_row "$run_id" "real" "$backend" "$pipeline" "1" "1" "$SINGLE_BLOCK_SIZE" "$SINGLE_WORKERS" "$SINGLE_BATCH" "$out_file"
}

run_multi() {
  local backend="$1"
  local pipeline="$2"
  local run_id="multi-${backend}-$(date +%s%N)"
  local out_file="/tmp/${run_id}.out"
  local dir="${DATA_ROOT}/${run_id}"

  cargo run --release --example primitive_pipeline_bench -- \
    real-multinode "$backend" "$dir" "$MULTI_NODES" "$MULTI_REPLICATION" "$MULTI_BLOCKS" "$MULTI_BLOCK_SIZE" "$MULTI_WORKERS_PER_NODE" "$MULTI_BATCH" \
    "$pipeline" "$VERIFY_STRIDE" "$VERIFY_SAMPLES" "$CLEAR_EXISTING" \
    | tee "$out_file"

  append_row "$run_id" "real-multinode" "$backend" "$pipeline" "$MULTI_NODES" "$MULTI_REPLICATION" "$MULTI_BLOCK_SIZE" "$MULTI_WORKERS_PER_NODE" "$MULTI_BATCH" "$out_file"
}

echo "[autonomous] results_csv=${RESULTS_CSV}"
echo "[autonomous] data_root=${DATA_ROOT}"

for backend in $BACKENDS; do
  for pipeline in $PIPELINES; do
    run_single "$backend" "$pipeline"
    run_multi "$backend" "$pipeline"
  done
done

echo "[autonomous] sweep complete"
echo "[autonomous] running autoimprover"
cargo run --release --example primitive_tradeoff_autoimprover -- analyze "$RESULTS_CSV" 25 25

if [ -x scripts/vice_scoreboard_from_csv.py ]; then
  echo "[autonomous] running VICE scoreboard"
  python3 scripts/vice_scoreboard_from_csv.py "$RESULTS_CSV"
fi
