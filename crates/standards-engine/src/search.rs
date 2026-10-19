//! An in-memory full-text index over the bundle.
//!
//! Units are what a hit can point at: a doc's title and intro, each section's own text
//! (up to the next heading of any level), each checklist item, and each skill's name and
//! description. Scoring is BM25 (a long section that mentions a term often doesn't drown a
//! short one about it) over a weighted term frequency: terms in a unit's heading count
//! more, and sections carry their doc's title as context, so a query naming a standard's
//! topic prefers that standard. A coverage factor ranks units matching every query term
//! above units matching one, and a verbatim match of the whole query earns a bonus.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;
use standards_core::{Bundle, Doc};

/// What a [`Hit`] points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HitKind {
    Doc,
    Skill,
}

/// One search result.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Hit {
    pub kind: HitKind,
    /// The doc id for [`HitKind::Doc`]; the skill name for [`HitKind::Skill`].
    pub doc: String,
    /// The section anchor within the doc; `None` for the doc as a whole or a skill.
    pub anchor: Option<String>,
    /// `Doc title`, `Doc title › Section`, or the skill name.
    pub title: String,
    /// Plain text around the first match.
    pub snippet: String,
    /// Higher is better; only comparable within one query.
    pub score: f32,
}

/// Weight of a term found in a unit's title relative to one found in its body.
const TITLE_WEIGHT: f32 = 3.0;
/// Weight of a doc's own title on its title unit, so `versioning` finds the doc first.
const DOC_TITLE_WEIGHT: f32 = 4.0;
/// Weight of the doc title's terms on each of its sections and checklist items.
const CONTEXT_WEIGHT: f32 = 1.5;
/// BM25 term-frequency saturation and length normalization.
const K1: f32 = 1.2;
const B: f32 = 0.75;
/// Multiplier when the query appears verbatim in the unit.
const PHRASE_BONUS: f32 = 1.5;
/// Snippet window: bytes kept before and after the first match.
const BEFORE: usize = 60;
const AFTER: usize = 160;

struct Unit {
    kind: HitKind,
    target: String,
    anchor: Option<String>,
    title: String,
    /// Cleaned, whitespace-collapsed text for snippets and phrase matching.
    text: String,
    title_terms: HashMap<String, u32>,
    context_terms: HashMap<String, u32>,
    body_terms: HashMap<String, u32>,
    title_weight: f32,
    /// Body length in terms, for BM25's length normalization.
    len: f32,
}

/// The text a unit is indexed from.
struct Source<'a> {
    heading: &'a str,
    /// The doc title, for a section or checklist item; empty otherwise.
    context: &'a str,
    body: &'a str,
    title_weight: f32,
}

impl Unit {
    fn new(kind: HitKind, target: &str, anchor: Option<&str>, title: String, src: &Source) -> Self {
        let text = clean(src.body);
        let body_terms = counts(&text);
        #[allow(clippy::cast_precision_loss)] // term counts are small
        let len = body_terms.values().sum::<u32>() as f32;
        Self {
            kind,
            target: target.to_owned(),
            anchor: anchor.map(str::to_owned),
            title_terms: counts(src.heading),
            context_terms: counts(src.context),
            body_terms,
            title,
            text,
            title_weight: src.title_weight,
            len,
        }
    }

    fn terms(&self) -> impl Iterator<Item = &String> {
        self.title_terms
            .keys()
            .chain(self.context_terms.keys())
            .chain(self.body_terms.keys())
    }

    /// Weighted term frequency.
    fn tf(&self, term: &str) -> f32 {
        #[allow(clippy::cast_precision_loss)] // term counts are small
        let tf = |m: &HashMap<String, u32>| m.get(term).copied().unwrap_or(0) as f32;
        self.title_weight * tf(&self.title_terms)
            + CONTEXT_WEIGHT * tf(&self.context_terms)
            + tf(&self.body_terms)
    }
}

/// The search index over one bundle.
pub struct Index {
    units: Vec<Unit>,
    /// Units containing each term.
    df: HashMap<String, usize>,
    /// Mean body length in terms.
    avg_len: f32,
}

impl Index {
    #[must_use]
    pub fn build(bundle: &Bundle) -> Self {
        let mut units = Vec::new();
        for doc in &bundle.docs {
            doc_units(doc, &mut units);
        }
        for skill in &bundle.skills {
            units.push(Unit::new(
                HitKind::Skill,
                &skill.name,
                None,
                skill.name.clone(),
                &Source {
                    heading: &skill.name,
                    context: "",
                    body: &skill.description,
                    title_weight: TITLE_WEIGHT,
                },
            ));
        }
        let mut df: HashMap<String, usize> = HashMap::new();
        for unit in &units {
            let terms: HashSet<&String> = unit.terms().collect();
            for term in terms {
                *df.entry(term.clone()).or_default() += 1;
            }
        }
        #[allow(clippy::cast_precision_loss)] // unit counts are small
        let avg_len = units.iter().map(|u| u.len).sum::<f32>() / units.len().max(1) as f32;
        Self {
            units,
            df,
            avg_len: avg_len.max(1.0),
        }
    }

    /// The best `limit` hits for `query`, at most one per (doc, anchor).
    #[must_use]
    pub fn search(&self, query: &str, limit: usize) -> Vec<Hit> {
        let mut terms: Vec<String> = tokens(query).collect();
        terms.sort();
        terms.dedup();
        if terms.is_empty() || limit == 0 {
            return Vec::new();
        }
        let phrase = collapse(&query.to_ascii_lowercase());
        #[allow(clippy::cast_precision_loss)] // unit and term counts are small
        let (n, total) = (self.units.len() as f32, terms.len() as f32);

        let mut scored: Vec<(f32, &Unit)> = self
            .units
            .iter()
            .filter_map(|unit| {
                let mut score = 0.0;
                let mut matched = 0.0;
                let norm = K1 * (1.0 - B + B * unit.len / self.avg_len);
                for term in &terms {
                    let tf = unit.tf(term);
                    if tf > 0.0 {
                        #[allow(clippy::cast_precision_loss)]
                        let df = self.df.get(term).copied().unwrap_or(1) as f32;
                        let idf = (1.0 + (n - df + 0.5) / (df + 0.5)).ln();
                        score += idf * tf * (K1 + 1.0) / (tf + norm);
                        matched += 1.0;
                    }
                }
                if matched == 0.0 {
                    return None;
                }
                let coverage = matched / total;
                score *= coverage * coverage;
                if unit.text.to_ascii_lowercase().contains(&phrase)
                    || unit.title.to_ascii_lowercase().contains(&phrase)
                {
                    score *= PHRASE_BONUS;
                }
                Some((score, unit))
            })
            .collect();
        scored.sort_by(|a, b| {
            b.0.total_cmp(&a.0)
                .then_with(|| a.1.target.cmp(&b.1.target))
                .then_with(|| a.1.anchor.cmp(&b.1.anchor))
        });

        let mut seen = HashSet::new();
        scored
            .into_iter()
            .filter(|(_, u)| seen.insert((u.kind, u.target.as_str(), u.anchor.as_deref())))
            .take(limit)
            .map(|(score, u)| Hit {
                kind: u.kind,
                doc: u.target.clone(),
                anchor: u.anchor.clone(),
                title: u.title.clone(),
                snippet: snippet(&u.text, &phrase, &terms),
                score,
            })
            .collect()
    }

    /// Whether any unit contains this (already normalized) term. For tests.
    #[cfg(test)]
    fn knows(&self, term: &str) -> bool {
        self.units.iter().any(|u| u.terms().any(|t| t == term))
    }
}

fn doc_units(doc: &Doc, out: &mut Vec<Unit>) {
    let id = doc.id.as_str();
    let md = doc.markdown.as_str();
    let intro_end = doc.sections.first().map_or(md.len(), |s| s.range.start);
    let intro: String = md[..intro_end]
        .lines()
        .filter(|l| !l.starts_with("# "))
        .collect::<Vec<_>>()
        .join("\n");
    out.push(Unit::new(
        HitKind::Doc,
        id,
        None,
        doc.title.clone(),
        &Source {
            heading: &doc.title,
            context: "",
            body: &intro,
            title_weight: DOC_TITLE_WEIGHT,
        },
    ));

    let checklist_anchor = doc
        .checklist
        .as_ref()
        .and_then(|_| doc.sections.iter().rev().find(|s| s.level == 2))
        .map(|s| s.anchor.as_str());
    for (i, section) in doc.sections.iter().enumerate() {
        let title = format!("{} › {}", doc.title, section.title);
        if Some(section.anchor.as_str()) == checklist_anchor {
            for item in doc.checklist.iter().flat_map(|c| &c.items) {
                out.push(Unit::new(
                    HitKind::Doc,
                    id,
                    Some(&section.anchor),
                    title.clone(),
                    &Source {
                        heading: "",
                        context: &doc.title,
                        body: &item.text,
                        title_weight: TITLE_WEIGHT,
                    },
                ));
            }
            continue;
        }
        // The section's own text: up to the next heading of any level.
        let end = doc
            .sections
            .get(i + 1)
            .map_or(section.range.end, |next| next.range.start)
            .min(section.range.end);
        let own = &md[section.range.start..end];
        let body = own.split_once('\n').map_or("", |(_, rest)| rest);
        out.push(Unit::new(
            HitKind::Doc,
            id,
            Some(&section.anchor),
            title,
            &Source {
                heading: &section.title,
                context: &doc.title,
                body,
                title_weight: TITLE_WEIGHT,
            },
        ));
    }
}

static LINK: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)] // a literal pattern; covered by every test
    Regex::new(r"!?\[([^\]]*)\]\([^)]*\)").unwrap()
});

/// Markdown to plain-ish text: links keep their text, emphasis markers go, whitespace
/// collapses to single spaces.
fn clean(markdown: &str) -> String {
    let text = LINK.replace_all(markdown, "$1");
    collapse(&text.replace("**", ""))
}

fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

const STOPWORDS: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "by", "for", "from", "if", "in", "into", "is", "it",
    "its", "no", "not", "of", "on", "or", "so", "that", "the", "this", "to", "with",
];

/// Lowercased, stemmed words of two or more characters, minus stopwords.
fn tokens(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() >= 2)
        .map(str::to_lowercase)
        .filter(|w| !STOPWORDS.contains(&w.as_str()))
        .map(|w| stem(&w))
}

/// A deliberately light stemmer: `versioning`, `versions` → `version`; `derived`,
/// `derive` → `deriv`; `services`, `service` → `servic`. The stem is a prefix of the word,
/// which the snippet finder relies on.
fn stem(word: &str) -> String {
    let mut w = word;
    for suffix in ["ing", "ed", "es", "s"] {
        if let Some(rest) = w.strip_suffix(suffix)
            && rest.chars().count() >= 4
        {
            w = rest;
            break;
        }
    }
    if let Some(rest) = w.strip_suffix('e')
        && rest.chars().count() >= 4
    {
        w = rest;
    }
    w.to_owned()
}

fn counts(text: &str) -> HashMap<String, u32> {
    let mut out = HashMap::new();
    for t in tokens(text) {
        *out.entry(t).or_default() += 1;
    }
    out
}

/// A window of `text` around the verbatim query if present, else the earliest term.
fn snippet(text: &str, phrase: &str, terms: &[String]) -> String {
    // ASCII lowercasing keeps byte offsets aligned with `text`.
    let lower = text.to_ascii_lowercase();
    let at = lower
        .find(phrase)
        .or_else(|| terms.iter().filter_map(|t| lower.find(t.as_str())).min());
    let (start, end) = match at {
        Some(at) => (at.saturating_sub(BEFORE), at + AFTER),
        None => (0, BEFORE + AFTER),
    };
    let mut start = floor_boundary(text, start);
    let end = floor_boundary(text, end.min(text.len()));
    // Don't cut a word in half at either end.
    if start > 0 {
        start = text[start..end].find(' ').map_or(start, |i| start + i + 1);
    }
    let end = if end < text.len() {
        text[start..end].rfind(' ').map_or(end, |i| start + i)
    } else {
        end
    };
    let mut out = String::new();
    if start > 0 {
        out.push('…');
    }
    out.push_str(&text[start..end]);
    if end < text.len() {
        out.push('…');
    }
    out
}

fn floor_boundary(text: &str, mut at: usize) -> usize {
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundle(files: &[(&str, &str)]) -> Bundle {
        Bundle::from_files(
            files.iter().map(|(p, b)| (*p, b.as_bytes().to_vec())),
            "c0ffee",
        )
        .unwrap()
    }

    #[test]
    fn stems() {
        assert_eq!(stem("versioning"), "version");
        assert_eq!(stem("versions"), "version");
        assert_eq!(stem("version"), "version");
        assert_eq!(stem("derived"), "deriv");
        assert_eq!(stem("derive"), "deriv");
        assert_eq!(stem("services"), "servic");
        assert_eq!(stem("service"), "servic");
        assert_eq!(stem("uses"), "uses");
        assert_eq!(stem("rules"), "rule");
    }

    #[test]
    fn tokens_drop_stopwords_and_short_words() {
        let t: Vec<String> = tokens("The `lib*` crates & a CI of x").collect();
        assert_eq!(t, ["lib", "crat", "ci"]);
    }

    #[test]
    fn clean_keeps_link_text() {
        assert_eq!(
            clean("See **[the doc](x.md#a)** and\n![img](p.png)  now"),
            "See the doc and img now"
        );
    }

    #[test]
    fn snippets_window_and_ellipses() {
        let text = collapse(&format!(
            "{} needle {}",
            "word ".repeat(40),
            "tail ".repeat(60)
        ));
        let s = snippet(&text, "needle", &["needle".into()]);
        assert!(s.starts_with('…') && s.ends_with('…'), "{s}");
        assert!(s.contains("needle"));
        assert!(!s.contains("  "));

        assert_eq!(snippet("short text", "zzz", &["zzz".into()]), "short text");
        assert_eq!(snippet("ab needle", "needle", &[]), "ab needle");
        let long = "x ".repeat(200);
        assert!(snippet(&long, "q", &[]).ends_with('…'));
        // Multi-byte text never splits a char.
        let uni = "↔".repeat(100);
        assert!(snippet(&uni, "q", &[]).ends_with('…'));
    }

    #[test]
    fn ranks_title_and_full_coverage_first() {
        let b = bundle(&[
            (
                "docs/alpha.md",
                "# Alpha\n\nIntro about widgets.\n\n## Widget rules\n\nWidgets are blue.\n\n### Deep\n\nnested widget text\n\n## Other\n\nnothing\n\n## Checklist\n\n- [ ] Widgets blue.\n- [ ] Gadget green.\n",
            ),
            ("docs/beta.md", "# Beta\n\nA widget once, and gadgets.\n"),
            (
                "skills/s-widget/SKILL.md",
                "---\nname: s-widget\ndescription: Makes widgets.\n---\nbody",
            ),
        ]);
        let idx = Index::build(&b);
        assert!(idx.knows("widget"));
        assert!(!idx.knows("absent"));

        let hits = idx.search("widget rules", 10);
        assert_eq!(hits[0].doc, "alpha");
        assert_eq!(hits[0].anchor.as_deref(), Some("widget-rules"));
        assert_eq!(hits[0].title, "Alpha › Widget rules");
        assert!(hits.windows(2).all(|w| w[0].score >= w[1].score));

        // One hit per (doc, anchor): two matching checklist items collapse to one.
        let hits = idx.search("blue", 10);
        let checklist: Vec<_> = hits
            .iter()
            .filter(|h| h.anchor.as_deref() == Some("checklist"))
            .collect();
        assert_eq!(checklist.len(), 1);

        let hits = idx.search("widget", 2);
        assert_eq!(hits.len(), 2);
        let skill = idx
            .search("widget", 20)
            .into_iter()
            .find(|h| h.kind == HitKind::Skill)
            .unwrap();
        assert_eq!(skill.doc, "s-widget");
        assert_eq!(skill.anchor, None);
        assert_eq!(skill.snippet, "Makes widgets.");

        assert_eq!(idx.search("widget", 0).len(), 0);
        assert_eq!(idx.search("the of", 5).len(), 0);
        assert_eq!(idx.search("zzzz", 5).len(), 0);
    }

    #[test]
    fn hits_serialize_snake_case() {
        let json = serde_json::to_value(HitKind::Skill).unwrap();
        assert_eq!(json, "skill");
    }
}
