use std::io::BufRead;

use rayon::prelude::*;
use serde_json::{Map, Value};

use crate::map;
use crate::model::{Columns, ParseReport};

pub const DEFAULT_BLOCK_LINES: usize = 262_144;

pub fn parse<R: BufRead>(reader: R) -> (Columns, ParseReport) {
    let mut columns = Columns::default();
    let mut report = ParseReport::default();
    for (idx, line) in reader.lines().enumerate() {
        match line {
            Ok(l) => parse_one(&l, idx as u64 + 1, &mut columns, &mut report),
            Err(e) => {
                report.lines_read += 1;
                report.skip(|| format!("line {}: read error: {e}", idx + 1));
            }
        }
    }
    (columns, report)
}

/// Streaming parallel parse: lines are gathered into blocks, each block is
/// parsed across the rayon pool, and every worker's columns are handed to
/// `sink` as soon as the block completes. Memory stays bounded by the block
/// size regardless of file size.
pub fn parse_chunked<R: BufRead>(
    reader: R,
    block_lines: usize,
    sink: &mut dyn FnMut(Columns, ParseReport),
) -> std::io::Result<()> {
    let block_lines = block_lines.max(1);
    let mut block: Vec<(u64, String)> = Vec::with_capacity(block_lines);
    let mut io_errors = ParseReport::default();

    for (idx, line) in reader.lines().enumerate() {
        let line_no = idx as u64 + 1;
        match line {
            Ok(l) => {
                if l.trim().is_empty() {
                    continue;
                }
                block.push((line_no, l));
                if block.len() == block_lines {
                    flush(&mut block, sink);
                }
            }
            Err(e) => {
                io_errors.lines_read += 1;
                io_errors.skip(|| format!("line {line_no}: read error: {e}"));
            }
        }
    }
    flush(&mut block, sink);
    if io_errors.skipped > 0 {
        sink(Columns::default(), io_errors);
    }
    Ok(())
}

fn flush(block: &mut Vec<(u64, String)>, sink: &mut dyn FnMut(Columns, ParseReport)) {
    if block.is_empty() {
        return;
    }
    let lines = std::mem::take(block);
    let per_worker = lines.len().div_ceil(rayon::current_num_threads().max(1));
    let results: Vec<(Columns, ParseReport)> = lines
        .par_chunks(per_worker.max(1))
        .map(|chunk| {
            let mut columns = Columns::default();
            let mut report = ParseReport::default();
            for (line_no, line) in chunk {
                parse_one(line, *line_no, &mut columns, &mut report);
            }
            (columns, report)
        })
        .collect();
    for (columns, report) in results {
        sink(columns, report);
    }
}

fn parse_one(line: &str, line_no: u64, columns: &mut Columns, report: &mut ParseReport) {
    if line.trim().is_empty() {
        return;
    }
    report.lines_read += 1;

    let value: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => {
            report.skip(|| format!("line {line_no}: invalid json: {e}"));
            return;
        }
    };
    let Some(obj) = value.as_object() else {
        report.skip(|| format!("line {line_no}: expected a json object"));
        return;
    };
    let empty = Map::new();
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
