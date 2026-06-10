use std::io::BufRead;

use serde_json::{Map, Value};

use crate::map;
use crate::model::{Columns, ParseReport};

/// Parse OTLP-JSON trace exports: either a single document or one export
/// object per line, which is what the OTel collector file exporter writes.
pub fn parse<R: BufRead>(mut reader: R) -> std::io::Result<(Columns, ParseReport)> {
    let mut text = String::new();
    reader.read_to_string(&mut text)?;

    let mut columns = Columns::default();
    let mut report = ParseReport::default();

    if let Ok(doc) = serde_json::from_str::<Value>(&text) {
        report.lines_read = 1;
        collect_export(&doc, &mut columns, &mut report);
        return Ok((columns, report));
    }

    for (idx, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        report.lines_read += 1;
        match serde_json::from_str::<Value>(line) {
            Ok(doc) => collect_export(&doc, &mut columns, &mut report),
            Err(e) => report.skip(|| format!("line {}: invalid json: {e}", idx + 1)),
        }
    }
    Ok((columns, report))
}

fn collect_export(doc: &Value, columns: &mut Columns, report: &mut ParseReport) {
    let resource_spans = doc
        .get("resourceSpans")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    if resource_spans.is_empty() {
        report.skip(|| "document has no resourceSpans".to_owned());
        return;
    }

    for rs in resource_spans {
        let scope_spans = rs
            .get("scopeSpans")
            .or_else(|| rs.get("instrumentationLibrarySpans"))
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default();
        for ss in scope_spans {
            let spans = ss
                .get("spans")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or_default();
            for span in spans {
                match normalize(span) {
                    Ok((flat, attrs)) => match map::from_parts(&flat, &attrs) {
                        Ok(s) => {
                            columns.push(s);
                            report.spans_parsed += 1;
                        }
                        Err(reason) => report.skip(|| format!("otlp span: {reason}")),
                    },
                    Err(reason) => report.skip(|| format!("otlp span: {reason}")),
                }
            }
        }
    }
}

type Obj = Map<String, Value>;

fn normalize(span: &Value) -> Result<(Obj, Obj), String> {
    let span = span.as_object().ok_or("span is not an object")?;
    let mut flat = Obj::new();

    copy_str(span, "traceId", &mut flat, "trace_id");
    copy_str(span, "spanId", &mut flat, "span_id");
    copy_str(span, "parentSpanId", &mut flat, "parent_span_id");
    copy_str(span, "name", &mut flat, "name");
    copy_raw(span, "startTimeUnixNano", &mut flat, "start_ns");
    copy_raw(span, "endTimeUnixNano", &mut flat, "end_ns");

    if let Some(status) = span.get("status").and_then(Value::as_object) {
        let code = status.get("code");
        let as_int = code.and_then(Value::as_i64);
        let as_str = code.and_then(Value::as_str);
        let label = match (as_int, as_str) {
            (Some(2), _) | (_, Some("STATUS_CODE_ERROR")) => "error",
            (Some(1), _) | (_, Some("STATUS_CODE_OK")) => "ok",
            _ => "unset",
        };
        flat.insert("status".into(), Value::String(label.into()));
        if label == "error" {
            if let Some(msg) = status.get("message").and_then(Value::as_str) {
                if !msg.is_empty() {
                    flat.insert("error_type".into(), Value::String(msg.into()));
                }
            }
        }
    }

    let mut attrs = Obj::new();
    if let Some(list) = span.get("attributes").and_then(Value::as_array) {
        for kv in list {
            let Some(key) = kv.get("key").and_then(Value::as_str) else {
                continue;
            };
            if let Some(value) = kv.get("value").and_then(unwrap_any_value) {
                attrs.insert(key.to_owned(), value);
            }
        }
    }
    Ok((flat, attrs))
}

fn unwrap_any_value(v: &Value) -> Option<Value> {
    let obj = v.as_object()?;
    for key in ["stringValue", "intValue", "doubleValue", "boolValue"] {
        if let Some(inner) = obj.get(key) {
            return Some(inner.clone());
        }
    }
    None
}

fn copy_str(src: &Obj, src_key: &str, dst: &mut Obj, dst_key: &str) {
    if let Some(s) = src.get(src_key).and_then(Value::as_str) {
        if !s.is_empty() {
            dst.insert(dst_key.into(), Value::String(s.to_owned()));
        }
    }
}

fn copy_raw(src: &Obj, src_key: &str, dst: &mut Obj, dst_key: &str) {
    if let Some(v) = src.get(src_key) {
        dst.insert(dst_key.into(), v.clone());
    }
}
