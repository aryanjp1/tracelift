from __future__ import annotations

import argparse
import json
import sys
from datetime import datetime, timezone

import polars as pl

from tracelift import __version__, load
from tracelift._frame import Summary


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="tracelift", description="Analyze agent/LLM trace files")
    parser.add_argument("-V", "--version", action="version", version=f"tracelift {__version__}")
    sub = parser.add_subparsers(dest="command", required=True)

    summarize = sub.add_parser("summarize", help="print a report for a trace file")
    summarize.add_argument("file", help="path to a JSONL or OTLP-JSON trace file")
    summarize.add_argument("--format", choices=["auto", "jsonl", "otlp"], default="auto")
    summarize.add_argument("--json", action="store_true", help="emit the summary as JSON")
    summarize.add_argument("--slowest", type=int, default=5, metavar="N")

    args = parser.parse_args(argv)
    try:
        traces = load(args.file, format=args.format)
    except (OSError, ValueError) as e:
        print(f"tracelift: {e}", file=sys.stderr)
        return 1

    if traces.df.height == 0:
        print(f"tracelift: no spans parsed from {args.file} "
              f"({traces.report.skipped} lines skipped)", file=sys.stderr)
        for sample in traces.report.error_samples[:5]:
            print(f"  {sample}", file=sys.stderr)
        return 1

    summary = traces.summary()
    if args.json:
        payload = summary.to_dict()
        payload["skipped_lines"] = traces.report.skipped
        print(json.dumps(payload, indent=2, default=str))
    else:
        print_report(args.file, summary, traces.report.skipped, traces.slowest(args.slowest))
    return 0


def print_report(path: str, s: Summary, skipped: int, slowest: pl.DataFrame) -> None:
    def ts(ns: int | None) -> str:
        if ns is None:
            return "-"
        return datetime.fromtimestamp(ns / 1e9, tz=timezone.utc).strftime("%Y-%m-%d %H:%M:%S")

    skipped_note = f", {skipped} skipped" if skipped else ""
    print(f"{path}: {s.spans} spans across {s.traces} traces{skipped_note}")
    print(f"window  {ts(s.start_ns)} -> {ts(s.end_ns)} UTC")
    print(f"calls   {s.llm_calls} llm / {s.tool_calls} tool")
    print(f"errors  {s.errors} ({s.error_rate:.1%})")
    if s.total_cost is not None:
        print(f"cost    ${s.total_cost:.4f}")
    cache = f" (+{s.cache_read_tokens:,} cached)" if s.cache_read_tokens else ""
    print(f"tokens  {s.input_tokens:,} in / {s.output_tokens:,} out{cache}")

    lat = []
    if s.p50_llm_ms is not None:
        lat.append(f"llm p50 {s.p50_llm_ms:.0f}ms p95 {s.p95_llm_ms:.0f}ms")
    if s.p50_tool_ms is not None:
        lat.append(f"tool p50 {s.p50_tool_ms:.0f}ms p95 {s.p95_tool_ms:.0f}ms")
    if lat:
        print(f"latency {'  '.join(lat)}")

    if s.cost_by_model.height:
        print("\ncost by model")
        print_table(
            s.cost_by_model,
            [("model", 36), ("spans", 6), ("cost", 10), ("input_tokens", 12), ("output_tokens", 13)],
        )
    if s.failing_tools.height:
        print("\nfailing tools")
        print_table(s.failing_tools, [("tool_name", 28), ("failures", 8), ("example", 36)])
    if slowest.height:
        print("\nslowest spans")
        print_table(slowest, [("name", 32), ("kind", 6), ("duration_ms", 11), ("status", 6)])


def print_table(df: pl.DataFrame, columns: list[tuple[str, int]]) -> None:
    header = "  ".join(name.ljust(width) for name, width in columns)
    print(f"  {header}")
    for row in df.to_dicts():
        cells = []
        for name, width in columns:
            value = row.get(name)
            if value is None:
                text = "-"
            elif name == "cost":
                text = f"${value:.4f}"
            elif isinstance(value, float):
                text = f"{value:.1f}"
            elif isinstance(value, int):
                text = f"{value:,}"
            else:
                text = str(value)
            cells.append(text[:width].ljust(width))
        print(f"  {'  '.join(cells)}")


if __name__ == "__main__":
    raise SystemExit(main())
