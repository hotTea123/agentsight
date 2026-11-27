// SPDX-License-Identifier: MIT
// Copyright (c) 2026 eunomia-bpf org.

//! OpenTelemetry GenAI exporter.
//!
//! Maps completed materialized `llm_call` rows onto
//! OpenTelemetry **GenAI semantic-convention** (`gen_ai.*`) spans and ships them
//! to an OpenTelemetry Collector via **OTLP/HTTP (JSON)** — the standard wire
//! format understood by the OTel Collector, Jaeger, Grafana Tempo, and the major
//! observability vendors. Because AgentSight captures the traffic at the kernel
//! (eBPF) level, this produces vendor-neutral GenAI telemetry for *any* agent
//! binary with **zero in-process instrumentation**.
//!
//! Spec: <https://opentelemetry.io/docs/specs/semconv/gen-ai/>
//!
//! Each completed call becomes one `chat {model}` CLIENT span. Correlation and
//! token extraction happen before this sink sees the row; this sink only maps
//! stable view data to OTLP.
//!
use crate::analyzers::AnalyzerError;
use crate::model::{AuditEventRow, LlmCallRow, ViewResult, ViewSink};
use crate::view::llm::provider_from_host;
use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;

/// Default OTLP/HTTP receiver endpoint (OpenTelemetry Collector).
const DEFAULT_OTLP_ENDPOINT: &str = "http://localhost:4318";

#[derive(Clone, Default)]
struct SpanInput {
    start_unix_nano: u128,
    provider: String,
    server_address: String,
    model: Option<String>,
    conversation_id: Option<String>,
    max_tokens: Option<i64>,
    temperature: Option<f64>,
    top_p: Option<f64>,
    protocol: Option<String>,
    connection_id: Option<String>,
    stream_id: Option<u32>,
    http_exchange_id: Option<String>,
    request_id: Option<String>,
    response_id: Option<String>,
    correlation_method: Option<String>,
    correlation_status: Option<String>,
    correlation_version: Option<u16>,
    completion_reason: Option<String>,
    confidence: Option<f32>,
    /// Opt-in: the request messages, captured only when content capture is on.
    input_messages: Option<String>,
}

/// Exports GenAI spans for LLM HTTP exchanges via OTLP/HTTP (JSON).
pub struct OtelExporter {
    /// Full traces URL, e.g. `http://localhost:4318/v1/traces`.
    traces_url: String,
    /// `service.name` reported on the OTLP Resource.
    service_name: String,
    /// Whether to attach prompt/completion content (`gen_ai.{input,output}.messages`).
    capture_content: bool,
    /// Export correlated file/process evidence as children of instrumented tool spans.
    tool_enhancement: bool,
    /// Trace IDs assigned to conversation/session identifiers in this recording.
    trace_ids: HashMap<String, String>,
    /// Recording-scoped trace used when a call has no correlation identifier.
    fallback_trace_id: String,
    client: Arc<Client<hyper_util::client::legacy::connect::HttpConnector, Full<Bytes>>>,
}

impl OtelExporter {
    /// Create an exporter. `endpoint` is the OTLP/HTTP base (e.g.
    /// `http://collector:4318`); when `None`, `OTEL_EXPORTER_OTLP_ENDPOINT` is
    /// honored, falling back to `http://localhost:4318`. A full traces endpoint
    /// in `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT` takes precedence and is used as-is.
    pub fn new(endpoint: Option<String>, capture_content: bool) -> Self {
        Self::new_with_tool_enhancement(endpoint, capture_content, false)
    }

    /// Create an exporter that augments existing instrumented tool spans.
    pub fn new_with_tool_enhancement(
        endpoint: Option<String>,
        capture_content: bool,
        tool_enhancement: bool,
    ) -> Self {
        let traces_url = if let Ok(full) = std::env::var("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT") {
            full
        } else {
            let base = endpoint
                .or_else(|| std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok())
                .unwrap_or_else(|| DEFAULT_OTLP_ENDPOINT.to_string());
            format!("{}/v1/traces", base.trim_end_matches('/'))
        };

        let service_name =
            std::env::var("OTEL_SERVICE_NAME").unwrap_or_else(|_| "agentsight".to_string());

        Self {
            traces_url,
            service_name,
            capture_content,
            tool_enhancement,
            trace_ids: HashMap::new(),
            fallback_trace_id: new_trace_id(),
            client: Arc::new(Client::builder(TokioExecutor::new()).build_http()),
        }
    }

    fn trace_id_for(&mut self, conversation_id: Option<&str>, session_id: Option<&str>) -> String {
        let key = conversation_id
            .filter(|id| !id.is_empty())
            .map(|id| format!("conversation:{id}"))
            .or_else(|| {
                session_id
                    .filter(|id| !id.is_empty())
                    .map(|id| format!("session:{id}"))
            });
        let Some(key) = key else {
            return self.fallback_trace_id.clone();
        };
        self.trace_ids
            .entry(key)
            .or_insert_with(new_trace_id)
            .clone()
    }
}

fn valid_hex_id(value: &str, length: usize) -> bool {
    value.len() == length
        && value.bytes().all(|byte| byte.is_ascii_hexdigit())
        && value.bytes().any(|byte| byte != b'0')
}

fn syscall_error_name(errno: i32) -> String {
    match errno {
        libc::EACCES => "EACCES".to_string(),
        libc::EPERM => "EPERM".to_string(),
        libc::ENOENT => "ENOENT".to_string(),
        libc::EEXIST => "EEXIST".to_string(),
        libc::EXDEV => "EXDEV".to_string(),
        libc::ENOTDIR => "ENOTDIR".to_string(),
        libc::EISDIR => "EISDIR".to_string(),
        libc::ENOTEMPTY => "ENOTEMPTY".to_string(),
        libc::EROFS => "EROFS".to_string(),
        libc::ENOSPC => "ENOSPC".to_string(),
        libc::EINVAL => "EINVAL".to_string(),
        libc::EBADF => "EBADF".to_string(),
        _ => format!("errno.{errno}"),
    }
}

fn build_correlated_audit_payload(service_name: &str, row: &AuditEventRow) -> Option<Value> {
    let trace_id = row
        .details
        .get("agentsight_trace_id")
        .and_then(Value::as_str)?;
    let parent_span_id = row
        .details
        .get("agentsight_parent_span_id")
        .and_then(Value::as_str)?;
    if !valid_hex_id(trace_id, 32) || !valid_hex_id(parent_span_id, 16) {
        return None;
    }

    let action = row.action.as_deref().unwrap_or("event");
    let span_name = match (row.audit_type.as_str(), action) {
		("file", "open") if is_read_only_file_open(row) => "agentsight.file.open",
        ("file", "write") => "agentsight.file.write",
		("file", "rename") => "agentsight.file.rename",
        ("process", "exec") => "agentsight.process.exec",
        _ => return None,
    };
    let mut attributes = vec![
        attr_str("agentsight.evidence.type", &row.audit_type),
        attr_str("agentsight.evidence.action", action),
    ];
    let mut status = json!({ "code": 1 });
    if let Some(target) = row.target.as_deref() {
        attributes.push(attr_str("agentsight.evidence.target", target));
        match row.audit_type.as_str() {
            "file" => attributes.push(attr_str("file.path", target)),
            "process" => attributes.push(attr_str("process.executable.path", target)),
            _ => {}
        }
    }
	match action {
		"open" => attributes.push(attr_str("file.access_mode", "read_only")),
		"write" => {
			if let Some(bytes) = row.details.get("total_bytes").and_then(Value::as_u64) {
				attributes.push(attr_int("file.write.bytes", bytes as i64));
			}
			if let Some(count) = row.details.get("count").and_then(Value::as_u64) {
				attributes.push(attr_int("file.write.operation_count", count as i64));
			}
		}
		"rename" => {
            // Legacy entry-only records have no result: do not report success.
            status = json!({ "code": 0 });
            if let Some(result) = row.details.get("syscall_return").and_then(Value::as_i64) {
                attributes.push(attr_int("syscall.return", result));
                if result == 0 {
                    status = json!({ "code": 1 });
                } else if (-4095..=-1).contains(&result) {
                    let errno = (-result) as i32;
                    let error_type = syscall_error_name(errno);
                    attributes.push(attr_int("syscall.errno", i64::from(errno)));
                    attributes.push(attr_str("error.type", &error_type));
                    status = json!({ "code": 2, "message": error_type });
                }
            }
            if let Some(name) = row.details.get("syscall_name").and_then(Value::as_str) {
                attributes.push(attr_str("syscall.name", name));
            }
            for (field, attribute) in [
                ("source_dirfd", "file.source.dirfd"),
                ("destination_dirfd", "file.destination.dirfd"),
                ("rename_flags", "file.rename.flags"),
                ("count", "file.rename.operation_count"),
            ] {
                if let Some(value) = row.details.get(field).and_then(Value::as_i64) {
                    attributes.push(attr_int(attribute, value));
                }
            }
			if let Some(source) = row.details.get("source_path").and_then(Value::as_str) {
				attributes.push(attr_str("file.source.path", source));
			}
			if let Some(destination) = row
				.details
				.get("destination_path")
				.and_then(Value::as_str)
			{
				attributes.push(attr_str("file.destination.path", destination));
			}
		}
		_ => {}
	}
    if let Some(pid) = row.pid {
        attributes.push(attr_int("process.pid", i64::from(pid)));
    }
    if let Some(comm) = row.comm.as_deref() {
        attributes.push(attr_str("process.command", comm));
    }
    for key in [
        "agentsight_correlation_source",
        "agentsight_task_id",
        "agentsight_bound_task_id",
    ] {
        if let Some(value) = row.details.get(key).and_then(Value::as_str) {
            attributes.push(attr_str(&key.replace('_', "."), value));
        }
    }
    if let Some(tid) = row.details.get("tid").and_then(Value::as_u64) {
        attributes.push(attr_int("thread.id", tid as i64));
    }

    let flags = row
        .details
        .get("agentsight_trace_flags")
        .and_then(Value::as_u64)
        .unwrap_or(1);
    let start_unix_nano = (row.timestamp_ms as u128) * 1_000_000;
    let duration_ns = if action == "rename" {
        row.details.get("duration_ns").and_then(Value::as_u64).filter(|n| *n > 0)
    } else {
        None
    }.unwrap_or(1_000);
    Some(json!({
        "resourceSpans": [{
            "resource": { "attributes": [attr_str("service.name", service_name)] },
            "scopeSpans": [{
                "scope": { "name": "agentsight", "version": env!("CARGO_PKG_VERSION") },
                "spans": [{
                    "traceId": trace_id,
                    "spanId": new_span_id(),
                    "parentSpanId": parent_span_id,
                    "flags": flags,
                    "name": span_name,
                    "kind": 1,
                    "startTimeUnixNano": start_unix_nano.to_string(),
                    "endTimeUnixNano": (start_unix_nano + u128::from(duration_ns)).to_string(),
                    "attributes": attributes,
                    "status": status
                }]
            }]
        }]
    }))
}

fn is_read_only_file_open(row: &AuditEventRow) -> bool {
	let flags = row.details.get("flags").and_then(Value::as_i64).unwrap_or(0);
	const O_ACCMODE: i64 = 0o3;
	const O_CREAT: i64 = 0o100;
	const O_TRUNC: i64 = 0o1000;
	const O_APPEND: i64 = 0o2000;
	(flags & O_ACCMODE) == 0 && (flags & (O_CREAT | O_TRUNC | O_APPEND)) == 0
}

/// Pull an integer token count from a usage object, tolerating both the
/// OpenAI (`prompt_tokens`/`completion_tokens`) and Anthropic/Responses
/// (`input_tokens`/`output_tokens`) field names.
fn usage_int(usage: &Value, names: &[&str]) -> Option<i64> {
    names
        .iter()
        .find_map(|n| usage.get(*n).and_then(|v| v.as_i64()))
}

/// Extract finish/stop reasons from a response body across provider shapes.
fn finish_reasons(body: &Value) -> Vec<String> {
    // OpenAI chat/completions: choices[].finish_reason
    if let Some(choices) = body.get("choices").and_then(|c| c.as_array()) {
        let reasons: Vec<String> = choices
            .iter()
            .filter_map(|c| {
                c.get("finish_reason")
                    .and_then(|r| r.as_str())
                    .map(String::from)
            })
            .collect();
        if !reasons.is_empty() {
            return reasons;
        }
    }
    // Anthropic messages: stop_reason
    if let Some(stop) = body.get("stop_reason").and_then(|v| v.as_str()) {
        return vec![stop.to_string()];
    }
    Vec::new()
}

/// Build an OTLP AnyValue JSON object for a string.
fn av_str(s: &str) -> Value {
    json!({ "stringValue": s })
}

/// Build an OTLP key/value attribute with a string value.
fn attr_str(key: &str, s: &str) -> Value {
    json!({ "key": key, "value": av_str(s) })
}

/// Build an OTLP key/value attribute with an int value (int64 is JSON-encoded as
/// a string per the OTLP/JSON spec).
fn attr_int(key: &str, n: i64) -> Value {
    json!({ "key": key, "value": { "intValue": n.to_string() } })
}

/// Build an OTLP key/value attribute with a double value.
fn attr_double(key: &str, n: f64) -> Value {
    json!({ "key": key, "value": { "doubleValue": n } })
}

/// Build an OTLP key/value attribute holding an array of strings.
fn attr_str_array(key: &str, items: &[String]) -> Value {
    let values: Vec<Value> = items.iter().map(|s| av_str(s)).collect();
    json!({ "key": key, "value": { "arrayValue": { "values": values } } })
}

/// Construct the OTLP/HTTP JSON `ExportTraceServiceRequest` body for a single
/// span built from a request/response pair.
fn build_otlp_payload(
    service_name: &str,
    trace_id: &str,
    span_id: &str,
    req: &SpanInput,
    end_unix_nano: u128,
    status_code: Option<u16>,
    response_body: Option<&Value>,
    capture_content: bool,
) -> Value {
    let model_name = req.model.as_deref().unwrap_or("unknown");

    let mut attributes = vec![
        attr_str("gen_ai.operation.name", "chat"),
        attr_str("gen_ai.provider.name", &req.provider),
        attr_str("server.address", &req.server_address),
    ];
    if let Some(conversation_id) = &req.conversation_id {
        attributes.push(attr_str("gen_ai.conversation.id", conversation_id));
    }
    if let Some(model) = &req.model {
        attributes.push(attr_str("gen_ai.request.model", model));
    }
    if let Some(mt) = req.max_tokens {
        attributes.push(attr_int("gen_ai.request.max_tokens", mt));
    }
    if let Some(t) = req.temperature {
        attributes.push(attr_double("gen_ai.request.temperature", t));
    }
    if let Some(p) = req.top_p {
        attributes.push(attr_double("gen_ai.request.top_p", p));
    }
    if let Some(protocol) = req.protocol.as_deref() {
        let (name, version) = protocol
            .split_once('/')
            .map(|(name, version)| (name.to_ascii_lowercase(), Some(version)))
            .unwrap_or_else(|| (protocol.to_ascii_lowercase(), None));
        attributes.push(attr_str("network.protocol.name", &name));
        if let Some(version) = version {
            attributes.push(attr_str("network.protocol.version", version));
        }
    }
    for (key, value) in [
        ("agentsight.connection.id", req.connection_id.as_deref()),
        (
            "agentsight.http.exchange.id",
            req.http_exchange_id.as_deref(),
        ),
        ("http.request.id", req.request_id.as_deref()),
        ("gen_ai.response.id", req.response_id.as_deref()),
        (
            "agentsight.correlation.method",
            req.correlation_method.as_deref(),
        ),
        (
            "agentsight.correlation.status",
            req.correlation_status.as_deref(),
        ),
        (
            "agentsight.completion.reason",
            req.completion_reason.as_deref(),
        ),
    ] {
        if let Some(value) = value {
            attributes.push(attr_str(key, value));
        }
    }
    if let Some(stream_id) = req.stream_id {
        attributes.push(attr_int("agentsight.http.stream_id", stream_id as i64));
    }
    if let Some(version) = req.correlation_version {
        attributes.push(attr_int("agentsight.correlation.version", version as i64));
    }
    if let Some(confidence) = req.confidence {
        attributes.push(attr_double(
            "agentsight.correlation.confidence",
            confidence as f64,
        ));
    }

    // Response-derived attributes.
    let mut span_status = json!({ "code": 1 }); // STATUS_CODE_OK
    if let Some(body) = response_body {
        if let Some(rmodel) = body.get("model").and_then(|v| v.as_str()) {
            attributes.push(attr_str("gen_ai.response.model", rmodel));
        }
        if let Some(id) = body.get("id").and_then(|v| v.as_str()) {
            attributes.push(attr_str("gen_ai.response.id", id));
        }
        if let Some(usage) = body.get("usage") {
            if let Some(input) = usage_int(usage, &["input_tokens", "prompt_tokens"]) {
                attributes.push(attr_int("gen_ai.usage.input_tokens", input));
            }
            if let Some(output) = usage_int(usage, &["output_tokens", "completion_tokens"]) {
                attributes.push(attr_int("gen_ai.usage.output_tokens", output));
            }
        }
        let reasons = finish_reasons(body);
        if !reasons.is_empty() {
            attributes.push(attr_str_array("gen_ai.response.finish_reasons", &reasons));
        }
        if capture_content {
            attributes.push(attr_str("gen_ai.output.messages", &body.to_string()));
        }
    }

    // HTTP error status -> span ERROR.
    if let Some(code) = status_code {
        attributes.push(attr_int("http.response.status_code", code as i64));
        if code >= 400 {
            span_status = json!({ "code": 2, "message": format!("HTTP {}", code) });
        }
    }

    if capture_content && let Some(msgs) = &req.input_messages {
        attributes.push(attr_str("gen_ai.input.messages", msgs));
    }

    json!({
        "resourceSpans": [{
            "resource": {
                "attributes": [ attr_str("service.name", service_name) ]
            },
            "scopeSpans": [{
                "scope": { "name": "agentsight", "version": env!("CARGO_PKG_VERSION") },
                "spans": [{
                    "traceId": trace_id,
                    "spanId": span_id,
                    "name": format!("chat {}", model_name),
                    "kind": 3, // SPAN_KIND_CLIENT
                    "startTimeUnixNano": req.start_unix_nano.to_string(),
                    "endTimeUnixNano": end_unix_nano.to_string(),
                    "attributes": attributes,
                    "status": span_status
                }]
            }]
        }]
    })
}

fn new_trace_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

fn new_span_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()[..16].to_string()
}

impl SpanInput {
    fn from_call(call: &LlmCallRow, capture_content: bool) -> Self {
        let request = &call.request;
        let host = call.host.as_deref().unwrap_or_default();
        Self {
            start_unix_nano: (call.start_timestamp_ms as u128) * 1_000_000,
            provider: call
                .provider
                .clone()
                .unwrap_or_else(|| provider_from_host(host)),
            server_address: host.to_string(),
            // The materialized row may carry a generic per-response ID here.
            // Only fields explicitly identified as a conversation or thread
            // are safe to use for grouping.
            conversation_id: explicit_conversation_id(request)
                .or_else(|| explicit_conversation_id(&call.response)),
            model: call.model.clone().or_else(|| {
                request
                    .get("model")
                    .and_then(Value::as_str)
                    .map(String::from)
            }),
            max_tokens: request
                .get("max_tokens")
                .or_else(|| request.get("max_output_tokens"))
                .and_then(Value::as_i64),
            temperature: request.get("temperature").and_then(Value::as_f64),
            top_p: request.get("top_p").and_then(Value::as_f64),
            protocol: call.protocol.clone(),
            connection_id: call.connection_id.clone(),
            stream_id: call.stream_id,
            http_exchange_id: call.http_exchange_id.clone(),
            request_id: call.request_id.clone(),
            response_id: call.response_id.clone(),
            correlation_method: call.correlation_method.clone(),
            correlation_status: call.correlation_status.clone(),
            correlation_version: call.correlation_version,
            completion_reason: call.completion_reason.clone(),
            confidence: call.confidence,
            input_messages: capture_content
                .then(|| {
                    request
                        .get("messages")
                        .or_else(|| request.get("input"))
                        .map(Value::to_string)
                })
                .flatten(),
        }
    }
}

impl ViewSink for OtelExporter {
    fn llm_call(&mut self, call: &LlmCallRow) -> ViewResult<()> {
        if self.tool_enhancement {
            return Ok(());
        }
        let Some(end_ms) = call.end_timestamp_ms else {
            return Ok(());
        };
        let span_input = SpanInput::from_call(call, self.capture_content);
        let trace_id = self.trace_id_for(
            span_input.conversation_id.as_deref(),
            call.session_id.as_deref(),
        );
        let span_id = new_span_id();
        let payload = build_otlp_payload(
            &self.service_name,
            &trace_id,
            &span_id,
            &span_input,
            (end_ms as u128) * 1_000_000,
            call.status_code,
            Some(&call.response),
            self.capture_content,
        );
        let client = self.client.clone();
        let url = self.traces_url.clone();
        tokio::spawn(async move {
            if let Err(e) = post_otlp(&client, &url, payload).await {
                log::warn!("OtelExporter: failed to export span: {}", e);
            }
        });
        Ok(())
    }

    fn audit_event(&mut self, row: &AuditEventRow) -> ViewResult<()> {
        if !self.tool_enhancement {
            return Ok(());
        }
        let Some(payload) = build_correlated_audit_payload(&self.service_name, row) else {
            return Ok(());
        };
        let client = self.client.clone();
        let url = self.traces_url.clone();
        tokio::spawn(async move {
            if let Err(e) = post_otlp(&client, &url, payload).await {
                log::warn!("OtelExporter: failed to export enhanced tool span: {}", e);
            }
        });
        Ok(())
    }
}

fn explicit_conversation_id(body: &Value) -> Option<String> {
    [
        "/conversation_id",
        "/conversationId",
        "/thread_id",
        "/threadId",
        "/metadata/conversation_id",
        "/metadata/conversationId",
        "/metadata/thread_id",
        "/metadata/threadId",
        "/conversation/id",
        "/thread/id",
        "/response/conversation_id",
        "/response/conversationId",
        "/response/thread_id",
        "/response/threadId",
        "/response/conversation/id",
        "/response/thread/id",
    ]
    .iter()
    .filter_map(|path| body.pointer(path).and_then(Value::as_str))
    .find(|value| !value.is_empty())
    .map(str::to_string)
}

/// POST an OTLP/HTTP JSON trace payload to the collector.
async fn post_otlp(
    client: &Client<hyper_util::client::legacy::connect::HttpConnector, Full<Bytes>>,
    url: &str,
    payload: Value,
) -> Result<(), AnalyzerError> {
    let body = serde_json::to_vec(&payload)?;
    let req = hyper::Request::builder()
        .method(hyper::Method::POST)
        .uri(url)
        .header(hyper::header::CONTENT_TYPE, "application/json")
        .body(Full::new(Bytes::from(body)))?;

    let resp = client.request(req).await?;
    let status = resp.status();
    if !status.is_success() {
        let bytes = resp.into_body().collect().await?.to_bytes();
        let text = String::from_utf8_lossy(&bytes);
        return Err(format!("collector returned {}: {}", status, text.trim()).into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn completed_call(
        id: &str,
        session_id: Option<&str>,
        conversation_id: Option<&str>,
        request: Value,
    ) -> LlmCallRow {
        LlmCallRow {
            id: id.to_string(),
            session_id: session_id.map(str::to_string),
            conversation_id: conversation_id.map(str::to_string),
            start_timestamp_ms: 1,
            end_timestamp_ms: Some(2),
            pid: Some(1),
            comm: Some("agent".to_string()),
            provider: Some("openai".to_string()),
            model: Some("model".to_string()),
            call_kind: Some("chat".to_string()),
            status: "complete".to_string(),
            error_type: None,
            finish_reason: None,
            host: Some("api.openai.com".to_string()),
            path: Some("/v1/chat/completions".to_string()),
            status_code: Some(200),
            input_tokens: 0,
            output_tokens: 0,
            total_tokens: 0,
            request,
            response: Value::Null,
            view_source: "view".to_string(),
            confidence: Some(0.75),
            ..Default::default()
        }
    }

    #[test]
    fn maps_providers() {
        assert_eq!(provider_from_host("api.openai.com"), "openai");
        assert_eq!(provider_from_host("api.anthropic.com"), "anthropic");
        assert_eq!(
            provider_from_host("generativelanguage.googleapis.com"),
            "gcp.gen_ai"
        );
        assert_eq!(
            provider_from_host("my-resource.openai.azure.com"),
            "azure.ai.openai"
        );
        // Unknown OpenAI-compatible host is reported as the host itself.
        assert_eq!(provider_from_host("localhost:8443"), "localhost:8443");
        assert_eq!(provider_from_host("www.codebuddy.ai"), "codebuddy");
        assert_eq!(provider_from_host("copilot.tencent.com"), "codebuddy");
    }

    #[test]
    fn parses_usage_both_shapes() {
        let openai = json!({ "usage": { "prompt_tokens": 12, "completion_tokens": 7 } });
        assert_eq!(
            usage_int(&openai["usage"], &["input_tokens", "prompt_tokens"]),
            Some(12)
        );
        assert_eq!(
            usage_int(&openai["usage"], &["output_tokens", "completion_tokens"]),
            Some(7)
        );

        let anthropic = json!({ "usage": { "input_tokens": 30, "output_tokens": 15 } });
        assert_eq!(
            usage_int(&anthropic["usage"], &["input_tokens", "prompt_tokens"]),
            Some(30)
        );
    }

    #[test]
    fn extracts_finish_reasons() {
        let openai = json!({ "choices": [{ "finish_reason": "stop" }] });
        assert_eq!(finish_reasons(&openai), vec!["stop".to_string()]);
        let anthropic = json!({ "stop_reason": "end_turn" });
        assert_eq!(finish_reasons(&anthropic), vec!["end_turn".to_string()]);
        let none = json!({ "foo": 1 });
        assert!(finish_reasons(&none).is_empty());
    }

    #[test]
    fn builds_payload_with_gen_ai_attributes() {
        let req = SpanInput {
            start_unix_nano: 1_000_000_000,
            provider: "openai".to_string(),
            server_address: "api.openai.com".to_string(),
            model: Some("gpt-4o".to_string()),
            conversation_id: Some("conv_123".to_string()),
            max_tokens: Some(256),
            temperature: Some(0.7),
            top_p: None,
            input_messages: None,
            ..Default::default()
        };
        let response = json!({
            "model": "gpt-4o-2024",
            "usage": { "prompt_tokens": 10, "completion_tokens": 5 },
            "choices": [{ "finish_reason": "stop" }]
        });
        let payload = build_otlp_payload(
            "agentsight",
            "0123456789abcdef0123456789abcdef",
            "0123456789abcdef",
            &req,
            2_000_000_000,
            Some(200),
            Some(&response),
            false,
        );

        let span = &payload["resourceSpans"][0]["scopeSpans"][0]["spans"][0];
        assert_eq!(span["name"], "chat gpt-4o");
        assert_eq!(span["kind"], 3);
        assert_eq!(span["startTimeUnixNano"], "1000000000");
        assert_eq!(span["endTimeUnixNano"], "2000000000");

        let attrs = span["attributes"].as_array().unwrap();
        let find = |k: &str| attrs.iter().find(|a| a["key"] == k).cloned();
        assert_eq!(
            find("gen_ai.operation.name").unwrap()["value"]["stringValue"],
            "chat"
        );
        assert_eq!(
            find("gen_ai.provider.name").unwrap()["value"]["stringValue"],
            "openai"
        );
        assert_eq!(
            find("gen_ai.request.model").unwrap()["value"]["stringValue"],
            "gpt-4o"
        );
        assert_eq!(
            find("gen_ai.conversation.id").unwrap()["value"]["stringValue"],
            "conv_123"
        );
        assert_eq!(
            explicit_conversation_id(&json!({ "conversation": { "id": "conv_123" } })).as_deref(),
            Some("conv_123")
        );
        assert_eq!(
            explicit_conversation_id(&json!({ "session_id": "sid_123" })),
            None
        );
        assert_eq!(
            find("gen_ai.request.max_tokens").unwrap()["value"]["intValue"],
            "256"
        );
        assert_eq!(
            find("gen_ai.usage.input_tokens").unwrap()["value"]["intValue"],
            "10"
        );
        assert_eq!(
            find("gen_ai.usage.output_tokens").unwrap()["value"]["intValue"],
            "5"
        );
        assert_eq!(span["status"]["code"], 1);
        // Content not captured by default.
        assert!(find("gen_ai.input.messages").is_none());
    }

    #[test]
    fn error_status_marks_span_error() {
        let req = SpanInput {
            start_unix_nano: 1,
            provider: "openai".to_string(),
            server_address: "api.openai.com".to_string(),
            model: Some("gpt-4o".to_string()),
            conversation_id: None,
            max_tokens: None,
            temperature: None,
            top_p: None,
            input_messages: None,
            ..Default::default()
        };
        let payload = build_otlp_payload("agentsight", "t", "s", &req, 2, Some(429), None, false);
        let span = &payload["resourceSpans"][0]["scopeSpans"][0]["spans"][0];
        assert_eq!(span["status"]["code"], 2);
    }

    #[test]
    fn correlates_trace_ids_by_explicit_conversation_then_session() {
        let mut exporter = OtelExporter::new(Some("http://localhost:4318".to_string()), false);

        let conversation_a = exporter.trace_id_for(Some("conversation-a"), Some("session-a"));
        assert_eq!(
            conversation_a,
            exporter.trace_id_for(Some("conversation-a"), Some("session-b"))
        );
        assert_ne!(
            conversation_a,
            exporter.trace_id_for(Some("conversation-b"), Some("session-a"))
        );

        let session_a = exporter.trace_id_for(None, Some("session-a"));
        assert_eq!(session_a, exporter.trace_id_for(None, Some("session-a")));
        assert_ne!(session_a, exporter.trace_id_for(None, Some("session-b")));
    }

    #[test]
    fn per_response_conversation_ids_do_not_split_one_session() {
        let mut exporter = OtelExporter::new(Some("http://localhost:4318".to_string()), false);
        let first = completed_call("first", Some("session-a"), Some("chatcmpl-a"), json!({}));
        let second = completed_call("second", Some("session-a"), Some("chatcmpl-b"), json!({}));

        let first_input = SpanInput::from_call(&first, false);
        let second_input = SpanInput::from_call(&second, false);
        assert!(first_input.conversation_id.is_none());
        assert_eq!(
            exporter.trace_id_for(
                first_input.conversation_id.as_deref(),
                first.session_id.as_deref()
            ),
            exporter.trace_id_for(
                second_input.conversation_id.as_deref(),
                second.session_id.as_deref()
            )
        );
    }

    #[test]
    fn explicit_conversation_overrides_different_sessions() {
        let mut exporter = OtelExporter::new(Some("http://localhost:4318".to_string()), false);
        let first = completed_call(
            "first",
            Some("session-a"),
            None,
            json!({ "metadata": { "conversation_id": "conversation-a" } }),
        );
        let mut second = completed_call("second", Some("session-b"), None, json!({}));
        second.response = json!({ "thread": { "id": "conversation-a" } });

        let first_input = SpanInput::from_call(&first, false);
        let second_input = SpanInput::from_call(&second, false);
        assert_eq!(
            first_input.conversation_id.as_deref(),
            Some("conversation-a")
        );
        assert_eq!(first_input.conversation_id, second_input.conversation_id);
        assert_eq!(
            exporter.trace_id_for(
                first_input.conversation_id.as_deref(),
                first.session_id.as_deref()
            ),
            exporter.trace_id_for(
                second_input.conversation_id.as_deref(),
                second.session_id.as_deref()
            )
        );
    }

    #[test]
    fn uses_one_recording_trace_without_correlation_ids() {
        let mut exporter = OtelExporter::new(Some("http://localhost:4318".to_string()), false);
        assert_eq!(
            exporter.trace_id_for(None, None),
            exporter.trace_id_for(None, None)
        );
        assert_ne!(new_span_id(), new_span_id());
    }

    #[test]
    fn correlated_file_open_keeps_instrumented_trace_and_parent() {
        let row = AuditEventRow {
            id: "audit-1".to_string(),
            timestamp_ms: 1_234,
            audit_type: "file".to_string(),
            pid: Some(42),
            comm: Some("python3".to_string()),
            subject: Some("python3".to_string()),
            action: Some("open".to_string()),
            target: Some("/tmp/input.txt".to_string()),
            status: Some("observed".to_string()),
            summary: None,
            details: json!({
                "tid": 43,
                "agentsight_trace_id": "0123456789abcdef0123456789abcdef",
                "agentsight_parent_span_id": "fedcba9876543210",
                "agentsight_trace_flags": 1,
                "agentsight_task_id": "0000aaaabbbbcccc",
                "agentsight_bound_task_id": "0000aaaabbbbcccc",
                "agentsight_correlation_source": "task"
            }),
            view_source: "view".to_string(),
            confidence: Some(0.75),
        };

        let payload = build_correlated_audit_payload("agentsight-enhancement", &row)
            .expect("correlated payload");
        let span = &payload["resourceSpans"][0]["scopeSpans"][0]["spans"][0];
        assert_eq!(span["traceId"], "0123456789abcdef0123456789abcdef");
        assert_eq!(span["parentSpanId"], "fedcba9876543210");
        assert_eq!(span["name"], "agentsight.file.open");
        assert_eq!(span["kind"], 1);
        assert_eq!(span["startTimeUnixNano"], "1234000000");
    }

    #[test]
    fn writable_file_open_is_kept_out_of_enhancement_export() {
        let row = AuditEventRow {
            id: "audit-write-open".to_string(),
            timestamp_ms: 1,
            audit_type: "file".to_string(),
            pid: Some(42),
            comm: Some("python3".to_string()),
            subject: None,
            action: Some("open".to_string()),
            target: Some("/tmp/context.tmp".to_string()),
            status: None,
            summary: None,
            details: json!({
                "flags": 577,
                "agentsight_trace_id": "0123456789abcdef0123456789abcdef",
                "agentsight_parent_span_id": "fedcba9876543210"
            }),
            view_source: "view".to_string(),
            confidence: None,
        };

        assert!(build_correlated_audit_payload("agentsight", &row).is_none());
    }

    #[test]
    fn correlated_write_and_rename_map_semantic_file_attributes() {
        let base_details = json!({
            "tid": 43,
            "agentsight_trace_id": "0123456789abcdef0123456789abcdef",
            "agentsight_parent_span_id": "fedcba9876543210",
            "agentsight_trace_flags": 1,
            "agentsight_task_id": "0000aaaabbbbcccc",
            "agentsight_bound_task_id": "0000aaaabbbbcccc",
            "agentsight_correlation_source": "task"
        });
        let mut write_details = base_details.clone();
        write_details["count"] = json!(2);
        write_details["total_bytes"] = json!(142);
        let write = AuditEventRow {
            id: "audit-write".to_string(),
            timestamp_ms: 2,
            audit_type: "file".to_string(),
            pid: Some(42),
            comm: Some("python3".to_string()),
            subject: None,
            action: Some("write".to_string()),
            target: Some("/tmp/context.tmp".to_string()),
            status: None,
            summary: None,
            details: write_details,
            view_source: "view".to_string(),
            confidence: None,
        };
        let write_payload = build_correlated_audit_payload("service", &write).expect("write payload");
        let write_span = &write_payload["resourceSpans"][0]["scopeSpans"][0]["spans"][0];
        assert_eq!(write_span["name"], "agentsight.file.write");
        assert!(write_span["attributes"].as_array().unwrap().iter().any(|attribute| {
            attribute["key"] == "file.write.bytes" && attribute["value"]["intValue"] == "142"
        }));

        let mut rename_details = base_details;
        rename_details["source_path"] = json!("/tmp/context.tmp");
        rename_details["destination_path"] = json!("/tmp/context.json");
        let mut rename = AuditEventRow {
            id: "audit-rename".to_string(),
            timestamp_ms: 3,
            audit_type: "file".to_string(),
            pid: Some(42),
            comm: Some("python3".to_string()),
            subject: None,
            action: Some("rename".to_string()),
            target: Some("/tmp/context.json".to_string()),
            status: None,
            summary: None,
            details: rename_details,
            view_source: "view".to_string(),
            confidence: None,
        };
        let rename_payload =
            build_correlated_audit_payload("service", &rename).expect("rename payload");
        let rename_span = &rename_payload["resourceSpans"][0]["scopeSpans"][0]["spans"][0];
        assert_eq!(rename_span["name"], "agentsight.file.rename");
        assert!(rename_span["attributes"].as_array().unwrap().iter().any(|attribute| {
            attribute["key"] == "file.source.path"
                && attribute["value"]["stringValue"] == "/tmp/context.tmp"
        }));
        assert!(rename_span["attributes"].as_array().unwrap().iter().any(|attribute| {
            attribute["key"] == "file.destination.path"
                && attribute["value"]["stringValue"] == "/tmp/context.json"
        }));
        assert_eq!(rename_span["status"]["code"], 0, "entry-only result is unknown");
        for (result, code, error) in [
            (0, 1, None),
            (-13, 2, Some("EACCES")),
            (-2, 2, Some("ENOENT")),
            (-999, 2, Some("errno.999")),
        ] {
            rename.details["syscall_return"] = json!(result);
            rename.details["duration_ns"] = json!(4500);
            rename.details["count"] = json!(1);
            let payload = build_correlated_audit_payload("service", &rename).unwrap();
            let span = &payload["resourceSpans"][0]["scopeSpans"][0]["spans"][0];
            assert_eq!(span["traceId"], "0123456789abcdef0123456789abcdef");
            assert_eq!(span["parentSpanId"], "fedcba9876543210");
            assert_ne!(span["spanId"], span["parentSpanId"]);
            assert_eq!(span["status"]["code"], code);
            assert_eq!(span["endTimeUnixNano"], "3004500");
            let attrs = span["attributes"].as_array().unwrap();
            assert!(attrs.iter().any(|a| a["key"] == "syscall.return"
                && a["value"]["intValue"] == result.to_string()));
            let error_attr = attrs.iter().find(|a| a["key"] == "error.type");
            assert_eq!(error_attr.map(|a| a["value"]["stringValue"].as_str().unwrap()), error);
        }
    }

    #[test]
    fn uncorrelated_audit_is_not_exported() {
        let row = AuditEventRow {
            id: "audit-1".to_string(),
            timestamp_ms: 1,
            audit_type: "file".to_string(),
            pid: None,
            comm: None,
            subject: None,
            action: Some("open".to_string()),
            target: Some("/tmp/input.txt".to_string()),
            status: None,
            summary: None,
            details: json!({}),
            view_source: "view".to_string(),
            confidence: None,
        };
        assert!(build_correlated_audit_payload("agentsight", &row).is_none());
    }
}
