from pathlib import Path

import polars as pl
import pytest

import tracelift

FIXTURES = Path(__file__).parent / "fixtures"


@pytest.fixture
def traces():
    return tracelift.load(FIXTURES / "sample.jsonl")


def test_load_counts(traces):
    assert len(traces) == 5
    assert traces.report.spans_parsed == 5
    assert traces.report.skipped == 2
    assert len(traces.report.error_samples) == 2


def test_schema_and_duration(traces):
    df = traces.df
    assert df.schema["input_tokens"] == pl.Int64
    assert df.schema["cost"] == pl.Float64
    row = df.filter(pl.col("span_id") == "s2").row(0, named=True)
    assert row["duration_ms"] == pytest.approx(1800.0)
    assert row["model"] == "gpt-4o-2024-08-06"
    assert row["provider"] == "openai"


def test_otlp_load():
    traces = tracelift.load(FIXTURES / "sample_otlp.json")
    assert len(traces) == 2
    tool = traces.df.filter(pl.col("kind") == "tool").row(0, named=True)
    assert tool["status"] == "error"
    assert tool["error_type"] == "connection reset"


def test_auto_detection_matches_explicit():
    auto = tracelift.load(FIXTURES / "sample_otlp.json", format="auto")
    explicit = tracelift.load(FIXTURES / "sample_otlp.json", format="otlp")
    assert auto.df.equals(explicit.df)


def test_bad_format_rejected():
    with pytest.raises(ValueError, match="unknown format"):
        tracelift.load(FIXTURES / "sample.jsonl", format="csv")


def test_missing_file():
    with pytest.raises(OSError):
        tracelift.load(FIXTURES / "does_not_exist.jsonl")
