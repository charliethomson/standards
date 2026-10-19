//! The real content of this repo parses, links resolve and templates render cleanly.
//!
//! These read the checkout from disk (core itself never does) and hand the bytes in.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use standards_core::{
    Applicability, Bundle, DocId, Manifest, StandardRef, Target, is_served, render,
};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn walk(dir: &Path, root: &Path, out: &mut Vec<(String, Vec<u8>)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let rel = path
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        if rel == "target" || rel == ".git" || rel.starts_with(".claude") {
            continue;
        }
        if path.is_dir() {
            walk(&path, root, out);
        } else {
            out.push((rel, fs::read(&path).unwrap()));
        }
    }
}

/// Every file in the checkout, served or not: the bundle filters with the allowlist.
static FILES: LazyLock<Vec<(String, Vec<u8>)>> = LazyLock::new(|| {
    let root = repo_root();
    let mut out = Vec::new();
    walk(&root, &root, &mut out);
    out
});

static BUNDLE: LazyLock<Bundle> =
    LazyLock::new(|| Bundle::from_files(FILES.iter().cloned(), "0123456").unwrap());

#[test]
fn every_doc_on_disk_is_a_doc_with_a_title() {
    let on_disk: BTreeSet<String> = FILES
        .iter()
        .filter_map(|(p, _)| DocId::from_path(p))
        .map(|id| id.to_string())
        .collect();
    let parsed: BTreeSet<String> = BUNDLE.docs.iter().map(|d| d.id.to_string()).collect();
    assert_eq!(on_disk, parsed);
    assert!(parsed.len() >= 30, "only {} docs", parsed.len());
    for doc in &BUNDLE.docs {
        assert!(!doc.title.trim().is_empty(), "{}: empty title", doc.id);
    }
    assert_eq!(BUNDLE.doc("versioning").unwrap().title, "Versioning");
}

#[test]
fn every_standard_has_a_checklist() {
    let exempt = ["overview", "glossary", "skills-and-mcp"];
    for doc in &BUNDLE.docs {
        let id = doc.id.as_str();
        if exempt.contains(&id) {
            continue;
        }
        assert!(doc.checklist.is_some(), "{id}: no trailing ## Checklist");
        let checklist = doc.checklist.as_ref().unwrap();
        assert!(!checklist.items.is_empty(), "{id}: empty checklist");
        for item in &checklist.items {
            assert!(!item.text.is_empty(), "{id}: empty item");
            assert!(
                !item.text.contains("      "),
                "{id}: continuation not joined: {}",
                item.text
            );
        }
    }
}

#[test]
fn checklist_continuations_join() {
    let items = &BUNDLE
        .doc("archetypes/cli-tool")
        .unwrap()
        .checklist
        .as_ref()
        .unwrap()
        .items;
    let item = items
        .iter()
        .find(|i| i.text.starts_with("`product_name!"))
        .unwrap();
    assert!(
        item.text
            .ends_with("([`../identifiers.md`](../identifiers.md))."),
        "{}",
        item.text
    );
    assert_eq!(
        item.links[0].target,
        Target::Doc {
            id: DocId::new("identifiers"),
            anchor: None
        }
    );
}

#[test]
fn every_archetype_row_resolves() {
    let names: Vec<&str> = BUNDLE.archetypes.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "cli-tool",
            "full-stack-product",
            "library",
            "orchestration",
            "vendored-app"
        ]
    );
    let mut plain_rows = Vec::new();
    for a in &BUNDLE.archetypes {
        assert!(!a.rows.is_empty(), "{}: no rows", a.name);
        let mut seen = BTreeSet::new();
        for row in &a.rows {
            assert_eq!(
                row.standards.len(),
                1,
                "{}: grouped row {:?}",
                a.name,
                row.standards
            );
            for s in &row.standards {
                match s {
                    StandardRef::Doc(id) => {
                        assert!(
                            BUNDLE.doc(id.as_str()).is_some(),
                            "{}: links missing doc {id}",
                            a.name
                        );
                        assert!(seen.insert(id.clone()), "{}: {id} listed twice", a.name);
                    }
                    StandardRef::Named(n) => plain_rows.push(format!("{}: {n}", a.name)),
                }
            }
        }
    }
    assert_eq!(plain_rows, ["orchestration: Secrets split"]);
}

#[test]
fn archetype_markers_parse() {
    let app = |arch: &str, doc: &str| {
        BUNDLE
            .archetype(arch)
            .unwrap()
            .row(&DocId::new(doc))
            .unwrap()
            .applicability
    };
    assert_eq!(
        app("cli-tool", "registry-publishing"),
        Applicability::Defining
    );
    assert_eq!(app("cli-tool", "deployment"), Applicability::NotApplicable);
    assert_eq!(app("cli-tool", "versioning"), Applicability::Applies);
    assert_eq!(app("library", "versioning"), Applicability::Optional);
    assert_eq!(app("library", "security"), Applicability::NotApplicable);
    assert_eq!(app("vendored-app", "ci-cd"), Applicability::Defining);
    assert_eq!(
        app("vendored-app", "registry-publishing"),
        Applicability::Defining
    );
    assert_eq!(app("vendored-app", "versioning"), Applicability::Adapted);
    assert_eq!(app("vendored-app", "identifiers"), Applicability::Optional);
    let secrets = BUNDLE
        .archetype("orchestration")
        .unwrap()
        .rows
        .iter()
        .find(|r| r.standards == [StandardRef::Named("Secrets split".into())])
        .unwrap();
    assert_eq!(secrets.applicability, Applicability::Defining);
}

#[test]
fn doc_links_resolve_and_rewrite_to_absolute() {
    let linker = BUNDLE.linker();
    let mut forms = BTreeSet::new();
    for doc in &BUNDLE.docs {
        let base = doc.id.path();
        for link in standards_core::Link::all(&base, &doc.markdown) {
            if let Target::Doc { id, .. } = &link.target {
                assert!(
                    BUNDLE.doc(id.as_str()).is_some(),
                    "{}: broken link {}",
                    doc.id,
                    link.href
                );
            }
            assert!(
                !matches!(link.target, Target::Unresolved(_)),
                "{}: unresolvable {}",
                doc.id,
                link.href
            );
            let form = if link.href.starts_with("../templates/") {
                "../templates/"
            } else if link.href.starts_with("../") {
                "../"
            } else if link.href.starts_with("archetypes/") {
                "archetypes/"
            } else if link.href.starts_with('#') {
                "#"
            } else if link.href.contains("://") {
                "external"
            } else {
                "sibling"
            };
            forms.insert(form);
        }
        let rewritten = BUNDLE.doc_markdown(&doc.id).unwrap();
        for link in standards_core::Link::all(&base, &rewritten) {
            assert!(
                matches!(link.target, Target::External(_) | Target::Anchor(_)),
                "{}: left relative after rewrite: {}",
                doc.id,
                link.href
            );
            if let Target::External(url) = &link.target
                && url.starts_with(standards_core::ORIGIN)
            {
                assert!(
                    url.contains("/d/") || url.contains("/t/"),
                    "{}: unexpected server url {url}",
                    doc.id
                );
            }
        }
    }
    for form in ["../templates/", "../", "archetypes/", "#", "sibling"] {
        assert!(
            forms.contains(form),
            "no `{form}` link in the docs any more; {forms:?}"
        );
    }
    assert_eq!(
        linker
            .href(&standards_core::resolve_link(
                "templates/SHORTCUTS.md",
                "standards/docs/shortcuts.md"
            ))
            .unwrap(),
        "https://standards.thmsn.dev/d/shortcuts"
    );
}

#[test]
fn anchored_doc_links_name_real_sections() {
    for doc in &BUNDLE.docs {
        for link in standards_core::Link::all(&doc.id.path(), &doc.markdown) {
            let (target_doc, anchor) = match &link.target {
                Target::Doc {
                    id,
                    anchor: Some(a),
                } => (BUNDLE.doc(id.as_str()).unwrap(), a),
                Target::Anchor(a) => (doc, a),
                _ => continue,
            };
            assert!(
                target_doc.section(anchor).is_some(),
                "{}: {} names no section in {}",
                doc.id,
                link.href,
                target_doc.id
            );
        }
    }
}

#[test]
fn deep_review_asset_keeps_free_text_braces() {
    let skill = BUNDLE.skill("thmsn-deep-review").unwrap();
    let asset = skill.asset("assets/review.template.md").unwrap();
    let body = std::str::from_utf8(&asset.bytes).unwrap();
    let vars = BTreeMap::from([
        ("PRODUCT".to_owned(), "zz-product".to_owned()),
        ("DATE".to_owned(), "2026-10-19".to_owned()),
    ]);
    let r = render(body, &vars);
    assert!(r.body.starts_with("# zz-product code review — 2026-10-19"));
    assert!(
        r.body.contains("{{Executive summary"),
        "free text was touched"
    );
    assert!(r.body.contains("{{Theme}}"));
    assert!(r.unfilled.contains("OVERALL_GRADE"));
    assert!(!r.unfilled.contains("PRODUCT"));
    assert!(
        r.unfilled
            .iter()
            .all(|k| k.chars().all(|c| c.is_ascii_uppercase() || c == '_'))
    );
    // Only the two keys changed: put them back and it is the original.
    assert_eq!(
        r.body
            .replace("zz-product", "{{PRODUCT}}")
            .replace("2026-10-19", "{{DATE}}"),
        body
    );
}

#[test]
fn python_braces_survive_render() {
    let t = BUNDLE.template("branding/build.py").unwrap();
    assert!(
        t.body.contains(":root {{\\n"),
        "fixture moved: no Python f-string braces"
    );
    let vars = BTreeMap::from([("PRODUCT".to_owned(), "Vision".to_owned())]);
    let r = t.render(&vars);
    assert!(r.unfilled.is_empty(), "{:?}", r.unfilled);
    for (before, after) in t.body.lines().zip(r.body.lines()) {
        if !before.contains("{{PRODUCT}}") {
            assert_eq!(before, after);
        }
    }
    assert!(
        r.body
            .contains("apps/windows/src/VisionApp/Tokens.generated.xaml")
    );
}

#[test]
fn templates_expose_their_placeholders() {
    assert!(!BUNDLE.templates.is_empty(), "no templates");
    let agents = BUNDLE.template("consuming-repo-AGENTS.md").unwrap();
    assert!(agents.placeholders.contains("PRODUCT"));
    for t in &BUNDLE.templates {
        assert!(is_served(&format!("templates/{}", t.path)), "{}", t.path);
    }
}

#[test]
fn skills_and_manifest() {
    let readme_is_not_a_skill = BUNDLE.skill("README.md").is_none();
    assert!(readme_is_not_a_skill);
    assert!(BUNDLE.skills.len() >= 5);
    for s in &BUNDLE.skills {
        assert!(s.name.starts_with("thmsn-"), "{}", s.name);
        assert!(!s.description.is_empty(), "{}: empty description", s.name);
        assert_eq!(s.description_sha256.len(), 64);
        assert_eq!(s.content_sha256.len(), 64);
    }
    let m = BUNDLE.manifest(standards_core::VERSION);
    assert_eq!(m.skills.len(), BUNDLE.skills.len());
    assert_eq!(m.archetypes.len(), 5);
    let json = serde_json::to_string(&m).unwrap();
    let back: Manifest = serde_json::from_str(&json).unwrap();
    assert_eq!(back, m);
    let deep = m
        .skills
        .iter()
        .find(|s| s.name == "thmsn-deep-review")
        .unwrap();
    assert!(
        deep.assets
            .contains(&"assets/review.template.md".to_owned())
    );
}

#[test]
fn unserved_files_never_reach_the_bundle() {
    let json = serde_json::to_string(&*BUNDLE).unwrap();
    for p in [
        "mcp/README.md",
        "bin/standards",
        "prompts/install.md",
        "NOTES.md",
        ".mcp.json",
    ] {
        assert!(!is_served(p));
    }
    assert!(
        !json.contains("/Users/"),
        "an absolute local path leaked into the bundle"
    );
    assert!(
        BUNDLE
            .changelog
            .as_deref()
            .unwrap()
            .starts_with("# Changelog")
    );
}
