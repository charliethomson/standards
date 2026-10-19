//! `ls` and `status`: gather a program's state into a serializable summary (the `--json`
//! shape), then render it exactly as `bin/ultron` prints it.

use std::fs;
use std::io::Write;
use std::path::Path;

use serde::Serialize;

use super::backlog::{self, Task, TaskState};
use super::state::{StateDir, ago, mtime, newest};

/// The script's ANSI colours, all empty when stdout is not a terminal.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Style {
    pub grn: &'static str,
    pub yel: &'static str,
    pub dim: &'static str,
    pub bold: &'static str,
    pub off: &'static str,
}

impl Style {
    pub(crate) fn new(colour: bool) -> Self {
        if colour {
            Self {
                grn: "\x1b[32m",
                yel: "\x1b[33m",
                dim: "\x1b[2m",
                bold: "\x1b[1m",
                off: "\x1b[0m",
            }
        } else {
            Self {
                grn: "",
                yel: "",
                dim: "",
                bold: "",
                off: "",
            }
        }
    }
}

/// One `ultron ls` row.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ProgramSummary {
    pub slug: String,
    pub tasks: usize,
    pub landed: usize,
    pub active: usize,
    pub blocked: usize,
    pub gated: usize,
    /// Newest mtime under the program, epoch seconds (0: no files).
    pub last_activity: i64,
    pub last: String,
}

fn read(path: &str) -> Option<Vec<u8>> {
    fs::read(path).ok()
}

/// The tasks in `<dir>/PROGRAM.md` (none without one).
pub(crate) fn tasks_in(dir: &str) -> Vec<Task> {
    read(&format!("{dir}/PROGRAM.md"))
        .map(|b| backlog::parse(&b))
        .unwrap_or_default()
}

pub(crate) fn summarize(state: &StateDir, now: i64) -> Vec<ProgramSummary> {
    state
        .list_programs()
        .into_iter()
        .map(|slug| {
            let dir = state.path_of(&slug);
            let tasks = tasks_in(&dir);
            let count = |f: &dyn Fn(&Task) -> bool| tasks.iter().filter(|t| f(t)).count();
            let last_activity = newest(&dir);
            ProgramSummary {
                tasks: tasks.len(),
                landed: count(&|t| t.state == TaskState::Landed),
                active: count(&|t| t.state == TaskState::Dispatched),
                blocked: count(&|t| t.state == TaskState::Blocked),
                gated: count(&|t| t.gated),
                last: ago(last_activity, now),
                last_activity,
                slug,
            }
        })
        .collect()
}

// The header is the script's printf with its literal arguments, kept as is.
#[allow(clippy::write_literal)]
pub(crate) fn render_ls(
    out: &mut dyn Write,
    state: &StateDir,
    rows: &[ProgramSummary],
    s: Style,
) -> std::io::Result<()> {
    if rows.is_empty() {
        return writeln!(out, "{}no programs under {}{}", s.dim, state.root(), s.off);
    }
    writeln!(
        out,
        "{}{:<24} {:>6} {:>7} {:>7} {:>8} {:>6}  {}{}",
        s.bold, "PROGRAM", "TASKS", "LANDED", "ACTIVE", "BLOCKED", "GATED", "LAST", s.off
    )?;
    for r in rows {
        let warn = |n: usize| if n > 0 { s.yel } else { "" };
        writeln!(
            out,
            "{:<24} {:>6} {:>7} {:>7} {}{:>8}{} {}{:>6}{}  {}{}{}",
            r.slug,
            r.tasks,
            r.landed,
            r.active,
            warn(r.blocked),
            r.blocked,
            s.off,
            warn(r.gated),
            r.gated,
            s.off,
            s.dim,
            r.last,
            s.off
        )?;
    }
    Ok(())
}

/// One `ultron status` row.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct TaskRow {
    pub id: String,
    pub state: TaskState,
    pub gated: bool,
    /// PLAN checkboxes ticked / total, when the journal has any.
    pub plan: Option<PlanProgress>,
    /// `<dir>/<id>.md`, when it exists.
    pub journal: Option<String>,
    pub journal_mtime: Option<i64>,
    /// The journal's age (`5m ago`), or `-` without one.
    pub journal_age: String,
    /// The first line of the journal's STATE section, cut to 60 bytes.
    pub now: String,
    #[serde(skip)]
    now_bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub(crate) struct PlanProgress {
    pub done: usize,
    pub total: usize,
}

/// Everything `ultron status <slug>` shows.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ProgramStatus {
    pub slug: String,
    pub dir: String,
    pub has_brief: bool,
    pub tasks: Vec<TaskRow>,
    /// Journals (`*.md` other than PROGRAM.md) with no backlog entry.
    pub orphans: Vec<String>,
    /// Tasks the backlog calls queued that already have a journal: the ledger is stale.
    pub stale_queued: Vec<String>,
    /// Not landed, and blocked or human-gated.
    pub waiting: Vec<String>,
}

fn is_file(path: &str) -> bool {
    Path::new(path).is_file()
}

// `"$dir"/*.md` is a case-sensitive glob.
#[allow(clippy::case_sensitive_file_extension_comparisons)]
pub(crate) fn program_status(slug: &str, dir: &str, now: i64) -> ProgramStatus {
    let tasks = tasks_in(dir);
    let rows: Vec<TaskRow> = tasks
        .iter()
        .map(|t| {
            let journal = format!("{dir}/{}.md", t.id);
            let body = if is_file(&journal) {
                read(&journal)
            } else {
                None
            };
            let exists = is_file(&journal);
            let journal_mtime = exists.then(|| mtime(&journal));
            let now_bytes = body.as_deref().map(backlog::state_line).unwrap_or_default();
            TaskRow {
                id: t.id.clone(),
                state: t.state,
                gated: t.gated,
                plan: body
                    .as_deref()
                    .and_then(backlog::plan_progress)
                    .map(|(done, total)| PlanProgress { done, total }),
                journal_age: journal_mtime.map_or_else(|| "-".into(), |m| ago(m, now)),
                journal: exists.then_some(journal),
                journal_mtime,
                now: String::from_utf8_lossy(&now_bytes).into_owned(),
                now_bytes,
            }
        })
        .collect();

    let mut md: Vec<String> = fs::read_dir(dir)
        .map(|it| {
            it.filter_map(std::result::Result::ok)
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| !n.starts_with('.') && n.ends_with(".md"))
                .collect()
        })
        .unwrap_or_default();
    md.sort();
    let orphans = md
        .iter()
        .filter(|n| is_file(&format!("{dir}/{n}")))
        .map(|n| n.trim_end_matches(".md").to_owned())
        .filter(|id| id != "PROGRAM" && !tasks.iter().any(|t| &t.id == id))
        .collect();
    let stale_queued = tasks
        .iter()
        .filter(|t| t.state == TaskState::Queued)
        .filter(|t| fs::File::open(format!("{dir}/{}.md", t.id)).is_ok())
        .map(|t| t.id.clone())
        .collect();
    let waiting = tasks
        .iter()
        .filter(|t| t.state != TaskState::Landed && (t.state == TaskState::Blocked || t.gated))
        .map(|t| t.id.clone())
        .collect();

    ProgramStatus {
        slug: slug.to_owned(),
        dir: dir.to_owned(),
        has_brief: is_file(&format!("{dir}/PROGRAM.md")),
        tasks: rows,
        orphans,
        stale_queued,
        waiting,
    }
}

#[allow(clippy::write_literal)]
pub(crate) fn render_status(
    out: &mut dyn Write,
    st: &ProgramStatus,
    s: Style,
) -> std::io::Result<()> {
    writeln!(
        out,
        "{}{}{}  {}{}{}",
        s.bold, st.slug, s.off, s.dim, st.dir, s.off
    )?;
    if !st.has_brief {
        writeln!(
            out,
            "{}! no PROGRAM.md \u{2014} this program has no brief{}",
            s.yel, s.off
        )?;
    }
    writeln!(out)?;

    if st.tasks.is_empty() {
        writeln!(out, "{}no backlog lines in PROGRAM.md{}", s.dim, s.off)?;
    } else {
        writeln!(
            out,
            "{}{:<12} {:<13} {:>6} {:>9}  {}{}",
            s.bold, "TASK", "STATE", "PLAN", "JOURNAL", "NOW", s.off
        )?;
        for r in &st.tasks {
            let state = r.state.as_str();
            let mut colour = match r.state {
                TaskState::Landed => s.grn,
                TaskState::Blocked => s.yel,
                TaskState::Dispatched => s.bold,
                TaskState::Queued => s.dim,
            };
            // The lock is four bytes but two columns wide: pad by hand, as the script does.
            let (label, pad) = if r.gated {
                if r.state == TaskState::Queued {
                    colour = s.yel;
                }
                (
                    format!("{state} \u{1f512}"),
                    13usize.saturating_sub(state.len() + 3),
                )
            } else {
                (state.to_owned(), 13usize.saturating_sub(state.len()))
            };
            let pad = pad.max(1);
            let plan = r
                .plan
                .map(|p| format!("{}/{}", p.done, p.total))
                .unwrap_or_default();
            write!(
                out,
                "{:<12} {colour}{label}{:pad$}{} {:>6} {}{:>9}{}  ",
                r.id, "", s.off, plan, s.dim, r.journal_age, s.off
            )?;
            out.write_all(&r.now_bytes)?;
            writeln!(out)?;
        }
    }

    if !st.orphans.is_empty() {
        let list = format!(" {}", st.orphans.join(" "));
        writeln!(
            out,
            "\n{}! journals with no backlog entry:{list}{}",
            s.yel, s.off
        )?;
    }
    if !st.stale_queued.is_empty() {
        let list = format!("{} ", st.stale_queued.join(" "));
        writeln!(
            out,
            "\n{}! journal exists but the ledger still says queued:{} {list}",
            s.yel, s.off
        )?;
    }
    if !st.waiting.is_empty() {
        let list = format!("{} ", st.waiting.join(" "));
        writeln!(out, "\n{}waiting on you:{} {list}", s.yel, s.off)?;
    }
    Ok(())
}
