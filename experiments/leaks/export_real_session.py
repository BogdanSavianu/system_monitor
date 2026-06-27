#!/usr/bin/env python3
"""Export one persisted monitor session from SQLite into ml-trainer CSV format.
"""

from __future__ import annotations

import argparse
import csv
import datetime as dt
import sqlite3
from collections import defaultdict
from pathlib import Path
from typing import DefaultDict


def parse_pid_set(raw: str) -> set[int]:
    values = set()
    raw = raw.strip()
    if not raw:
        return values
    for token in raw.split(","):
        token = token.strip()
        if not token:
            continue
        values.add(int(token))
    return values


def load_session_rows(db_path: Path, session_id: str) -> list[tuple[int, int, str, float]]:
    conn = sqlite3.connect(str(db_path))
    try:
        cur = conn.cursor()
        cur.execute(
            """
            SELECT
                pid,
                collected_at_ms,
                name,
                physical_mem_kb,
                cpu_rel
            FROM process_samples
            WHERE session_id = ?
            ORDER BY pid ASC, collected_at_ms ASC
            """,
            (session_id,),
        )
        rows = cur.fetchall()
    finally:
        conn.close()

    # pid, ts_ms, process_name, mem_kb, cpu_rel
    return [(int(r[0]), int(r[1]), str(r[2]), float(r[3]), float(r[4])) for r in rows]


def determine_label(label_mode: str, pid: int, leak_pids: set[int]) -> int | None:
    if label_mode == "safe":
        return 0
    if label_mode == "mixed":
        return 1 if pid in leak_pids else 0
    if label_mode == "leak_only":
        return 1 if pid in leak_pids else None
    raise ValueError(f"unsupported label mode: {label_mode}")


def export_csv(
    out_csv: Path,
    session_id: str,
    scenario: str,
    min_samples_per_pid: int,
    label_mode: str,
    leak_pids: set[int],
    rows: list[tuple[int, int, str, float, float]],
) -> tuple[int, int, int]:
    grouped: DefaultDict[int, list[tuple[int, str, float, float]]] = defaultdict(list)
    for pid, ts_ms, name, mem_kb, cpu_rel in rows:
        grouped[pid].append((ts_ms, name, mem_kb, cpu_rel))

    out_csv.parent.mkdir(parents=True, exist_ok=True)

    exported_pids = 0
    skipped_short = 0
    exported_rows = 0

    with out_csv.open("w", newline="", encoding="utf-8") as f:
        writer = csv.writer(f)
        writer.writerow(
            [
                "scenario",
                "label",
                "run_id",
                "step",
                "elapsed_s",
                "leaked_kb_step",
                "leaked_kb_total",
                "workload_kb_this_step",
                "observed_memory_kb",
            ]
        )

        for pid in sorted(grouped.keys()):
            samples = grouped[pid]
            if len(samples) < min_samples_per_pid:
                skipped_short += 1
                continue

            label = determine_label(label_mode, pid, leak_pids)
            if label is None:
                continue

            run_id = f"{session_id}:{pid}"
            t0 = samples[0][0]
            leak_total = 0.0
            prev_mem = samples[0][2]

            for step, (ts_ms, _name, mem_kb, cpu_rel) in enumerate(samples):
                elapsed_s = max(0.0, (ts_ms - t0) / 1000.0)
                mem_delta = max(0.0, mem_kb - prev_mem)
                leaked_step = mem_delta if label == 1 else 0.0
                leak_total += leaked_step
                workload = max(0.0, cpu_rel)

                writer.writerow(
                    [
                        scenario,
                        label,
                        run_id,
                        step,
                        f"{elapsed_s:.3f}",
                        f"{leaked_step:.3f}",
                        f"{leak_total:.3f}",
                        f"{workload:.3f}",
                        f"{mem_kb:.3f}",
                    ]
                )

                exported_rows += 1
                prev_mem = mem_kb

            exported_pids += 1

    return exported_pids, skipped_short, exported_rows


def write_meta(
    out_csv: Path,
    session_id: str,
    scenario: str,
    label_mode: str,
    leak_pids: set[int],
    exported_pids: int,
    skipped_short: int,
    exported_rows: int,
) -> None:
    meta_path = Path(str(out_csv) + ".meta")
    now = dt.datetime.utcnow().replace(microsecond=0).isoformat() + "Z"
    with meta_path.open("w", encoding="utf-8") as f:
        f.write(f"session_id={session_id}\n")
        f.write(f"scenario={scenario}\n")
        f.write(f"label_mode={label_mode}\n")
        f.write("leak_pids=" + ",".join(str(p) for p in sorted(leak_pids)) + "\n")
        f.write(f"generated_at={now}\n")
        f.write(f"exported_pids={exported_pids}\n")
        f.write(f"skipped_pids_too_short={skipped_short}\n")
        f.write(f"exported_rows={exported_rows}\n")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Export one monitor session from history.db to ml-trainer CSV"
    )
    parser.add_argument("--db", required=True, help="Path to history.db")
    parser.add_argument("--session-id", required=True, help="Session UUID to export")
    parser.add_argument("--out-csv", required=True, help="Output CSV path")
    parser.add_argument(
        "--scenario",
        required=True,
        help="Scenario name written into CSV rows (e.g. real_safe_s1)",
    )
    parser.add_argument(
        "--label-mode",
        choices=["safe", "mixed", "leak_only"],
        default="safe",
        help="safe=all 0, mixed=only leak pids are 1, leak_only=export only leak pids",
    )
    parser.add_argument(
        "--leak-pids",
        default="",
        help="Comma-separated leak PIDs for mixed/leak_only labeling",
    )
    parser.add_argument(
        "--min-samples-per-pid",
        type=int,
        default=24,
        help="Skip PIDs with fewer than this many samples",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    db_path = Path(args.db)
    out_csv = Path(args.out_csv)
    leak_pids = parse_pid_set(args.leak_pids)

    if not db_path.is_file():
        raise SystemExit(f"db file not found: {db_path}")

    if args.label_mode in {"mixed", "leak_only"} and not leak_pids:
        raise SystemExit("--leak-pids is required for mixed or leak_only labeling")

    rows = load_session_rows(db_path, args.session_id)
    if not rows:
        raise SystemExit(
            f"no rows found in process_samples for session_id={args.session_id}"
        )

    exported_pids, skipped_short, exported_rows = export_csv(
        out_csv=out_csv,
        session_id=args.session_id,
        scenario=args.scenario,
        min_samples_per_pid=args.min_samples_per_pid,
        label_mode=args.label_mode,
        leak_pids=leak_pids,
        rows=rows,
    )

    if exported_rows == 0:
        raise SystemExit("no rows exported (check labels, pid filters, min sample threshold)")

    write_meta(
        out_csv=out_csv,
        session_id=args.session_id,
        scenario=args.scenario,
        label_mode=args.label_mode,
        leak_pids=leak_pids,
        exported_pids=exported_pids,
        skipped_short=skipped_short,
        exported_rows=exported_rows,
    )

    print("real session export complete")
    print(f"db={db_path}")
    print(f"session_id={args.session_id}")
    print(f"out_csv={out_csv}")
    print(f"scenario={args.scenario}")
    print(f"label_mode={args.label_mode}")
    print(f"leak_pids={','.join(str(p) for p in sorted(leak_pids))}")
    print(f"exported_pids={exported_pids}")
    print(f"skipped_pids_too_short={skipped_short}")
    print(f"exported_rows={exported_rows}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
