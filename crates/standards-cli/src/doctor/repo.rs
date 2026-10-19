//! Repo-scope checks: `.standards.conf`, the `AGENTS.md` line, and the leftovers of the
//! `standards/` submodule. Fixes stage their changes and never commit.

use std::path::{Path, PathBuf};

use serde_json::Value;
use standards_core::StandardsConf;

use super::files::{self, git, link_target, points_into, write_atomic};
use super::{Cx, Finding};
use crate::error::{Error, Result};
use crate::scaffold::CONF_FILE;

/// The submodule's name and path in every legacy repo.
const SUBMODULE: &str = "standards";

/// Whether the repo opted in: a `.standards.conf`, or the old submodule's leftovers.
#[must_use]
pub fn is_standards_repo(root: &Path) -> bool {
    root.join(CONF_FILE).exists()
        || !submodule_sections(root).is_empty()
        || !legacy_skill_links(root).is_empty()
}

// ── repo.conf ───────────────────────────────────────────────────────────────

/// The repo's `.standards.conf`: `Ok(None)` when absent, `Err` when it does not parse.
fn read_conf(root: &Path) -> std::result::Result<Option<StandardsConf>, String> {
    match std::fs::read_to_string(root.join(CONF_FILE)) {
        Ok(text) => StandardsConf::parse(&text)
            .map(Some)
            .map_err(|e| e.to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("{CONF_FILE}: {e}")),
    }
}

pub fn conf(cx: &Cx<'_>) -> Finding {
    let conf = match read_conf(cx.root()) {
        Ok(Some(c)) => c,
        Ok(None) => return Finding::fail(format!("no {CONF_FILE}; run `standards init`")),
        Err(e) => return Finding::fail(e),
    };
    let Some(archetype) = &conf.archetype else {
        return Finding::fail(format!("{CONF_FILE} has no ARCHETYPE; add one"));
    };
    if let Some(m) = &cx.manifest
        && !m.archetypes.iter().any(|a| a == archetype)
    {
        return Finding::fail(format!(
            "unknown archetype `{archetype}` in {CONF_FILE}; one of: {}",
            m.archetypes.join(", ")
        ));
    }
    Finding::ok(format!("product={} archetype={archetype}", conf.product))
}

// ── repo.agents ─────────────────────────────────────────────────────────────

const AGENTS: &str = "AGENTS.md";
const LINE_PREFIX: &str = "Standards: ";

/// The `Standards:` line for `conf` (L15; templates/consuming-repo-AGENTS.md line 1).
#[must_use]
pub fn standards_line(archetype: &str, product: &str) -> String {
    format!(
        "Standards: archetype={archetype} product={product} — rules via the `standards` MCP ({}). Repo-specific overrides below.",
        standards_core::ORIGIN
    )
}

fn wanted_line(cx: &Cx<'_>) -> Option<String> {
    let conf = read_conf(cx.root()).ok().flatten()?;
    Some(standards_line(conf.archetype.as_deref()?, &conf.product))
}

pub fn agents(cx: &Cx<'_>) -> Finding {
    let Some(want) = wanted_line(cx) else {
        return Finding::skip(format!("needs a {CONF_FILE} with an ARCHETYPE"));
    };
    let Ok(text) = std::fs::read_to_string(cx.root().join(AGENTS)) else {
        return Finding::fail(format!("no {AGENTS}")).fixable(true);
    };
    match text.lines().find(|l| l.starts_with(LINE_PREFIX)) {
        Some(have) if have.trim_end() == want => Finding::ok(format!("{AGENTS} has the line")),
        Some(have) => Finding::fail(format!("the {AGENTS} line disagrees with {CONF_FILE}"))
            .fixable(true)
            .details(vec![format!("have: {have}"), format!("want: {want}")]),
        None => Finding::fail(format!("no `Standards:` line in {AGENTS}")).fixable(true),
    }
}

/// Rewrite the first `Standards: ` line, or insert one as line 1.
fn with_line(text: &str, want: &str) -> String {
    if text.lines().any(|l| l.starts_with(LINE_PREFIX)) {
        let mut done = false;
        return text
            .split_inclusive('\n')
            .map(|line| {
                if !done && line.starts_with(LINE_PREFIX) {
                    done = true;
                    let ending = &line[line.trim_end_matches(['\r', '\n']).len()..];
                    format!("{want}{ending}")
                } else {
                    line.to_owned()
                }
            })
            .collect();
    }
    if text.is_empty() {
        format!("{want}\n")
    } else {
        format!("{want}\n\n{text}")
    }
}

pub fn fix_agents(cx: &Cx<'_>) -> Result<Vec<String>> {
    let want = wanted_line(cx).ok_or_else(|| Error::Fail(format!("no usable {CONF_FILE}")))?;
    let path = cx.root().join(AGENTS);
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(Error::io(&path, e)),
    };
    let had = text.lines().any(|l| l.starts_with(LINE_PREFIX));
    write_atomic(&path, with_line(&text, &want).as_bytes())?;
    git(cx.root(), &["add", "--", AGENTS])?;
    Ok(vec![if had {
        format!("rewrote the line in {AGENTS} (staged)")
    } else {
        format!("inserted the line at the top of {AGENTS} (staged)")
    }])
}

// ── legacy.submodule ────────────────────────────────────────────────────────

/// A `[submodule "…"]` section of `.gitmodules`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Section {
    name: String,
    path: Option<String>,
}

fn sections(text: &str) -> Vec<Section> {
    let mut out: Vec<Section> = Vec::new();
    for line in text.lines().map(str::trim) {
        if let Some(rest) = line.strip_prefix("[submodule") {
            let name = rest.trim().trim_end_matches(']').trim().trim_matches('"');
            out.push(Section {
                name: name.to_owned(),
                path: None,
            });
        } else if let (Some(last), Some((k, v))) = (out.last_mut(), line.split_once('='))
            && k.trim() == "path"
        {
            last.path = Some(v.trim().to_owned());
        }
    }
    out
}

fn gitmodules(root: &Path) -> Vec<Section> {
    std::fs::read_to_string(root.join(".gitmodules"))
        .map(|t| sections(&t))
        .unwrap_or_default()
}

/// The `.gitmodules` sections that are the standards submodule.
fn submodule_sections(root: &Path) -> Vec<Section> {
    gitmodules(root)
        .into_iter()
        .filter(|s| s.name == SUBMODULE || s.path.as_deref() == Some(SUBMODULE))
        .collect()
}

fn modules_dir(root: &Path) -> Option<PathBuf> {
    let dir = files::git_common_dir(root)
        .ok()?
        .join("modules")
        .join(SUBMODULE);
    dir.exists().then_some(dir)
}

pub fn submodule(cx: &Cx<'_>) -> Finding {
    let root = cx.root();
    let mut details: Vec<String> = submodule_sections(root)
        .iter()
        .map(|s| format!(".gitmodules: [submodule \"{}\"]", s.name))
        .collect();
    if let Some(dir) = modules_dir(root) {
        details.push(format!(
            "{}",
            dir.strip_prefix(root).unwrap_or(&dir).display()
        ));
    }
    if details.is_empty() {
        return Finding::ok("no standards submodule");
    }
    Finding::fail("the standards submodule is still here")
        .fixable(true)
        .details(details)
}

pub fn fix_submodule(cx: &Cx<'_>) -> Result<Vec<String>> {
    let root = cx.root();
    let mut done = Vec::new();
    for s in submodule_sections(root) {
        let path = s.path.clone().unwrap_or_else(|| s.name.clone());
        // Fails harmlessly when it was never initialised.
        let _ = git(root, &["submodule", "deinit", "-f", "--", &path]);
        if git(root, &["rm", "-q", "-f", "--", &path]).is_ok() {
            done.push(format!("git submodule deinit + git rm {path}"));
        } else {
            git(
                root,
                &[
                    "config",
                    "-f",
                    ".gitmodules",
                    "--remove-section",
                    &format!("submodule.{}", s.name),
                ],
            )?;
            git(root, &["add", "--", ".gitmodules"])?;
            done.push(format!(
                "removed [submodule \"{}\"] from .gitmodules",
                s.name
            ));
        }
    }
    if root.join(".gitmodules").exists() && gitmodules(root).is_empty() {
        done.push(files::remove(root, ".gitmodules")?);
    }
    if let Some(dir) = modules_dir(root) {
        std::fs::remove_dir_all(&dir).map_err(|e| Error::io(&dir, e))?;
        done.push(format!(
            "removed {}",
            dir.strip_prefix(root).unwrap_or(&dir).display()
        ));
    }
    Ok(done)
}

// ── legacy.skill-links ──────────────────────────────────────────────────────

const SKILLS: &str = ".claude/skills";

/// `.claude/skills/thmsn-*` symlinks into `standards/`: (relative path, target).
fn legacy_skill_links(root: &Path) -> Vec<(String, PathBuf)> {
    let Ok(entries) = std::fs::read_dir(root.join(SKILLS)) else {
        return Vec::new();
    };
    let mut links: Vec<(String, PathBuf)> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if !name.starts_with("thmsn-") {
                return None;
            }
            let target = link_target(&e.path())?;
            points_into(&target, &[SUBMODULE]).then(|| (format!("{SKILLS}/{name}"), target))
        })
        .collect();
    links.sort();
    links
}

pub fn skill_links(cx: &Cx<'_>) -> Finding {
    let links = legacy_skill_links(cx.root());
    if links.is_empty() {
        return Finding::ok("no skill links into standards/");
    }
    Finding::fail(format!(
        "{} skill link(s) into standards/; the skills come from ~/.claude/skills now",
        links.len()
    ))
    .fixable(true)
    .details(
        links
            .iter()
            .map(|(p, t)| format!("{p} -> {}", t.display()))
            .collect(),
    )
}

pub fn fix_skill_links(cx: &Cx<'_>) -> Result<Vec<String>> {
    let root = cx.root();
    let mut done = Vec::new();
    for (rel, _) in legacy_skill_links(root) {
        done.push(files::remove(root, &rel)?);
    }
    let dir = root.join(SKILLS);
    if std::fs::read_dir(&dir).is_ok_and(|mut d| d.next().is_none()) {
        let _ = std::fs::remove_dir(&dir);
    }
    Ok(done)
}

// ── legacy.mcp-json ─────────────────────────────────────────────────────────

const MCP_JSON: &str = ".mcp.json";

/// The untouched submodule template: a `//` key, no servers, nothing else.
fn is_template(v: &Value) -> bool {
    let Some(obj) = v.as_object() else {
        return false;
    };
    let servers_empty = obj
        .get("mcpServers")
        .is_none_or(|s| s.as_object().is_some_and(serde_json::Map::is_empty));
    obj.contains_key("//") && servers_empty && obj.keys().all(|k| k == "//" || k == "mcpServers")
}

/// Whether any string in `v` is a path into the old submodule.
fn mentions_submodule(v: &Value) -> bool {
    match v {
        Value::String(s) => {
            s.starts_with("standards/")
                || s.starts_with("./standards/")
                || s.contains("/standards/")
        }
        Value::Array(a) => a.iter().any(mentions_submodule),
        Value::Object(o) => o.values().any(mentions_submodule),
        _ => false,
    }
}

/// Server names whose entry points into `standards/`.
fn legacy_servers(v: &Value) -> Vec<String> {
    v.get("mcpServers")
        .and_then(Value::as_object)
        .map(|servers| {
            servers
                .iter()
                .filter(|(_, e)| mentions_submodule(e))
                .map(|(k, _)| k.clone())
                .collect()
        })
        .unwrap_or_default()
}

fn read_mcp_json(root: &Path) -> Option<std::result::Result<Value, String>> {
    let text = std::fs::read_to_string(root.join(MCP_JSON)).ok()?;
    Some(serde_json::from_str(&text).map_err(|e| format!("{MCP_JSON} does not parse: {e}")))
}

pub fn mcp_json(cx: &Cx<'_>) -> Finding {
    match read_mcp_json(cx.root()) {
        None => Finding::ok(format!("no {MCP_JSON}")),
        Some(Err(e)) => Finding::warn(e),
        Some(Ok(v)) if is_template(&v) => {
            Finding::fail(format!("{MCP_JSON} is the untouched submodule template")).fixable(true)
        }
        Some(Ok(v)) => {
            let legacy = legacy_servers(&v);
            if legacy.is_empty() {
                Finding::ok(format!("{MCP_JSON} has no standards/ paths"))
            } else {
                Finding::fail(format!("{MCP_JSON} has servers under standards/"))
                    .fixable(true)
                    .details(legacy)
            }
        }
    }
}

pub fn fix_mcp_json(cx: &Cx<'_>) -> Result<Vec<String>> {
    let root = cx.root();
    let Some(Ok(mut v)) = read_mcp_json(root) else {
        return Ok(Vec::new());
    };
    let mut done = Vec::new();
    if !is_template(&v) {
        let legacy = legacy_servers(&v);
        if let Some(servers) = v.get_mut("mcpServers").and_then(Value::as_object_mut) {
            for name in &legacy {
                servers.shift_remove(name);
                done.push(format!("removed server `{name}` from {MCP_JSON}"));
            }
        }
        let emptied = v
            .get("mcpServers")
            .and_then(Value::as_object)
            .is_some_and(serde_json::Map::is_empty)
            && v.as_object()
                .is_some_and(|o| o.keys().all(|k| k == "//" || k == "mcpServers"));
        if !emptied {
            let mut text =
                serde_json::to_string_pretty(&v).map_err(|e| Error::Fail(e.to_string()))?;
            text.push('\n');
            write_atomic(&root.join(MCP_JSON), text.as_bytes())?;
            git(root, &["add", "--", MCP_JSON])?;
            done.push(format!("staged {MCP_JSON}"));
            return Ok(done);
        }
    }
    done.push(files::remove(root, MCP_JSON)?);
    Ok(done)
}

// ── legacy.ci-override ──────────────────────────────────────────────────────

/// `submodule_override` lines naming `standards` (1-based line numbers).
fn woodpecker_hits(text: &str) -> Vec<usize> {
    let lines: Vec<&str> = text.lines().collect();
    let indent = |l: &str| l.len() - l.trim_start().len();
    let mut hits = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let Some(rest) = line.trim_start().strip_prefix("submodule_override:") else {
            continue;
        };
        if rest.contains(SUBMODULE) {
            hits.push(i + 1);
            continue;
        }
        let base = indent(line);
        for (j, next) in lines.iter().enumerate().skip(i + 1) {
            let t = next.trim_start();
            if t.is_empty() || t.starts_with('#') {
                continue;
            }
            if indent(next) <= base {
                break;
            }
            if t.starts_with("standards:") {
                hits.push(j + 1);
            }
        }
    }
    hits
}

/// `submodules: true` / `recursive` lines (1-based).
fn workflow_hits(text: &str) -> Vec<usize> {
    text.lines()
        .enumerate()
        .filter_map(|(i, line)| {
            let value = line.trim_start().strip_prefix("submodules:")?;
            let value = value
                .split('#')
                .next()
                .unwrap_or("")
                .trim()
                .trim_matches(['"', '\'']);
            matches!(value, "true" | "recursive").then_some(i + 1)
        })
        .collect()
}

/// The YAML files directly in `dir`, sorted.
fn yaml_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension().is_some_and(|e| {
                    e.eq_ignore_ascii_case("yml") || e.eq_ignore_ascii_case("yaml")
                })
        })
        .collect();
    out.sort();
    out
}

pub fn ci_override(cx: &Cx<'_>) -> Finding {
    let root = cx.root();
    let mut woodpecker = yaml_files(&root.join(".woodpecker"));
    for name in [".woodpecker.yml", ".woodpecker.yaml"] {
        if root.join(name).is_file() {
            woodpecker.push(root.join(name));
        }
    }
    // `submodules: true` is only a leftover when standards was the repo's only submodule.
    let other_submodules = gitmodules(root)
        .iter()
        .any(|s| s.name != SUBMODULE && s.path.as_deref() != Some(SUBMODULE));
    let workflows = if other_submodules {
        Vec::new()
    } else {
        yaml_files(&root.join(".github").join("workflows"))
    };

    let mut details = Vec::new();
    let mut report = |files: &[PathBuf], scan: fn(&str) -> Vec<usize>| {
        for file in files {
            let Ok(text) = std::fs::read_to_string(file) else {
                continue;
            };
            let lines: Vec<&str> = text.lines().collect();
            for n in scan(&text) {
                details.push(format!(
                    "{}:{n}: {}",
                    file.strip_prefix(root).unwrap_or(file).display(),
                    lines[n - 1].trim()
                ));
            }
        }
    };
    report(&woodpecker, woodpecker_hits);
    report(&workflows, workflow_hits);
    if details.is_empty() {
        return Finding::ok("no CI submodule handling for standards");
    }
    Finding::warn(format!(
        "{} CI line(s) still fetch the standards submodule; edit them by hand",
        details.len()
    ))
    .details(details)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn line_rewrite_and_insert() {
        let want = standards_line("library", "id");
        assert_eq!(
            want,
            "Standards: archetype=library product=id — rules via the `standards` MCP (https://standards.thmsn.dev). Repo-specific overrides below."
        );
        assert_eq!(with_line("", &want), format!("{want}\n"));
        assert_eq!(
            with_line("# AGENTS\n", &want),
            format!("{want}\n\n# AGENTS\n")
        );
        assert_eq!(
            with_line("x\r\nStandards: old\r\nStandards: other\r\n", &want),
            format!("x\r\n{want}\r\nStandards: other\r\n")
        );
        assert_eq!(with_line("Standards: old", &want), want);
    }

    #[test]
    fn gitmodules_sections() {
        let text = "; comment\n[submodule \"standards\"]\n\tpath = standards\n\turl = x\n\
                    [submodule \"vendor/x\"]\n path = third/x\n";
        assert_eq!(
            sections(text),
            vec![
                Section {
                    name: "standards".into(),
                    path: Some("standards".into())
                },
                Section {
                    name: "vendor/x".into(),
                    path: Some("third/x".into())
                },
            ]
        );
        assert_eq!(sections("; only comments\n"), Vec::<Section>::new());
    }

    #[test]
    fn mcp_json_shapes() {
        assert!(is_template(&json!({"//": "x", "mcpServers": {}})));
        assert!(is_template(&json!({"//": "x"})));
        assert!(!is_template(&json!({"mcpServers": {}})));
        assert!(!is_template(&json!({"//": "x", "mcpServers": {"a": {}}})));
        assert!(!is_template(&json!({"//": "x", "other": 1})));
        assert!(!is_template(&json!([])));
        let v = json!({"mcpServers": {
            "grafana": {"command": "standards/mcp/grafana/run.sh"},
            "komodo": {"command": "node", "args": ["./standards/mcp/komodo/index.js"]},
            "abs": {"command": "/h/git/standards/mcp/x"},
            "web": {"type": "http", "url": "https://standards.thmsn.dev/mcp"},
            "n": {"command": "npx", "args": [1, true, null]}
        }});
        assert_eq!(legacy_servers(&v), ["grafana", "komodo", "abs"]);
    }

    #[test]
    fn ci_scans() {
        let wp = [
            "clone:",
            "  git:",
            "    settings:",
            "      submodule_override:",
            "        # note",
            "",
            "        other: x",
            "        standards: https://x",
            "      tags: true",
            "steps:",
            "  submodule_override: { standards: y }",
        ]
        .join("\n");
        let wp = wp.as_str();
        assert_eq!(woodpecker_hits(wp), vec![8, 11]);
        assert_eq!(
            woodpecker_hits("submodule_override:\n  other: x\n"),
            Vec::<usize>::new()
        );
        let gh = [
            "steps:",
            "  - uses: actions/checkout@v4",
            "    with:",
            "      submodules: true",
            "      submodules: 'recursive' # c",
            "      submodules: false",
        ]
        .join("\n");
        let gh = gh.as_str();
        assert_eq!(workflow_hits(gh), vec![4, 5]);
    }
}
