//! `standards-server`: serves the embedded standards (`dev.thmsn.standards.server`).
//!
//! ```text
//! standards-server [serve]   load config, start telemetry, serve until SIGTERM/SIGINT
//! standards-server openapi   print the OpenAPI document (committed as api/openapi.json)
//! standards-server version   print the derived version
//! ```
//!
//! Boot order: config, telemetry, bundle (fatal if the embedded content does not parse),
//! state, serve with a 10s graceful drain. Everything worth testing is in the library;
//! this file is wiring.

use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use libproduct::product_name;
use poem::Server;
use poem::listener::TcpListener;
use standards_api::{AppState, Config, app, rest};
use tokio_util::sync::CancellationToken;

product_name!("dev.thmsn.standards.server");

type BoxError = Box<dyn std::error::Error>;

#[tokio::main]
async fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        None | Some("serve") => {}
        Some("openapi") => {
            println!("{}", rest::spec());
            return ExitCode::SUCCESS;
        }
        Some("version" | "--version" | "-V") => {
            println!("{}", standards_core::VERSION);
            return ExitCode::SUCCESS;
        }
        Some(other) => {
            eprintln!("unknown command `{other}`; usage: standards-server [serve|openapi|version]");
            return ExitCode::from(2);
        }
    }

    // Every libpath root (liblog's file) sits under LIBPATH_BASE_DIR in the container;
    // never force-create one (docs/configuration.md).
    libpath::set_create_dirs(false);

    let config = match Config::load() {
        Ok(c) => c,
        Err(error) => {
            eprintln!("refusing to boot: config: {error}");
            return ExitCode::FAILURE;
        }
    };
    let guard = init_telemetry(&config);
    let result = run(config).await;
    if let Err(error) = &result {
        tracing::error!(error.type = "serve", %error, "standards-server stopped with an error");
    }
    liblog::force_cleanup(guard);
    if result.is_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn init_telemetry(config: &Config) -> liblog::LoggingGuard {
    // Publish the identity before liblog reads it (service.name) or libpath derives a root.
    let product_name = PRODUCT_NAME;
    let _ = product_name.set_global();

    let otlp = config.otlp_endpoint.as_deref();
    let sampling = config.sampling();
    let guard = liblog::builder()
        .endpoint(otlp)
        .sampling(sampling)
        .deployment_environment(if config.production {
            "production"
        } else {
            "development"
        })
        .init();

    let build = standards_core::build_info();
    tracing::info!(
        version = standards_core::VERSION,
        commit = build.as_ref().map(|b| b.git.commit_short_hash.as_str()),
        dirty = build.as_ref().map(|b| b.git.dirty),
        "starting standards-server"
    );
    tracing::info!(
        otlp_export = otlp.is_some(),
        production = config.production,
        ?sampling,
        "telemetry initialised"
    );
    standards_api::request::install_panic_hook();
    guard
}

async fn run(config: Config) -> Result<(), BoxError> {
    let bundle = standards_db::bundle().map_err(|error| {
        tracing::error!(error.type = "bundle", %error, "refusing to boot: embedded content does not parse");
        error
    })?;
    let dirty = standards_core::build_info().is_some_and(|b| b.git.dirty);
    let state = AppState::new(Arc::clone(&bundle), &config, standards_core::VERSION, dirty);
    tracing::info!(
        commit = bundle.commit.as_str(),
        docs = bundle.docs.len(),
        skills = bundle.skills.len(),
        templates = bundle.templates.len(),
        etag = state.etag.is_some(),
        "content loaded"
    );

    let shutdown = CancellationToken::new();
    libsignal::cancel_after_signal(shutdown.clone());

    tracing::info!(bind = %config.bind, "serving");
    Server::new(TcpListener::bind(&config.bind))
        .run_with_graceful_shutdown(
            app(state),
            shutdown.cancelled_owned(),
            // 10s drain so `docker stop` finishes in-flight requests.
            Some(Duration::from_secs(10)),
        )
        .await?;
    Ok(())
}
