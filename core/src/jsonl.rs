use std::io::BufRead;

use serde_json::{Map, Value};

use crate::map;
use crate::model::{Columns, ParseReport};

pub fn parse<R: BufRead>(reader: R) -> (Columns, ParseReport) {
    let mut columns = Columns::default();
    let mut report = ParseReport::default();
    let empty = Map::new();

    for (idx, line) in reader.lines().enumerate() {
        let line_no = idx + 1;
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                report.lines_read += 1;
                report.skip(|| format!("line {line_no}: read error: {e}"));
                continue;
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        report.lines_read += 1;

        let value: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                report.skip(|| format!("line {line_no}: invalid json: {e}"));
                continue;
            }
        };
        let Some(obj) = value.as_object() else {
            report.skip(|| format!("line {line_no}: expected a json object"));
            continue;
        };
        let attrs = obj
            .get("attributes")
            .and_then(Value::as_object)
            .unwrap_or(&empty);

        match map::from_parts(obj, attrs) {
            Ok(span) => {
                columns.push(span);
                report.spans_parsed += 1;
            }
            Err(reason) => report.skip(|| format!("line {line_no}: {reason}")),
        }
    }

    (columns, report)
}
