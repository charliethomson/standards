//! Shared by the integration tests: the real `standards-server` app on an ephemeral
//! loopback port, a scratch environment that never points at the real `$HOME`, and a way to
//! run the cli in-process with captured output (tests are silent).

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use poem::listener::TcpAcceptor;
use poem::{Endpoint, Response, Server};
use standards_cli::env::{EnvConfig, Platform};
use standards_cli::{Env, Io};

/// Serve `app` on a fresh loopback port from its own runtime thread; returns the base URL.
/// The thread lives until the test process exits.
pub fn serve<E>(app: E) -> String
where
    E: Endpoint<Output = Response> + 'static,
{
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let port = listener.local_addr().unwrap().port();
    listener.set_nonblocking(true).unwrap();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async move {
            let acceptor = TcpAcceptor::from_std(listener).expect("acceptor");
            let _ = Server::new_with_acceptor(acceptor).run(app).await;
        });
    });
    format!("http://127.0.0.1:{port}")
}

/// The real server over the real embedded content, started once per test binary.
pub fn server() -> &'static str {
    static URL: OnceLock<String> = OnceLock::new();
    URL.get_or_init(|| {
        let bundle = standards_db::bundle().expect("embedded content parses");
        let state = standards_api::AppState::new(
            Arc::clone(&bundle),
            &standards_api::Config::default(),
            "0.1.42",
            false,
        );
        serve(standards_api::app(state))
    })
}

/// A loopback URL nothing listens on.
pub fn dead_host() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    format!("http://127.0.0.1:{port}")
}

/// A scratch machine: `home` and `cwd` under one temp dir.
pub struct Scratch {
    pub dir: tempfile::TempDir,
    pub env: Env,
}

impl Scratch {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        let cwd = dir.path().join("work");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&cwd).unwrap();
        let env = Env {
            home,
            xdg_cache_home: None,
            cwd,
            current_exe: None,
            version: "0.1.0".into(),
            platform: Platform {
                os: "linux".into(),
                arch: "amd64".into(),
                libc: None,
            },
            config: EnvConfig::default(),
        };
        Self { dir, env }
    }

    /// A directory under the work dir holding a `.git`, as `git init` leaves it.
    pub fn repo(&mut self, name: &str) -> PathBuf {
        let root = self.env.cwd.join(name);
        std::fs::create_dir_all(root.join(".git")).unwrap();
        self.env.cwd.clone_from(&root);
        root
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }
}

/// What a run printed.
pub struct Run {
    pub code: u8,
    pub out: String,
    pub err: String,
}

impl Run {
    pub fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.out).expect(&self.out)
    }
}

/// Run `standards <args>` in-process against `env`.
pub fn run(env: &Env, args: &[&str]) -> Run {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = {
        let mut io = Io {
            out: &mut out,
            err: &mut err,
        };
        let program_args = std::iter::once("standards").chain(args.iter().copied());
        standards_cli::run(program_args, env.clone(), &mut io)
    };
    Run {
        code,
        out: String::from_utf8(out).unwrap(),
        err: String::from_utf8(err).unwrap(),
    }
}

/// Every file under `root` (relative, sorted), `.git` excluded.
pub fn tree(root: &Path) -> Vec<String> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.file_name().is_some_and(|n| n == ".git") {
                continue;
            }
            if p.is_dir() {
                walk(root, &p, out);
            } else {
                out.push(p.strip_prefix(root).unwrap().display().to_string());
            }
        }
    }
    let mut out = Vec::new();
    if root.exists() {
        walk(root, root, &mut out);
    }
    out.sort();
    out
}
