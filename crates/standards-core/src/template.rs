//! Templates: `templates/**`, with strict `{{KEY}}` placeholders (L8).
//!
//! A placeholder is exactly `{{` + `[A-Z_]+` + `}}`. Anything else in braces is content:
//! the deep-review asset's free-text `{{Executive summary — ...}}`, Python's `{{` in an
//! f-string, `{{ .Values }}`. Rendering replaces only the keys it is given and reports the
//! placeholders it left in place.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use regex::{Captures, Regex};
use serde::Serialize;

static PLACEHOLDER: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)] // a literal pattern; covered by every test
    Regex::new(r"\{\{([A-Z_]+)\}\}").unwrap()
});

/// A file under `templates/`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Template {
    /// Relative to `templates/` (`rust/build.rs`).
    pub path: String,
    pub body: String,
    /// Every `{{KEY}}` in `body`.
    pub placeholders: BTreeSet<String>,
}

/// A rendered body and the placeholders that had no value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Rendered {
    pub body: String,
    pub unfilled: BTreeSet<String>,
}

impl Template {
    #[must_use]
    pub fn new(path: String, body: String) -> Self {
        let placeholders = placeholders(&body);
        Self {
            path,
            body,
            placeholders,
        }
    }

    #[must_use]
    pub fn render(&self, vars: &BTreeMap<String, String>) -> Rendered {
        render(&self.body, vars)
    }
}

/// Every `{{KEY}}` in `body`.
#[must_use]
pub fn placeholders(body: &str) -> BTreeSet<String> {
    PLACEHOLDER
        .captures_iter(body)
        .map(|c| c[1].to_owned())
        .collect()
}

/// Replace each `{{KEY}}` whose key is in `vars`; leave every other byte alone.
#[must_use]
pub fn render(body: &str, vars: &BTreeMap<String, String>) -> Rendered {
    let mut unfilled = BTreeSet::new();
    let body = PLACEHOLDER
        .replace_all(body, |c: &Captures| {
            if let Some(v) = vars.get(&c[1]) {
                v.clone()
            } else {
                unfilled.insert(c[1].to_owned());
                c[0].to_owned()
            }
        })
        .into_owned();
    Rendered { body, unfilled }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn strict_placeholders() {
        let body =
            "{{PRODUCT}} {{PRODUCT_UPPER}}_X {{ PRODUCT }} {{Product}} {{A-B}} {{}} {{{BIN}}}";
        assert_eq!(
            placeholders(body),
            ["BIN", "PRODUCT", "PRODUCT_UPPER"].map(String::from).into()
        );
    }

    #[test]
    fn renders_only_given_keys() {
        let t = Template::new(
            "x".into(),
            "{{PRODUCT}}/{{BIN}} {{Free text}} f\"{{\\n{x}}}\"".into(),
        );
        let r = t.render(&vars(&[("PRODUCT", "vision"), ("UNUSED", "u")]));
        assert_eq!(r.body, "vision/{{BIN}} {{Free text}} f\"{{\\n{x}}}\"");
        assert_eq!(r.unfilled, ["BIN".to_owned()].into());
        let r = t.render(&vars(&[("PRODUCT", "a"), ("BIN", "b")]));
        assert!(r.unfilled.is_empty());
        assert_eq!(r.body, "a/b {{Free text}} f\"{{\\n{x}}}\"");
    }

    #[test]
    fn values_are_not_re_expanded() {
        let r = render("{{A}}", &vars(&[("A", "{{B}}"), ("B", "no")]));
        assert_eq!(r.body, "{{B}}");
        assert!(r.unfilled.is_empty());
    }
}
