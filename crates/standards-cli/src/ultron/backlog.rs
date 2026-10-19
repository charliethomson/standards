//! What `bin/ultron` reads out of a program's files, as pure functions over bytes: the
//! backlog in `PROGRAM.md`, a journal's PLAN progress, and a journal's STATE line.
//!
//! Each mirrors one awk or grep program in the script, in the C locale the script runs in
//! (bytes, ASCII-only case folding), so the output is byte-identical.

use serde::Serialize;

/// A task's state as the backlog declares it. `done` reads as landed and `in-flight` as
/// dispatched; a task with no state word anywhere is queued.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TaskState {
    Landed,
    Dispatched,
    Blocked,
    Queued,
}

impl TaskState {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Landed => "landed",
            Self::Dispatched => "dispatched",
            Self::Blocked => "blocked",
            Self::Queued => "queued",
        }
    }
}

/// One backlog task: its id, the state from the last row that names one, and whether any
/// row marks it human-gated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct Task {
    pub id: String,
    pub state: TaskState,
    pub gated: bool,
}

/// Split into awk records: on `\n`, with a trailing newline not making an empty last record.
fn lines(text: &[u8]) -> impl Iterator<Item = &[u8]> {
    let text = text.strip_suffix(b"\n").unwrap_or(text);
    let empty = text.is_empty();
    text.split(|&b| b == b'\n').filter(move |_| !empty)
}

/// POSIX `[[:space:]]` in the C locale.
fn is_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\x0b' | b'\x0c' | b'\r')
}

/// The length of a `[A-Z][A-Z0-9]+-[0-9]+` match starting exactly at `s[0]`, if any.
fn task_id_at(s: &[u8]) -> Option<usize> {
    if !s.first().is_some_and(u8::is_ascii_uppercase) {
        return None;
    }
    let mut i = 1;
    while s
        .get(i)
        .is_some_and(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
    {
        i += 1;
    }
    // `[A-Z0-9]+` needs one, and it cannot hold the `-`, so greedy is the only parse.
    if i < 2 || s.get(i) != Some(&b'-') {
        return None;
    }
    let digits = s[i + 1..].iter().take_while(|b| b.is_ascii_digit()).count();
    (digits > 0).then_some(i + 1 + digits)
}

/// The leftmost task id in a line (awk `match`).
pub(crate) fn first_task_id(line: &[u8]) -> Option<&[u8]> {
    (0..line.len()).find_map(|start| task_id_at(&line[start..]).map(|n| &line[start..start + n]))
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    find(hay, needle).is_some()
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// The state word nearest the start of the (lowercased) row, so a trailing note cannot
/// override the state declared at the front.
fn first_state(low: &[u8]) -> Option<TaskState> {
    const WORDS: [(&[u8], TaskState); 6] = [
        (b"landed", TaskState::Landed),
        (b"done", TaskState::Landed),
        (b"dispatched", TaskState::Dispatched),
        (b"in-flight", TaskState::Dispatched),
        (b"blocked", TaskState::Blocked),
        (b"queued", TaskState::Queued),
    ];
    WORDS
        .iter()
        .filter_map(|(w, st)| find(low, w).map(|p| (p, *st)))
        .min_by_key(|(p, _)| *p)
        .map(|(_, st)| st)
}

/// Is this a backlog row for `id`: a table row whose first cell is just the id, or a list
/// item that starts with a task id? Prose and dependency diagrams are not.
fn is_row(line: &[u8], id: &[u8]) -> bool {
    let ws = line.iter().take_while(|&&b| is_space(b)).count();
    let rest = &line[ws..];
    if rest.first() == Some(&b'|') {
        let cell = &rest[1..];
        let cell = cell.split(|&b| b == b'|').next().unwrap_or_default();
        let cell: Vec<u8> = cell
            .iter()
            .copied()
            .filter(|b| b.is_ascii_alphanumeric() || *b == b'-')
            .collect();
        if cell == id {
            return true;
        }
    }
    // `^[[:space:]]*[-*][[:space:]]*\**<task id>`
    if matches!(rest.first(), Some(b'-' | b'*')) {
        let rest = &rest[1..];
        let rest = &rest[rest.iter().take_while(|&&b| is_space(b)).count()..];
        let rest = &rest[rest.iter().take_while(|&&b| b == b'*').count()..];
        if task_id_at(rest).is_some() {
            return true;
        }
    }
    false
}

/// `PROGRAM.md` -> its tasks in first-seen order.
///
/// A real brief carries a task twice: once in the plan (a backlog row, no state) and again in
/// a dispatch-log row that does carry one. A task only counts when it is the first cell of a
/// table row or heads a list item, and its state is read from the last such row that has a
/// state word, which is the dispatch log. Prose never sets state or gates.
pub(crate) fn parse(brief: &[u8]) -> Vec<Task> {
    let mut tasks: Vec<Task> = Vec::new();
    for line in lines(brief) {
        let Some(id) = first_task_id(line) else {
            continue;
        };
        if !is_row(line, id) {
            continue;
        }
        let low = line.to_ascii_lowercase();
        let gated = contains(&low, b"human-gated")
            || contains(line, b"[HUMAN]")
            || contains(line, "🔒".as_bytes());
        let idx = if let Some(i) = tasks.iter().position(|t| t.id.as_bytes() == id) {
            i
        } else {
            tasks.push(Task {
                id: String::from_utf8_lossy(id).into_owned(),
                state: TaskState::Queued,
                gated: false,
            });
            tasks.len() - 1
        };
        tasks[idx].gated |= gated;
        if let Some(st) = first_state(&low) {
            tasks[idx].state = st;
        }
    }
    tasks
}

/// PLAN checkbox progress: lines starting (after spaces and dashes) with `[x]` / `[ ]`.
/// `None` when the journal has no checkboxes.
pub(crate) fn plan_progress(journal: &[u8]) -> Option<(usize, usize)> {
    let mut done = 0;
    let mut total = 0;
    for line in lines(journal) {
        let lead = line
            .iter()
            .take_while(|&&b| is_space(b) || b == b'-')
            .count();
        match &line[lead..] {
            [b'[', b'x' | b'X', b']', ..] => {
                done += 1;
                total += 1;
            }
            [b'[', b' ', b']', ..] => total += 1,
            _ => {}
        }
    }
    (total > 0).then_some((done, total))
}

/// The run of `[#*>[:space:]|-]` a section heading may start with, and the word after it,
/// if that word is followed by `[[:space:]:|]` or the end of the line.
fn heading_word(line: &[u8]) -> Option<&[u8]> {
    let lead = line
        .iter()
        .take_while(|&&b| matches!(b, b'#' | b'*' | b'>' | b'|' | b'-') || is_space(b))
        .count();
    let rest = &line[lead..];
    ["STATE", "PLAN", "TOUCHED", "VERIFY", "DECISIONS", "BLOCKED"]
        .into_iter()
        .map(str::as_bytes)
        .find(|w| {
            rest.starts_with(w)
                && rest
                    .get(w.len())
                    .is_none_or(|&b| is_space(b) || b == b':' || b == b'|')
        })
}

/// The first meaningful line of a journal's STATE section, cut to 60 bytes (the script's
/// `cut -c1-60` is GNU cut, which counts bytes). Empty when there is none.
pub(crate) fn state_line(journal: &[u8]) -> Vec<u8> {
    let mut grab = false;
    let mut found: Option<Vec<u8>> = None;
    for line in lines(journal) {
        let heading = heading_word(line);
        if heading == Some(b"STATE".as_slice()) {
            grab = true;
            // sub(/^[^A-Za-z]*STATE[^A-Za-z]*/, "")
            let at = find(line, b"STATE").unwrap_or_default() + b"STATE".len();
            let rest = &line[at..];
            let rest = &rest[rest.iter().take_while(|b| !b.is_ascii_alphabetic()).count()..];
            if rest.len() > 2 {
                found = Some(rest.to_vec());
                break;
            }
            continue;
        }
        if grab && heading.is_some() {
            break;
        }
        // awk's NF: any byte that is not a blank or newline makes a field.
        if grab && line.iter().any(|&b| !matches!(b, b' ' | b'\t' | b'\n')) {
            let lead = line
                .iter()
                .take_while(|&&b| is_space(b) || b == b'>' || b == b'*' || b == b'-')
                .count();
            found = Some(line[lead..].to_vec());
            break;
        }
    }
    let mut out = found.unwrap_or_default();
    out.truncate(60);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(brief: &str) -> Vec<(String, &'static str, bool)> {
        parse(brief.as_bytes())
            .into_iter()
            .map(|t| (t.id, t.state.as_str(), t.gated))
            .collect()
    }

    #[test]
    fn task_ids_need_two_leading_chars_and_digits() {
        assert_eq!(first_task_id(b"see STD-23 now"), Some(&b"STD-23"[..]));
        assert_eq!(first_task_id(b"X-1 then AB-2"), Some(&b"AB-2"[..]));
        assert_eq!(first_task_id(b"A1B-22-3"), Some(&b"A1B-22"[..]));
        assert_eq!(first_task_id(b"lower-1 ABC- AB-x"), None);
        assert_eq!(first_task_id(b""), None);
    }

    #[test]
    fn rows_count_prose_does_not() {
        let brief = "\
# Plan
| Task | What |
|---|---|
| STD-1 | a thing |
| **STD-2** | queued |
- STD-3 dispatched: doing it
* **STD-4** blocked on review
**STD-5** landed
STD-6 is blocked on STD-1 landing
STD-1 -> STD-7 [HUMAN]
| see STD-8 | no |
";
        assert_eq!(
            ids(brief),
            vec![
                ("STD-1".into(), "queued", false),
                ("STD-2".into(), "queued", false),
                ("STD-3".into(), "dispatched", false),
                ("STD-4".into(), "blocked", false),
                ("STD-5".into(), "landed", false),
            ]
        );
    }

    #[test]
    fn last_stated_row_wins_and_nearest_word_counts() {
        let brief = "\
| STD-1 | plan |
| STD-2 | human-gated |
- STD-1 in-flight (was queued)
- STD-1 done, nothing blocked
- STD-1 no state word here
- STD-2 🔒 dispatched
- STD-3 [HUMAN] queued
";
        assert_eq!(
            ids(brief),
            vec![
                ("STD-1".into(), "landed", false),
                ("STD-2".into(), "dispatched", true),
                ("STD-3".into(), "queued", true),
            ]
        );
        assert_eq!(parse(b""), vec![]);
    }

    #[test]
    fn plan_progress_counts_checkboxes() {
        let j = b"PLAN\n- [x] one\n  - [X] two\n- [ ] three\n1. [x] numbered\n[ ]four";
        assert_eq!(plan_progress(j), Some((2, 4)));
        assert_eq!(plan_progress(b"no boxes\n"), None);
    }

    #[test]
    fn state_line_reads_inline_or_next_line() {
        assert_eq!(
            state_line(b"PLAN\n- [x] a\nSTATE: step 3 of 5\n"),
            b"step 3 of 5"
        );
        assert_eq!(
            state_line(b"## STATE\n\n> - waiting on CI\nTOUCHED\n"),
            b"waiting on CI"
        );
        assert_eq!(state_line(b"**STATE:** \xe2\x80\x94 4 left\n"), b"left");
        // Too short inline: keep reading the section.
        assert_eq!(state_line(b"STATE: ok\nnext line\n"), b"next line");
        // The section ends before anything meaningful.
        assert_eq!(state_line(b"STATE\n\nVERIFY: x\nlater\n"), b"");
        assert_eq!(state_line(b"no state here\nSTATEMENT x\n"), b"");
        assert_eq!(state_line(b"STATE\n---\n"), b"");
    }

    #[test]
    fn state_line_cuts_at_sixty_bytes() {
        let long = format!("STATE: {}\u{2014}tail\n", "a".repeat(58));
        let got = state_line(long.as_bytes());
        assert_eq!(got.len(), 60);
        assert_eq!(&got[58..], &[0xe2, 0x80]);
    }
}
