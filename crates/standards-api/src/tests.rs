//! The mount map over the real embedded content, through poem's `TestClient`.

use std::sync::Arc;

use poem::Endpoint;
use poem::http::StatusCode;
use poem::test::{TestClient, TestResponse};
use serde_json::Value;

use super::*;

const COMMIT_ETAG: &str = "\"testcommit\"";

/// The real bundle, with a fixed tag so the caching path is exercised whatever the tree.
fn state() -> AppState {
    let bundle = standards_db::bundle().expect("embedded content parses");
    let mut state = AppState::new(bundle, &Config::default(), "0.1.42", false);
    state.etag = Some(COMMIT_ETAG.to_owned());
    state
}

fn client() -> (TestClient<impl Endpoint>, AppState) {
    let state = state();
    (TestClient::new(app(state.clone())), state)
}

async fn json(resp: TestResponse) -> Value {
    resp.0.into_body().into_json().await.expect("json body")
}

async fn text(resp: TestResponse) -> String {
    resp.0.into_body().into_string().await.expect("body")
}

#[tokio::test]
async fn manifest_lists_every_skill_with_both_hashes() {
    let (c, state) = client();
    let resp = c.get("/manifest.json").send().await;
    resp.assert_status_is_ok();
    resp.assert_header("etag", COMMIT_ETAG);
    resp.assert_header("cache-control", caching::CACHE_CONTROL_VALUE);
    let m = json(resp).await;

    let skills = m["skills"].as_array().expect("skills");
    assert_eq!(skills.len(), state.engine.bundle().skills.len());
    assert_ne!(skills.len(), 0);
    let hex64 = |v: &Value| {
        v.as_str()
            .is_some_and(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
    };
    for s in skills {
        assert!(hex64(&s["description_sha256"]), "{s}");
        assert!(hex64(&s["content_sha256"]), "{s}");
    }
    assert_eq!(m["server_version"], "0.1.42");
    assert_eq!(m["min_cli_version"], standards_core::MIN_CLI_VERSION);
    assert_eq!(m["cli_install_url"], standards_core::CLI_INSTALL_URL);
    assert_eq!(m["commit"], state.engine.commit());
    assert!(
        m["archetypes"]
            .as_array()
            .is_some_and(|a| a.iter().any(|x| x == "cli-tool"))
    );

    // The contract's copy is the same document.
    let api = json(c.get("/api/v1/manifest").send().await).await;
    assert_eq!(api, m);
    // And it parses as the core type the cli shares.
    let _: standards_core::Manifest = serde_json::from_value(m).expect("core manifest");
}

#[tokio::test]
async fn docs_list_and_a_slash_id_both_ways() {
    let (c, _) = client();
    let list = json(c.get("/api/v1/docs").send().await).await;
    assert!(
        list.as_array()
            .unwrap()
            .iter()
            .any(|d| d["id"] == "archetypes/cli-tool")
    );

    for path in [
        "/api/v1/docs/archetypes/cli-tool",
        "/api/v1/docs/archetypes%2Fcli-tool",
    ] {
        let resp = c.get(path).send().await;
        resp.assert_status_is_ok();
        let doc = json(resp).await;
        assert_eq!(doc["id"], "archetypes/cli-tool", "{path}");
        assert!(doc["markdown"].as_str().unwrap().starts_with("# "));
        assert_ne!(doc["sections"], serde_json::json!([]));
    }

    let doc = json(c.get("/api/v1/docs/versioning").send().await).await;
    assert_eq!(doc["id"], "versioning");
    // Links are absolute (L25): no relative `.md` link survives.
    assert!(
        !doc["markdown"]
            .as_str()
            .unwrap()
            .contains("](versioning.md")
    );
}

#[tokio::test]
async fn checklist_applies_and_not_applicable() {
    let (c, state) = client();
    let resp = c
        .get("/api/v1/checklist")
        .query("standard", &"versioning")
        .query("archetype", &"cli-tool")
        .send()
        .await;
    resp.assert_status_is_ok();
    let cl = json(resp).await;
    assert_eq!(cl["standard"], "versioning");
    assert_eq!(cl["archetype"], "cli-tool");
    assert_ne!(cl["standard_items"], serde_json::json!([]));

    // Find a real N/A row in the content rather than hard-coding one.
    let (standard, archetype) = state
        .engine
        .archetypes()
        .iter()
        .find_map(|a| {
            a.rows.iter().find_map(|r| {
                (r.applicability == standards_core::Applicability::NotApplicable)
                    .then_some(())
                    .and_then(|()| {
                        r.standards.iter().find_map(|s| match s {
                            standards_core::StandardRef::Doc(id) => {
                                Some((id.to_string(), a.name.clone()))
                            }
                            standards_core::StandardRef::Named(_) => None,
                        })
                    })
            })
        })
        .expect("some archetype marks a standard N/A");
    let cl = json(
        c.get("/api/v1/checklist")
            .query("standard", &standard)
            .query("archetype", &archetype)
            .send()
            .await,
    )
    .await;
    assert_eq!(cl["applicability"], "not_applicable", "{cl}");
    assert!(cl["note"].as_str().is_some_and(|n| !n.is_empty()));
    assert_eq!(cl["standard_items"], serde_json::json!([]));
    assert_eq!(cl["archetype_items"], serde_json::json!([]));
}

#[tokio::test]
async fn templates_list_render_and_raw() {
    let (c, state) = client();
    let list = json(c.get("/api/v1/templates").send().await).await;
    assert!(
        list.as_array()
            .unwrap()
            .iter()
            .any(|t| t["path"] == "rust/config.rs")
    );

    // rust/config.rs has {{PRODUCT}} and {{PRODUCT_UPPER}}: fill one, leave one.
    let resp = c
        .get("/api/v1/templates/rust/config.rs")
        .query("PRODUCT", &"demo")
        .send()
        .await;
    resp.assert_status_is_ok();
    let r = json(resp).await;
    assert_eq!(r["path"], "rust/config.rs");
    let body = r["body"].as_str().unwrap();
    assert!(body.contains("demo") && !body.contains("{{PRODUCT}}"));
    assert_eq!(r["unfilled"], serde_json::json!(["PRODUCT_UPPER"]));

    let raw = c.get("/t/rust/config.rs").send().await;
    raw.assert_status_is_ok();
    raw.assert_content_type("text/plain; charset=utf-8");
    raw.assert_header("etag", COMMIT_ETAG);
    assert_eq!(
        text(raw).await,
        state.engine.template("rust/config.rs").unwrap().body
    );
}

#[tokio::test]
async fn skills_and_a_nested_asset() {
    let (c, state) = client();
    let list = json(c.get("/api/v1/skills").send().await).await;
    let logs = list
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == "thmsn-logs")
        .expect("thmsn-logs listed");
    assert!(
        logs["assets"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a == "scripts/obsq.sh")
    );

    let skill = json(c.get("/api/v1/skills/thmsn-logs").send().await).await;
    let core = state.engine.skill("thmsn-logs").unwrap();
    assert_eq!(skill["content_sha256"], core.content_sha256.as_str());
    assert_eq!(
        skill["description_sha256"],
        core.description_sha256.as_str()
    );
    assert_eq!(skill["body"], core.body.as_str());

    let want = &state
        .engine
        .skill_asset("thmsn-logs", "scripts/obsq.sh")
        .unwrap()
        .bytes;
    for path in [
        "/api/v1/skills/thmsn-logs/assets/scripts/obsq.sh",
        "/api/v1/skills/thmsn-logs/assets/scripts%2Fobsq.sh",
    ] {
        let resp = c.get(path).send().await;
        resp.assert_status_is_ok();
        resp.assert_content_type("application/octet-stream");
        let got = resp.0.into_body().into_vec().await.unwrap();
        assert_eq!(&got, want, "{path}");
    }
}

#[tokio::test]
async fn search_finds_and_validates_limit() {
    let (c, _) = client();
    let resp = c
        .get("/api/v1/search")
        .query("q", &"versioning")
        .query("limit", &3)
        .send()
        .await;
    resp.assert_status_is_ok();
    let hits = json(resp).await;
    let hits = hits.as_array().unwrap();
    assert!(!hits.is_empty() && hits.len() <= 3);
    assert_eq!(hits[0]["doc"], "versioning");
    assert_eq!(hits[0]["kind"], "doc");

    let skill_hit = json(
        c.get("/api/v1/search")
            .query("q", &"thmsn-logs")
            .send()
            .await,
    )
    .await;
    assert!(
        skill_hit
            .as_array()
            .unwrap()
            .iter()
            .any(|h| h["kind"] == "skill")
    );

    for (q, status) in [
        ("limit=0&q=x", 400),
        ("limit=51&q=x", 400),
        ("limit=1", 400),
    ] {
        let resp = c.get(format!("/api/v1/search?{q}")).send().await;
        resp.assert_status(StatusCode::from_u16(status).unwrap());
        let body = json(resp).await;
        assert_eq!(body["$type"], "http.400", "{q}: {body}");
    }
}

#[tokio::test]
async fn misses_are_keyed_json_404s() {
    let (c, _) = client();
    for (path, key) in [
        ("/api/v1/docs/nope", "unknown_doc"),
        ("/api/v1/docs/archetypes/nope", "unknown_doc"),
        (
            "/api/v1/checklist?standard=nope&archetype=cli-tool",
            "unknown_doc",
        ),
        (
            "/api/v1/checklist?standard=versioning&archetype=nope",
            "unknown_archetype",
        ),
        ("/api/v1/templates/nope.rs", "unknown_template"),
        ("/api/v1/templates/..%2F..%2FCargo.toml", "unknown_template"),
        ("/api/v1/skills/nope", "unknown_skill"),
        ("/api/v1/skills/nope/assets/x", "unknown_skill"),
        (
            "/api/v1/skills/thmsn-logs/assets/nope/x.sh",
            "unknown_asset",
        ),
        (
            "/api/v1/skills/thmsn-logs/assets/..%2F..%2FREADME.md",
            "unknown_asset",
        ),
        ("/t/nope.rs", "unknown_template"),
    ] {
        let resp = c.get(path).send().await;
        resp.assert_status(StatusCode::NOT_FOUND);
        assert!(
            resp.0.headers().get("etag").is_none(),
            "{path}: a miss is not cached"
        );
        let body = json(resp).await;
        assert_eq!(
            body["$type"],
            format!("dev.thmsn.standards.server.content.error.{key}"),
            "{path}: {body}"
        );
        assert!(body["message"].as_str().is_some_and(|m| !m.is_empty()));
    }

    let resp = c.get("/nothing-here").send().await;
    resp.assert_status(StatusCode::NOT_FOUND);
    assert_eq!(json(resp).await["$type"], "http.404");
}

#[tokio::test]
async fn revalidation_answers_304() {
    let (c, _) = client();
    let resp = c
        .get("/api/v1/docs/versioning")
        .header("if-none-match", COMMIT_ETAG)
        .send()
        .await;
    resp.assert_status(StatusCode::NOT_MODIFIED);
    resp.assert_header("etag", COMMIT_ETAG);
    assert_eq!(text(resp).await, "");

    let resp = c
        .get("/api/v1/docs/versioning")
        .header("if-none-match", "\"other\"")
        .send()
        .await;
    resp.assert_status_is_ok();
}

#[tokio::test]
async fn untagged_builds_send_no_cache_headers() {
    let mut state = state();
    state.etag = None;
    let c = TestClient::new(app(state));
    let resp = c
        .get("/manifest.json")
        .header("if-none-match", COMMIT_ETAG)
        .send()
        .await;
    resp.assert_status_is_ok();
    assert!(resp.0.headers().get("etag").is_none());
    assert!(resp.0.headers().get("cache-control").is_none());
}

#[tokio::test]
async fn standard_mounts() {
    let (c, state) = client();

    let resp = c.get("/install").send().await;
    resp.assert_status(StatusCode::FOUND);
    resp.assert_header("location", INSTALL_REDIRECT);

    let resp = c.get("/health").send().await;
    resp.assert_status_is_ok();
    assert_eq!(json(resp).await["status"], "ok");

    let v = json(c.get("/version").send().await).await;
    assert_eq!(v["version"], "0.1.42");
    assert_eq!(v["commit"], state.engine.commit());
    assert!(v.get("build_info").is_some());
    if let Some(bi) = standards_core::build_info() {
        assert_eq!(v["build_info"]["git"]["commitHash"], bi.git.commit_hash);
    }

    let spec = c.get("/openapi.json").send().await;
    spec.assert_status_is_ok();
    let spec = json(spec).await;
    assert_eq!(spec["servers"][0]["url"], "/api/v1");

    c.get("/api/docs").send().await.assert_status_is_ok();
}

#[tokio::test]
async fn metrics_count_requests_by_contract_route() {
    let (c, _) = client();
    c.get("/api/v1/docs/archetypes/cli-tool")
        .send()
        .await
        .assert_status_is_ok();
    c.get("/api/v1/skills/nope")
        .send()
        .await
        .assert_status(StatusCode::NOT_FOUND);
    c.get("/nothing-here")
        .send()
        .await
        .assert_status(StatusCode::NOT_FOUND);

    let resp = c.get("/api/metrics").send().await;
    resp.assert_status_is_ok();
    resp.assert_content_type(metrics::OPENMETRICS);
    let body = text(resp).await;
    for needle in [
        "standards_http_requests_total{method=\"GET\",route=\"/api/v1/docs/{id}\",status=\"200\"} 1",
        "standards_http_requests_total{method=\"GET\",route=\"/api/v1/skills/{name}\",status=\"404\"} 1",
        "standards_http_requests_total{method=\"GET\",route=\"unmatched\",status=\"404\"} 1",
        "standards_build_info{version=\"0.1.42\"",
    ] {
        assert!(body.contains(needle), "missing {needle:?} in:\n{body}");
    }
}

#[tokio::test]
async fn a_panicking_handler_is_a_json_500() {
    #[poem::handler]
    #[allow(clippy::panic)] // the point of the test
    fn boom() -> &'static str {
        panic!("boom")
    }
    let state = state();
    let metrics = state.metrics.clone();
    let ep = Route::new()
        .at("/boom", get(boom))
        .map_to_response()
        .with(poem::middleware::CatchPanic::new().with_handler(request::panic_response))
        .around(move |ep, req| {
            request::serve(
                ep,
                req,
                metrics.clone(),
                Arc::new(request::RouteTemplates::new()),
            )
        });
    let resp = TestClient::new(ep).get("/boom").send().await;
    resp.assert_status(StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(json(resp).await["$type"], "panic");
}

#[test]
fn the_app_uses_the_configured_allowed_hosts() {
    let config = Config {
        allowed_hosts: "a.example b.example".into(),
        ..Config::default()
    };
    let state = AppState::new(standards_db::bundle().unwrap(), &config, "0.0.1", true);
    assert_eq!(*state.allowed_hosts, ["a.example", "b.example"]);
    assert_eq!(state.etag, None, "a dirty build is not tagged");
}

/// The committed `api/openapi.json` is what the server emits (docs/contracts.md drift
/// check). Regenerate with
/// `cargo run -q -p standards-api --bin standards-server -- openapi > api/openapi.json`.
#[test]
fn the_committed_contract_is_what_the_server_emits() {
    let committed = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../api/openapi.json"
    ))
    .expect("api/openapi.json exists");
    assert_eq!(
        committed.trim_end(),
        rest::spec().trim_end(),
        "api/openapi.json is stale: regenerate it"
    );
}
