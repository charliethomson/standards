//! The process's Prometheus registry and every `standards_*` series (docs/observability.md).
//!
//! Families:
//! - `standards_http_requests_total{method,route,status}` and
//!   `standards_http_request_duration_seconds{method,route}`, from the outermost request
//!   layer, so error responses and panics count too. `route` is the contract template
//!   (`/api/v1/docs/{id}`), the matched pattern for routes outside the contract, or
//!   `unmatched`; `method` is a standard method or `other`.
//! - `standards_build_info{version,commit} 1` and `standards_process_start_time_seconds`.
//! - `standards_content_items{kind}`: docs, archetypes, skills and templates served.
//!
//! Label values come from bounded sets only, never a path a client typed.

use std::sync::Arc;
use std::sync::atomic::AtomicI64;
use std::time::{SystemTime, UNIX_EPOCH};

use prometheus_client::encoding::{EncodeLabelSet, text};
use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::metrics::histogram::{Histogram, exponential_buckets};
use prometheus_client::registry::Registry;
use standards_core::Bundle;

/// The `OpenMetrics` content type: `prometheus-client` only emits `OpenMetrics` text.
pub const OPENMETRICS: &str = "application/openmetrics-text; version=1.0.0; charset=utf-8";

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
struct HttpLabels {
    method: &'static str,
    route: String,
    status: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
struct RouteLabels {
    method: &'static str,
    route: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
struct BuildLabels {
    version: String,
    commit: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
struct KindLabel {
    kind: &'static str,
}

/// A request method as a bounded label.
fn method_label(method: &str) -> &'static str {
    match method {
        "GET" => "GET",
        "HEAD" => "HEAD",
        "POST" => "POST",
        "PUT" => "PUT",
        "PATCH" => "PATCH",
        "DELETE" => "DELETE",
        "OPTIONS" => "OPTIONS",
        _ => "other",
    }
}

fn duration_histogram() -> Histogram {
    // 1ms .. ~16s
    Histogram::new(exponential_buckets(0.001, 2.0, 15))
}

/// The registry and its families. Cheap to clone.
#[derive(Clone)]
pub struct Metrics {
    registry: Arc<Registry>,
    requests: Family<HttpLabels, Counter>,
    durations: Family<RouteLabels, Histogram, fn() -> Histogram>,
}

impl Metrics {
    /// Register every family, with the build and content series set from `bundle`.
    #[must_use]
    pub fn new(version: &str, bundle: &Bundle) -> Self {
        let mut registry = Registry::default();

        let requests = Family::<HttpLabels, Counter>::default();
        registry.register(
            "standards_http_requests",
            "HTTP responses by method, route template and status",
            requests.clone(),
        );
        let durations: Family<RouteLabels, Histogram, fn() -> Histogram> =
            Family::new_with_constructor(duration_histogram);
        registry.register(
            "standards_http_request_duration_seconds",
            "Time to response headers by method and route template",
            durations.clone(),
        );

        let build = Family::<BuildLabels, Gauge>::default();
        build
            .get_or_create(&BuildLabels {
                version: version.to_owned(),
                commit: bundle.commit.clone(),
            })
            .set(1);
        registry.register(
            "standards_build_info",
            "The running build: derived version and content commit",
            build,
        );

        let start = Gauge::<i64, AtomicI64>::default();
        start.set(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX)),
        );
        registry.register(
            "standards_process_start_time_seconds",
            "Unix time the process started",
            start,
        );

        let content = Family::<KindLabel, Gauge<i64, AtomicI64>>::default();
        for (kind, n) in [
            ("doc", bundle.docs.len()),
            ("archetype", bundle.archetypes.len()),
            ("skill", bundle.skills.len()),
            ("template", bundle.templates.len()),
        ] {
            content
                .get_or_create(&KindLabel { kind })
                .set(i64::try_from(n).unwrap_or(i64::MAX));
        }
        registry.register(
            "standards_content_items",
            "Items served from the embedded content, by kind",
            content,
        );

        let metrics = Self {
            registry: Arc::new(registry),
            requests,
            durations,
        };
        // Pre-seed one series per status class so alerts on them can fire from zero.
        for status in ["200", "304", "400", "404", "500"] {
            let _ = metrics.requests.get_or_create(&HttpLabels {
                method: "GET",
                route: "unmatched".to_owned(),
                status: status.to_owned(),
            });
        }
        let _ = metrics.durations.get_or_create(&RouteLabels {
            method: "GET",
            route: "unmatched".to_owned(),
        });
        metrics
    }

    /// Count one response.
    pub fn http_request(&self, method: &str, route: &str, status: u16, secs: f64) {
        let method = method_label(method);
        self.requests
            .get_or_create(&HttpLabels {
                method,
                route: route.to_owned(),
                status: status.to_string(),
            })
            .inc();
        self.durations
            .get_or_create(&RouteLabels {
                method,
                route: route.to_owned(),
            })
            .observe(secs);
    }

    /// The `OpenMetrics` text for a scrape.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::new();
        // Writing into a String cannot fail.
        let _ = text::encode(&mut out, &self.registry);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_a_sample_for_every_family() {
        let bundle = standards_db::bundle().expect("bundle");
        let m = Metrics::new("1.2.3", &bundle);
        m.http_request("GET", "/api/v1/docs/{id}", 200, 0.01);
        m.http_request("BREW", "/x", 418, 0.01);
        let out = m.render();
        for needle in [
            "standards_http_requests_total{method=\"GET\",route=\"/api/v1/docs/{id}\",status=\"200\"} 1",
            "standards_http_requests_total{method=\"other\",route=\"/x\",status=\"418\"} 1",
            "standards_http_requests_total{method=\"GET\",route=\"unmatched\",status=\"404\"} 0",
            "standards_http_request_duration_seconds_count{method=\"GET\",route=\"/api/v1/docs/{id}\"} 1",
            "standards_build_info{version=\"1.2.3\",commit=\"",
            "standards_process_start_time_seconds ",
            "standards_content_items{kind=\"skill\"} ",
            "standards_content_items{kind=\"doc\"} ",
        ] {
            assert!(out.contains(needle), "missing {needle:?} in:\n{out}");
        }
        assert!(out.trim_end().ends_with("# EOF"));
    }
}
