//! `.standards.conf`: a consuming repo's identity, `KEY=VALUE` lines (L14).
//!
//! Parsed, never sourced: no expansion, no command substitution, no `export`.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// A repo's product identity for the standards tooling.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StandardsConf {
    /// `PRODUCT`: the slug (`vision`). Required.
    pub product: String,
    /// `PRODUCT_UPPER`: the env prefix stem. Defaults to [`StandardsConf::default_upper`].
    pub product_upper: String,
    /// `LIB`: the product's library crate, when it has one. Empty means none.
    pub lib: Option<String>,
    /// `ARCHETYPE`. Older files predate it, so it may be absent.
    pub archetype: Option<String>,
}

impl StandardsConf {
    /// Parse the file's text. Blank lines and `#` comments are skipped; a value may be
    /// wrapped in matching single or double quotes. Unknown keys are ignored.
    pub fn parse(text: &str) -> Result<Self> {
        let mut values: BTreeMap<String, String> = BTreeMap::new();
        for (i, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                return Err(Error::Conf {
                    line: i + 1,
                    reason: "expected KEY=VALUE".into(),
                });
            };
            let key = key.trim();
            if key.is_empty()
                || !key
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
            {
                return Err(Error::Conf {
                    line: i + 1,
                    reason: format!("`{key}` is not a KEY"),
                });
            }
            values.insert(key.to_owned(), unquote(value.trim()).to_owned());
        }
        let mut take = |k: &str| values.remove(k).filter(|v| !v.is_empty());
        let product = take("PRODUCT").ok_or_else(|| Error::ConfMissing {
            key: "PRODUCT".into(),
        })?;
        let product_upper = take("PRODUCT_UPPER").unwrap_or_else(|| Self::default_upper(&product));
        Ok(Self {
            lib: take("LIB"),
            archetype: take("ARCHETYPE"),
            product,
            product_upper,
        })
    }

    /// `PRODUCT` upper-cased with `-` turned into `_` (`my-app` → `MY_APP`).
    #[must_use]
    pub fn default_upper(product: &str) -> String {
        product.to_uppercase().replace('-', "_")
    }

    /// The file's text, in the order `standards init` writes it.
    #[must_use]
    pub fn to_conf_string(&self) -> String {
        let mut out = String::from("# Product identity for the standards tooling.\n");
        let _ = writeln!(out, "PRODUCT={}", self.product);
        let _ = writeln!(out, "PRODUCT_UPPER={}", self.product_upper);
        let _ = writeln!(out, "LIB={}", self.lib.as_deref().unwrap_or_default());
        let _ = writeln!(
            out,
            "ARCHETYPE={}",
            self.archetype.as_deref().unwrap_or_default()
        );
        out
    }

    /// Template variables this identity fills: `PRODUCT`, `PRODUCT_UPPER`, and `LIB` /
    /// `ARCHETYPE` when set.
    #[must_use]
    pub fn vars(&self) -> BTreeMap<String, String> {
        let mut vars = BTreeMap::from([
            ("PRODUCT".to_owned(), self.product.clone()),
            ("PRODUCT_UPPER".to_owned(), self.product_upper.clone()),
        ]);
        if let Some(lib) = &self.lib {
            vars.insert("LIB".to_owned(), lib.clone());
        }
        if let Some(a) = &self.archetype {
            vars.insert("ARCHETYPE".to_owned(), a.clone());
        }
        vars
    }
}

fn unquote(v: &str) -> &str {
    for q in ['"', '\''] {
        if let Some(inner) = v.strip_prefix(q).and_then(|s| s.strip_suffix(q)) {
            return inner;
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_install_output() {
        let text = "# Product identity for the standards tooling (genericizer + lint).\n\
                    PRODUCT=vision\nPRODUCT_UPPER=VISION\nLIB=\nARCHETYPE=full-stack-product\n";
        let c = StandardsConf::parse(text).unwrap();
        assert_eq!(c.product, "vision");
        assert_eq!(c.product_upper, "VISION");
        assert_eq!(c.lib, None);
        assert_eq!(c.archetype.as_deref(), Some("full-stack-product"));
        assert_eq!(StandardsConf::parse(&c.to_conf_string()).unwrap(), c);
    }

    #[test]
    fn defaults_and_quotes() {
        let c = StandardsConf::parse("PRODUCT='my-app'\n\n  LIB = \"libmy\"  \nOTHER=x\n").unwrap();
        assert_eq!(c.product_upper, "MY_APP");
        assert_eq!(c.lib.as_deref(), Some("libmy"));
        assert_eq!(c.archetype, None);
        let vars = c.vars();
        assert_eq!(vars["PRODUCT"], "my-app");
        assert_eq!(vars["PRODUCT_UPPER"], "MY_APP");
        assert_eq!(vars["LIB"], "libmy");
        assert!(!vars.contains_key("ARCHETYPE"));
    }

    #[test]
    fn never_sourced() {
        let c = StandardsConf::parse("PRODUCT=$(rm -rf ~)\nARCHETYPE=${X:-y}\n").unwrap();
        assert_eq!(c.product, "$(rm -rf ~)");
        assert_eq!(c.archetype.as_deref(), Some("${X:-y}"));
        assert_eq!(c.vars()["ARCHETYPE"], "${X:-y}");
    }

    #[test]
    fn errors() {
        let e = |t: &str| StandardsConf::parse(t).unwrap_err().to_string();
        assert_eq!(e("LIB=x\n"), ".standards.conf: PRODUCT missing");
        assert_eq!(e("PRODUCT=\n"), ".standards.conf: PRODUCT missing");
        assert_eq!(
            e("PRODUCT=x\njunk\n"),
            ".standards.conf line 2: expected KEY=VALUE"
        );
        assert_eq!(
            e("export PRODUCT=x\n"),
            ".standards.conf line 1: `export PRODUCT` is not a KEY"
        );
        assert_eq!(e("=x\n"), ".standards.conf line 1: `` is not a KEY");
    }
}
