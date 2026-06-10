# tracelift

Load agent/LLM trace files into [polars](https://pola.rs) and find out
which tool calls failed, what each step cost, and where the latency went.

tracelift reads JSONL and OTLP-JSON trace files through a Rust parsing
core and gives you a normalized span table plus a handful of opinionated
queries. It is a file analyzer, not an observability platform: no
collector, no storage, no UI. If you can export your traces to a file,
you can analyze them.

## Install

```
pip install tracelift
```

## CLI

```
tracelift summarize traces.jsonl
```

```
traces.jsonl: 48211 spans across 3120 traces, 14 skipped
window  2026-05-01 00:02:11 -> 2026-05-31 23:58:40 UTC
calls   18450 llm / 21033 tool
errors  412 (0.9%)
cost    $184.3022
tokens  41,203,118 in / 3,801,440 out (+9,113,021 cached)
latency llm p50 840ms p95 3210ms  tool p50 122ms p95 980ms

cost by model
  model                cost       ...
```

`--json` emits the same summary as JSON for scripting.

## Python

```python
import tracelift

traces = tracelift.load("traces.jsonl")

traces.failures()                  # spans with status == "error"
traces.cost_by("model")            # also: tool_name, agent_name, provider, kind
traces.slowest(10)
traces.token_totals()
traces.latency_breakdown("trace-id")   # tree order, with per-span self time
traces.df                          # the full polars DataFrame, do anything
```

Every helper returns a polars DataFrame, so anything tracelift does not
answer directly is one `group_by` away.

## Input formats

**JSONL** — one span per line, either flat fields matching the schema
below or an `attributes` object with OpenTelemetry GenAI semantic
convention keys (`gen_ai.provider.name`, `gen_ai.usage.input_tokens`,
`gen_ai.tool.name`, ...). Deprecated semconv aliases (`gen_ai.system`,
`gen_ai.usage.prompt_tokens`/`completion_tokens`) are accepted.

**OTLP-JSON** — `resourceSpans` documents as written by OpenTelemetry
SDK file exporters and the collector's file exporter, single-document or
one export per line.

Malformed lines never abort a parse: they are skipped, counted and
reported with samples in `traces.report`.

## Span schema

One row per span: `trace_id`, `span_id`, `parent_span_id`, `name`,
`kind` (llm/tool/agent/chain/other), `start_ns`, `end_ns`, `duration_ms`,
`provider`, `model`, `input_tokens`, `output_tokens`,
`cache_read_tokens`, `cost`, `tool_name`, `tool_call_id`, `agent_name`,
`conversation_id`, `error_type`, `status` (ok/error/unset).

`cost` is read from the trace when present, never estimated. Pricing
tables go stale; a wrong silent estimate is worse than a null column.

## Not supported (yet)

- OTLP protobuf input (JSON only)
- live collection of any kind
- cost estimation from token counts
- name/PII scrubbing — strip sensitive data before sharing reports

## Development

Rust core in `core/`, PyO3 bindings in `bindings/`, Python API in
`python/tracelift/`. `cargo test` covers parsing, `pytest` covers the
API and CLI. Build locally with `pip install -e . --no-build-isolation`
(needs a Rust toolchain).

## License

MIT
