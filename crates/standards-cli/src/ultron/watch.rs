//! `ultron watch`: live status on the alternate screen, redrawn only when the program's
//! fingerprint changes; waits for a program when there is none yet.

use std::io::Write;

use super::host::Host;
use super::state::{StateDir, die, fingerprint};
use super::view::{Style, program_status, render_status};
use crate::{Error, Result};

/// `[-n|--interval <seconds>] [<program>]`, parsed and checked as the script does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WatchArgs {
    pub interval: u64,
    pub slug: Option<String>,
}

impl WatchArgs {
    pub(crate) fn parse(args: &[&str]) -> Result<Self> {
        let mut interval: &str = "2";
        let mut slug = None;
        let mut i = 0;
        while i < args.len() {
            match args[i] {
                "-n" | "--interval" => {
                    interval = args
                        .get(i + 1)
                        .ok_or_else(|| die("--interval needs a value"))?;
                    i += 2;
                }
                a if a.starts_with('-') => return Err(die(format!("unknown option: {a}"))),
                a => {
                    slug = Some(a.to_owned()).filter(|s| !s.is_empty());
                    i += 1;
                }
            }
        }
        if interval.is_empty() || !interval.bytes().all(|b| b.is_ascii_digit()) {
            return Err(die("--interval must be a whole number of seconds"));
        }
        let interval: u64 = interval
            .parse()
            .map_err(|_| die("--interval must be a whole number of seconds"))?;
        if interval < 1 {
            return Err(die("--interval must be at least 1 second"));
        }
        Ok(Self { interval, slug })
    }
}

const ENTER: &str = "\x1b[?1049h\x1b[?25l"; // alternate screen, hide cursor
const LEAVE: &str = "\x1b[?25h\x1b[?1049l";
const CLEAR: &str = "\x1b[H\x1b[2J";
const WAITING: &str = "@waiting";

/// One redraw decision per poll.
pub(crate) struct Watcher<'a> {
    pub state: &'a StateDir,
    pub args: WatchArgs,
    pub style: Style,
    /// What was last drawn: a fingerprint, or [`WAITING`]. `None` until the first draw, so
    /// an empty program still draws once.
    last: Option<String>,
}

impl<'a> Watcher<'a> {
    pub(crate) fn new(state: &'a StateDir, args: WatchArgs, style: Style) -> Self {
        Self {
            state,
            args,
            style,
            last: None,
        }
    }

    /// Redraw if anything changed since the last poll. `Ok(true)` when it drew.
    pub(crate) fn tick(&mut self, out: &mut dyn Write, host: &dyn Host) -> Result<bool> {
        let s = self.style;
        let target = self
            .args
            .slug
            .clone()
            .or_else(|| self.state.list_programs().into_iter().next());
        let Some(target) = target else {
            // No program yet: wait for one. Starting the watch before ultron creates the
            // program is the normal order of operations.
            if self.last.as_deref() == Some(WAITING) {
                return Ok(false);
            }
            writeln!(
                out,
                "{CLEAR}{}waiting for a program under {} \u{2026}{}",
                s.dim,
                self.state.root(),
                s.off
            )
            .map_err(stdout)?;
            self.last = Some(WAITING.into());
            return Ok(true);
        };
        let sig = fingerprint(&self.state.path_of(&target));
        if self.last.as_deref() == Some(sig.as_str()) {
            return Ok(false);
        }
        let stamp = host.clock();
        let dir = self.state.program_dir(&target)?;
        write!(out, "{CLEAR}").map_err(stdout)?;
        render_status(out, &program_status(&target, &dir, host.now()), s).map_err(stdout)?;
        writeln!(
            out,
            "\n{}last change {stamp} \u{b7} polling every {}s \u{b7} ctrl-c to stop{}",
            s.dim, self.args.interval, s.off
        )
        .map_err(stdout)?;
        out.flush().map_err(stdout)?;
        self.last = Some(sig);
        Ok(true)
    }

    /// Take over the screen and poll until interrupted; the screen is restored on the way
    /// out, error or not.
    pub(crate) fn run(&mut self, out: &mut dyn Write, host: &mut dyn Host) -> Result<()> {
        host.trap_signals();
        write!(out, "{ENTER}").map_err(stdout)?;
        let result = loop {
            if let Err(e) = self.tick(out, host) {
                break Err(e);
            }
            if !host.nap(self.args.interval) {
                break Ok(());
            }
        };
        write!(out, "{LEAVE}").map_err(stdout)?;
        out.flush().map_err(stdout)?;
        result
    }
}

fn stdout(e: std::io::Error) -> Error {
    Error::io(std::path::Path::new("<stdout>"), e)
}
