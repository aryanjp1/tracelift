"""Generate a synthetic agent trace file for benchmarking.

Usage: python bench/generate.py out.jsonl --spans 2000000
Shape mirrors a multi-agent pipeline: agent root spans with llm and tool
children, occasional failures, semconv attribute style for llm spans.
"""

from __future__ import annotations

import argparse
import json
import random

MODELS = [
    ("openai", "gpt-4o-2024-08-06", 2.5e-6, 1e-5),
    ("openai", "gpt-4o-mini-2024-07-18", 1.5e-7, 6e-7),
    ("anthropic", "claude-sonnet-4-6", 3e-6, 1.5e-5),
    ("anthropic", "claude-haiku-4-5-20251001", 8e-7, 4e-6),
]
TOOLS = ["search_web", "fetch_listing", "dedupe", "verify_entity", "draft_reply"]
ERRORS = ["TimeoutError", "RateLimitError", "ConnectionError", "ValidationError"]


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("out")
    parser.add_argument("--spans", type=int, default=1_000_000)
    parser.add_argument("--seed", type=int, default=7)
    args = parser.parse_args()

    rng = random.Random(args.seed)
    now_ns = 1_749_500_000_000_000_000
    written = 0
    trace_no = 0

    with open(args.out, "w") as f:
        while written < args.spans:
            trace_no += 1
            trace_id = f"t{trace_no:08d}"
            t = now_ns + trace_no * 60_000_000_000
            root_id = f"{trace_id}-root"
            steps = rng.randint(2, 6)
            root_end = t
            children = []

            for step in range(steps):
                start = root_end + rng.randint(1, 50) * 1_000_000
                if rng.random() < 0.55:
                    provider, model, in_price, out_price = rng.choice(MODELS)
                    tokens_in = rng.randint(200, 6000)
                    tokens_out = rng.randint(30, 1200)
                    dur = rng.randint(300, 4000) * 1_000_000
                    children.append(
                        {
                            "trace_id": trace_id,
                            "span_id": f"{trace_id}-s{step}",
                            "parent_span_id": root_id,
                            "name": f"chat {model}",
                            "start_ns": start,
                            "end_ns": start + dur,
                            "attributes": {
                                "gen_ai.operation.name": "chat",
                                "gen_ai.provider.name": provider,
                                "gen_ai.response.model": model,
                                "gen_ai.usage.input_tokens": tokens_in,
                                "gen_ai.usage.output_tokens": tokens_out,
                                "cost": round(tokens_in * in_price + tokens_out * out_price, 8),
                            },
                            "status": "ok",
                        }
                    )
                else:
                    tool = rng.choice(TOOLS)
                    failed = rng.random() < 0.02
                    dur = rng.randint(20, 1500) * 1_000_000
                    span = {
                        "trace_id": trace_id,
                        "span_id": f"{trace_id}-s{step}",
                        "parent_span_id": root_id,
                        "name": tool,
                        "start_ns": start,
                        "end_ns": start + dur,
                        "attributes": {"gen_ai.tool.name": tool},
                        "status": "error" if failed else "ok",
                    }
                    if failed:
                        span["error_type"] = rng.choice(ERRORS)
                    children.append(span)
                root_end = start + dur

            root = {
                "trace_id": trace_id,
                "span_id": root_id,
                "name": "lead-pipeline",
                "kind": "agent",
                "start_ns": t,
                "end_ns": root_end + 5_000_000,
                "agent_name": "pipeline",
                "status": "ok",
            }
            for span in [root, *children]:
                f.write(json.dumps(span, separators=(",", ":")) + "\n")
            written += 1 + len(children)

    print(f"wrote {written} spans to {args.out}")


if __name__ == "__main__":
    main()
