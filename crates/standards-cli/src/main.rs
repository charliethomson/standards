//! `standards`: the fleet standards cli (`dev.thmsn.standards.cli`). Wiring only; the
//! commands are in the library.

use std::process::ExitCode;

use libproduct::product_name;
use standards_cli::{Env, Io, exit};

product_name!("dev.thmsn.standards.cli");

fn main() -> ExitCode {
    let product_name = PRODUCT_NAME;
    let _ = product_name.set_global();

    let env = match Env::from_process() {
        Ok(env) => env,
        Err(error) => {
            eprintln!("error: environment: {error}");
            return ExitCode::from(exit::USAGE);
        }
    };
    let mut out = std::io::stdout().lock();
    let mut err = std::io::stderr().lock();
    let mut io = Io {
        out: &mut out,
        err: &mut err,
    };
    ExitCode::from(standards_cli::run(std::env::args_os(), env, &mut io))
}
