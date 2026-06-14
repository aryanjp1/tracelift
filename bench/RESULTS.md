# Benchmark results

tracelift vs an idiomatic pandas loader, parsing the same JSONL agent
trace file into the same normalized schema and running the same five
aggregations.

## Method

- Generate the input: `python bench/generate.py bench/data/big.jsonl --spans 3000000`
- Run: `python bench/run.py bench/data/big.jsonl --repeat 3`
- Each measurement runs in a fresh subprocess. Time is wall-clock around
  the work; peak RSS is the process maximum from `getrusage`. Reported
  values are the median time and the max peak RSS over 3 repeats.
- Both engines parse into the identical column set (trace/span ids,
  parent, name, kind, timing, provider, model, tokens, tool, cost,
  status, duration). "load + summary" additionally runs: filter errors,
  cost by model, median latency by tool, token totals, top-5 slowest.

## Environment

- 13th Gen Intel Core i7-13620H, 16 logical cores, 31 GB RAM
- Linux, Python 3.12.7, polars 1.41, pandas 2.2
- Input: 889 MB JSONL, 3,000,004 spans

## Results

| stage          | engine    | median time | peak RSS |
|----------------|-----------|-------------|----------|
| load only      | tracelift | 2.25 s      | 1616 MB  |
| load only      | pandas    | 14.26 s     | 4208 MB  |
| load + summary | tracelift | 2.74 s      | 4268 MB  |
| load + summary | pandas    | 15.47 s     | 4208 MB  |

Loading: **~6x faster and ~2.6x less memory.** With the aggregations
included, **~5.6x faster**; peak memory is comparable, because polars
allocates hash tables for the group-bys (this is working memory during
`summary()`, released afterward — the resident table stays near the
load-only figure).

## Honesty notes

- pandas is a fair baseline here, not a strawman: it parses the same
  fields and builds the same DataFrame shape. A `pd.read_json(lines=True)`
  call would be terser but would not flatten the `attributes` /
  semconv-alias structure tracelift handles, so it is not equivalent.
- Numbers are hardware-specific; reproduce locally with the commands
  above before quoting them.
