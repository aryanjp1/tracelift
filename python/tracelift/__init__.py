"""Fast agent/LLM trace analysis on top of polars.

Load JSONL or OTLP-JSON trace files into a normalized span table and ask
the questions that matter for agent systems: which tool calls failed,
what each step cost, where the latency went.
"""

from tracelift._frame import ParseReport, Summary, TraceSet, load

__version__ = "0.1.0"

__all__ = ["load", "TraceSet", "ParseReport", "Summary", "__version__"]
