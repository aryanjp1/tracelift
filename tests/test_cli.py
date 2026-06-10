import json
from pathlib import Path

import pytest

from tracelift.cli import main

FIXTURES = Path(__file__).parent / "fixtures"


def test_summarize_text(capsys):
    assert main(["summarize", str(FIXTURES / "sample.jsonl")]) == 0
    out = capsys.readouterr().out
    assert "5 spans across 2 traces" in out
    assert "2 skipped" in out
    assert "cost by model" in out
    assert "gpt-4o-2024-08-06" in out
    assert "failing tools" in out
    assert "search_web" in out


def test_summarize_json(capsys):
    assert main(["summarize", str(FIXTURES / "sample.jsonl"), "--json"]) == 0
    payload = json.loads(capsys.readouterr().out)
    assert payload["spans"] == 5
    assert payload["skipped_lines"] == 2
    assert payload["failing_tools"][0]["tool_name"] == "search_web"


def test_summarize_otlp(capsys):
    assert main(["summarize", str(FIXTURES / "sample_otlp.json")]) == 0
    assert "2 spans across 1 traces" in capsys.readouterr().out


def test_missing_file_exit_code(capsys):
    assert main(["summarize", str(FIXTURES / "missing.jsonl")]) == 1
    assert "tracelift:" in capsys.readouterr().err


def test_empty_file_exit_code(tmp_path, capsys):
    empty = tmp_path / "empty.jsonl"
    empty.write_text("not json\n")
    assert main(["summarize", str(empty)]) == 1
    err = capsys.readouterr().err
    assert "no spans parsed" in err


def test_version(capsys):
    with pytest.raises(SystemExit) as exc:
        main(["--version"])
    assert exc.value.code == 0
