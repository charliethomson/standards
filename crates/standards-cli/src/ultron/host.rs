//! What `ultron` takes from the running process besides files: the clock, a line of stdin
//! (`gc`'s prompt), and sleeping between `watch` polls, which ctrl-c cuts short. Tests swap
//! in a scripted host so nothing waits or reads the terminal.

use std::io::BufRead;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub(crate) trait Host {
    /// Now, in epoch seconds.
    fn now(&self) -> i64;
    /// Local wall-clock time as `HH:MM:SS`.
    fn clock(&self) -> String;
    /// One line of input, without its newline; `None` at end of input.
    fn read_line(&mut self) -> Option<String>;
    /// Route ctrl-c and SIGTERM to [`Host::nap`] instead of killing the process, so `watch`
    /// can leave the alternate screen.
    fn trap_signals(&mut self);
    /// Sleep for `secs`; `false` when interrupted, which ends `watch`.
    fn nap(&mut self, secs: u64) -> bool;
}

static INTERRUPTED: AtomicBool = AtomicBool::new(false);

/// The real process.
pub(crate) struct RealHost;

impl Host for RealHost {
    fn now(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|d| i64::try_from(d.as_secs()).ok())
            .unwrap_or(0)
    }

    fn clock(&self) -> String {
        chrono::Local::now().format("%H:%M:%S").to_string()
    }

    fn read_line(&mut self) -> Option<String> {
        let mut line = String::new();
        match std::io::stdin().lock().read_line(&mut line) {
            Ok(0) | Err(_) => None,
            Ok(_) => Some(line.trim_end_matches(['\n', '\r']).to_owned()),
        }
    }

    fn trap_signals(&mut self) {
        // A second install (a second watch in one process) fails harmlessly: the first
        // handler already sets the same flag.
        let _ = ctrlc::set_handler(|| INTERRUPTED.store(true, Ordering::SeqCst));
    }

    fn nap(&mut self, secs: u64) -> bool {
        // Short slices, so a signal lands within a tenth of a second rather than after the
        // whole interval.
        let until = Instant::now() + Duration::from_secs(secs);
        while Instant::now() < until {
            if INTERRUPTED.swap(false, Ordering::SeqCst) {
                return false;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        !INTERRUPTED.swap(false, Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_host_clock_and_interrupt() {
        let mut h = RealHost;
        assert!(h.now() > 1_700_000_000);
        let c = h.clock();
        assert_eq!(c.len(), 8, "{c}");
        assert_eq!(c.as_bytes()[2], b':');
        INTERRUPTED.store(true, Ordering::SeqCst);
        assert!(!h.nap(1));
        assert!(h.nap(0));
    }
}
