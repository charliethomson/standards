//! The tools over the real embedded content, and one MCP exchange over HTTP through `app()`.

use std::collections::BTreeMap;
use std::sync::Arc;

use poem::test::{TestClient, TestResponse};
use serde_json::{Value, json};
use standards_core::Bundle;

use super::*;
use crate::config::Config;

fn real_state() -> AppState {
    let bundle = standards_db::bundle().expect("embedded content parses");
    AppState::new(bundle, &Config::default(), "0.1.42", false)
}

fn server() -> Standards {
    Standards::new(real_state())
}

/// A tiny bundle with a binary skill asset, which the real content doesn't have.
fn tiny_server() -> Standards {
    let files: Vec<(&str, Vec<u8>)> = vec![
        ("docs/v.md", b"# V\n\nNo checklist here.\n".to_vec()),
        (
            "docs/archetypes/lib.md",
            b"# Archetype: Lib\n\n| Standard | How it applies here |\n|---|---|\n| [V](../v.md) | Self. |\n"
                .to_vec(),
        ),
        (
            "skills/s/SKILL.md",
            b"---\nname: s\ndescription: d\n---\nBody.\n".to_vec(),
        ),
        ("skills/s/bin/blob", vec![0xff, 0xfe, 0x00]),
    ];
    let bundle = Bundle::from_files(files, "abc").expect("tiny bundle");
    Standards::new(AppState::new(
        Arc::new(bundle),
        &Config::default(),
        "0.0.1",
        false,
    ))
}

fn search(s: &Standards, query: &str, limit: Option<usize>) -> ToolResult {
    s.search(Parameters(SearchArgs {
        query: query.to_owned(),
        limit,
    }))
}

#[test]
fn search_ranks_hits_and_names_the_call_that_opens_each() {
    let s = server();
    let out = search(&s, "derived version commit count", Some(3)).unwrap();
    assert!(out.starts_with("3 results"), "{out}");
    assert!(out.contains("get_doc(id=\"versioning\""), "{out}");
    assert!(
        out.contains("](https://standards.thmsn.dev/d/versioning"),
        "{out}"
    );

    let skills = search(&s, "janitor stale worktrees", Some(50)).unwrap();
    assert!(skills.contains("get_skill(name=\""), "{skills}");

    // The limit is clamped, not an error.
    let one = search(&s, "versioning", Some(0)).unwrap();
    assert!(one.starts_with("1 results"), "{one}");

    assert!(
        search(&s, "  ", None)
            .unwrap_err()
            .contains("`query` is empty")
    );
    let none = search(&s, "zzqqxxnotaword", None).unwrap();
    assert!(none.starts_with("No matches"), "{none}");
}

#[test]
fn get_doc_whole_or_one_section_with_absolute_links() {
    let s = server();
    let doc = s
        .get_doc(Parameters(GetDocArgs {
            id: "archetypes/cli-tool".into(),
            section: None,
        }))
        .unwrap();
    assert!(doc.starts_with("# "), "{doc}");
    assert!(doc.contains("Source: https://standards.thmsn.dev/d/archetypes/cli-tool"));
    assert!(doc.contains("Sections (pass one as `section`): "));
    assert!(doc.contains("`checklist`"));
    assert!(!doc.contains("](../"), "relative links are rewritten");

    let section = s
        .get_doc(Parameters(GetDocArgs {
            id: "versioning".into(),
            section: Some("#checklist".into()),
        }))
        .unwrap();
    assert!(section.starts_with("## Checklist"), "{section}");
    assert!(section.contains("Source: https://standards.thmsn.dev/d/versioning#checklist"));

    // A blank section is the whole doc.
    let whole = s
        .get_doc(Parameters(GetDocArgs {
            id: "versioning".into(),
            section: Some(" ".into()),
        }))
        .unwrap();
    assert!(whole.contains("Sections (pass one as `section`)"));

    let miss = s
        .get_doc(Parameters(GetDocArgs {
            id: "nope".into(),
            section: None,
        }))
        .unwrap_err();
    assert!(miss.starts_with("no doc `nope`. "), "{miss}");
    assert!(
        miss.contains("`versioning`") && miss.contains("`search`"),
        "{miss}"
    );

    let miss = s
        .get_doc(Parameters(GetDocArgs {
            id: "versioning".into(),
            section: Some("nope".into()),
        }))
        .unwrap_err();
    assert!(
        miss.contains("Its sections: ") && miss.contains("`checklist`"),
        "{miss}"
    );
}

fn checklist(s: &Standards, standard: &str, archetype: &str) -> ToolResult {
    s.get_checklist(Parameters(GetChecklistArgs {
        standard: standard.into(),
        archetype: archetype.into(),
    }))
}

#[test]
fn get_checklist_composes_applies_and_explains_na() {
    let s = server();
    let out = checklist(&s, "versioning", "cli-tool").unwrap();
    assert!(out.starts_with("# Checklist: "), "{out}");
    assert!(out.contains("for the `cli-tool` archetype"));
    assert!(out.contains("Applicability: **"));
    assert!(out.contains("](https://standards.thmsn.dev/d/versioning)"));
    assert!(out.contains("](https://standards.thmsn.dev/d/archetypes/cli-tool)"));
    assert!(out.contains("- [ ] "));

    // Every N/A row in the real tables answers with its reason and nothing to check.
    let engine = s.engine();
    let mut seen_na = false;
    for arch in engine.archetypes() {
        for row in &arch.rows {
            if row.applicability != Applicability::NotApplicable {
                continue;
            }
            for standard in &row.standards {
                if let standards_core::StandardRef::Doc(id) = standard {
                    let out = checklist(&s, id.as_str(), &arch.name).unwrap();
                    assert!(out.contains("**does not apply**"), "{out}");
                    assert!(out.contains("Nothing to check"), "{out}");
                    assert!(!out.contains("- [ ] "), "{out}");
                    seen_na = true;
                }
            }
        }
    }
    assert!(seen_na, "the real tables have N/A rows");

    // The archetype doc as its own standard is not listed twice.
    let own = checklist(&s, "archetypes/cli-tool", "cli-tool").unwrap();
    assert_eq!(own.matches("\n## [").count(), 1, "{own}");

    let miss = checklist(&s, "versioning", "nope").unwrap_err();
    assert!(
        miss.starts_with("no archetype `nope`. Archetypes: "),
        "{miss}"
    );
    assert!(miss.contains("`cli-tool`"));
    assert!(
        checklist(&s, "nope", "cli-tool")
            .unwrap_err()
            .starts_with("no doc")
    );
}

#[test]
fn get_checklist_unlisted_standard_and_missing_checklists() {
    let s = tiny_server();
    let own = checklist(&s, "v", "lib").unwrap();
    assert!(own.contains("This doc has no checklist"), "{own}");
    assert!(own.contains("The archetype has no checklist"), "{own}");
    assert!(own.contains("How it applies here: Self."), "{own}");
    let unlisted = checklist(&s, "archetypes/lib", "lib").unwrap();
    assert!(unlisted.contains("doesn't list it"), "{unlisted}");
}

#[test]
fn get_template_renders_given_vars_and_lists_the_rest() {
    let s = server();
    let path = s
        .engine()
        .templates()
        .into_iter()
        .find(|t| t.placeholders.len() >= 2)
        .expect("a template with two placeholders");
    let first = path.placeholders[0].clone();
    let vars = BTreeMap::from([(first.clone(), "zzfilled".to_owned())]);
    let out = s
        .get_template(Parameters(GetTemplateArgs {
            path: path.path.clone(),
            vars: Some(vars),
        }))
        .unwrap();
    assert!(out.starts_with(&format!(
        "Template `{}` (https://standards.thmsn.dev/t/",
        path.path
    )));
    assert!(out.contains("zzfilled"));
    assert!(!out.contains(&format!("{{{{{first}}}}}")));
    assert!(
        out.contains(&format!("`{{{{{}}}}}`", path.placeholders[1])),
        "{out}"
    );
    assert!(out.contains("Unfilled placeholders"));

    let all: BTreeMap<_, _> = path
        .placeholders
        .iter()
        .map(|k| (k.clone(), "x".to_owned()))
        .collect();
    let filled = s
        .get_template(Parameters(GetTemplateArgs {
            path: format!("templates/{}", path.path),
            vars: Some(all),
        }))
        .unwrap();
    assert!(filled.contains("Every placeholder is filled."));

    let miss = s
        .get_template(Parameters(GetTemplateArgs {
            path: "nope.rs".into(),
            vars: None,
        }))
        .unwrap_err();
    assert!(
        miss.starts_with("no template `nope.rs`. Templates: "),
        "{miss}"
    );
}

#[test]
fn fences_outlast_the_body() {
    assert_eq!(longest_backtick_run("a ``` b ```` c"), 4);
    assert_eq!(longest_backtick_run("none"), 0);
    let s = server();
    // A markdown template with code fences in it still comes back in one block.
    let out = s
        .get_template(Parameters(GetTemplateArgs {
            path: "VERSIONING.md".into(),
            vars: None,
        }))
        .unwrap();
    assert!(out.contains("\n````md\n"), "{out}");
    assert!(out.ends_with("\n````\n"), "{out}");
}

#[test]
fn get_skill_and_its_assets() {
    let s = server();
    let out = s
        .get_skill(Parameters(GetSkillArgs {
            name: "thmsn-logs".into(),
        }))
        .unwrap();
    assert!(out.contains("Assets (fetch with get_skill_asset(name=\"thmsn-logs\""));
    assert!(
        out.contains("- `scripts/obsq.sh`"),
        "nested paths kept: {out}"
    );

    let plain = s
        .get_skill(Parameters(GetSkillArgs {
            name: "thmsn-janitor".into(),
        }))
        .unwrap();
    assert!(!plain.contains("Assets ("));

    let asset = s
        .get_skill_asset(Parameters(GetSkillAssetArgs {
            name: "thmsn-deep-review".into(),
            path: "assets/review.template.md".into(),
        }))
        .unwrap();
    let raw = s
        .engine()
        .skill_asset("thmsn-deep-review", "assets/review.template.md")
        .unwrap();
    assert_eq!(asset.as_bytes(), raw.bytes.as_slice());

    let miss = s
        .get_skill(Parameters(GetSkillArgs {
            name: "nope".into(),
        }))
        .unwrap_err();
    assert!(miss.starts_with("no skill `nope`. Skills: ") && miss.contains("`thmsn-logs`"));
    let miss = s
        .get_skill_asset(Parameters(GetSkillAssetArgs {
            name: "thmsn-logs".into(),
            path: "../SKILL.md".into(),
        }))
        .unwrap_err();
    assert!(
        miss.contains("Its assets: ") && miss.contains("`scripts/obsq.sh`"),
        "{miss}"
    );
    let miss = s
        .get_skill_asset(Parameters(GetSkillAssetArgs {
            name: "nope".into(),
            path: "x".into(),
        }))
        .unwrap_err();
    assert!(miss.starts_with("no skill `nope`"), "{miss}");
}

#[test]
fn a_binary_asset_says_so() {
    let s = tiny_server();
    let err = s
        .get_skill_asset(Parameters(GetSkillAssetArgs {
            name: "s".into(),
            path: "bin/blob".into(),
        }))
        .unwrap_err();
    assert!(err.contains("is binary (3 bytes"), "{err}");
    assert!(err.contains("/api/v1/skills/s/assets/bin/blob"), "{err}");
    let skill = s
        .get_skill(Parameters(GetSkillArgs { name: "s".into() }))
        .unwrap();
    assert!(skill.contains("- `bin/blob`"));
    let miss = s
        .get_doc(Parameters(GetDocArgs {
            id: "v".into(),
            section: Some("x".into()),
        }))
        .unwrap_err();
    assert!(miss.contains("Its sections: (none)"), "{miss}");
}

#[test]
fn version_prompts_and_info() {
    let s = server();
    let v = s.version().0;
    assert_eq!(v.server_version, "0.1.42");
    assert_eq!(v.commit, s.engine().commit());

    let prompts = s.prompts().prompts;
    let skills = s.engine().skills();
    assert_eq!(prompts.len(), skills.len());
    for (p, sk) in prompts.iter().zip(&skills) {
        assert_eq!(p.name, sk.name);
        assert_eq!(p.description.as_deref(), Some(sk.description.as_str()));
        let args = p.arguments.as_ref().unwrap();
        assert_eq!(args[0].name, "args");
        assert_eq!(args[0].required, Some(false));
    }

    let text = |r: &GetPromptResult| {
        serde_json::to_value(r).unwrap()["messages"][0]["content"]["text"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let with = s
        .prompt("thmsn-janitor", Some("review the login screen"))
        .unwrap();
    assert!(text(&with).ends_with("\n\nARGUMENTS: review the login screen"));
    let without = s.prompt("thmsn-janitor", Some("  ")).unwrap();
    assert!(!text(&without).contains("ARGUMENTS:"));
    assert_eq!(
        text(&without),
        s.engine().skill("thmsn-janitor").unwrap().body.trim_end()
    );
    let err = s.prompt("nope", None).unwrap_err();
    assert!(err.message.contains("no skill `nope`"));

    let info = serde_json::to_value(s.get_info()).unwrap();
    assert_eq!(info["instructions"], INSTRUCTIONS);
    assert_eq!(info["serverInfo"]["name"], "standards");
    assert_eq!(info["serverInfo"]["version"], "0.1.42");
}

// ── over HTTP, through the whole app ──

const MODERN: &str = "2026-07-28";

async fn rpc(
    c: &TestClient<impl poem::Endpoint>,
    host: &str,
    version: &str,
    body: &Value,
) -> TestResponse {
    let method = body["method"].as_str().unwrap_or_default().to_owned();
    let mut req = c
        .post(PATH)
        .header("host", host)
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .header("mcp-protocol-version", version)
        .header("mcp-method", method.as_str());
    if let Some(name) = body["params"]["name"].as_str() {
        req = req.header("mcp-name", name);
    }
    req.body(body.to_string()).send().await
}

async fn result(resp: TestResponse) -> Value {
    resp.assert_status_is_ok();
    resp.assert_content_type("application/json");
    let v: Value = resp.0.into_body().into_json().await.expect("json-rpc body");
    assert!(v.get("error").is_none(), "{v}");
    v["result"].clone()
}

fn modern(method: &str, params: Value) -> Value {
    let mut params = params;
    params["_meta"] = json!({
        "io.modelcontextprotocol/protocolVersion": MODERN,
        "io.modelcontextprotocol/clientInfo": {"name": "test", "version": "0"},
        "io.modelcontextprotocol/clientCapabilities": {},
    });
    json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params})
}

#[tokio::test]
async fn an_mcp_exchange_over_http() {
    let state = real_state();
    let skills = state.engine.skills().len();
    let c = TestClient::new(crate::app(state));

    // server/discover carries the instructions.
    let discover = result(
        rpc(
            &c,
            "localhost",
            MODERN,
            &modern("server/discover", json!({})),
        )
        .await,
    )
    .await;
    assert_eq!(discover["instructions"], INSTRUCTIONS);
    assert!(discover["capabilities"]["tools"].is_object());
    assert!(discover["capabilities"]["prompts"].is_object());

    let tools = result(rpc(&c, "localhost", MODERN, &modern("tools/list", json!({}))).await).await;
    let mut names: Vec<&str> = tools["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "get_checklist",
            "get_doc",
            "get_skill",
            "get_skill_asset",
            "get_template",
            "search",
            "version"
        ]
    );

    let call =
        |name: &str, args: Value| modern("tools/call", json!({"name": name, "arguments": args}));
    let checklist = result(
        rpc(
            &c,
            "127.0.0.1",
            MODERN,
            &call(
                "get_checklist",
                json!({"standard": "versioning", "archetype": "cli-tool"}),
            ),
        )
        .await,
    )
    .await;
    assert_eq!(checklist["isError"], false);
    let text = checklist["content"][0]["text"].as_str().unwrap();
    assert!(text.starts_with("# Checklist: "), "{text}");

    // An unknown id is a tool error the model reads, not a JSON-RPC error.
    let miss = result(
        rpc(
            &c,
            "localhost",
            MODERN,
            &call("get_doc", json!({"id": "nope"})),
        )
        .await,
    )
    .await;
    assert_eq!(miss["isError"], true);
    assert!(
        miss["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("`search`")
    );

    let version = result(rpc(&c, "localhost", MODERN, &call("version", json!({}))).await).await;
    assert_eq!(version["structuredContent"]["server_version"], "0.1.42");

    let prompts =
        result(rpc(&c, "localhost", MODERN, &modern("prompts/list", json!({}))).await).await;
    assert_eq!(prompts["prompts"].as_array().unwrap().len(), skills);
    let prompt = result(
        rpc(
            &c,
            "localhost",
            MODERN,
            &modern(
                "prompts/get",
                json!({"name": "thmsn-janitor", "arguments": {"args": "x y"}}),
            ),
        )
        .await,
    )
    .await;
    assert!(
        prompt["messages"][0]["content"]["text"]
            .as_str()
            .unwrap()
            .ends_with("ARGUMENTS: x y")
    );
}

#[tokio::test]
async fn an_older_client_gets_the_instructions_from_initialize_statelessly() {
    let c = TestClient::new(crate::app(real_state()));
    let init = rpc(
        &c,
        "localhost",
        "2025-06-18",
        &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": "2025-06-18", "capabilities": {},
            "clientInfo": {"name": "test", "version": "0"}}}),
    )
    .await;
    assert!(init.0.headers().get("mcp-session-id").is_none());
    let init = result(init).await;
    assert_eq!(init["instructions"], INSTRUCTIONS);
}

#[tokio::test]
async fn a_host_off_the_allowlist_is_refused() {
    let c = TestClient::new(crate::app(real_state()));
    let resp = rpc(
        &c,
        "evil.example",
        MODERN,
        &modern("server/discover", json!({})),
    )
    .await;
    resp.assert_status(poem::http::StatusCode::FORBIDDEN);

    // The public hostname is on the default list.
    let ok = rpc(
        &c,
        "standards.thmsn.dev",
        MODERN,
        &modern("server/discover", json!({})),
    )
    .await;
    ok.assert_status_is_ok();
}
