//! The engine over the content the server actually embeds. Assertions lean on structure
//! (ids, anchors, applicability) rather than prose, so doc rewrites don't break them.

use std::collections::BTreeMap;
use std::sync::{Arc, LazyLock};

use standards_core::{Applicability, VERSION};
use standards_engine::{Engine, Error, HitKind};

static ENGINE: LazyLock<Engine> = LazyLock::new(|| Engine::new(standards_db::bundle().unwrap()));

#[test]
fn search_finds_the_versioning_standard() {
    let hits = ENGINE.search("derived version", 10);
    assert!(!hits.is_empty(), "empty");
    assert_eq!(hits[0].kind, HitKind::Doc);
    assert_eq!(hits[0].doc, "versioning", "{hits:#?}");
    assert!(hits.len() <= 10);
    for hit in &hits {
        assert!(!hit.snippet.is_empty(), "{hit:?}");
        assert!(hit.score > 0.0);
        if let Some(anchor) = &hit.anchor {
            let doc = ENGINE.bundle().doc(&hit.doc).unwrap();
            assert!(doc.section(anchor).is_some(), "{hit:?}");
        }
    }
}

#[test]
fn search_finds_skills_by_name() {
    let hits = ENGINE.search("janitor", 20);
    assert!(
        hits.iter()
            .any(|h| h.kind == HitKind::Skill && h.doc == "thmsn-janitor"),
        "{hits:#?}"
    );
    assert_eq!(ENGINE.search("", 5).len(), 0);
}

#[test]
fn not_applicable_is_a_value_with_the_reason() {
    let c = ENGINE
        .checklist("service-architecture", "cli-tool")
        .unwrap();
    assert_eq!(c.applicability, Applicability::NotApplicable);
    assert!(c.listed);
    assert!(
        c.note.as_deref().is_some_and(|n| n.contains("N/A")),
        "{c:?}"
    );
    assert_eq!(c.standard_items.len(), 0);
}

#[test]
fn applicable_standard_has_items_from_both_docs() {
    let c = ENGINE.checklist("versioning", "library").unwrap();
    assert_ne!(c.applicability, Applicability::NotApplicable);
    assert!(!c.standard_items.is_empty(), "empty");
    assert!(!c.archetype_items.is_empty(), "empty");
    // L25: relative links in items are made absolute.
    for item in c.standard_items.iter().chain(&c.archetype_items) {
        assert!(!item.text.contains("](../"), "{}", item.text);
    }
}

#[test]
fn every_archetype_row_composes() {
    for arch in ENGINE.archetypes() {
        for row in &arch.rows {
            for standard in &row.standards {
                if let standards_core::StandardRef::Doc(id) = standard {
                    let c = ENGINE.checklist(id.as_str(), &arch.name).unwrap();
                    assert_eq!(c.applicability, row.applicability);
                }
            }
        }
    }
}

#[test]
fn docs_and_sections_have_absolute_links() {
    let docs = ENGINE.docs();
    assert!(docs.len() >= 30);
    let v = ENGINE.doc("versioning").unwrap();
    assert!(!v.markdown.contains("](../"));
    let checklist = ENGINE.doc_section("versioning", "checklist").unwrap();
    assert!(checklist.markdown.starts_with("## "));
    assert_eq!(
        ENGINE.doc("no-such-doc").unwrap_err(),
        Error::UnknownDoc {
            id: "no-such-doc".into()
        }
    );
}

#[test]
fn templates_list_and_render_report_unfilled() {
    let list = ENGINE.templates();
    let t = list
        .iter()
        .find(|t| t.placeholders.len() >= 2)
        .expect("a template with two placeholders");
    let first = &t.placeholders[0];
    let vars = BTreeMap::from([(first.clone(), "filled".to_owned())]);
    let r = ENGINE.render_template(&t.path, &vars).unwrap();
    assert!(!r.unfilled.contains(first));
    assert_eq!(r.unfilled.len(), t.placeholders.len() - 1);
    assert!(r.body.contains("filled"));

    let prefixed = format!("templates/{}", t.path);
    assert_eq!(ENGINE.template(&prefixed).unwrap().path, t.path);
    assert!(matches!(
        ENGINE.render_template("nope.md", &vars),
        Err(Error::UnknownTemplate { .. })
    ));
    assert!(matches!(
        ENGINE.template("../bin/standards"),
        Err(Error::UnknownTemplate { .. })
    ));
}

#[test]
fn skills_and_nested_assets() {
    let skills = ENGINE.skills();
    let logs = skills.iter().find(|s| s.name == "thmsn-logs").unwrap();
    assert!(logs.assets.iter().any(|a| a == "scripts/obsq.sh"));

    let asset = ENGINE.skill_asset("thmsn-logs", "scripts/obsq.sh").unwrap();
    assert!(!asset.bytes.is_empty(), "empty");
    assert_eq!(
        ENGINE
            .skill_asset("thmsn-logs", "./scripts//obsq.sh")
            .unwrap()
            .path,
        "scripts/obsq.sh"
    );
    assert!(ENGINE.skill("thmsn-logs").unwrap().body.contains('#'));
    assert_eq!(
        ENGINE.skill_asset("thmsn-logs", "../thmsn-janitor/SKILL.md"),
        Err(Error::UnknownAsset {
            skill: "thmsn-logs".into(),
            path: "../thmsn-janitor/SKILL.md".into()
        })
    );
    assert_eq!(
        ENGINE.skill("nope").unwrap_err(),
        Error::UnknownSkill {
            name: "nope".into()
        }
    );
}

#[test]
fn manifest_matches_the_bundle() {
    let m = ENGINE.manifest(VERSION);
    assert_eq!(m.commit, ENGINE.commit());
    assert_eq!(m.commit, standards_db::commit());
    assert_eq!(m.server_version, VERSION);
    assert_eq!(m.skills.len(), ENGINE.skills().len());
    assert!(m.archetypes.iter().any(|a| a == "cli-tool"));
    assert!(Arc::ptr_eq(
        ENGINE.bundle(),
        &standards_db::bundle().unwrap()
    ));
}
