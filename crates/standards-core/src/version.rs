//! Derived versions, `MAJOR.MINOR.<commit count>` (docs/versioning.md), compared
//! numerically so `0.10.x` sorts after `0.9.x`.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DerivedVersion {
    pub major: u64,
    pub minor: u64,
    /// The commit count.
    pub count: u64,
}

impl FromStr for DerivedVersion {
    type Err = String;

    /// `1.2.3` or `v1.2.3`. Anything after a `-` or `+` (a pre-release or build suffix)
    /// is ignored.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let core = s.trim().trim_start_matches('v');
        let core = core.split(['-', '+']).next().unwrap_or(core);
        let parts: Vec<&str> = core.split('.').collect();
        let [major, minor, count] = parts.as_slice() else {
            return Err(format!("`{s}` is not MAJOR.MINOR.COUNT"));
        };
        let n = |p: &str| {
            p.parse::<u64>()
                .map_err(|_| format!("`{s}` is not MAJOR.MINOR.COUNT"))
        };
        Ok(Self {
            major: n(major)?,
            minor: n(minor)?,
            count: n(count)?,
        })
    }
}

impl fmt::Display for DerivedVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> DerivedVersion {
        s.parse().unwrap()
    }

    #[test]
    fn parses_and_orders_numerically() {
        assert_eq!(
            v("0.1.300"),
            DerivedVersion {
                major: 0,
                minor: 1,
                count: 300
            }
        );
        assert_eq!(v("v1.2.3-dirty").to_string(), "1.2.3");
        assert!(v("0.10.1") > v("0.9.500"));
        assert!(v("0.1.301") > v("0.1.300"));
        assert!(v(crate::VERSION) >= v(crate::MIN_CLI_VERSION));
        assert!("1.2".parse::<DerivedVersion>().is_err());
        assert!("1.x.3".parse::<DerivedVersion>().is_err());
    }
}
