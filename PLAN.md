# tracelift — implementation plan

Fast agent-trace processing: parse and query large LLM/agent trace files
(JSONL and OTLP-JSON) to answer where latency went, what each step cost,
and which tool calls failed. Rust parsing core, polars-facing Python API,
one-command CLI report.

Positioning: "polars for agent traces". Not an observability platform —
no collection, no storage, no UI. It reads trace files you already have.

## Normalized span schema

Every input format maps into this flat model. One row per span.

| column            | type    | notes                                          |
|-------------------|---------|------------------------------------------------|
| trace_id          | str     | required; spans missing it are counted+skipped |
| span_id           | str     | required                                       |
| parent_span_id    | str?    | null for root spans                            |
| name              | str     | span/operation display name                    |
| kind              | str     | llm \| tool \| agent \| chain \| other         |
| start_ns          | i64     | unix epoch nanoseconds                         |
| end_ns            | i64     | unix epoch nanoseconds                         |
| duration_ms       | f64     | derived: (end_ns - start_ns) / 1e6             |
| provider          | str?    | gen_ai.provider.name                           |
| model             | str?    | response model, falling back to request model  |
| input_tokens      | i64?    | gen_ai.usage.input_tokens                      |
| output_tokens     | i64?    | gen_ai.usage.output_tokens                     |
| cache_read_tokens | i64?    | gen_ai.usage.cache_read.input_tokens           |
| cost              | f64?    | taken from trace if present; never estimated   |
| tool_name         | str?    | gen_ai.tool.name                               |
| tool_call_id      | str?    | gen_ai.tool.call.id                            |
| agent_name        | str?    | gen_ai.agent.name                              |
| conversation_id   | str?    | gen_ai.conversation.id                         |
| error_type        | str?    | error.type; null means span succeeded          |
| status            | str     | ok \| error \| unset                           |

Design rules:
- cost is read, never computed. Pricing tables go stale; estimating
  silently is worse than a null column. A pricing helper can come later
  as an explicit opt-in.
- kind is inferred from gen_ai.operation.name / tool attributes when the
  emitter does not set one.

## Attribute mapping (pinned 2026-06-10, OTel semconv registry)

Current names (canonical):

| semconv attribute                       | column            |
|-----------------------------------------|-------------------|
| gen_ai.provider.name                    | provider          |
| gen_ai.request.model                    | model (fallback)  |
| gen_ai.response.model                   | model (primary)   |
| gen_ai.usage.input_tokens               | input_tokens      |
| gen_ai.usage.output_tokens              | output_tokens     |
| gen_ai.usage.cache_read.input_tokens    | cache_read_tokens |
| gen_ai.operation.name                   | kind inference    |
| gen_ai.tool.name                        | tool_name         |
| gen_ai.tool.call.id                     | tool_call_id      |
| gen_ai.agent.name                       | agent_name        |
| gen_ai.conversation.id                  | conversation_id   |
| error.type                              | error_type        |

Deprecated aliases, accepted on ingest (most emitters still use them):

| deprecated                    | maps to                     |
|-------------------------------|-----------------------------|
| gen_ai.system                 | gen_ai.provider.name        |
| gen_ai.usage.prompt_tokens    | gen_ai.usage.input_tokens   |
| gen_ai.usage.completion_tokens| gen_ai.usage.output_tokens  |

Canonical wins when both forms are present.

## Input formats, v0.1

1. JSONL: one span object per line. Accepts either flat keys matching the
   schema above or an `attributes` object holding semconv keys.
   Malformed lines: skip, count, report. The parser must never panic.
2. OTLP-JSON: the `resourceSpans -> scopeSpans -> spans` shape produced by
   OTel SDK file exporters and `otelcol` file exporter. Attribute values
   unwrapped from the `{"stringValue": ...}` envelopes.

Out of scope v0.1: live collection, OTLP protobuf, storage, UI, sampling,
cost estimation.

## Repo layout

    tracelift/
      Cargo.toml              workspace
      core/                   rust crate tracelift-core (parsing, no pyo3)
      bindings/               rust crate tracelift-py (pyo3 wrapper)
      python/tracelift/       python package: API, queries, CLI
      tests/                  pytest suite + fixtures
      bench/                  benchmark scripts + results
      .github/workflows/      ci.yml, release.yml

Split core/bindings so the core stays testable with plain cargo test and
usable from a future native CLI.

## Python API (frozen for v0.1)

    tracelift.load(path, format="auto") -> TraceSet
    TraceSet.df                -> polars.DataFrame (the escape hatch)
    TraceSet.failures()        -> polars.DataFrame
    TraceSet.cost_by(key)      -> polars.DataFrame, key in {model, tool_name, agent_name, provider}
    TraceSet.token_totals()    -> dict
    TraceSet.slowest(n=10)     -> polars.DataFrame
    TraceSet.latency_breakdown(trace_id) -> polars.DataFrame (tree order)
    TraceSet.summary()         -> Summary (drives the CLI report)
    load_report(path)          -> skipped-line / skipped-span counts

CLI: `tracelift summarize FILE [--format jsonl|otlp|auto] [--json]`

## Phases

0. this plan
1. core crate: model, jsonl + otlp parsers, columnar batch output,
   fixture tests, malformed-input tests. gate: cargo test green, garbage
   bytes never panic.
2. pyo3 bindings + python package + queries. gate: pytest green, parse
   releases the GIL.
3. cli + terminal report. gate: runs on fixtures.
4. dogfood: SolarScout adapter emitting schema JSONL; month-of-traces
   analysis is the primary launch asset (aggregates only, no customer
   data). synthetic traces remain for benchmarks.
5. bench vs pandas baseline + README + CI + wheels + PyPI.

## Risks

- OTLP-JSON shape variance between SDKs: pin fixtures from real exporter
  output, document what is supported.
- Schema lock-in: the flat model is v0.1 surface; additive columns are
  fine, renames are not.
- Benchmark honesty: state hardware, warmup, median-of-N; compare against
  an idiomatic pandas script, not a strawman.
