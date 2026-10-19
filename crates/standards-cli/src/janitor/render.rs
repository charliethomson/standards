//! The human output, line for line as bin/janitor printed it.

use std::io::Write;

use super::clean::{CleanResult, Outcome as StepOutcome};
use super::scan::{Report, Summary, Verdict};

/// ANSI colours, or all empty when stdout isn't a terminal.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub grn: &'static str,
    pub yel: &'static str,
    pub red: &'static str,
    pub dim: &'static str,
    pub bold: &'static str,
    pub off: &'static str,
}

impl Palette {
    #[must_use]
    pub fn new(color: bool) -> Self {
        if color {
            Self {
                grn: "\x1b[32m",
                yel: "\x1b[33m",
                red: "\x1b[31m",
                dim: "\x1b[2m",
                bold: "\x1b[1m",
                off: "\x1b[0m",
            }
        } else {
            Self {
                grn: "",
                yel: "",
                red: "",
                dim: "",
                bold: "",
                off: "",
            }
        }
    }

    fn of(&self, v: &Verdict) -> &'static str {
        match v {
            Verdict::Prune | Verdict::RemoveMerged | Verdict::RemoveKeptOnBranch { .. } => self.grn,
            Verdict::InUse | Verdict::Locked => self.yel,
            Verdict::Dirty { .. } | Verdict::DetachedUnmerged => self.red,
            Verdict::Recent => self.dim,
        }
    }
}

/// KiB, human: `512K`, `1.5M`, `12M`, `3.0G`. Units step at 1024, one decimal under 10.
#[must_use]
pub fn human(kib: u64) -> String {
    const UNITS: [&str; 4] = ["K", "M", "G", "T"];
    #[allow(clippy::cast_precision_loss)]
    let mut k = kib as f64;
    let mut i = 0;
    while k >= 1024.0 && i < 3 {
        k /= 1024.0;
        i += 1;
    }
    if k < 10.0 && i > 0 {
        format!("{k:.1}{}", UNITS[i])
    } else {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let whole = k as u64;
        format!("{whole}{}", UNITS[i])
    }
}

fn row(
    name: &str,
    age: &str,
    size: &str,
    target: &str,
    verdict: &str,
    colour: &str,
    p: &Palette,
) -> String {
    format!(
        "  {name:<34} {age:>5} {size:>7} {target:>7}  {colour}{verdict}{}",
        p.off
    )
}

/// Every shown repo: a header, then a row per linked worktree and main-checkout target.
pub fn report(out: &mut dyn Write, report: &Report, p: &Palette) {
    for repo in report.repos.iter().filter(|r| r.shown()) {
        let _ = writeln!(out, "\n{}{}{}", p.bold, repo.name, p.off);
        let _ = writeln!(
            out,
            "  {}{:<34} {:>5} {:>7} {:>7}  verdict{}",
            p.dim, "worktree", "age", "size", "target", p.off
        );
        for wt in &repo.worktrees {
            let colour = p.of(&wt.verdict);
            let line = match &wt.measured {
                None => row(&wt.short, "-", "-", "-", &wt.verdict_text, colour, p),
                Some(m) => {
                    let target = if m.target_kib > 0 {
                        human(m.target_kib)
                    } else {
                        "-".to_owned()
                    };
                    row(
                        &wt.short,
                        &m.age,
                        &human(m.size_kib),
                        &target,
                        &wt.verdict_text,
                        colour,
                        p,
                    )
                }
            };
            let _ = writeln!(out, "{line}");
        }
        for t in &repo.main_targets {
            let size = human(t.kib);
            let line = row(
                &t.short,
                &format!("{}d", t.idle_days),
                &size,
                &size,
                "clear (main checkout target)",
                p.grn,
                p,
            );
            let _ = writeln!(out, "{line}");
        }
    }
}

/// The closing line: what `clean` would do.
#[must_use]
pub fn summary(s: &Summary, p: &Palette) -> String {
    let targets = match (s.targets, s.clearing_targets) {
        (0, _) => String::new(),
        (n, true) => format!(", {n} target dir(s) to clear ({})", human(s.targets_kib)),
        (_, false) => format!(
            ", plus {} in target dirs of kept worktrees (--targets)",
            human(s.targets_kib)
        ),
    };
    format!(
        "\n{}{} worktree(s) to remove{} ({}), {} gone to prune{targets}",
        p.bold,
        s.remove,
        p.off,
        human(s.remove_kib),
        s.prune
    )
}

/// What `clean` did, one line per step, then the total.
pub fn clean(out: &mut dyn Write, r: &CleanResult, p: &Palette) {
    for step in &r.results {
        let path = step.path.display();
        let _ = match &step.outcome {
            StepOutcome::Removed { deleted_branch } => {
                let _ = writeln!(out, "{}removed{} {path}", p.grn, p.off);
                match deleted_branch {
                    Some(b) => writeln!(out, "        deleted branch {b}"),
                    None => Ok(()),
                }
            }
            StepOutcome::SkippedChanged => writeln!(
                out,
                "{}skipped{} {path} (changed since the scan)",
                p.yel, p.off
            ),
            StepOutcome::SkippedRefused => writeln!(
                out,
                "{}skipped{} {path} (git refused, left as is)",
                p.yel, p.off
            ),
            StepOutcome::Cleared => writeln!(out, "{}cleared{} {path}", p.grn, p.off),
            StepOutcome::Failed | StepOutcome::Pruned => Ok(()),
        };
    }
    let _ = writeln!(out, "\nfreed about {}", human(r.freed_kib));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn human_matches_the_awk() {
        assert_eq!(human(0), "0K");
        assert_eq!(human(1023), "1023K");
        assert_eq!(human(1024), "1.0M");
        assert_eq!(human(1536), "1.5M");
        assert_eq!(human(10 * 1024), "10M");
        assert_eq!(human(10 * 1024 - 1), "10.0M");
        assert_eq!(human(1024 * 1024 * 3), "3.0G");
        assert_eq!(human(1024u64.pow(3) * 2), "2.0T");
        assert_eq!(human(1024u64.pow(4) * 2), "2048T");
    }

    #[test]
    fn colours_on_and_off() {
        let on = Palette::new(true);
        assert_eq!(on.of(&Verdict::Locked), "\x1b[33m");
        assert_eq!(on.of(&Verdict::DetachedUnmerged), "\x1b[31m");
        assert_eq!(on.of(&Verdict::Recent), "\x1b[2m");
        assert_eq!(on.of(&Verdict::Prune), "\x1b[32m");
        let off = Palette::new(false);
        assert_eq!(off.of(&Verdict::InUse), "");
        assert_eq!(
            row("a", "1d", "2K", "-", "keep: locked", off.yel, &off),
            format!(
                "  {:<34} {:>5} {:>7} {:>7}  keep: locked",
                "a", "1d", "2K", "-"
            )
        );
    }

    #[test]
    fn summary_wording() {
        let p = Palette::new(false);
        let mut s = Summary {
            remove: 2,
            remove_kib: 2048,
            prune: 1,
            targets: 0,
            targets_kib: 0,
            clearing_targets: false,
        };
        assert_eq!(
            summary(&s, &p),
            "\n2 worktree(s) to remove (2.0M), 1 gone to prune"
        );
        s.targets = 3;
        s.targets_kib = 100;
        assert!(
            summary(&s, &p).ends_with(", plus 100K in target dirs of kept worktrees (--targets)")
        );
        s.clearing_targets = true;
        assert!(summary(&s, &p).ends_with(", 3 target dir(s) to clear (100K)"));
    }
}
