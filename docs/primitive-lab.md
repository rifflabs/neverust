# Primitive Lab

`Primitive Lab` gives two Rust-native tools:

1. `primitive_pipeline_bench`: compose primitives in-memory and benchmark throughput.
2. `primitive_tradeoff_autoimprover`: analyze run CSVs, compute tradeoff matrix + Pareto set, and synthesize better candidates.
3. `scripts/autonomous_moosefs_research.sh`: run real backend sweeps and emit matrix CSV for autoimprover input.

## 1) In-Memory Primitive Pipeline Bench

Run a single-node in-memory composition:

```bash
cargo run --release --example primitive_pipeline_bench -- \
  inmem 300000 1048576 24 xor,blake3,index_mod:4194304 512 64
```

Run a multinode replication simulation:

```bash
cargo run --release --example primitive_pipeline_bench -- \
  multinode 8 2 500000 524288 12 xor,blake3,index_xorfold:4194304 256 128
```

Pipeline tokens:
- `id` / `identity`
- `xor`
- `blake3`
- `sha256`
- `index_mod:<buckets>`
- `index_xorfold:<buckets>`

Run real single-node backend benchmarking:

```bash
cargo run --release --example primitive_pipeline_bench -- \
  real deltaflat /tmp/neverust-real 50000 1048576 16 256 \
  xor,blake3,index_mod:4194304 256 2048 true
```

Run real multinode backend benchmarking:

```bash
cargo run --release --example primitive_pipeline_bench -- \
  real-multinode deltaflat /tmp/neverust-real-multi 4 2 80000 524288 8 128 \
  xor,blake3,index_xorfold:4194304 256 2048 true
```

## 2) Tradeoff Matrix + Autoimprover

Create template:

```bash
cargo run --release --example primitive_tradeoff_autoimprover -- \
  template /tmp/primitive_tradeoff.csv
```

Analyze results:

```bash
cargo run --release --example primitive_tradeoff_autoimprover -- \
  analyze /tmp/primitive_tradeoff.csv 12 16
```

Expected metric columns:
- `throughput_mibps` (maximize)
- `p99_ms` (minimize)
- `cpu_pct` (minimize)
- `mem_mb` (minimize)
- `write_amp` (minimize)
- `read_amp` (minimize)
- `durability_score` (maximize)
- `correctness_failures` (minimize, hard penalty)
- `reorder_violations` (minimize, hard penalty)
- `gc_violations` (minimize, hard penalty)

Primitive columns:
- Prefer `p_*` columns (for example `p_hash`, `p_layout`, `p_commit`).
- If no `p_*` columns exist, all non-metric columns are treated as primitives.

## 3) Autonomous MooseFS Sweep

```bash
scripts/autonomous_moosefs_research.sh \
  /tmp/neverust_primitive_runs.csv \
  /tmp/neverust-autonomous-research
```

Then inspect synthesized recommendations:

```bash
cargo run --release --example primitive_tradeoff_autoimprover -- \
  analyze /tmp/neverust_primitive_runs.csv 25 25
```
