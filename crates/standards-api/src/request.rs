//! The request layer: one `http.request` span and one `http completed` line per
//! request, errors and panics included (<https://standards.thmsn.dev/d/tracing>).
//!
//! It is the outermost layer, so it sees what every inner one produced:
//!
//! - **The span** is a root on the caller's trace id. Every caller here is an app,
//!   a browser or the CLI, none of which exports the span its `traceparent`
//!   names, so the span *adopts* the trace (same id, no parent, a link) rather
//!   than continuing it; continuing would leave every trace rootless in Tempo.
//! - **The completion line** goes out for every response, at ERROR for a 5xx and
//!   INFO otherwise (DEBUG for the hot routes), with a fixed field set: a 200
//!   carries `error.type = ""`. A failure's one log is this line; the error
//!   mapper records onto it through [`record_error`] instead of logging.
//! - **The response** gets `x-standards-trace-id`, and an error body that is not
//!   already JSON becomes `{"message", "$type", "traceId"}`.
//!
//! Fields learned deep inside a handler (who the caller is, why it failed) reach
//! the request span through a task-local handle, because by then
//! `Span::current()` is some child span.
//!
//! Copied from `templates/rust/request_layer.rs` with the product filled in.

// The only panics are on a poisoned request-context mutex, which no code path here can
// poison (nothing panics while holding it).
#![allow(clippy::missing_panics_doc)]

use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::metrics::Metrics;
use liblog::OpenTelemetrySpanExt as _;
use liblog::opentelemetry::trace::TraceContextExt as _;
use poem::PathPattern;
use poem::http::{HeaderMap, HeaderValue, StatusCode};
use poem::{Endpoint, Request, Response};
use poem_openapi::OperationId;
use tracing::Instrument as _;

/// `operationId` → the route template the contract names it by.
pub type RouteTemplates = std::collections::HashMap<String, String>;

/// The response header that echoes the trace id, so a client can show `Ref:`.
pub const TRACE_ID_HEADER: &str = "x-standards-trace-id";

/// What the handler side learned about this request, read back by the layer.
#[derive(Default)]
struct Learned {
    enduser_id: Option<String>,
    auth_method: Option<&'static str>,
    error_type: Option<String>,
    error_message: Option<String>,
}

struct RequestCtx {
    span: tracing::Span,
    learned: Mutex<Learned>,
}

tokio::task_local! {
    static REQUEST: Arc<RequestCtx>;
}

fn with_ctx(f: impl FnOnce(&RequestCtx)) {
    let _ = REQUEST.try_with(|ctx| f(ctx));
}

/// The authenticated caller's public id. A handle, never a secret.
pub fn record_enduser(public_id: &str, auth_method: &'static str) {
    with_ctx(|ctx| {
        ctx.span.record("enduser.id", public_id);
        ctx.span.record("auth.method", auth_method);
        let mut learned = ctx.learned.lock().expect("request ctx poisoned");
        learned.enduser_id = Some(public_id.to_owned());
        learned.auth_method = Some(auth_method);
    });
}

/// Why this request failed: a stable `kind` (an error key, `not_found`, `panic`)
/// and, for a server-side failure, a sanitised detail for the log. Recorded on
/// the request span; the completion line carries both. Never logs itself, so a
/// failure is logged once.
pub fn record_error(kind: &str, detail: Option<String>) {
    with_ctx(|ctx| {
        let mut learned = ctx.learned.lock().expect("request ctx poisoned");
        learned.error_type = Some(kind.to_owned());
        learned.error_message = detail;
    });
}

/// The trace id of the request being served, for an error body. `None` outside
/// a request or with no span export installed.
#[must_use]
pub fn current_trace_id() -> Option<String> {
    let mut out = None;
    with_ctx(|ctx| out = trace_id_of(&ctx.span));
    out
}

fn trace_id_of(span: &tracing::Span) -> Option<String> {
    let id = span.context().span().span_context().trace_id();
    (id != liblog::opentelemetry::trace::TraceId::INVALID).then(|| id.to_string())
}

/// The JSON error body every client reads: `message` for the user, `$type` for
/// code, `traceId` for `Ref:`.
fn error_json(kind: &str, message: &str, trace_id: Option<&str>) -> String {
    serde_json::json!({ "message": message, "$type": kind, "traceId": trace_id }).to_string()
}

/// An error response in that shape, for the request being served.
#[must_use]
pub fn error_response(status: StatusCode, kind: &str, message: &str) -> Response {
    Response::builder()
        .status(status)
        .content_type(JSON)
        .body(error_json(kind, message, current_trace_id().as_deref()))
}

const JSON: &str = "application/json; charset=utf-8";

/// Routes that are polled, not used: their span and line are DEBUG, which is
/// not exported. The metric still counts them.
fn is_hot(path: &str) -> bool {
    matches!(path, "/health" | "/api/metrics" | "/openapi.json") || path.starts_with("/api/docs")
}

/// A client identity header, if it is printable ASCII of at most 64 bytes.
/// Anything else is dropped, never truncated: a mangled id is worse than none.
fn identity(headers: &HeaderMap, name: &str) -> Option<String> {
    let value = headers.get(name)?.to_str().ok()?;
    (!value.is_empty() && value.len() <= 64 && value.bytes().all(|b| (0x20..0x7f).contains(&b)))
        .then(|| value.to_owned())
}

/// The operation name the dashboard groups by: the `operationId`, or a fixed
/// name for the routes outside the contract.
fn api_operation(operation: Option<&'static str>, path: &str) -> &'static str {
    if let Some(id) = operation {
        return id;
    }
    match path {
        "/health" => "health",
        "/api/metrics" => "metrics",
        "/openapi.json" => "openapi",
        docs if docs == "/api/docs" || docs.starts_with("/api/docs/") => "api_docs",
        "/manifest.json" => "manifest_json",
        "/version" => "version",
        "/install" => "install",
        "/mcp" => "mcp",
        raw if raw.starts_with("/t/") => "raw_template",
        "/" => "web_index",
        "/search" => "web_search",
        doc if doc.starts_with("/d/") => "web_doc",
        asset if asset.starts_with("/static/") => "web_static",
        _ => "unmatched",
    }
}

/// Coarse status class, kept for the dashboard's Loki panels.
fn http_outcome(status: u16) -> &'static str {
    match status {
        200..=299 => "success",
        300..=399 => "redirect",
        400..=499 => "client_error",
        500..=599 => "server_error",
        _ => "other",
    }
}

macro_rules! request_span {
    ($level:expr, $method:expr, $path:expr) => {
        tracing::span!(
            $level,
            "http.request",
            otel.kind = "server",
            otel.name = $method,
            otel.status_code = tracing::field::Empty,
            error.type = tracing::field::Empty,
            http.request.method = $method,
            http.route = tracing::field::Empty,
            url.path = $path,
            http.response.status_code = tracing::field::Empty,
            api_operation = tracing::field::Empty,
            enduser.id = tracing::field::Empty,
            auth.method = tracing::field::Empty,
            client.app = tracing::field::Empty,
            client.version = tracing::field::Empty,
            client.platform = tracing::field::Empty,
            client.install_id = tracing::field::Empty,
            session.id = tracing::field::Empty,
        )
    };
}

/// The client's identity headers onto the span; returns `client.app` for the
/// completion line (span fields don't reach Loki).
fn record_identity(span: &tracing::Span, headers: &HeaderMap) -> String {
    let client = [
        ("client.app", identity(headers, "x-standards-client")),
        (
            "client.version",
            identity(headers, "x-standards-client-version"),
        ),
        ("client.platform", identity(headers, "x-standards-platform")),
        (
            "client.install_id",
            identity(headers, "x-standards-install-id"),
        ),
        ("session.id", identity(headers, "x-standards-session-id")),
    ];
    for (field, value) in &client {
        if let Some(value) = value {
            span.record(*field, value.as_str());
        }
    }
    client[0].1.clone().unwrap_or_default()
}

/// A plain-text error body (poem's own, or a handler's `from_string`) rewritten
/// as the JSON every client reads. The text is passed through: every plain-text
/// error here is written for the client (`auth service unavailable`), and the
/// one source of internal detail, `to_poem`, already answers JSON.
async fn to_json_error(response: &mut Response, kind: &str, trace_id: Option<&str>) {
    let status = response.status();
    let text = response.take_body().into_string().await.unwrap_or_default();
    let text = match text.trim() {
        "" => status.canonical_reason().unwrap_or("error").to_owned(),
        text => text.to_owned(),
    };
    response.headers_mut().insert(
        poem::http::header::CONTENT_TYPE,
        HeaderValue::from_static(JSON),
    );
    response.set_body(error_json(kind, &text, trace_id));
}

/// The layer itself, for `Route::around`.
#[allow(clippy::too_many_lines)] // one linear pass (span, call, response, metric, line) whose field set must stay in one place
pub async fn serve<E: Endpoint<Output = Response>>(
    ep: Arc<E>,
    req: Request,
    metrics: Metrics,
    routes: Arc<RouteTemplates>,
) -> poem::Result<Response> {
    let method = req.method().as_str().to_owned();
    // The path for the span only: no query string, ever. Ids in it are public
    // ids and refs, never user prose (`tracing.md`, Privacy).
    let path = req.uri().path().to_owned();
    let hot = is_hot(&path);
    let span = if hot {
        request_span!(tracing::Level::DEBUG, method.as_str(), path.as_str())
    } else {
        request_span!(tracing::Level::INFO, method.as_str(), path.as_str())
    };
    // Before the span is entered or read: both fix its trace id.
    let cx = liblog::extract_context(&liblog::HeaderExtractor(req.headers()));
    liblog::adopt_client_trace(&span, &cx);

    let client_app = record_identity(&span, req.headers());

    let ctx = Arc::new(RequestCtx {
        span: span.clone(),
        learned: Mutex::new(Learned::default()),
    });
    let start = Instant::now();
    let result = REQUEST
        .scope(ctx.clone(), ep.call(req).instrument(span.clone()))
        .await;
    let duration = start.elapsed();

    let (route, operation, mut response) = match result {
        Ok(resp) => (
            resp.data::<PathPattern>().map(|p| p.0.to_string()),
            resp.data::<OperationId>().map(|o| o.0),
            resp,
        ),
        Err(err) => (
            err.data::<PathPattern>().map(|p| p.0.to_string()),
            err.data::<OperationId>().map(|o| o.0),
            err.into_response(),
        ),
    };
    let status = response.status();
    let trace_id = trace_id_of(&span);
    let learned = std::mem::take(&mut *ctx.learned.lock().expect("request ctx poisoned"));

    let error_type = if status.is_client_error() || status.is_server_error() {
        let kind = learned
            .error_type
            .clone()
            .unwrap_or_else(|| format!("http.{}", status.as_u16()));
        // The site's own error pages (web.rs) stay HTML.
        if !is_json(&response) && !is_html(&response) {
            to_json_error(&mut response, &kind, trace_id.as_deref()).await;
        }
        kind
    } else {
        String::new()
    };
    if let Some(id) = trace_id
        .as_deref()
        .and_then(|id| HeaderValue::from_str(id).ok())
    {
        response.headers_mut().insert(TRACE_ID_HEADER, id);
    }

    let operation = api_operation(operation, &path);
    // The contract's own template for an operation (`/api/v1/orders/{id}`,
    // not poem's `:param0`); the matched pattern for the few routes outside it;
    // `unmatched` for a miss, whose pattern is only the mount it fell through.
    let route = match routes.get(operation) {
        Some(template) => Some(template.clone()),
        None if operation == "unmatched" => None,
        None => route,
    };
    let route_label = route.clone().unwrap_or_else(|| "unmatched".to_owned());
    let status_i64 = i64::from(status.as_u16());
    let duration_ms = i64::try_from(duration.as_millis()).unwrap_or(i64::MAX);
    if let Some(route) = route.as_deref() {
        span.record("http.route", route);
        span.record("otel.name", format!("{method} {route}"));
    }
    span.record("http.response.status_code", status_i64);
    span.record("api_operation", operation);
    if status.is_server_error() {
        span.record("otel.status_code", "ERROR");
        span.record("error.type", error_type.as_str());
    }
    metrics.http_request(
        &method,
        &route_label,
        status.as_u16(),
        duration.as_secs_f64(),
    );

    let enduser = learned.enduser_id.unwrap_or_default();
    let error_message = learned.error_message.unwrap_or_default();
    let outcome = http_outcome(status.as_u16());
    // One line per request with one field set, so a query can always key off
    // `error.type` and `trace_id` (the metadata a `Ref:` resolves through).
    macro_rules! completed {
        ($level:expr) => {
            tracing::event!(
                parent: &span,
                $level,
                method = method.as_str(),
                route = route_label.as_str(),
                api_operation = operation,
                status = status_i64,
                duration_ms,
                outcome,
                error.type = error_type.as_str(),
                error.message = error_message.as_str(),
                enduser.id = enduser.as_str(),
                client.app = client_app.as_str(),
                "http completed"
            )
        };
    }
    if status.is_server_error() {
        completed!(tracing::Level::ERROR);
    } else if hot {
        completed!(tracing::Level::DEBUG);
    } else {
        completed!(tracing::Level::INFO);
    }
    Ok(response)
}

fn is_html(response: &Response) -> bool {
    response
        .headers()
        .get(poem::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("text/html"))
}

fn is_json(response: &Response) -> bool {
    response
        .headers()
        .get(poem::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("application/json"))
}

/// What a panicking handler answers, from inside the request layer, so the panic
/// is a logged 500 with a trace id. The payload is not logged: it can hold
/// whatever the panicking code was formatting.
#[must_use]
pub fn panic_response(_payload: Box<dyn std::any::Any + Send + 'static>) -> Response {
    record_error("panic", None);
    error_response(StatusCode::INTERNAL_SERVER_ERROR, "panic", "internal error")
}

/// A panic anywhere else (a background task) is one ERROR line, with where it
/// happened. Installed once at startup; the previous hook still prints to stderr.
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_default();
        tracing::error!(error.type = "panic", location = location.as_str(), "panicked");
        previous(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_headers_are_dropped_not_truncated() {
        let mut headers = HeaderMap::new();
        headers.insert("x-standards-client", HeaderValue::from_static("web"));
        headers.insert(
            "x-standards-session-id",
            HeaderValue::from_str(&"a".repeat(65)).unwrap(),
        );
        headers.insert("x-standards-platform", HeaderValue::from_static(""));
        assert_eq!(
            identity(&headers, "x-standards-client").as_deref(),
            Some("web")
        );
        assert_eq!(identity(&headers, "x-standards-session-id"), None);
        assert_eq!(identity(&headers, "x-standards-platform"), None);
        assert_eq!(identity(&headers, "x-standards-install-id"), None);
    }
}
