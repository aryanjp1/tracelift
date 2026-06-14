"""Child process for bench/run.py: one load+summarize, prints 'seconds max_rss_kb'."""

from __future__ import annotations

import resource
import sys
import time


def load_tracelift(path: str):
    import tracelift

    return tracelift.load(path)


def load_pandas(path: str):
    """Idiomatic pandas load into the same normalized schema tracelift produces."""
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
            if "trace_id" not in obj or "span_id" not in obj:
                continue
            attrs = obj.get("attributes", {}) or {}
            rows.append(
                {
                    "trace_id": obj.get("trace_id"),
                    "span_id": obj.get("span_id"),
                    "parent_span_id": obj.get("parent_span_id"),
                    "name": obj.get("name"),
                    "kind": obj.get("kind"),
                    "start_ns": obj.get("start_ns"),
                    "end_ns": obj.get("end_ns"),
                    "provider": attrs.get("gen_ai.provider.name") or attrs.get("gen_ai.system"),
                    "model": attrs.get("gen_ai.response.model") or attrs.get("gen_ai.request.model"),
                    "input_tokens": attrs.get("gen_ai.usage.input_tokens")
                    or attrs.get("gen_ai.usage.prompt_tokens"),
                    "output_tokens": attrs.get("gen_ai.usage.output_tokens")
                    or attrs.get("gen_ai.usage.completion_tokens"),
                    "tool_name": attrs.get("gen_ai.tool.name"),
                    "cost": attrs.get("cost"),
                    "status": "error" if obj.get("error_type") else obj.get("status", "unset"),
                }
            )
    df = pd.DataFrame(rows)
    df["duration_ms"] = (df["end_ns"].fillna(0) - df["start_ns"].fillna(0)) / 1e6
    return df


def summarize_pandas(df) -> None:
    df[df.status == "error"]
    df.groupby("model")["cost"].sum()
    df.groupby("tool_name")["duration_ms"].median()
    df[["input_tokens", "output_tokens"]].sum()
    df.nlargest(5, "duration_ms")


def main() -> None:
    mode, stage, path = sys.argv[1], sys.argv[2], sys.argv[3]
    start = time.perf_counter()
    if mode == "tracelift":
        traces = load_tracelift(path)
        if stage == "full":
            traces.summary()
    elif mode == "pandas":
        df = load_pandas(path)
        if stage == "full":
            summarize_pandas(df)
    else:
        raise SystemExit(f"unknown mode {mode}")
    elapsed = time.perf_counter() - start
    max_rss_kb = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
    print(f"{elapsed:.3f} {max_rss_kb}")


if __name__ == "__main__":
    main()
