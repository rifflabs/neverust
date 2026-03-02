#!/usr/bin/env python3
"""
VICE scoreboard for blockstore primitive runs.

VICE here means:
- explicit falsifiable gates,
- no hand-wavy "improved" claims,
- clear PASS/FAIL/OPEN rollups.
"""

from __future__ import annotations

import csv
import json
import os
import sys
from dataclasses import dataclass, asdict
from datetime import datetime, timezone
from pathlib import Path
from typing import Dict, List


@dataclass
class GateResult:
    name: str
    status: str
    observed: str
    threshold: str
    run_id: str


def parse_float(value: str, default: float = 0.0) -> float:
    try:
        return float((value or "").strip())
    except Exception:
        return default


def parse_int(value: str, default: int = 0) -> int:
    try:
        return int(float((value or "").strip()))
    except Exception:
        return default


def gate_integrity_zero_failures(row: Dict[str, str]) -> GateResult:
    v = parse_int(row.get("correctness_failures", "0"), 999999)
    return GateResult(
        name="G01_integrity_zero_failures",
        status="PASS" if v == 0 else "FAIL",
        observed=f"correctness_failures={v}",
        threshold="==0",
        run_id=row.get("run_id", ""),
    )


def gate_ordering_zero_violations(row: Dict[str, str]) -> GateResult:
    v = parse_int(row.get("reorder_violations", "0"), 999999)
    return GateResult(
        name="G02_ordering_zero_violations",
        status="PASS" if v == 0 else "FAIL",
        observed=f"reorder_violations={v}",
        threshold="==0",
        run_id=row.get("run_id", ""),
    )


def gate_gc_zero_violations(row: Dict[str, str]) -> GateResult:
    v = parse_int(row.get("gc_violations", "0"), 999999)
    return GateResult(
        name="G03_gc_zero_violations",
        status="PASS" if v == 0 else "FAIL",
        observed=f"gc_violations={v}",
        threshold="==0",
        run_id=row.get("run_id", ""),
    )


def gate_throughput_floor(row: Dict[str, str], floor_mibps: float) -> GateResult:
    v = parse_float(row.get("throughput_mibps", "0"), 0.0)
    return GateResult(
        name="G04_throughput_floor",
        status="PASS" if v >= floor_mibps else "FAIL",
        observed=f"throughput_mibps={v:.2f}",
        threshold=f">={floor_mibps:.2f}",
        run_id=row.get("run_id", ""),
    )


def gate_write_amp(row: Dict[str, str], max_write_amp: float) -> GateResult:
    v = parse_float(row.get("write_amp", "0"), 9999.0)
    return GateResult(
        name="G05_write_amp_ceiling",
        status="PASS" if v <= max_write_amp else "FAIL",
        observed=f"write_amp={v:.4f}",
        threshold=f"<={max_write_amp:.4f}",
        run_id=row.get("run_id", ""),
    )


def evaluate_row(row: Dict[str, str], throughput_floor: float, write_amp_max: float) -> List[GateResult]:
    return [
        gate_integrity_zero_failures(row),
        gate_ordering_zero_violations(row),
        gate_gc_zero_violations(row),
        gate_throughput_floor(row, throughput_floor),
        gate_write_amp(row, write_amp_max),
    ]


def summarize(gates: List[GateResult]) -> Dict[str, int]:
    summary = {"PASS": 0, "FAIL": 0, "OPEN": 0, "TOTAL": 0}
    for g in gates:
        summary[g.status] = summary.get(g.status, 0) + 1
        summary["TOTAL"] += 1
    return summary


def main() -> int:
    if len(sys.argv) < 2:
        print("Usage: scripts/vice_scoreboard_from_csv.py <runs.csv>")
        return 2

    csv_path = Path(sys.argv[1])
    if not csv_path.exists():
        print(f"ERROR: missing csv: {csv_path}")
        return 2

    throughput_floor = parse_float(os.getenv("VICE_THROUGHPUT_FLOOR_MIBPS", "1024"), 1024.0)
    write_amp_max = parse_float(os.getenv("VICE_WRITE_AMP_MAX", "2.0"), 2.0)

    rows: List[Dict[str, str]] = []
    with csv_path.open(newline="") as f:
        rows = list(csv.DictReader(f))

    all_gates: List[GateResult] = []
    per_run: Dict[str, List[GateResult]] = {}
    for row in rows:
        run_id = row.get("run_id", "")
        gates = evaluate_row(row, throughput_floor, write_amp_max)
        per_run[run_id] = gates
        all_gates.extend(gates)

    summary = summarize(all_gates)

    out = {
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "input_csv": str(csv_path),
        "thresholds": {
            "throughput_floor_mibps": throughput_floor,
            "write_amp_max": write_amp_max,
        },
        "summary": summary,
        "runs": {
            run_id: [asdict(g) for g in gates]
            for run_id, gates in per_run.items()
        },
    }

    out_json = csv_path.with_suffix(".vice-scoreboard.json")
    out_txt = csv_path.with_suffix(".vice-scoreboard.txt")
    out_json.write_text(json.dumps(out, indent=2))

    with out_txt.open("w") as f:
        f.write("VICE Scoreboard\n")
        f.write(f"input_csv={csv_path}\n")
        f.write(
            f"thresholds throughput_floor_mibps={throughput_floor:.2f} write_amp_max={write_amp_max:.4f}\n"
        )
        f.write(
            f"summary PASS={summary['PASS']} FAIL={summary['FAIL']} OPEN={summary['OPEN']} TOTAL={summary['TOTAL']}\n"
        )
        for run_id, gates in per_run.items():
            f.write(f"run={run_id}\n")
            for gate in gates:
                f.write(
                    f"  {gate.name} {gate.status} observed={gate.observed} threshold={gate.threshold}\n"
                )

    print(f"VICE_SCOREBOARD_JSON={out_json}")
    print(f"VICE_SCOREBOARD_TXT={out_txt}")
    print(
        f"VICE_SUMMARY PASS={summary['PASS']} FAIL={summary['FAIL']} OPEN={summary['OPEN']} TOTAL={summary['TOTAL']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
