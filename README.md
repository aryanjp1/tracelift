# tracelift

I kept exporting agent traces and then writing the same throwaway pandas
script to answer three questions: which tool calls failed, what each step
cost, and where the latency went. tracelift is that script, done properly
once: a Rust core that parses the trace file and a small polars-based API
on top.

It is a file analyzer, not an observability platform. No collector, no
storage, no dashboard to run. If you can export your traces to JSONL or
OTLP-JSON, you can point tracelift at the file and get answers.

## Install

```
pip install tracelift
```

## Use it from the command line

```
tracelift summarize traces.jsonl
```

Running it on the demo file in this repo:

```
tests/fixtures/sample.jsonl: 5 spans across 2 traces, 2 skipped
window  2025-06-09 20:13:20 -> 2025-06-09 20:13:34 UTC
calls   3 llm / 1 tool
errors  1 (20.0%)
cost    $0.0248
tokens  2,500 in / 680 out
latency llm p50 1800ms p95 3000ms  tool p50 400ms p95 400ms

cost by model
  model                        spans   cost       input_tokens  output_tokens
  gpt-4o-2024-08-06            1       $0.0145    1,200         350
  claude-sonnet-4-6           1       $0.0092    900           210
  claude-haiku-4-5-20251001   1       $0.0011    400           120

failing tools
  tool_name     failures  example
  search_web    1         TimeoutError
```

Add `--json` to get the same summary as JSON for scripts and CI.

## Use it from Python

```python
import tracelift

traces = tracelift.load("traces.jsonl")

traces.failures()                    # every span with status == "error"
traces.cost_by("model")              # or tool_name, agent_name, provider, kind
traces.slowest(10)
traces.token_totals()
traces.latency_breakdown("trace-id")  # spans in tree order, with self time
traces.df                            # the underlying polars DataFrame
```

Every helper hands back a polars DataFrame, so the moment you need
something tracelift does not do directly, it is one `group_by` away. The
library does not hide the data from you.

## Speed

The reason this is in Rust and not the pandas script: parsing a
889 MB / 3,000,000-span JSONL file, against an idiomatic pandas loader
that builds the exact same columns. 16-core i7-13620H, polars 1.41,
median of three runs.

| stage          | tracelift      | pandas          |
|----------------|----------------|-----------------|
| load only      | 2.3 s, 1.6 GB  | 14.3 s, 4.2 GB  |
| load + summary | 2.7 s, 4.3 GB  | 15.5 s, 4.2 GB  |

About 6x faster to load and 2.6x less memory; roughly 5.6x faster once
the aggregations are included. The numbers are hardware-specific. To
check them yourself: `python bench/generate.py bench/data/big.jsonl
--spans 3000000` then `python bench/run.py bench/data/big.jsonl`. Method
and caveats are in [bench/RESULTS.md](bench/RESULTS.md).

## What it reads

JSONL, one span per line. Either flat fields that match the schema below,
or an `attributes` object using OpenTelemetry GenAI semantic convention
keys (`gen_ai.provider.name`, `gen_ai.usage.input_tokens`,
`gen_ai.tool.name`, and so on). The older alias names still emitted by a
lot of tooling (`gen_ai.system`, `gen_ai.usage.prompt_tokens`,
`gen_ai.usage.completion_tokens`) are accepted too.

OTLP-JSON, the `resourceSpans` shape that OpenTelemetry SDK file
exporters and the collector's file exporter produce, as a single document
or one export per line.

A broken line never takes down the parse. Bad lines are skipped, counted,
and a few samples are kept in `traces.report` so you can see what was
dropped.

## The span table

One row per span:

`trace_id`, `span_id`, `parent_span_id`, `name`, `kind`
(llm / tool / agent / chain / other), `start_ns`, `end_ns`,
`duration_ms`, `provider`, `model`, `input_tokens`, `output_tokens`,
`cache_read_tokens`, `cost`, `tool_name`, `tool_call_id`, `agent_name`,
`conversation_id`, `error_type`, `status` (ok / error / unset).

One deliberate choice: `cost` is read from the trace if it is there, and
left null if it is not. tracelift will not multiply tokens by a built-in
price list, because those lists go stale and a wrong number that looks
right is worse than an honest blank.

## What it does not do

- OTLP protobuf input. JSON only for now.
- Collecting traces. It reads files you already have.
- Estimating cost from token counts. See above.
- Scrubbing names or PII. Strip anything sensitive before you share a
  report.

## Building from source

Rust core in `core/`, the PyO3 bindings in `bindings/`, the Python API
and CLI in `python/tracelift/`. `cargo test` covers the parser, `pytest`
covers the API and CLI. For a local dev build you need a Rust toolchain,
then `pip install -e . --no-build-isolation`.

## License

MIT
