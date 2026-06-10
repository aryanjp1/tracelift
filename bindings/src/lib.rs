use std::path::Path;

use pyo3::exceptions::{PyIOError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use tracelift_core::{parse_path, Format};

#[pyfunction]
#[pyo3(signature = (path, format = "auto"))]
fn parse_file<'py>(
    py: Python<'py>,
    path: &str,
    format: &str,
) -> PyResult<(Bound<'py, PyDict>, Bound<'py, PyDict>)> {
    let fmt: Format = format.parse().map_err(PyValueError::new_err)?;
    let path = Path::new(path).to_owned();
    let (cols, report) = py
        .allow_threads(move || parse_path(&path, fmt))
        .map_err(|e| PyIOError::new_err(e.to_string()))?;

    let columns = PyDict::new(py);
    columns.set_item("trace_id", cols.trace_id)?;
    columns.set_item("span_id", cols.span_id)?;
    columns.set_item("parent_span_id", cols.parent_span_id)?;
    columns.set_item("name", cols.name)?;
    columns.set_item("kind", cols.kind)?;
    columns.set_item("start_ns", cols.start_ns)?;
    columns.set_item("end_ns", cols.end_ns)?;
    columns.set_item("provider", cols.provider)?;
    columns.set_item("model", cols.model)?;
    columns.set_item("input_tokens", cols.input_tokens)?;
    columns.set_item("output_tokens", cols.output_tokens)?;
    columns.set_item("cache_read_tokens", cols.cache_read_tokens)?;
    columns.set_item("cost", cols.cost)?;
    columns.set_item("tool_name", cols.tool_name)?;
    columns.set_item("tool_call_id", cols.tool_call_id)?;
    columns.set_item("agent_name", cols.agent_name)?;
    columns.set_item("conversation_id", cols.conversation_id)?;
    columns.set_item("error_type", cols.error_type)?;
    columns.set_item("status", cols.status)?;

    let summary = PyDict::new(py);
    summary.set_item("lines_read", report.lines_read)?;
    summary.set_item("spans_parsed", report.spans_parsed)?;
    summary.set_item("skipped", report.skipped)?;
    summary.set_item("error_samples", report.error_samples)?;

    Ok((columns, summary))
}

#[pymodule]
fn _tracelift(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(parse_file, m)?)?;
    Ok(())
}
