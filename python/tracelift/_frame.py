from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import polars as pl

from tracelift._tracelift import parse_file

SCHEMA: dict[str, pl.DataType] = {
    "trace_id": pl.Utf8,
    "span_id": pl.Utf8,
    "parent_span_id": pl.Utf8,
    "name": pl.Utf8,
    "kind": pl.Utf8,
    "start_ns": pl.Int64,
    "end_ns": pl.Int64,
    "provider": pl.Utf8,
    "model": pl.Utf8,
    "input_tokens": pl.Int64,
    "output_tokens": pl.Int64,
    "cache_read_tokens": pl.Int64,
    "cost": pl.Float64,
    "tool_name": pl.Utf8,
    "tool_call_id": pl.Utf8,
    "agent_name": pl.Utf8,
    "conversation_id": pl.Utf8,
    "error_type": pl.Utf8,
    "status": pl.Utf8,
}

GROUP_KEYS = ("model", "tool_name", "agent_name", "provider", "kind")


@dataclass(frozen=True)
class ParseReport:
    lines_read: int
    spans_parsed: int
    skipped: int
    error_samples: list[str] = field(default_factory=list)


@dataclass(frozen=True)
class Summary:
    spans: int
    traces: int
    start_ns: int | None
    end_ns: int | None
    errors: int
    error_rate: float
    llm_calls: int
    tool_calls: int
    total_cost: float | None
    input_tokens: int
    output_tokens: int
    cache_read_tokens: int
    p50_llm_ms: float | None
    p95_llm_ms: float | None
    p50_tool_ms: float | None
    p95_tool_ms: float | None
    cost_by_model: pl.DataFrame
    failing_tools: pl.DataFrame
    slowest: pl.DataFrame

    def to_dict(self) -> dict[str, Any]:
        out = {k: v for k, v in self.__dict__.items() if not isinstance(v, pl.DataFrame)}
        out["cost_by_model"] = self.cost_by_model.to_dicts()
        out["failing_tools"] = self.failing_tools.to_dicts()
        out["slowest"] = self.slowest.to_dicts()
        return out


def load(path: str | Path, format: str = "auto") -> TraceSet:
    """Parse a trace file into a TraceSet.

    format: "jsonl", "otlp" or "auto" (default; sniffs the file head).
    Malformed lines are skipped and counted in TraceSet.report.
    """
    columns, report = parse_file(str(path), format)
    df = pl.DataFrame(columns, schema=SCHEMA).with_columns(
        ((pl.col("end_ns") - pl.col("start_ns")) / 1e6).alias("duration_ms")
    )
    return TraceSet(df, ParseReport(**report), source=str(path))


class TraceSet:
    """A normalized span table with agent-shaped queries on top.

    The underlying polars DataFrame is always available as .df for any
    analysis these helpers do not cover.
    """

    def __init__(self, df: pl.DataFrame, report: ParseReport, source: str = "") -> None:
        self.df = df
        self.report = report
        self.source = source

    def __len__(self) -> int:
        return self.df.height

    def __repr__(self) -> str:
        return (
            f"TraceSet(spans={self.df.height}, "
            f"traces={self.df['trace_id'].n_unique()}, skipped={self.report.skipped})"
        )

    def failures(self) -> pl.DataFrame:
        return self.df.filter(pl.col("status") == "error").select(
            "trace_id",
            "span_id",
            "name",
            "kind",
            "tool_name",
            "model",
            "error_type",
            "duration_ms",
        )

    def cost_by(self, key: str = "model") -> pl.DataFrame:
        if key not in GROUP_KEYS:
            raise ValueError(f"key must be one of {GROUP_KEYS}, got {key!r}")
        return (
            self.df.filter(pl.col(key).is_not_null())
            .group_by(key)
            .agg(
                pl.len().alias("spans"),
                pl.col("cost").sum().alias("cost"),
                pl.col("input_tokens").sum().alias("input_tokens"),
                pl.col("output_tokens").sum().alias("output_tokens"),
                pl.col("duration_ms").median().alias("p50_ms"),
            )
            .sort("cost", descending=True, nulls_last=True)
        )

    def token_totals(self) -> dict[str, int]:
        sums = self.df.select(
            pl.col("input_tokens").sum().alias("input"),
            pl.col("output_tokens").sum().alias("output"),
            pl.col("cache_read_tokens").sum().alias("cache_read"),
        ).row(0)
        input_t, output_t, cache_t = (int(v or 0) for v in sums)
        return {
            "input": input_t,
            "output": output_t,
            "cache_read": cache_t,
            "total": input_t + output_t,
        }

    def slowest(self, n: int = 10) -> pl.DataFrame:
        return (
            self.df.sort("duration_ms", descending=True)
            .head(n)
            .select(
                "trace_id",
                "span_id",
                "name",
                "kind",
                "model",
                "tool_name",
                "duration_ms",
                "status",
            )
        )

    def latency_breakdown(self, trace_id: str) -> pl.DataFrame:
        """Spans of one trace in tree order with depth and self time.

        self_ms is the span's duration minus the summed duration of its
        direct children — where the time was actually spent.
        """
        spans = self.df.filter(pl.col("trace_id") == trace_id).sort("start_ns")
        if spans.height == 0:
            raise KeyError(f"trace_id {trace_id!r} not found")

        rows = spans.to_dicts()
        by_parent: dict[str | None, list[dict]] = {}
        child_time: dict[str, float] = {}
        ids = {r["span_id"] for r in rows}
        for r in rows:
            parent = r["parent_span_id"] if r["parent_span_id"] in ids else None
            by_parent.setdefault(parent, []).append(r)
            if parent is not None:
                child_time[parent] = child_time.get(parent, 0.0) + r["duration_ms"]

        ordered: list[dict] = []

        def walk(parent: str | None, depth: int) -> None:
            for r in by_parent.get(parent, []):
                ordered.append(
                    {
                        "depth": depth,
                        "span_id": r["span_id"],
                        "name": r["name"],
                        "kind": r["kind"],
                        "duration_ms": r["duration_ms"],
                        "self_ms": max(r["duration_ms"] - child_time.get(r["span_id"], 0.0), 0.0),
                        "status": r["status"],
                    }
                )
                walk(r["span_id"], depth + 1)

        walk(None, 0)
        return pl.DataFrame(ordered)

    def summary(self) -> Summary:
        df = self.df
        llm = df.filter(pl.col("kind") == "llm")
        tool = df.filter(pl.col("kind") == "tool")
        errors = df.filter(pl.col("status") == "error").height
        cost = df["cost"].sum()
        tokens = self.token_totals()

        failing_tools = (
            tool.filter(pl.col("status") == "error")
            .group_by("tool_name")
            .agg(pl.len().alias("failures"), pl.col("error_type").drop_nulls().first().alias("example"))
            .sort("failures", descending=True)
        )

        def quantile(frame: pl.DataFrame, q: float) -> float | None:
            if frame.height == 0:
                return None
            value = frame["duration_ms"].quantile(q)
            return float(value) if value is not None else None

        return Summary(
            spans=df.height,
            traces=df["trace_id"].n_unique(),
            start_ns=None if df.height == 0 else int(df["start_ns"].min()),
            end_ns=None if df.height == 0 else int(df["end_ns"].max()),
            errors=errors,
            error_rate=errors / df.height if df.height else 0.0,
            llm_calls=llm.height,
            tool_calls=tool.height,
            total_cost=None if cost is None else float(cost),
            input_tokens=tokens["input"],
            output_tokens=tokens["output"],
            cache_read_tokens=tokens["cache_read"],
            p50_llm_ms=quantile(llm, 0.5),
            p95_llm_ms=quantile(llm, 0.95),
            p50_tool_ms=quantile(tool, 0.5),
            p95_tool_ms=quantile(tool, 0.95),
            cost_by_model=self.cost_by("model"),
            failing_tools=failing_tools,
            slowest=self.slowest(5),
        )
