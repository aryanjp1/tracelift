use std::path::Path;
use std::sync::Arc;

use arrow::array::{ArrayRef, Float64Array, Int64Array, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::pyarrow::ToPyArrow;
use arrow::record_batch::RecordBatch;
use pyo3::exceptions::{PyIOError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use tracelift_core::{parse_path_chunked, Columns, Format, ParseReport, DEFAULT_BLOCK_LINES};

#[pyfunction]
#[pyo3(signature = (path, format = "auto"))]
fn parse_file<'py>(
    py: Python<'py>,
    path: &str,
    format: &str,
) -> PyResult<(Vec<Py<PyAny>>, Bound<'py, PyDict>)> {
    let fmt: Format = format.parse().map_err(PyValueError::new_err)?;
    let path = Path::new(path).to_owned();
    // Parse fully off the GIL, coalescing each block's per-worker columns
    // into one batch per block. Arrow batches are built here (still off the
    // GIL): the source Vecs free as each block converts, so peak memory
    // tracks a single block rather than the whole file plus a full copy.
    let (batches, report) = py
        .detach(move || {
            let mut batches: Vec<RecordBatch> = Vec::new();
            let mut block = Columns::default();
            let mut report = ParseReport::default();
            parse_path_chunked(&path, fmt, DEFAULT_BLOCK_LINES, &mut |cols, rep| {
                block.append(cols);
                report.merge(rep);
                if block.len() >= DEFAULT_BLOCK_LINES {
                    batches.push(to_batch(std::mem::take(&mut block)));
                }
            })?;
            if !block.is_empty() || batches.is_empty() {
                batches.push(to_batch(block));
            }
            Ok::<_, std::io::Error>((batches, report))
        })
        .map_err(|e| PyIOError::new_err(e.to_string()))?;

    let batches = batches
        .into_iter()
        .map(|b| Ok(b.to_pyarrow(py)?.into()))
        .collect::<PyResult<Vec<Py<PyAny>>>>()?;

    let summary = PyDict::new(py);
    summary.set_item("lines_read", report.lines_read)?;
    summary.set_item("spans_parsed", report.spans_parsed)?;
    summary.set_item("skipped", report.skipped)?;
    summary.set_item("error_samples", report.error_samples)?;

    Ok((batches, summary))
}

fn to_batch(cols: Columns) -> RecordBatch {
    fn utf8(values: Vec<String>) -> ArrayRef {
        Arc::new(StringArray::from_iter_values(values))
    }
    fn opt_utf8(values: Vec<Option<String>>) -> ArrayRef {
        Arc::new(values.into_iter().collect::<StringArray>())
    }
    fn statics(values: Vec<&'static str>) -> ArrayRef {
        Arc::new(StringArray::from_iter_values(values))
    }

    let fields = vec![
        Field::new("trace_id", DataType::Utf8, false),
        Field::new("span_id", DataType::Utf8, false),
        Field::new("parent_span_id", DataType::Utf8, true),
        Field::new("name", DataType::Utf8, false),
        Field::new("kind", DataType::Utf8, false),
        Field::new("start_ns", DataType::Int64, false),
        Field::new("end_ns", DataType::Int64, false),
        Field::new("provider", DataType::Utf8, true),
        Field::new("model", DataType::Utf8, true),
        Field::new("input_tokens", DataType::Int64, true),
        Field::new("output_tokens", DataType::Int64, true),
        Field::new("cache_read_tokens", DataType::Int64, true),
        Field::new("cost", DataType::Float64, true),
        Field::new("tool_name", DataType::Utf8, true),
        Field::new("tool_call_id", DataType::Utf8, true),
        Field::new("agent_name", DataType::Utf8, true),
        Field::new("conversation_id", DataType::Utf8, true),
        Field::new("error_type", DataType::Utf8, true),
        Field::new("status", DataType::Utf8, false),
    ];
    let columns: Vec<ArrayRef> = vec![
        utf8(cols.trace_id),
        utf8(cols.span_id),
        opt_utf8(cols.parent_span_id),
        utf8(cols.name),
        statics(cols.kind),
        Arc::new(Int64Array::from(cols.start_ns)),
        Arc::new(Int64Array::from(cols.end_ns)),
        opt_utf8(cols.provider),
        opt_utf8(cols.model),
        Arc::new(Int64Array::from(cols.input_tokens)),
        Arc::new(Int64Array::from(cols.output_tokens)),
        Arc::new(Int64Array::from(cols.cache_read_tokens)),
        Arc::new(Float64Array::from(cols.cost)),
        opt_utf8(cols.tool_name),
        opt_utf8(cols.tool_call_id),
        opt_utf8(cols.agent_name),
        opt_utf8(cols.conversation_id),
        opt_utf8(cols.error_type),
        statics(cols.status),
    ];
    RecordBatch::try_new(Arc::new(Schema::new(fields)), columns)
        .expect("schema and column shapes are built together")
}

#[pymodule]
fn _tracelift(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(parse_file, m)?)?;
    Ok(())
}
