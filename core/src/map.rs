use serde_json::{Map, Value};

use crate::model::{Span, SpanKind, Status};

type Obj = Map<String, Value>;

/// Build a normalized span from a flat object plus a semconv attribute map.
/// JSONL passes the line object as `flat`; OTLP passes structural fields as
/// `flat` and the unwrapped attribute list as `attrs`. Canonical semconv
/// names win over deprecated aliases, attributes win over flat keys.
pub fn from_parts(flat: &Obj, attrs: &Obj) -> Result<Span, String> {
    let trace_id = require_str(flat, "trace_id")?;
    let span_id = require_str(flat, "span_id")?;
    let start_ns = require_ns(flat, "start_ns")?;
    let end_ns = require_ns(flat, "end_ns")?;

    let provider = pick_str(
        flat,
        attrs,
        &["gen_ai.provider.name", "gen_ai.system", "provider"],
    );
    let model = pick_str(
        flat,
        attrs,
        &["gen_ai.response.model", "gen_ai.request.model", "model"],
    );
    let tool_name = pick_str(flat, attrs, &["gen_ai.tool.name", "tool_name"]);
    let agent_name = pick_str(flat, attrs, &["gen_ai.agent.name", "agent_name"]);
    let error_type = pick_str(flat, attrs, &["error.type", "error_type"]);

    let status = match flat.get("status").and_then(Value::as_str) {
        Some("ok") => Status::Ok,
        Some("error") => Status::Error,
        Some("unset") | None if error_type.is_some() => Status::Error,
        Some("unset") => Status::Unset,
        Some(_) => Status::Unset,
        None => Status::Unset,
    };

    let kind = flat
        .get("kind")
        .and_then(Value::as_str)
        .and_then(SpanKind::parse)
        .unwrap_or_else(|| {
            infer_kind(
                attrs.get("gen_ai.operation.name").and_then(Value::as_str),
                tool_name.is_some(),
                agent_name.is_some(),
                model.is_some(),
            )
        });

    Ok(Span {
        trace_id,
        span_id,
        parent_span_id: opt_str(flat, "parent_span_id"),
        name: flat
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        kind,
        start_ns,
        end_ns,
        provider,
        model,
        input_tokens: pick_i64(
            flat,
            attrs,
            &[
                "gen_ai.usage.input_tokens",
                "gen_ai.usage.prompt_tokens",
                "input_tokens",
            ],
        ),
        output_tokens: pick_i64(
            flat,
            attrs,
            &[
                "gen_ai.usage.output_tokens",
                "gen_ai.usage.completion_tokens",
                "output_tokens",
            ],
        ),
        cache_read_tokens: pick_i64(
            flat,
            attrs,
            &["gen_ai.usage.cache_read.input_tokens", "cache_read_tokens"],
        ),
        cost: pick_f64(flat, attrs, &["cost"]),
        tool_name,
        tool_call_id: pick_str(flat, attrs, &["gen_ai.tool.call.id", "tool_call_id"]),
        agent_name,
        conversation_id: pick_str(flat, attrs, &["gen_ai.conversation.id", "conversation_id"]),
        error_type,
        status,
    })
}

fn infer_kind(
    operation: Option<&str>,
    has_tool: bool,
    has_agent: bool,
    has_model: bool,
) -> SpanKind {
    if has_tool {
        return SpanKind::Tool;
    }
    match operation {
        Some("chat" | "text_completion" | "generate_content" | "embeddings") => SpanKind::Llm,
        Some("invoke_agent" | "create_agent") => SpanKind::Agent,
        Some("execute_tool") => SpanKind::Tool,
        _ if has_agent => SpanKind::Agent,
        _ if has_model => SpanKind::Llm,
        _ => SpanKind::Other,
    }
}

fn require_str(obj: &Obj, key: &str) -> Result<String, String> {
    match obj.get(key).and_then(Value::as_str) {
        Some(s) if !s.is_empty() => Ok(s.to_owned()),
        _ => Err(format!("missing required field {key:?}")),
    }
}

fn require_ns(obj: &Obj, key: &str) -> Result<i64, String> {
    obj.get(key)
        .and_then(value_as_i64)
        .ok_or_else(|| format!("missing or non-integer field {key:?}"))
}

fn opt_str(obj: &Obj, key: &str) -> Option<String> {
    obj.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

fn pick<'a>(flat: &'a Obj, attrs: &'a Obj, keys: &[&str]) -> Option<&'a Value> {
    keys.iter()
        .find_map(|k| attrs.get(*k))
        .or_else(|| keys.iter().find_map(|k| flat.get(*k)))
}

fn pick_str(flat: &Obj, attrs: &Obj, keys: &[&str]) -> Option<String> {
    pick(flat, attrs, keys)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

fn pick_i64(flat: &Obj, attrs: &Obj, keys: &[&str]) -> Option<i64> {
    pick(flat, attrs, keys).and_then(value_as_i64)
}

fn pick_f64(flat: &Obj, attrs: &Obj, keys: &[&str]) -> Option<f64> {
    pick(flat, attrs, keys).and_then(|v| match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.parse().ok(),
        _ => None,
    })
}

// OTLP-JSON serializes 64-bit integers as strings; accept both shapes.
fn value_as_i64(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }
}
