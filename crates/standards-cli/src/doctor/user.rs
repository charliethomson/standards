//! User-scope checks: what this machine needs to reach the standards.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Map, Value, json};
use standards_core::{DerivedVersion, SkillEntry};

use super::files::{self, link_target, points_into, tilde, write_atomic};
use super::{Cx, Finding};
use crate::error::{Error, Result};
use crate::self_update;

/// The MCP server's name in Claude Code.
pub const MCP_NAME: &str = "standards";

/// What the `SessionStart` hook runs.
pub const HOOK_COMMAND: &str = "standards doctor --quiet";

/// The marker that makes a skill directory doctor's own (L11).
pub const MARKER: &str = ".standards-managed";

/// The names `legacy.path-links` looks for in `~/.local/bin`.
const PATH_LINKS: [&str; 3] = ["standards", "janitor", "ultron"];

// ── mcp.registered ──────────────────────────────────────────────────────────

fn mcp_url(cx: &Cx<'_>) -> String {
    format!("{}/mcp", cx.host)
}

fn claude_json(cx: &Cx<'_>) -> PathBuf {
    cx.home().join(".claude.json")
}

/// `mcpServers.standards` from `~/.claude.json`, read only (it holds OAuth keys).
fn registered_entry(cx: &Cx<'_>) -> std::result::Result<Option<Value>, String> {
    let path = claude_json(cx);
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("{}: {e}", tilde(&path, cx.home()))),
    };
    let v: Value = serde_json::from_str(&text)
        .map_err(|e| format!("{} does not parse: {e}", tilde(&path, cx.home())))?;
    Ok(v.get("mcpServers").and_then(|s| s.get(MCP_NAME)).cloned())
}

fn claude(cx: &Cx<'_>) -> Option<PathBuf> {
    files::which("claude", cx.ctx.env.path.as_deref())
}

pub fn mcp_registered(cx: &Cx<'_>) -> Finding {
    let want = mcp_url(cx);
    let entry = match registered_entry(cx) {
        Ok(e) => e,
        Err(e) => return Finding::fail(e),
    };
    let problem = match &entry {
        None => "not registered".to_owned(),
        Some(e)
            if e.get("type").and_then(Value::as_str) == Some("http")
                && e.get("url").and_then(Value::as_str) == Some(want.as_str()) =>
        {
            return Finding::ok(format!("{MCP_NAME} -> {want}"));
        }
        Some(e) => format!(
            "registered as {} {}, want http {want}",
            e.get("type").and_then(Value::as_str).unwrap_or("?"),
            e.get("url")
                .or_else(|| e.get("command"))
                .and_then(Value::as_str)
                .unwrap_or("?")
        ),
    };
    if claude(cx).is_none() {
        return Finding::fail(format!(
            "{problem}; `claude` is not on PATH, so it can't be fixed"
        ));
    }
    Finding::fail(problem).fixable(true)
}

/// `claude mcp remove` (when an entry exists) then `claude mcp add`, with the child's
/// `HOME` and `PATH` set to this run's.
pub fn fix_mcp_registered(cx: &Cx<'_>) -> Result<Vec<String>> {
    let bin = claude(cx).ok_or_else(|| Error::Fail("`claude` is not on PATH".to_owned()))?;
    let run = |args: &[&str]| -> Result<String> {
        let mut cmd = Command::new(&bin);
        cmd.args(args).env("HOME", cx.home());
        if let Some(path) = &cx.ctx.env.path {
            cmd.env("PATH", path);
        }
        files::output(&mut cmd, &format!("claude {}", args.join(" ")))?;
        Ok(format!("ran claude {}", args.join(" ")))
    };
    let mut done = Vec::new();
    if registered_entry(cx).ok().flatten().is_some() {
        done.push(run(&["mcp", "remove", "-s", "user", MCP_NAME])?);
    }
    let url = mcp_url(cx);
    done.push(run(&[
        "mcp", "add", "-s", "user", "-t", "http", MCP_NAME, &url,
    ])?);
    Ok(done)
}

// ── mcp.reachable ───────────────────────────────────────────────────────────

pub fn mcp_reachable(cx: &Cx<'_>) -> Finding {
    match &cx.reach {
        None => Finding::skip("not checked with --quiet"),
        Some(Ok(took)) => {
            let commit = cx
                .manifest
                .as_ref()
                .map(|m| m.commit.chars().take(7).collect::<String>())
                .unwrap_or_default();
            Finding::ok(format!(
                "{} answered in {}ms (commit {commit})",
                cx.host,
                took.as_millis()
            ))
        }
        Some(Err(e)) => Finding::warn(e.clone()),
    }
}

// ── skills.synced ───────────────────────────────────────────────────────────

fn skills_dir(cx: &Cx<'_>) -> PathBuf {
    cx.home().join(".claude").join("skills")
}

/// The stub `SKILL.md` for a served skill (SPECS.md "Stub SKILL.md").
#[must_use]
pub fn stub(skill: &SkillEntry) -> String {
    format!(
        "---\nname: {name}\ndescription: {description}\n---\n\
         This skill's instructions are served by the standards MCP server.\n\
         Call `get_skill(\"{name}\")` on the `standards` MCP server and follow the returned body\n\
         exactly, as this skill's content. If the server is unreachable, say so and stop; don't\n\
         improvise the procedure.\n",
        name = skill.name,
        description = skill.description,
    )
}

/// The `.standards-managed` marker for a served skill.
#[must_use]
pub fn marker(skill: &SkillEntry) -> String {
    format!(
        "content_sha256={}\ndescription_sha256={}\n",
        skill.content_sha256, skill.description_sha256
    )
}

/// What `skills.synced` would do to one directory.
#[derive(Debug, Clone, PartialEq, Eq)]
enum SkillAction {
    Write {
        name: String,
        why: &'static str,
    },
    Remove {
        name: String,
        why: &'static str,
    },
    /// A served skill's name taken by a directory without the marker: left alone.
    Unmanaged {
        name: String,
    },
}

fn skill_plan(cx: &Cx<'_>) -> Vec<SkillAction> {
    let Some(manifest) = &cx.manifest else {
        return Vec::new();
    };
    let dir = skills_dir(cx);
    let mut plan = Vec::new();
    let served: BTreeSet<&str> = manifest.skills.iter().map(|s| s.name.as_str()).collect();
    for skill in &manifest.skills {
        let path = dir.join(&skill.name);
        let name = skill.name.clone();
        if let Some(target) = link_target(&path) {
            if points_into(&target, &["standards", "skills"]) {
                plan.push(SkillAction::Write {
                    name,
                    why: "legacy link",
                });
            } else {
                plan.push(SkillAction::Unmanaged { name });
            }
            continue;
        }
        if !path.exists() {
            plan.push(SkillAction::Write {
                name,
                why: "missing",
            });
            continue;
        }
        if !path.join(MARKER).is_file() {
            plan.push(SkillAction::Unmanaged { name });
            continue;
        }
        let current = std::fs::read_to_string(path.join("SKILL.md")).ok() == Some(stub(skill))
            && std::fs::read_to_string(path.join(MARKER)).ok() == Some(marker(skill));
        if !current {
            plan.push(SkillAction::Write {
                name,
                why: "out of date",
            });
        }
    }
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return plan;
    };
    let mut extra: Vec<SkillAction> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if served.contains(name.as_str()) {
                return None;
            }
            let path = e.path();
            if let Some(target) = link_target(&path) {
                return points_into(&target, &["standards", "skills"]).then_some(
                    SkillAction::Remove {
                        name,
                        why: "legacy link, no longer served",
                    },
                );
            }
            (path.is_dir() && path.join(MARKER).is_file()).then_some(SkillAction::Remove {
                name,
                why: "no longer served",
            })
        })
        .collect();
    extra.sort_by(|a, b| format!("{a:?}").cmp(&format!("{b:?}")));
    plan.extend(extra);
    plan
}

pub fn skills_synced(cx: &Cx<'_>) -> Finding {
    let Some(manifest) = &cx.manifest else {
        return Finding::skip("no manifest (server unreachable and nothing cached)");
    };
    let plan = skill_plan(cx);
    let mut details = Vec::new();
    let (mut drift, mut unmanaged) = (0, 0);
    for action in &plan {
        match action {
            SkillAction::Write { name, why } | SkillAction::Remove { name, why } => {
                drift += 1;
                details.push(format!("{name}: {why}"));
            }
            SkillAction::Unmanaged { name } => {
                unmanaged += 1;
                details.push(format!(
                    "{name}: not managed by standards (no {MARKER}); left alone"
                ));
            }
        }
    }
    let total = manifest.skills.len();
    if drift > 0 {
        return Finding::fail(format!(
            "{drift} of {total} skill stubs to sync in {}",
            tilde(&skills_dir(cx), cx.home())
        ))
        .fixable(true)
        .details(details);
    }
    if unmanaged > 0 {
        return Finding::warn(format!(
            "{unmanaged} served skill name(s) taken by unmanaged dirs"
        ))
        .details(details);
    }
    Finding::ok(format!("{total} skill stubs current"))
}

pub fn fix_skills_synced(cx: &Cx<'_>) -> Result<Vec<String>> {
    let Some(manifest) = &cx.manifest else {
        return Ok(Vec::new());
    };
    let dir = skills_dir(cx);
    let mut done = Vec::new();
    for action in skill_plan(cx) {
        match action {
            SkillAction::Write { name, why } => {
                let Some(skill) = manifest.skills.iter().find(|s| s.name == name) else {
                    continue;
                };
                let path = dir.join(&name);
                if link_target(&path).is_some() {
                    std::fs::remove_file(&path).map_err(|e| Error::io(&path, e))?;
                }
                write_atomic(&path.join("SKILL.md"), stub(skill).as_bytes())?;
                write_atomic(&path.join(MARKER), marker(skill).as_bytes())?;
                done.push(format!("wrote {name} ({why})"));
            }
            SkillAction::Remove { name, why } => {
                let path = dir.join(&name);
                if link_target(&path).is_some() {
                    std::fs::remove_file(&path).map_err(|e| Error::io(&path, e))?;
                } else {
                    std::fs::remove_dir_all(&path).map_err(|e| Error::io(&path, e))?;
                }
                done.push(format!("removed {name} ({why})"));
            }
            SkillAction::Unmanaged { .. } => {}
        }
    }
    Ok(done)
}

// ── hook.session-start ──────────────────────────────────────────────────────

fn settings_path(cx: &Cx<'_>) -> PathBuf {
    cx.home().join(".claude").join("settings.json")
}

/// The hook entry merged under `hooks.SessionStart` (L13).
#[must_use]
pub fn hook_entry() -> Value {
    json!({
        "matcher": "startup",
        "hooks": [{ "type": "command", "command": HOOK_COMMAND, "timeout": 10 }]
    })
}

/// Whether any `SessionStart` entry already runs [`HOOK_COMMAND`].
fn has_hook(settings: &Value) -> bool {
    settings
        .pointer("/hooks/SessionStart")
        .and_then(Value::as_array)
        .is_some_and(|entries| {
            entries.iter().any(|entry| {
                entry
                    .get("hooks")
                    .and_then(Value::as_array)
                    .is_some_and(|hooks| {
                        hooks.iter().any(|h| {
                            h.get("command").and_then(Value::as_str).map(str::trim)
                                == Some(HOOK_COMMAND)
                        })
                    })
            })
        })
}

/// `settings.json` parsed; `None` when the file does not exist.
fn read_settings(path: &Path) -> Result<Option<Value>> {
    match std::fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text)
            .map(Some)
            .map_err(|e| Error::Fail(format!("{} does not parse: {e}", path.display()))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(Error::io(path, e)),
    }
}

pub fn hook_session_start(cx: &Cx<'_>) -> Finding {
    let path = settings_path(cx);
    let shown = tilde(&path, cx.home());
    match read_settings(&path) {
        Err(e) => Finding::fail(format!("{e}; fix it by hand")),
        Ok(Some(v)) if !v.is_object() => {
            Finding::fail(format!("{shown} is not a JSON object; fix it by hand"))
        }
        Ok(Some(v)) if has_hook(&v) => Finding::ok(format!("{shown} runs `{HOOK_COMMAND}`")),
        Ok(Some(mut v)) => match merge_hook(&mut v) {
            Ok(()) => missing_hook(),
            Err(e) => Finding::fail(e.to_string()),
        },
        Ok(None) => missing_hook(),
    }
}

fn missing_hook() -> Finding {
    Finding::fail(format!("no `SessionStart` hook running `{HOOK_COMMAND}`")).fixable(true)
}

/// Merge [`hook_entry`] into `settings`, keeping everything else.
fn merge_hook(settings: &mut Value) -> Result<()> {
    let not = |what: &str| Error::Fail(format!("settings.json: {what}; fix it by hand"));
    let root = settings
        .as_object_mut()
        .ok_or_else(|| not("not a JSON object"))?;
    let hooks = root
        .entry("hooks")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or_else(|| not("`hooks` is not an object"))?;
    let session = hooks
        .entry("SessionStart")
        .or_insert_with(|| Value::Array(Vec::new()))
        .as_array_mut()
        .ok_or_else(|| not("`hooks.SessionStart` is not a list"))?;
    session.push(hook_entry());
    Ok(())
}

pub fn fix_hook_session_start(cx: &Cx<'_>) -> Result<Vec<String>> {
    let path = settings_path(cx);
    let existing = read_settings(&path)?;
    let mut done = Vec::new();
    let had_file = existing.is_some();
    let mut settings = existing.unwrap_or_else(|| json!({}));
    if !has_hook(&settings) {
        merge_hook(&mut settings)?;
    }
    if had_file {
        let backup = path.with_file_name("settings.json.bak");
        std::fs::copy(&path, &backup).map_err(|e| Error::io(&backup, e))?;
        done.push(format!("backed up to {}", tilde(&backup, cx.home())));
    }
    let mut text =
        serde_json::to_string_pretty(&settings).map_err(|e| Error::Fail(e.to_string()))?;
    text.push('\n');
    write_atomic(&path, text.as_bytes())?;
    done.push(format!(
        "added the SessionStart hook to {}",
        tilde(&path, cx.home())
    ));
    Ok(done)
}

// ── cli.current ─────────────────────────────────────────────────────────────

pub fn cli_current(cx: &Cx<'_>) -> Finding {
    let Some(manifest) = &cx.manifest else {
        return Finding::skip("no manifest (server unreachable and nothing cached)");
    };
    let running = &cx.ctx.env.version;
    let Ok(current) = running.parse::<DerivedVersion>() else {
        return Finding::warn(format!(
            "running {running}, not a release version; can't compare"
        ));
    };
    let Ok(min) = manifest.min_cli_version.parse::<DerivedVersion>() else {
        return Finding::warn(format!(
            "the server's min_cli_version `{}` does not parse",
            manifest.min_cli_version
        ));
    };
    if current < min {
        return Finding::fail(format!(
            "{current} is older than the server's minimum {min}"
        ))
        .fixable(true);
    }
    if cx.quiet {
        return Finding::ok(format!("{current} (minimum {min})"));
    }
    let env = &cx.ctx.env;
    match self_update::resolve(
        env.registry_url(),
        &env.platform,
        env.config.registry_token.as_deref(),
    ) {
        Ok(latest) => match latest.version.parse::<DerivedVersion>() {
            Ok(v) if v > current => {
                Finding::warn(format!("{current}; {v} is available")).fixable(true)
            }
            _ => Finding::ok(format!("{current} is the latest")),
        },
        Err(e) => Finding::ok(format!("{current} (minimum {min}); latest unknown: {e}")),
    }
}

pub fn fix_cli_current(cx: &Cx<'_>) -> Result<Vec<String>> {
    let out = self_update::run(cx.ctx, &self_update::Args { check: false })?;
    Ok(out.human)
}

// ── legacy.path-links ───────────────────────────────────────────────────────

fn legacy_links(cx: &Cx<'_>) -> Vec<(PathBuf, PathBuf)> {
    let bin = cx.home().join(".local").join("bin");
    PATH_LINKS
        .iter()
        .filter_map(|name| {
            let path = bin.join(name);
            let target = link_target(&path)?;
            points_into(&target, &["standards", "bin"]).then_some((path, target))
        })
        .collect()
}

pub fn path_links(cx: &Cx<'_>) -> Finding {
    let links = legacy_links(cx);
    if links.is_empty() {
        return Finding::ok("no links into an old standards/bin");
    }
    let details = links
        .iter()
        .map(|(p, t)| format!("{} -> {}", tilde(p, cx.home()), t.display()))
        .collect();
    Finding::fail(format!(
        "{} link(s) into an old standards/bin; the cli has `standards janitor` and `standards ultron`",
        links.len()
    ))
    .fixable(true)
    .details(details)
}

pub fn fix_path_links(cx: &Cx<'_>) -> Result<Vec<String>> {
    legacy_links(cx)
        .into_iter()
        .map(|(path, _)| {
            std::fs::remove_file(&path).map_err(|e| Error::io(&path, e))?;
            Ok(format!("removed {}", tilde(&path, cx.home())))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry() -> SkillEntry {
        SkillEntry {
            name: "thmsn-x".into(),
            description: "Does x. Use when y.".into(),
            description_sha256: "d".into(),
            content_sha256: "c".into(),
            assets: vec![],
        }
    }

    #[test]
    fn stub_is_the_spec_text() {
        assert_eq!(
            stub(&entry()),
            "---\nname: thmsn-x\ndescription: Does x. Use when y.\n---\n\
             This skill's instructions are served by the standards MCP server.\n\
             Call `get_skill(\"thmsn-x\")` on the `standards` MCP server and follow the returned body\n\
             exactly, as this skill's content. If the server is unreachable, say so and stop; don't\n\
             improvise the procedure.\n"
        );
        assert_eq!(marker(&entry()), "content_sha256=c\ndescription_sha256=d\n");
    }

    #[test]
    fn hook_merge_keeps_order_and_never_duplicates() {
        let mut v: Value = serde_json::from_str(
            r#"{"z":1,"hooks":{"Stop":[],"SessionStart":[{"matcher":"resume","hooks":[]}]},"a":2}"#,
        )
        .unwrap();
        assert!(!has_hook(&v));
        merge_hook(&mut v).unwrap();
        assert!(has_hook(&v));
        let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["z", "hooks", "a"]);
        assert_eq!(v["hooks"]["SessionStart"].as_array().unwrap().len(), 2);
        assert_eq!(v["hooks"]["SessionStart"][1], hook_entry());

        for bad in [r"[]", r#"{"hooks":[]}"#, r#"{"hooks":{"SessionStart":{}}}"#] {
            let mut v: Value = serde_json::from_str(bad).unwrap();
            assert!(merge_hook(&mut v).is_err(), "{bad}");
        }
    }
}
