// Telemetry bootstrap. Call at the top of main(), before serving.
// Replace {{PRODUCT}}/{{SERVICE}}. See standards/docs/observability.md.
//
// Cargo.toml:  liblog = { git = "ssh://git@github.com/charliethomson/liblog.git" }
//              libproduct = { git = "https://github.com/charliethomson/libpath" } // member crate of the libpath repo
// .cargo/config.toml:  [build] rustflags = ["--cfg", "tracing_unstable"]

use libproduct::product_name;

fn init_telemetry() -> liblog::LoggingGuard {
    // Reverse-domain product name → OTel service.name. Set before liblog init.
    product_name!("dev.thmsn.{{PRODUCT}}.{{SERVICE}}");

    let otlp = std::env::var("OTLP_ENDPOINT").ok();
    let production = std::env::var("PRODUCTION").is_ok();
    let sample_rate = std::env::var("SAMPLE_RATE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(if production { 0.1 } else { 1.0 });

    let guard = liblog::builder()
        .endpoint(otlp.as_deref())
        .sample_rate(sample_rate)
        .deployment_environment(if production { "production" } else { "development" })
        .init();

    tracing::info!(version = env!("APP_VERSION"), "starting {{PRODUCT}}-{{SERVICE}}");
    tracing::info!(
        otlp_export = otlp.is_some(),
        production,
        sample_rate,
        "telemetry initialised"
    );
    guard
}

// Usage:
//   let guard = init_telemetry();
//   let result = run().await;
//   liblog::force_cleanup(guard);   // flush OTLP before exit
//   result
