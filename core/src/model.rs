#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanKind {
    Llm,
    Tool,
    Agent,
    Chain,
    Other,
}

impl SpanKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SpanKind::Llm => "llm",
            SpanKind::Tool => "tool",
            SpanKind::Agent => "agent",
            SpanKind::Chain => "chain",
            SpanKind::Other => "other",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "llm" => Some(SpanKind::Llm),
            "tool" => Some(SpanKind::Tool),
            "agent" => Some(SpanKind::Agent),
            "chain" => Some(SpanKind::Chain),
            "other" => Some(SpanKind::Other),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok,
    Error,
    Unset,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Ok => "ok",
            Status::Error => "error",
            Status::Unset => "unset",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Span {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub name: String,
    pub kind: SpanKind,
    pub start_ns: i64,
    pub end_ns: i64,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub cost: Option<f64>,
    pub tool_name: Option<String>,
    pub tool_call_id: Option<String>,
    pub agent_name: Option<String>,
    pub conversation_id: Option<String>,
    pub error_type: Option<String>,
    pub status: Status,
}

/// Column-oriented output: one entry per span across all vectors.
#[derive(Debug, Default)]
pub struct Columns {
    pub trace_id: Vec<String>,
    pub span_id: Vec<String>,
    pub parent_span_id: Vec<Option<String>>,
    pub name: Vec<String>,
    pub kind: Vec<&'static str>,
    pub start_ns: Vec<i64>,
    pub end_ns: Vec<i64>,
    pub provider: Vec<Option<String>>,
    pub model: Vec<Option<String>>,
    pub input_tokens: Vec<Option<i64>>,
    pub output_tokens: Vec<Option<i64>>,
    pub cache_read_tokens: Vec<Option<i64>>,
    pub cost: Vec<Option<f64>>,
    pub tool_name: Vec<Option<String>>,
    pub tool_call_id: Vec<Option<String>>,
    pub agent_name: Vec<Option<String>>,
    pub conversation_id: Vec<Option<String>>,
    pub error_type: Vec<Option<String>>,
    pub status: Vec<&'static str>,
}

impl Columns {
    pub fn len(&self) -> usize {
        self.trace_id.len()
    }

    pub fn append(&mut self, mut other: Columns) {
        self.trace_id.append(&mut other.trace_id);
        self.span_id.append(&mut other.span_id);
        self.parent_span_id.append(&mut other.parent_span_id);
        self.name.append(&mut other.name);
        self.kind.append(&mut other.kind);
        self.start_ns.append(&mut other.start_ns);
        self.end_ns.append(&mut other.end_ns);
        self.provider.append(&mut other.provider);
        self.model.append(&mut other.model);
        self.input_tokens.append(&mut other.input_tokens);
        self.output_tokens.append(&mut other.output_tokens);
        self.cache_read_tokens.append(&mut other.cache_read_tokens);
        self.cost.append(&mut other.cost);
        self.tool_name.append(&mut other.tool_name);
        self.tool_call_id.append(&mut other.tool_call_id);
        self.agent_name.append(&mut other.agent_name);
        self.conversation_id.append(&mut other.conversation_id);
        self.error_type.append(&mut other.error_type);
        self.status.append(&mut other.status);
    }

    pub fn is_empty(&self) -> bool {
        self.trace_id.is_empty()
    }

    pub fn push(&mut self, span: Span) {
        self.trace_id.push(span.trace_id);
        self.span_id.push(span.span_id);
        self.parent_span_id.push(span.parent_span_id);
        self.name.push(span.name);
        self.kind.push(span.kind.as_str());
        self.start_ns.push(span.start_ns);
        self.end_ns.push(span.end_ns);
        self.provider.push(span.provider);
        self.model.push(span.model);
        self.input_tokens.push(span.input_tokens);
        self.output_tokens.push(span.output_tokens);
        self.cache_read_tokens.push(span.cache_read_tokens);
        self.cost.push(span.cost);
        self.tool_name.push(span.tool_name);
        self.tool_call_id.push(span.tool_call_id);
        self.agent_name.push(span.agent_name);
        self.conversation_id.push(span.conversation_id);
        self.error_type.push(span.error_type);
        self.status.push(span.status.as_str());
    }
}

const ERROR_SAMPLE_LIMIT: usize = 10;

#[derive(Debug, Default)]
pub struct ParseReport {
    pub lines_read: u64,
    pub spans_parsed: u64,
    pub skipped: u64,
    pub error_samples: Vec<String>,
}

impl ParseReport {
    pub fn skip(&mut self, context: impl FnOnce() -> String) {
        self.skipped += 1;
        if self.error_samples.len() < ERROR_SAMPLE_LIMIT {
            self.error_samples.push(context());
        }
    }

    pub fn merge(&mut self, other: ParseReport) {
        self.lines_read += other.lines_read;
        self.spans_parsed += other.spans_parsed;
        self.skipped += other.skipped;
        for sample in other.error_samples {
            if self.error_samples.len() == ERROR_SAMPLE_LIMIT {
                break;
            }
            self.error_samples.push(sample);
        }
    }
}
