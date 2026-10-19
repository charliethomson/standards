// Telemetry bootstrap. Call at the top of main(), before serving.
// Replace {{PRODUCT}}/{{SERVICE}}. See https://standards.thmsn.dev/d/observability and https://standards.thmsn.dev/d/tracing.
//
// Cargo.toml:  liblog = { git = "ssh://git@github.com/charliethomson/liblog.git" }
//              libproduct = { git = "https://github.com/charliethomson/libpath" } // member crate of the libpath repo
// .cargo/config.toml:  [build] rustflags = ["--cfg", "tracing_unstable"]

use libproduct::product_name;

// Reverse-domain product name → OTel service.name. This only DECLARES it; init_telemetry publishes it.
product_name!("dev.thmsn.{{PRODUCT}}.{{SERVICE}}");

/// `PRODUCTION=true|1|yes` (any case). Not `is_ok()`: that makes `PRODUCTION=false` prod.
fn production() -> bool {
    std::env::var("PRODUCTION")
        .is_ok_and(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
}

/// Always-on: the collector tail-samples (docs/tracing.md). `SAMPLE_RATE` pins a
/// parent-based ratio for local work; `OTEL_TRACES_SAMPLER` overrides both inside liblog.
fn sampling() -> liblog::Sampling {
    match std::env::var("SAMPLE_RATE").ok().and_then(|s| s.parse().ok()) {
        Some(rate) => liblog::Sampling::Ratio(rate),
        None => liblog::Sampling::AlwaysOn,
    }
}

fn init_telemetry() -> liblog::LoggingGuard {
    // Publish the product name, before liblog init and before anything reads a libpath root.
    // Skip this and ProductName::global() stays None: service.name falls back and
    // logs/config/cache all land under dev.thmsn.unspecified, with no error.
    // Bind to a local first — product_name! expands to a `const` holding a
    // LazyLock, so borrowing it inline acts on a fresh copy and trips
    // clippy::borrow_interior_mutable_const.
    let product_name = PRODUCT_NAME;
    product_name
        .set_global()
        .expect("failed to set global product name");

    let otlp = std::env::var("OTLP_ENDPOINT").ok();
    let production = production();
    let sampling = sampling();

    // Per-layer filters are liblog defaults; tune them from env, not here
    // (LIBLOG_OTEL_FILTER / LIBLOG_LOG_FILTER / LIBLOG_JSON_FILTER / LIBLOG_STDOUT_FILTER).
    let guard = liblog::builder()
        .endpoint(otlp.as_deref())
        .sampling(sampling)
        .deployment_environment(if production { "production" } else { "development" })
        // If this binary speaks a protocol over stdio (e.g. an MCP server), add:
        //     .console_stream(liblog::ConsoleStream::Stderr)
        // Otherwise the first log record is written into the wire and the peer's
        // parser fails, silently. See https://standards.thmsn.dev/d/observability.
        .init();

    tracing::info!(
        version = env!("APP_VERSION"),
        otlp_export = otlp.is_some(),
        production,
        ?sampling,
        "starting {{PRODUCT}}-{{SERVICE}}"
    );
    guard
}

// Usage:
//   let guard = init_telemetry();
//   let result = run().await;
//   liblog::force_cleanup(guard);   // flush OTLP before exit
//   result
