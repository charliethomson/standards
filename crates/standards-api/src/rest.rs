//! The REST mirror at `/api/v1` (L6): what the `standards` cli reads. Public, read-only.
//!
//! Parameters that hold a slash (a doc id, a template path, a skill asset path) are one
//! path parameter, sent percent-encoded; raw slashes are accepted too ([`crate::paths`]).

use std::collections::BTreeMap;

use poem::Request;
use poem::web::Data;
use poem_openapi::param::{Path, Query};
use poem_openapi::payload::{Binary, Json};
use poem_openapi::types::{ToJSON, Type};
use poem_openapi::{ApiResponse, OpenApi, OpenApiService, Tags};

use crate::dto::{
    Checklist, Doc, DocSummary, ErrorBody, Manifest, RenderedTemplate, SearchHit, Skill,
    SkillSummary, TemplateSummary,
};
use crate::error;
use crate::state::AppState;

/// Where the service is mounted; the spec's `servers` entry.
pub const API_MOUNT: &str = "/api/v1";

/// The **contract** version: it names the shape clients build against and moves only when
/// the surface does. The build version is on `GET /version`.
pub const API_VERSION: &str = "1.0.0";

/// Default and largest `limit` for `GET /search`.
const SEARCH_DEFAULT: usize = 10;
const SEARCH_MAX: usize = 50;

#[derive(Tags)]
enum Tag {
    /// Rendered standards and archetypes as markdown.
    Docs,
    /// A standard's checklist as it applies to one archetype.
    Checklists,
    /// Files under `templates/`, rendered server-side.
    Templates,
    /// Shared skills and their assets.
    Skills,
    /// Full-text search over docs, sections, checklist items and skills.
    Search,
    /// What the cli syncs against.
    Manifest,
}

/// 200 with a JSON body, or a 404 naming what was not found.
#[derive(ApiResponse)]
pub enum Found<T: ToJSON + Type + Send> {
    #[oai(status = 200)]
    Ok(Json<T>),
    /// No such doc, archetype, template, skill or asset.
    #[oai(status = 404)]
    NotFound(Json<ErrorBody>),
}

impl<T: ToJSON + Type + Send> Found<T> {
    fn from(result: standards_engine::Result<T>) -> Self {
        match result {
            Ok(v) => Self::Ok(Json(v)),
            Err(e) => Self::NotFound(Json(error::body(&e))),
        }
    }
}

/// An asset's bytes, or a 404.
#[derive(ApiResponse)]
pub enum AssetResponse {
    #[oai(status = 200)]
    Ok(Binary<Vec<u8>>),
    /// No such skill or asset.
    #[oai(status = 404)]
    NotFound(Json<ErrorBody>),
}

pub struct Api;

#[OpenApi]
impl Api {
    /// List docs
    ///
    /// Every doc under `docs/`, archetypes included, with whether it has a checklist.
    #[oai(
        path = "/docs",
        method = "get",
        operation_id = "listDocs",
        tag = "Tag::Docs"
    )]
    async fn list_docs(&self, state: Data<&AppState>) -> Json<Vec<DocSummary>> {
        Json(state.engine.docs().into_iter().map(Into::into).collect())
    }

    /// Get a doc
    ///
    /// The doc's markdown with links made absolute, and its outline. `id` is the path under
    /// `docs/` without `.md`; send a slash in it percent-encoded
    /// (`archetypes%2Fcli-tool`) or raw.
    #[oai(
        path = "/docs/:id",
        method = "get",
        operation_id = "getDoc",
        tag = "Tag::Docs"
    )]
    async fn get_doc(&self, state: Data<&AppState>, id: Path<String>) -> Found<Doc> {
        Found::from(state.engine.doc(&id).map(Into::into))
    }

    /// Get a checklist
    ///
    /// The standard's checklist plus the archetype's own, with how the archetype says the
    /// standard applies. A standard marked N/A is a 200 with `applicability:
    /// not_applicable`, the reason in `note`, and no items.
    #[oai(
        path = "/checklist",
        method = "get",
        operation_id = "getChecklist",
        tag = "Tag::Checklists"
    )]
    async fn get_checklist(
        &self,
        state: Data<&AppState>,
        /// The standard's doc id (`versioning`).
        standard: Query<String>,
        /// The archetype's name (`cli-tool`).
        archetype: Query<String>,
    ) -> Found<Checklist> {
        Found::from(
            state
                .engine
                .checklist(&standard, &archetype)
                .map(Into::into),
        )
    }

    /// List templates
    #[oai(
        path = "/templates",
        method = "get",
        operation_id = "listTemplates",
        tag = "Tag::Templates"
    )]
    async fn list_templates(&self, state: Data<&AppState>) -> Json<Vec<TemplateSummary>> {
        Json(
            state
                .engine
                .templates()
                .into_iter()
                .map(Into::into)
                .collect(),
        )
    }

    /// Render a template
    ///
    /// Every query parameter `KEY=VALUE` fills `{{KEY}}`; placeholders with no value stay
    /// in the body and are listed in `unfilled`. `path` is relative to `templates/`
    /// (`rust%2Fbuild.rs`, or with a raw slash). The raw file is at `/t/{path}`.
    #[oai(
        path = "/templates/:path",
        method = "get",
        operation_id = "renderTemplate",
        tag = "Tag::Templates"
    )]
    async fn render_template(
        &self,
        state: Data<&AppState>,
        req: &Request,
        path: Path<String>,
    ) -> Found<RenderedTemplate> {
        let vars: BTreeMap<String, String> = req.params().unwrap_or_default();
        Found::from(
            state
                .engine
                .render_template(&path, &vars)
                .map(|r| RenderedTemplate {
                    path: path.0.clone(),
                    body: r.body,
                    unfilled: r.unfilled.into_iter().collect(),
                }),
        )
    }

    /// List skills
    #[oai(
        path = "/skills",
        method = "get",
        operation_id = "listSkills",
        tag = "Tag::Skills"
    )]
    async fn list_skills(&self, state: Data<&AppState>) -> Json<Vec<SkillSummary>> {
        Json(state.engine.skills().into_iter().map(Into::into).collect())
    }

    /// Get a skill
    ///
    /// The `SKILL.md` body as written, its assets with their hashes, and the two hashes the
    /// manifest carries.
    #[oai(
        path = "/skills/:name",
        method = "get",
        operation_id = "getSkill",
        tag = "Tag::Skills"
    )]
    async fn get_skill(&self, state: Data<&AppState>, name: Path<String>) -> Found<Skill> {
        Found::from(state.engine.skill(&name).map(Into::into))
    }

    /// Get a skill asset
    ///
    /// The file's bytes. `path` is relative to the skill directory with nesting kept
    /// (`scripts%2Fobsq.sh`, or with a raw slash).
    #[oai(
        path = "/skills/:name/assets/:path",
        method = "get",
        operation_id = "getSkillAsset",
        tag = "Tag::Skills"
    )]
    async fn get_skill_asset(
        &self,
        state: Data<&AppState>,
        name: Path<String>,
        path: Path<String>,
    ) -> AssetResponse {
        match state.engine.skill_asset(&name, &path) {
            Ok(asset) => AssetResponse::Ok(Binary(asset.bytes.clone())),
            Err(e) => AssetResponse::NotFound(Json(error::body(&e))),
        }
    }

    /// Search
    ///
    /// The best hits for `q` over doc titles and intros, sections, checklist items and
    /// skills, best first.
    #[oai(
        path = "/search",
        method = "get",
        operation_id = "search",
        tag = "Tag::Search"
    )]
    async fn search(
        &self,
        state: Data<&AppState>,
        /// The query.
        q: Query<String>,
        /// How many hits, at most 50.
        #[oai(validator(minimum(value = "1"), maximum(value = "50")))]
        limit: Query<Option<usize>>,
    ) -> Json<Vec<SearchHit>> {
        let limit = limit.0.unwrap_or(SEARCH_DEFAULT).min(SEARCH_MAX);
        Json(
            state
                .engine
                .search(&q, limit)
                .into_iter()
                .map(Into::into)
                .collect(),
        )
    }

    /// Get the manifest
    ///
    /// The same document as `/manifest.json`: commit, versions, archetypes, and each skill
    /// with its description and content hashes.
    #[oai(
        path = "/manifest",
        method = "get",
        operation_id = "getManifest",
        tag = "Tag::Manifest"
    )]
    async fn get_manifest(&self, state: Data<&AppState>) -> Json<Manifest> {
        Json(state.manifest.as_ref().clone().into())
    }
}

/// The service mounted at [`API_MOUNT`].
#[must_use]
pub fn service() -> OpenApiService<Api, ()> {
    OpenApiService::new(Api, "standards", API_VERSION)
        .description(
            "Fleet engineering standards, read-only. Content is fixed per commit; \
             responses carry `ETag: \"<commit>\"`.",
        )
        .server(API_MOUNT)
}

/// The document `GET /openapi.json` serves and `standards-server openapi` prints.
#[must_use]
pub fn spec() -> String {
    service().spec()
}
