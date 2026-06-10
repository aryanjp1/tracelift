"""Child process for bench/run.py: one load+summarize, prints 'seconds max_rss_kb'."""

from __future__ import annotations

import resource
import sys
import time


def run_tracelift(path: str) -> None:
    import tracelift

    traces = tracelift.load(path)
    traces.summary()


def run_pandas(path: str) -> None:
    import json

    import pandas as pd

    rows = []
    with open(path) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                obj = json.loads(line)
            except json.JSONDecodeError:
                continue
            attrs = obj.get("attributes", {}) or {}
            if "trace_id" not in obj or "span_id" not in obj:
                continue
            rows.append(
                {
                    "trace_id": obj.get("trace_id"),
                    "span_id": obj.get("span_id"),
                    "model": attrs.get("gen_ai.response.model") or attrs.get("gen_ai.request.model"),
                    "tool_name": attrs.get("gen_ai.tool.name"),
                    "input_tokens": attrs.get("gen_ai.usage.input_tokens"),
                    "output_tokens": attrs.get("gen_ai.usage.output_tokens"),
                    "cost": attrs.get("cost"),
                    "status": "error" if obj.get("error_type") else obj.get("status", "unset"),
                    "duration_ms": (obj.get("end_ns", 0) - obj.get("start_ns", 0)) / 1e6,
                }
            )
    df = pd.DataFrame(rows)
    df[df.status == "error"]
    df.groupby("model")["cost"].sum()
    df.groupby("tool_name")["duration_ms"].median()
    df[["input_tokens", "output_tokens"]].sum()
    df.nlargest(5, "duration_ms")


def main() -> None:
    mode, path = sys.argv[1], sys.argv[2]
    start = time.perf_counter()
    if mode == "tracelift":
        run_tracelift(path)
    elif mode == "pandas":
        run_pandas(path)
    else:
        raise SystemExit(f"unknown mode {mode}")
    elapsed = time.perf_counter() - start
    max_rss_kb = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
    print(f"{elapsed:.3f} {max_rss_kb}")


if __name__ == "__main__":
    main()
