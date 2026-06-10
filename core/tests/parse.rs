use std::io::Cursor;

use tracelift_core::{parse_jsonl, parse_otlp, parse_path, Format};

const SAMPLE_JSONL: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../tests/fixtures/sample.jsonl"
));
const SAMPLE_OTLP: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../tests/fixtures/sample_otlp.json"
));

#[test]
fn jsonl_counts_and_skips() {
    let (cols, report) = parse_jsonl(Cursor::new(SAMPLE_JSONL));
    assert_eq!(cols.len(), 5);
    assert_eq!(report.spans_parsed, 5);
    assert_eq!(report.skipped, 2);
    assert_eq!(report.error_samples.len(), 2);
    assert!(report.error_samples[0].contains("invalid json"));
    assert!(report.error_samples[1].contains("trace_id"));
}

#[test]
fn jsonl_semconv_mapping() {
    let (cols, _) = parse_jsonl(Cursor::new(SAMPLE_JSONL));
    let i = cols.span_id.iter().position(|s| s == "s2").unwrap();
    assert_eq!(cols.provider[i].as_deref(), Some("openai"));
    assert_eq!(cols.model[i].as_deref(), Some("gpt-4o-2024-08-06"));
    assert_eq!(cols.input_tokens[i], Some(1200));
    assert_eq!(cols.output_tokens[i], Some(350));
    assert_eq!(cols.cost[i], Some(0.0145));
    assert_eq!(cols.kind[i], "llm");
}

#[test]
fn deprecated_aliases_accepted() {
    let (cols, _) = parse_jsonl(Cursor::new(SAMPLE_JSONL));
    let i = cols.span_id.iter().position(|s| s == "s4").unwrap();
    assert_eq!(cols.provider[i].as_deref(), Some("anthropic"));
    assert_eq!(cols.input_tokens[i], Some(900));
    assert_eq!(cols.output_tokens[i], Some(210));
}

#[test]
fn canonical_wins_over_alias() {
    let (cols, _) = parse_jsonl(Cursor::new(SAMPLE_JSONL));
    let i = cols.span_id.iter().position(|s| s == "s6").unwrap();
    assert_eq!(cols.input_tokens[i], Some(400));
}

#[test]
fn error_type_implies_error_status() {
    let (cols, _) = parse_jsonl(Cursor::new(SAMPLE_JSONL));
    let i = cols.span_id.iter().position(|s| s == "s3").unwrap();
    assert_eq!(cols.status[i], "error");
    assert_eq!(cols.kind[i], "tool");
    assert_eq!(cols.tool_name[i].as_deref(), Some("search_web"));
}

#[test]
fn otlp_document() {
    let (cols, report) = parse_otlp(Cursor::new(SAMPLE_OTLP)).unwrap();
    assert_eq!(cols.len(), 2);
    assert_eq!(report.spans_parsed, 2);
    assert_eq!(report.skipped, 0);

    let llm = cols
        .span_id
        .iter()
        .position(|s| s == "b7ad6b7169203331")
        .unwrap();
    assert_eq!(cols.kind[llm], "llm");
    assert_eq!(cols.input_tokens[llm], Some(640));
    assert_eq!(cols.status[llm], "ok");
    assert_eq!(cols.start_ns[llm], 1_749_500_020_000_000_000);

    let tool = cols
        .span_id
        .iter()
        .position(|s| s == "c5e1b3aa00112233")
        .unwrap();
    assert_eq!(cols.kind[tool], "tool");
    assert_eq!(cols.status[tool], "error");
    assert_eq!(cols.error_type[tool].as_deref(), Some("connection reset"));
    assert_eq!(
        cols.parent_span_id[tool].as_deref(),
        Some("b7ad6b7169203331")
    );
}

#[test]
fn garbage_never_panics() {
    let garbage: Vec<u8> = (0..=255u8).cycle().take(64 * 1024).collect();
    let (cols, _) = parse_jsonl(Cursor::new(garbage.clone()));
    assert_eq!(cols.len(), 0);
    let _ = parse_otlp(Cursor::new(garbage));

    let half_json = br#"{"trace_id":"t","span_id":"s","start_ns":1,"end_"#;
    let (cols, report) = parse_jsonl(Cursor::new(&half_json[..]));
    assert_eq!(cols.len(), 0);
    assert_eq!(report.skipped, 1);
}

#[test]
fn auto_detection() {
    let dir = std::env::temp_dir();
    let jsonl_path = dir.join("tracelift_test_auto.jsonl");
    let otlp_path = dir.join("tracelift_test_auto_otlp.json");
    std::fs::write(&jsonl_path, SAMPLE_JSONL).unwrap();
    std::fs::write(&otlp_path, SAMPLE_OTLP).unwrap();

    let (cols, _) = parse_path(&jsonl_path, Format::Auto).unwrap();
    assert_eq!(cols.len(), 5);
    let (cols, _) = parse_path(&otlp_path, Format::Auto).unwrap();
    assert_eq!(cols.len(), 2);

    std::fs::remove_file(jsonl_path).ok();
    std::fs::remove_file(otlp_path).ok();
}
