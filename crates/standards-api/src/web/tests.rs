//! Smoke test for the site over the real content: every page answers and every doc link
//! it renders resolves. The markup itself is not tested (docs/testing.md, L24).

use std::collections::BTreeSet;

use poem::http::StatusCode;
use poem::test::TestClient;

use super::render::local_href;
use crate::{AppState, Config, app};

fn state() -> AppState {
    let bundle = standards_db::bundle().expect("embedded content parses");
    AppState::new(bundle, &Config::default(), "0.1.42", false)
}

/// The doc id a site-local href names (`/d/archetypes/cli-tool#x` → `archetypes/cli-tool`).
fn doc_id_of(href: &str) -> Option<&str> {
    let rest = local_href(href).strip_prefix("/d/")?;
    let id = rest.split(['#', '?']).next().unwrap_or(rest);
    (!id.is_empty()).then_some(id)
}

/// Every `href="/d/..."` in `html`, as doc ids.
fn linked(html: &str) -> BTreeSet<String> {
    html.split("href=\"")
        .skip(1)
        .filter_map(|s| s.split('"').next())
        .filter_map(doc_id_of)
        .map(str::to_owned)
        .collect()
}

#[tokio::test]
async fn every_doc_renders_and_every_link_resolves() {
    let state = state();
    let ids: BTreeSet<String> = state
        .engine
        .docs()
        .iter()
        .map(|d| d.id.as_str().to_owned())
        .collect();
    let c = TestClient::new(app(state));

    let index = c.get("/").send().await;
    index.assert_status_is_ok();
    index.assert_header("content-security-policy", super::CSP);
    let index = index.0.into_body().into_string().await.unwrap();
    let mut broken: Vec<String> = Vec::new();
    let from_index = linked(&index);
    assert_eq!(
        from_index.intersection(&ids).count(),
        ids.len(),
        "the index links every doc"
    );

    for id in &ids {
        let resp = c.get(format!("/d/{id}")).send().await;
        assert_eq!(resp.0.status(), StatusCode::OK, "/d/{id}");
        let html = resp.0.into_body().into_string().await.unwrap();
        assert!(
            !html.contains("href=\"https://standards.thmsn.dev/"),
            "{id} keeps an absolute link"
        );
        broken.extend(
            linked(&html)
                .into_iter()
                .filter(|l| !ids.contains(l))
                .map(|l| format!("{id} -> {l}")),
        );
    }
    broken.extend(from_index.difference(&ids).map(|l| format!("index -> {l}")));
    assert!(broken.is_empty(), "broken links: {broken:?}");

    let missing = c.get("/d/no-such-doc").send().await;
    missing.assert_status(StatusCode::NOT_FOUND);
    missing.assert_content_type("text/html; charset=utf-8");

    for path in [
        "/static/site.css",
        "/static/search.js",
        "/search?q=versioning",
        "/search",
    ] {
        c.get(path).send().await.assert_status_is_ok();
    }
    c.get("/static/nope.js")
        .send()
        .await
        .assert_status(StatusCode::NOT_FOUND);
}
