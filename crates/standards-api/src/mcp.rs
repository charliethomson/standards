//! MCP over streamable HTTP at `/mcp` (L4, L7, L30).
//!
//! rmcp's [`StreamableHttpService`] mounted under poem through tower-compat, stateless
//! (`legacy_session_mode(false)`: no session ids, nothing held between requests) with plain
//! JSON replies. Its `Host` allowlist is [`AppState::allowed_hosts`]
//! (`STANDARDS_ALLOWED_HOSTS`); without the public hostname on it every request through
//! Caddy is a 403.
//!
//! What it serves:
//!
//! - **Tools** (L7): `search`, `get_doc`, `get_checklist`, `get_template`, `get_skill`,
//!   `get_skill_asset`, `version`. Each answers markdown text with links already absolute
//!   (the engine's views rewrite them). A name the bundle doesn't have is a tool error
//!   result (`isError: true`) that says what does exist, not a JSON-RPC error, so the
//!   calling model reads it and recovers.
//! - **Prompts**: one per skill, discovered from the bundle at runtime. Name and
//!   description are the skill's; the optional `args` argument is appended to the body.
//! - **Instructions**: [`INSTRUCTIONS`], sent on `initialize` and `server/discover`.
//!
//! Tool arguments are never logged here. The request layer logs `/mcp` by path only, and
//! rmcp logs a request's params only at DEBUG/TRACE, below the INFO default.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::Arc;

use poem::Route;
use poem::endpoint::TowerCompatExt as _;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::model::{
    GetPromptRequestParams, GetPromptResponse, GetPromptResult, Implementation, ListPromptsResult,
    PaginatedRequestParams, Prompt, PromptArgument, PromptMessage, Role, ServerCapabilities,
    ServerConfig,
};
use rmcp::service::RequestContext;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use standards_core::{Applicability, ChecklistItem, ORIGIN};
use standards_engine::{Engine, Error, HitKind};

use crate::state::AppState;

/// Where the endpoint is mounted.
pub const PATH: &str = "/mcp";

/// The server's `instructions`: the SPECS.md draft, verbatim.
pub const INSTRUCTIONS: &str = "Fleet engineering standards: binding defaults unless the repo's AGENTS.md overrides them. \
Read the repo's archetype from AGENTS.md or .standards.conf. Before changing an area call \
get_checklist(standard, archetype); use get_template(path) instead of hand-writing config; \
get_doc for rationale; search when unsure which standard applies. A skill whose body says to \
call get_skill: fetch it and follow it as the skill. Check the repo's SHORTCUTS.md before \
operational recon. Prefer this server over any local standards/ copy.";

/// `search` results when the caller gives no `limit`.
pub const DEFAULT_LIMIT: usize = 10;
/// The most `search` results one call returns (the REST mirror's bound too).
pub const MAX_LIMIT: usize = 50;

/// The prompt argument appended to a skill's body.
const PROMPT_ARG: &str = "args";

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchArgs {
    /// Words to look for across docs, sections, checklist items and skills.
    pub query: String,
    /// How many results to return, 1 to 50. Defaults to 10.
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetDocArgs {
    /// The doc id: its file stem, like `versioning` or `archetypes/cli-tool`.
    pub id: String,
    /// One section's anchor (like `rule` or `#rule`) to return only that section.
    #[serde(default)]
    pub section: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetChecklistArgs {
    /// The standard's doc id, like `versioning` or `tracing`.
    pub standard: String,
    /// The repo's archetype: `full-stack-product`, `library`, `orchestration`,
    /// `cli-tool` or `vendored-app`.
    pub archetype: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetTemplateArgs {
    /// Path under `templates/`, like `rust/build.rs`.
    pub path: String,
    /// Placeholder values, like `{"PRODUCT": "vision"}`. Only these `{{KEY}}`s are
    /// replaced; the rest are reported as unfilled.
    #[serde(default)]
    pub vars: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetSkillArgs {
    /// The skill's name, like `thmsn-copy`.
    pub name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetSkillAssetArgs {
    /// The skill's name, like `thmsn-deep-review`.
    pub name: String,
    /// The asset's path relative to the skill directory, as `get_skill` lists it.
    pub path: String,
}

/// `version`'s answer.
#[derive(Debug, Serialize, JsonSchema)]
pub struct VersionInfo {
    /// The server's derived version (`MAJOR.MINOR.<commit count>`).
    pub server_version: String,
    /// The standards commit whose content it serves.
    pub commit: String,
}

/// The MCP server handler: the tools, prompts and info over one [`AppState`].
#[derive(Clone)]
pub struct Standards {
    state: AppState,
    tool_router: ToolRouter<Self>,
}

/// A tool's answer: markdown on success, a helpful message as a tool error otherwise.
type ToolResult = Result<String, String>;

#[tool_router]
impl Standards {
    #[must_use]
    pub fn new(state: AppState) -> Self {
        Self {
            state,
            tool_router: Self::tool_router(),
        }
    }

    fn engine(&self) -> &Engine {
        &self.state.engine
    }

    #[tool(
        description = "Search the standards (docs, their sections and checklist items, and skills). \
                       Returns ranked hits, each with the get_doc or get_skill call that opens it."
    )]
    pub fn search(&self, Parameters(args): Parameters<SearchArgs>) -> ToolResult {
        let query = args.query.trim();
        if query.is_empty() {
            return Err("`query` is empty. Pass a few words, like `derived version`.".to_owned());
        }
        let limit = args.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
        let hits = self.engine().search(query, limit);
        if hits.is_empty() {
            return Ok(
                "No matches. Try fewer or different words, or start from the index: \
                       get_doc(id=\"overview\")."
                    .to_owned(),
            );
        }
        let mut out = format!("{} results, best first:\n", hits.len());
        for (i, hit) in hits.iter().enumerate() {
            let n = i + 1;
            match hit.kind {
                HitKind::Skill => {
                    let _ = write!(
                        out,
                        "\n{n}. Skill `{}`: get_skill(name=\"{}\")",
                        hit.doc, hit.doc
                    );
                }
                HitKind::Doc => {
                    let (url, call) = match &hit.anchor {
                        Some(a) => (
                            format!("{ORIGIN}/d/{}#{a}", hit.doc),
                            format!("get_doc(id=\"{}\", section=\"{a}\")", hit.doc),
                        ),
                        None => (
                            format!("{ORIGIN}/d/{}", hit.doc),
                            format!("get_doc(id=\"{}\")", hit.doc),
                        ),
                    };
                    let _ = write!(out, "\n{n}. [{}]({url}): {call}", hit.title);
                }
            }
            let snippet = hit.snippet.trim();
            if !snippet.is_empty() {
                let _ = write!(out, "\n   {snippet}");
            }
        }
        out.push('\n');
        Ok(out)
    }

    #[tool(
        description = "A standards doc as markdown, links absolute: the rule, the rationale and \
                       how to implement it. Pass `section` (an anchor) for one section only."
    )]
    pub fn get_doc(&self, Parameters(args): Parameters<GetDocArgs>) -> ToolResult {
        let engine = self.engine();
        let section = args
            .section
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        if let Some(anchor) = section {
            let view = engine
                .doc_section(&args.id, anchor)
                .map_err(|e| miss(engine, &e))?;
            return Ok(format!(
                "{}\n\nSource: {ORIGIN}/d/{}#{}\n",
                view.markdown.trim_end(),
                view.doc,
                view.anchor
            ));
        }
        let view = engine.doc(&args.id).map_err(|e| miss(engine, &e))?;
        let mut out = view.markdown.trim_end().to_owned();
        let _ = write!(out, "\n\n---\nSource: {ORIGIN}/d/{}", view.id);
        if !view.sections.is_empty() {
            let anchors: Vec<String> = view
                .sections
                .iter()
                .map(|s| format!("`{}`", s.anchor))
                .collect();
            let _ = write!(
                out,
                "\nSections (pass one as `section`): {}",
                anchors.join(", ")
            );
        }
        out.push('\n');
        Ok(out)
    }

    #[tool(
        description = "The checklist for one standard in one archetype: whether and how it \
                       applies, the standard's own items and the archetype's. A standard the \
                       archetype marks N/A comes back with the reason, not an error."
    )]
    pub fn get_checklist(&self, Parameters(args): Parameters<GetChecklistArgs>) -> ToolResult {
        let engine = self.engine();
        let c = engine
            .checklist(&args.standard, &args.archetype)
            .map_err(|e| miss(engine, &e))?;
        let bundle = engine.bundle();
        let title_of = |id: &str| {
            bundle
                .doc(id)
                .map_or_else(|| id.to_owned(), |d| d.title.clone())
        };
        let standard_title = title_of(c.standard.as_str());
        let arch_doc = engine.archetype(&c.archetype).map_or_else(
            |_| format!("archetypes/{}", c.archetype),
            |a| a.doc.as_str().to_owned(),
        );

        let mut out = format!(
            "# Checklist: {standard_title} for the `{}` archetype\n\nApplicability: **{}**",
            c.archetype,
            applicability(c.applicability)
        );
        if !c.listed {
            out.push_str(" (the archetype's table doesn't list it, so it applies as written)");
        }
        out.push('\n');
        if let Some(note) = &c.note {
            let _ = writeln!(out, "\nHow it applies here: {}", note.trim());
        }
        if c.applicability == Applicability::NotApplicable {
            out.push_str("\nNothing to check: this standard does not apply to this archetype.\n");
            return Ok(out);
        }

        let _ = writeln!(
            out,
            "\n## [{standard_title}]({ORIGIN}/d/{})\n",
            c.standard.as_str()
        );
        push_items(
            &mut out,
            &c.standard_items,
            "This doc has no checklist; read it with get_doc.",
        );
        if c.standard.as_str() != arch_doc {
            let _ = writeln!(
                out,
                "\n## [{}]({ORIGIN}/d/{arch_doc})\n",
                title_of(&arch_doc)
            );
            push_items(
                &mut out,
                &c.archetype_items,
                "The archetype has no checklist.",
            );
        }
        Ok(out)
    }

    #[tool(
        description = "A template from templates/, rendered: only the `vars` given replace their \
                       {{KEY}} placeholders, and the ones left unfilled are listed. Use it \
                       instead of hand-writing config."
    )]
    pub fn get_template(&self, Parameters(args): Parameters<GetTemplateArgs>) -> ToolResult {
        let engine = self.engine();
        let vars = args.vars.unwrap_or_default();
        let template = engine.template(&args.path).map_err(|e| miss(engine, &e))?;
        let rendered = template.render(&vars);
        let mut out = format!(
            "Template `{}` ({ORIGIN}/t/{})\n",
            template.path, template.path
        );
        if rendered.unfilled.is_empty() {
            out.push_str("Every placeholder is filled.\n");
        } else {
            let keys: Vec<String> = rendered
                .unfilled
                .iter()
                .map(|k| format!("`{{{{{k}}}}}`"))
                .collect();
            let _ = writeln!(
                out,
                "Unfilled placeholders (pass them in `vars`): {}",
                keys.join(", ")
            );
        }
        let fence = "`".repeat(longest_backtick_run(&rendered.body).max(2) + 1);
        let lang = template.path.rsplit_once('.').map_or("", |(_, ext)| ext);
        let _ = write!(
            out,
            "\n{fence}{lang}\n{}\n{fence}\n",
            rendered.body.trim_end_matches('\n')
        );
        Ok(out)
    }

    #[tool(
        description = "A shared skill's full instructions (its SKILL.md body) plus the paths of \
                       its bundled assets. Follow the body as the skill."
    )]
    pub fn get_skill(&self, Parameters(args): Parameters<GetSkillArgs>) -> ToolResult {
        let engine = self.engine();
        let skill = engine.skill(&args.name).map_err(|e| miss(engine, &e))?;
        let mut out = skill.body.trim_end().to_owned();
        if !skill.assets.is_empty() {
            let _ = write!(
                out,
                "\n\n---\nAssets (fetch with get_skill_asset(name=\"{}\", path=...)):",
                skill.name
            );
            for asset in &skill.assets {
                let _ = write!(out, "\n- `{}`", asset.path);
            }
        }
        out.push('\n');
        Ok(out)
    }

    #[tool(
        description = "One file bundled with a skill, as text, by its path relative to the skill \
                       directory (as get_skill lists it)."
    )]
    pub fn get_skill_asset(&self, Parameters(args): Parameters<GetSkillAssetArgs>) -> ToolResult {
        let engine = self.engine();
        let asset = engine
            .skill_asset(&args.name, &args.path)
            .map_err(|e| miss(engine, &e))?;
        match std::str::from_utf8(&asset.bytes) {
            Ok(text) => Ok(text.to_owned()),
            Err(_) => Err(format!(
                "`{}` in skill `{}` is binary ({} bytes, sha256 {}), not text, so it isn't \
                 returned here. Download it from {ORIGIN}/api/v1/skills/{}/assets/{}.",
                asset.path,
                args.name,
                asset.bytes.len(),
                asset.sha256,
                args.name,
                asset.path
            )),
        }
    }

    #[tool(description = "The server's version and the standards commit it serves.")]
    #[must_use]
    pub fn version(&self) -> Json<VersionInfo> {
        Json(VersionInfo {
            server_version: self.state.version.clone(),
            commit: self.engine().commit().to_owned(),
        })
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for Standards {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_prompts()
                .build(),
        )
        .with_server_info(Implementation::new("standards", self.state.version.clone()))
        .with_instructions(INSTRUCTIONS)
    }

    // Hand-rolled, not #[prompt_router]: one prompt per skill, from the bundle.
    async fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, McpError> {
        Ok(self.prompts())
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<GetPromptResponse, McpError> {
        let args = request
            .arguments
            .as_ref()
            .and_then(|a| a.get(PROMPT_ARG))
            .and_then(|v| v.as_str());
        self.prompt(&request.name, args).map(Into::into)
    }
}

impl Standards {
    /// One prompt per skill: its name, its description, an optional `args`.
    #[must_use]
    pub fn prompts(&self) -> ListPromptsResult {
        let prompts = self
            .engine()
            .skills()
            .into_iter()
            .map(|s| {
                Prompt::new(
                    s.name,
                    Some(s.description),
                    Some(vec![
                        PromptArgument::new(PROMPT_ARG)
                            .with_description("Free-form arguments for the skill")
                            .with_required(false),
                    ]),
                )
            })
            .collect();
        ListPromptsResult::with_all_items(prompts)
    }

    /// The skill's body as one user message, with `args` appended when given.
    pub fn prompt(&self, name: &str, args: Option<&str>) -> Result<GetPromptResult, McpError> {
        let engine = self.engine();
        let skill = engine
            .skill(name)
            .map_err(|e| McpError::invalid_params(miss(engine, &e), None))?;
        let mut text = skill.body.trim_end().to_owned();
        if let Some(args) = args.map(str::trim).filter(|a| !a.is_empty()) {
            let _ = write!(text, "\n\nARGUMENTS: {args}");
        }
        Ok(
            GetPromptResult::new(vec![PromptMessage::new_text(Role::User, text)])
                .with_description(skill.description.clone()),
        )
    }
}

/// How an archetype says a standard applies, in words.
fn applicability(a: Applicability) -> &'static str {
    match a {
        Applicability::Applies => "applies",
        Applicability::Defining => "defining (the archetype is built around it)",
        Applicability::Adapted => "adapted (applies, changed as the note says)",
        Applicability::Optional => "optional (applies only as the note says)",
        Applicability::NotApplicable => "does not apply",
    }
}

fn push_items(out: &mut String, items: &[ChecklistItem], none: &str) {
    if items.is_empty() {
        let _ = writeln!(out, "{none}");
    }
    for item in items {
        let _ = writeln!(out, "- [ ] {}", item.text);
    }
}

fn longest_backtick_run(s: &str) -> usize {
    s.split(|c| c != '`').map(str::len).max().unwrap_or(0)
}

/// A missed lookup, as a message that says what does exist and where to look.
fn miss(engine: &Engine, err: &Error) -> String {
    let list = |names: Vec<String>| {
        if names.is_empty() {
            "(none)".to_owned()
        } else {
            names
                .iter()
                .map(|n| format!("`{n}`"))
                .collect::<Vec<_>>()
                .join(", ")
        }
    };
    let hint = match err {
        Error::UnknownDoc { .. } => format!(
            "Doc ids are file stems. Known: {}. Call `search` if you're not sure which one.",
            list(
                engine
                    .docs()
                    .into_iter()
                    .map(|d| d.id.as_str().to_owned())
                    .collect()
            )
        ),
        Error::UnknownSection { doc, .. } => format!(
            "Its sections: {}. Omit `section` for the whole doc.",
            list(
                engine
                    .doc(doc)
                    .map(|d| d.sections.into_iter().map(|s| s.anchor).collect())
                    .unwrap_or_default()
            )
        ),
        Error::UnknownArchetype { .. } => format!(
            "Archetypes: {}.",
            list(engine.archetypes().iter().map(|a| a.name.clone()).collect())
        ),
        Error::UnknownSkill { .. } => format!(
            "Skills: {}.",
            list(engine.skills().into_iter().map(|s| s.name).collect())
        ),
        Error::UnknownAsset { skill, .. } => format!(
            "Its assets: {}.",
            list(
                engine
                    .skill(skill)
                    .map(|s| s.assets.iter().map(|a| a.path.clone()).collect())
                    .unwrap_or_default()
            )
        ),
        Error::UnknownTemplate { .. } => format!(
            "Templates: {}.",
            list(engine.templates().into_iter().map(|t| t.path).collect())
        ),
    };
    format!("{err}. {hint}")
}

/// The streamable HTTP service over `state`: stateless, JSON replies, `Host` checked
/// against [`AppState::allowed_hosts`].
#[must_use]
pub fn service(state: &AppState) -> StreamableHttpService<Standards, LocalSessionManager> {
    let config = StreamableHttpServerConfig::default()
        .with_legacy_session_mode(false)
        .with_json_response(true)
        .with_allowed_hosts(state.allowed_hosts.iter().cloned());
    // Built once; each request clones it (the tool router is shared, not rebuilt).
    let handler = Standards::new(state.clone());
    StreamableHttpService::new(
        move || Ok(handler.clone()),
        Arc::new(LocalSessionManager::default()),
        config,
    )
}

/// Add the MCP endpoint at [`PATH`] to `route`.
#[must_use]
pub fn mount(route: Route, state: &AppState) -> Route {
    route.at(PATH, service(state).compat())
}

#[cfg(test)]
mod tests;
