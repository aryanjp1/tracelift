"""Benchmark tracelift against a pandas baseline on the same file.

Usage: python bench/run.py traces.jsonl [--repeat 3]
Each measurement runs in a fresh subprocess; wall time is measured inside
the child, peak RSS is taken from the child's rusage. Honest numbers:
median of N repeats, hardware stated by you when publishing.
"""

from __future__ import annotations

import argparse
import os
import statistics
import subprocess
import sys

MEASURE = os.path.join(os.path.dirname(__file__), "_measure.py")


def run_once(mode: str, stage: str, path: str) -> tuple[float, float]:
    result = subprocess.run(
        [sys.executable, MEASURE, mode, stage, path], stdout=subprocess.PIPE, check=True
    )
    elapsed, max_rss_kb = result.stdout.decode().split()
    return float(elapsed), float(max_rss_kb) / 1024


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("file")
    parser.add_argument("--repeat", type=int, default=3)
    args = parser.parse_args()

    size_mb = os.path.getsize(args.file) / 1e6
    print(f"file: {args.file} ({size_mb:.0f} MB), median of {args.repeat}\n")

    for stage in ("load", "full"):
        label = "load only" if stage == "load" else "load + summary"
        print(f"[{label}]")
        print(f"  {'engine':<10} {'median s':>9} {'peak RSS MB':>12}")
        for mode in ("tracelift", "pandas"):
            times, rss = [], []
            for _ in range(args.repeat):
                t, m = run_once(mode, stage, args.file)
                times.append(t)
                rss.append(m)
            print(f"  {mode:<10} {statistics.median(times):>9.2f} {max(rss):>12.0f}")
        print()


if __name__ == "__main__":
    main()
