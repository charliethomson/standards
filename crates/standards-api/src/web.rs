//! The rendered HTML site (L6, L25):
//!
//! ```text
//! /              index: archetypes with their standards tables, then every doc by group
//! /d/{id..}      a doc (ids have slashes: /d/archetypes/cli-tool)
//! /search?q=     server-rendered results, so search works without JS
//! /static/*      site.css and search.js, compiled in
//! ```
//!
//! The content is immutable for the life of the process, so the index and every doc page
//! are rendered once at startup ([`Site::build`]); only search renders per request. Pages
//! carry the commit `ETag` (caching.rs) and a strict CSP: no inline script, scripts and
//! styles from this origin only. The query string is never logged (request.rs logs the
//! path alone).

mod page;
mod render;

use std::collections::HashMap;
use std::sync::Arc;

use poem::http::StatusCode;
use poem::middleware::SetHeader;
use poem::web::{Data, Html, Path, Query};
use poem::{EndpointExt as _, IntoResponse, Response, Route, get, handler};
use serde::Deserialize;
use standards_core::Archetype;
use standards_engine::Engine;

use crate::caching;
use crate::state::AppState;

use page::{Entry, Section, Shell};

const CSS: &str = include_str!("web/static/site.css");
const JS: &str = include_str!("web/static/search.js");

/// No inline script or style; everything from this origin. `connect-src 'self'` is for
/// search.js calling `/api/v1/search`.
pub const CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self'; \
                       img-src 'self' data:; connect-src 'self'; form-action 'self'; \
                       base-uri 'none'; frame-ancestors 'none'";

/// Results on the search page.
const SEARCH_LIMIT: usize = 30;
/// Longer queries are cut, not refused.
const QUERY_MAX: usize = 200;

/// Every page, rendered.
pub struct Site {
    shell: Shell,
    index: String,
    docs: HashMap<String, String>,
}

impl Site {
    /// Render the index and every doc from the engine's content.
    #[must_use]
    pub fn build(state: &AppState) -> Self {
        let engine = &state.engine;
        let commit = engine.commit().to_owned();
        let shell = Shell {
            asset_version: if commit == standards_db::UNKNOWN_COMMIT {
                state.version.clone()
            } else {
                commit.chars().take(12).collect()
            },
            version: state.version.clone(),
            commit,
        };

        let summaries = engine.docs();
        let titles: HashMap<String, String> = summaries
            .iter()
            .map(|d| (d.id.as_str().to_owned(), d.title.clone()))
            .collect();
        let title_of = |id: &str| titles.get(id).cloned().unwrap_or_else(|| id.to_owned());
        let (sections, archetypes) = group_docs(engine);

        let linker = engine.bundle().linker();
        let pitched: Vec<(String, &Archetype)> = archetypes
            .iter()
            .map(|a| {
                let pitch = engine
                    .doc(a.doc.as_str())
                    .ok()
                    .and_then(|d| render::first_paragraph(&d.markdown))
                    .unwrap_or_default();
                (pitch, *a)
            })
            .collect();
        let index_body = page::index(
            &pitched,
            &title_of,
            &|arch: &Archetype, note: &str| linker.rewrite(&arch.doc.path(), note),
            &sections,
        );
        let index_page = shell.page("Standards", DESCRIPTION, "", &index_body);

        // Every doc, with its group and neighbours in index order.
        let flat: Vec<(&str, &Entry)> = sections
            .iter()
            .flat_map(|s| s.entries.iter().map(move |e| (s.title.as_str(), e)))
            .collect();
        let mut docs = HashMap::new();
        for (i, (group, entry)) in flat.iter().enumerate() {
            let Ok(view) = engine.doc(&entry.id) else {
                continue;
            };
            let near = |j: Option<usize>| {
                j.and_then(|j| flat.get(j)).map(|(_, e)| page::Link {
                    id: &e.id,
                    title: &e.title,
                })
            };
            let body = page::doc(
                &view,
                Some(group),
                near(i.checked_sub(1)),
                near(Some(i + 1)),
            );
            let title = format!("{} · Standards", view.title);
            docs.insert(entry.id.clone(), shell.page(&title, DESCRIPTION, "", &body));
        }

        Self {
            shell,
            index: index_page,
            docs,
        }
    }

    /// A doc page by id, if there is one.
    #[must_use]
    pub fn doc(&self, id: &str) -> Option<&str> {
        self.docs.get(id).map(String::as_str)
    }

    /// Every doc id with a page.
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.docs.keys().map(String::as_str)
    }
}

/// The docs in index order, and the archetypes in that order.
///
/// Both the order and each doc's one-line rule are core's ([`standards_core::outline`]), so
/// this index, `/llms.txt` and `/llms-full.txt` list the docs the same way; the rule is
/// markdown there and inline HTML here.
fn group_docs(engine: &Engine) -> (Vec<Section>, Vec<&Archetype>) {
    let sections: Vec<Section> = standards_core::outline(engine.bundle())
        .into_iter()
        .map(|group| Section {
            title: group.title,
            entries: group
                .entries
                .into_iter()
                .map(|entry| Entry {
                    id: entry.id.as_str().to_owned(),
                    title: entry.title,
                    blurb: entry.summary.as_deref().map(render::inline),
                })
                .collect(),
        })
        .collect();

    // The archetype cards above the doc list follow the same order as their group.
    let order: Vec<&str> = sections
        .iter()
        .find(|s| s.title == standards_core::ARCHETYPES_GROUP)
        .map(|s| s.entries.iter().map(|e| e.id.as_str()).collect())
        .unwrap_or_default();
    let mut archetypes: Vec<&Archetype> = engine.archetypes().iter().collect();
    archetypes.sort_by_key(|a| {
        order
            .iter()
            .position(|id| *id == a.doc.as_str())
            .unwrap_or(usize::MAX)
    });
    (sections, archetypes)
}

const DESCRIPTION: &str = "How this fleet of repos is built.";

#[handler]
fn index(Data(site): Data<&Arc<Site>>) -> Html<String> {
    Html(site.index.clone())
}

#[handler]
fn doc(Data(site): Data<&Arc<Site>>, Path(id): Path<String>) -> Response {
    let id = id.trim_end_matches('/');
    if let Some(page) = site.doc(id) {
        return Html(page.to_owned()).into_response();
    }
    let body = page::not_found(id);
    Html(
        site.shell
            .page("Not found · Standards", DESCRIPTION, "", &body),
    )
    .with_status(StatusCode::NOT_FOUND)
    .into_response()
}

#[derive(Deserialize)]
struct SearchParams {
    q: Option<String>,
}

#[handler]
fn search(
    Data(site): Data<&Arc<Site>>,
    Data(state): Data<&AppState>,
    Query(params): Query<SearchParams>,
) -> Html<String> {
    let q: String = params
        .q
        .unwrap_or_default()
        .chars()
        .take(QUERY_MAX)
        .collect();
    let hits = if q.trim().is_empty() {
        Vec::new()
    } else {
        state.engine.search(&q, SEARCH_LIMIT)
    };
    let title = if q.trim().is_empty() {
        "Search · Standards".to_owned()
    } else {
        format!("{} · Search · Standards", q.trim())
    };
    Html(
        site.shell
            .page(&title, DESCRIPTION, &q, &page::search(&q, &hits)),
    )
}

#[handler]
fn asset(Path(path): Path<String>) -> Response {
    let (body, content_type) = match path.as_str() {
        "site.css" => (CSS, "text/css; charset=utf-8"),
        "search.js" => (JS, "text/javascript; charset=utf-8"),
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    Response::builder().content_type(content_type).body(body)
}

/// The security headers on every page and asset (docs/security.md, Edge headers).
fn headers() -> SetHeader {
    SetHeader::new()
        .overriding("content-security-policy", CSP)
        .overriding("x-content-type-options", "nosniff")
        .overriding("referrer-policy", "no-referrer")
        .overriding("x-frame-options", "DENY")
}

/// Add the HTML routes to `route`.
#[must_use]
pub fn mount(route: Route, state: &AppState) -> Route {
    let site = Arc::new(Site::build(state));
    let cache = || caching::Immutable::new(state.etag.clone());
    route
        .at(
            "/",
            get(index).data(site.clone()).with(cache()).with(headers()),
        )
        .at(
            "/d/*path",
            get(doc).data(site.clone()).with(cache()).with(headers()),
        )
        .at(
            "/search",
            get(search).data(site).with(cache()).with(headers()),
        )
        .at("/static/*path", get(asset).with(cache()).with(headers()))
}

#[cfg(test)]
mod tests;
