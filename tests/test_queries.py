from pathlib import Path

import polars as pl
import pytest

import tracelift

FIXTURES = Path(__file__).parent / "fixtures"


@pytest.fixture
def traces():
    return tracelift.load(FIXTURES / "sample.jsonl")


def test_failures(traces):
    failures = traces.failures()
    assert failures.height == 1
    row = failures.row(0, named=True)
    assert row["tool_name"] == "search_web"
    assert row["error_type"] == "TimeoutError"


def test_cost_by_model(traces):
    by_model = traces.cost_by("model")
    assert by_model.height == 3
    top = by_model.row(0, named=True)
    assert top["model"] == "gpt-4o-2024-08-06"
    assert top["cost"] == pytest.approx(0.0145)


def test_cost_by_rejects_unknown_key(traces):
    with pytest.raises(ValueError, match="key must be one of"):
        traces.cost_by("nonsense")


def test_token_totals(traces):
    totals = traces.token_totals()
    assert totals["input"] == 1200 + 900 + 400
    assert totals["output"] == 350 + 210 + 120
    assert totals["total"] == totals["input"] + totals["output"]


def test_slowest(traces):
    slowest = traces.slowest(2)
    assert slowest.height == 2
    assert slowest["duration_ms"][0] >= slowest["duration_ms"][1]


def test_latency_breakdown_tree(traces):
    tree = traces.latency_breakdown("t1")
    assert tree["span_id"].to_list() == ["s1", "s2", "s3"]
    assert tree["depth"].to_list() == [0, 1, 1]
    root = tree.row(0, named=True)
    # 2500ms total, children cover 1800 + 400
    assert root["self_ms"] == pytest.approx(300.0)


def test_latency_breakdown_missing_trace(traces):
    with pytest.raises(KeyError):
        traces.latency_breakdown("nope")


def test_summary(traces):
    s = traces.summary()
    assert s.spans == 5
    assert s.traces == 2
    assert s.errors == 1
    assert s.llm_calls == 3
    assert s.tool_calls == 1
    assert s.total_cost == pytest.approx(0.0145 + 0.0092 + 0.0011)
    assert s.failing_tools.height == 1
    payload = s.to_dict()
    assert isinstance(payload["cost_by_model"], list)
