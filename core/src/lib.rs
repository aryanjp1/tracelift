mod jsonl;
mod map;
pub mod model;
mod otlp;

use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::Path;

pub use model::{Columns, ParseReport};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Jsonl,
    Otlp,
    Auto,
}

impl std::str::FromStr for Format {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "jsonl" => Ok(Format::Jsonl),
            "otlp" => Ok(Format::Otlp),
            "auto" => Ok(Format::Auto),
            other => Err(format!(
                "unknown format {other:?}, expected jsonl, otlp or auto"
            )),
        }
    }
}

pub fn parse_path(path: &Path, format: Format) -> std::io::Result<(Columns, ParseReport)> {
    let mut file = File::open(path)?;
    let format = match format {
        Format::Auto => {
            let detected = detect_format(&mut file)?;
            file.seek(SeekFrom::Start(0))?;
            detected
        }
        f => f,
    };
    let reader = BufReader::with_capacity(1 << 20, file);
    match format {
        Format::Jsonl => Ok(jsonl::parse(reader)),
        Format::Otlp => otlp::parse(reader),
        Format::Auto => unreachable!(),
    }
}

// OTLP-JSON exports are objects whose top level carries "resourceSpans";
// anything else line-shaped is treated as schema/semconv JSONL.
fn detect_format<R: Read>(reader: &mut R) -> std::io::Result<Format> {
    let mut head = [0u8; 4096];
    let n = reader.read(&mut head)?;
    let head = String::from_utf8_lossy(&head[..n]);
    let probe = head.trim_start();
    if probe.starts_with('{') && head.contains("resourceSpans") {
        return Ok(Format::Otlp);
    }
    Ok(Format::Jsonl)
}

pub fn parse_jsonl<R: BufRead>(reader: R) -> (Columns, ParseReport) {
    jsonl::parse(reader)
}

pub fn parse_otlp<R: BufRead>(reader: R) -> std::io::Result<(Columns, ParseReport)> {
    otlp::parse(reader)
}
