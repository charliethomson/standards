//! `self-update` against a fake registry on loopback. The "current exe" is always a file in
//! a temp dir; nothing outside it is replaced.

mod harness;

use std::io::Write as _;
use std::sync::Arc;

use harness::{Scratch, dead_host, run, serve};
use poem::http::StatusCode;
use poem::web::{Data, Query};
use poem::{EndpointExt as _, Request, Response, Route, get, handler};
use sha2::{Digest as _, Sha256};

/// How the fake registry answers.
#[derive(Clone)]
struct Fake {
    status: u16,
    version: String,
    file_name: String,
    artifact: Vec<u8>,
    /// The sha256 it claims; `None` means the real one.
    claimed_sha: Option<String>,
    /// When set, `resolve` needs `Authorization: Bearer <token>`.
    token: Option<String>,
}

impl Fake {
    fn new(version: &str, binary: &[u8]) -> Self {
        Self {
            status: 200,
            version: version.into(),
            file_name: "standards-linux-amd64.tar.gz".into(),
            artifact: tar_gz(&[("dist/README", b"x"), ("dist/standards", binary)]),
            claimed_sha: None,
            token: None,
        }
    }

    fn start(self) -> String {
        let app = Route::new()
            .at(
                "/api/v1/products/dev.thmsn.standards/apps/cli/resolve",
                get(resolve),
            )
            .at("/download/artifact", get(download))
            .data(Arc::new(self));
        serve(app.map_to_response())
    }
}

#[derive(serde::Deserialize)]
struct Params {
    os: String,
    arch: String,
}

#[handler]
fn resolve(Data(fake): Data<&Arc<Fake>>, Query(p): Query<Params>, req: &Request) -> Response {
    if let Some(token) = &fake.token {
        let auth = req.header("authorization").unwrap_or_default();
        if auth != format!("Bearer {token}") {
            return Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .finish();
        }
    }
    if fake.status != 200 {
        return Response::builder()
            .status(StatusCode::from_u16(fake.status).unwrap())
            .finish();
    }
    let sha = fake
        .claimed_sha
        .clone()
        .unwrap_or_else(|| hex::encode(Sha256::digest(&fake.artifact)));
    let body = serde_json::json!({
        "product": "dev.thmsn.standards",
        "app": "cli",
        "version": fake.version,
        "target": format!("{}-{}", p.os, p.arch),
        "download_url": "/download/artifact?exp=1&sig=x",
        "file_name": fake.file_name,
        "sha256": sha,
        "file_size": fake.artifact.len(),
        "bin_name": "standards",
    });
    Response::builder()
        .content_type("application/json")
        .body(body.to_string())
}

#[handler]
fn download(Data(fake): Data<&Arc<Fake>>) -> Vec<u8> {
    fake.artifact.clone()
}

fn tar_gz(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut builder = tar::Builder::new(flate2::write::GzEncoder::new(
        Vec::new(),
        flate2::Compression::fast(),
    ));
    for (path, bytes) in files {
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o755);
        header.set_cksum();
        builder.append_data(&mut header, path, *bytes).unwrap();
    }
    let mut gz = builder.into_inner().unwrap();
    gz.flush().unwrap();
    gz.finish().unwrap()
}

/// A scratch env whose current exe is `<tmp>/bin/standards` holding `old`.
fn scratch(registry: &str) -> Scratch {
    let mut s = Scratch::new();
    let bin = s.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let exe = bin.join("standards");
    std::fs::write(&exe, b"old").unwrap();
    s.env.current_exe = Some(exe);
    s.env.config.registry_url = Some(registry.to_owned());
    s
}

fn exe(s: &Scratch) -> Vec<u8> {
    std::fs::read(s.env.current_exe.as_ref().unwrap()).unwrap()
}

#[test]
fn check_reports_without_installing() {
    let s = scratch(&Fake::new("0.1.7", b"new").start());
    let r = run(&s.env, &["self-update", "--check"]);
    assert_eq!(r.code, 0, "{}", r.err);
    assert_eq!(
        r.out,
        "standards 0.1.7 is available (running 0.1.0); run `standards self-update`\n"
    );
    let r = run(&s.env, &["--json", "self-update", "--check"]);
    let v = r.json();
    assert_eq!(v["data"]["update_available"], true);
    assert_eq!(v["data"]["latest"], "0.1.7");
    assert_eq!(v["data"]["target"], "linux-amd64");
    assert_eq!(v["data"]["installed"], false);
    assert_eq!(exe(&s), b"old");
}

#[test]
fn updates_atomically_in_place() {
    let s = scratch(&Fake::new("0.2.1", b"new binary").start());
    let r = run(&s.env, &["--json", "self-update"]);
    assert_eq!(r.code, 0, "{}", r.out);
    let v = r.json();
    assert_eq!(v["data"]["installed"], true);
    assert_eq!(exe(&s), b"new binary");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(s.env.current_exe.as_ref().unwrap())
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o755);
    }
    // Only the exe is left in its directory: no temp file.
    assert_eq!(std::fs::read_dir(s.path().join("bin")).unwrap().count(), 1);
}

#[test]
fn current_or_newer_is_a_no_op() {
    let s = scratch(&Fake::new("0.1.0", b"same").start());
    let r = run(&s.env, &["self-update"]);
    assert_eq!(r.code, 0);
    assert_eq!(r.out, "standards 0.1.0 is current\n");
    assert_eq!(exe(&s), b"old");
}

#[test]
fn raw_binary_artifacts_and_the_bearer_token() {
    let mut fake = Fake::new("1.0.0", b"");
    fake.file_name = "standards".into();
    fake.artifact = b"raw".to_vec();
    fake.token = Some("t0k".into());
    let mut s = scratch(&fake.start());

    let r = run(&s.env, &["self-update"]);
    assert_eq!(r.code, 1);
    assert!(r.err.contains("set REGISTRY_TOKEN"), "{}", r.err);
    assert_eq!(exe(&s), b"old");

    s.env.config.registry_token = Some("t0k".into());
    let r = run(&s.env, &["self-update"]);
    assert_eq!(r.code, 0, "{}", r.err);
    assert!(r.out.starts_with("updated standards 0.1.0 -> 1.0.0"));
    assert_eq!(exe(&s), b"raw");
}

#[test]
fn checksum_mismatch_leaves_the_binary_alone() {
    let mut fake = Fake::new("9.0.0", b"evil");
    fake.claimed_sha = Some("00".repeat(32));
    let s = scratch(&fake.start());
    let r = run(&s.env, &["--json", "self-update"]);
    assert_eq!(r.code, 1);
    assert_eq!(
        r.json()["error"]["$type"],
        "dev.thmsn.standards.cli.registry.error.mismatch"
    );
    assert_eq!(exe(&s), b"old");
}

#[test]
fn missing_binary_in_archive_fails() {
    let mut fake = Fake::new("9.0.0", b"");
    fake.artifact = tar_gz(&[("other", b"x")]);
    let s = scratch(&fake.start());
    let r = run(&s.env, &["self-update"]);
    assert_eq!(r.code, 1);
    assert!(r.err.contains("artifact has no `standards`"), "{}", r.err);
}

#[test]
fn registry_errors_map_to_exit_codes() {
    for (status, code, needle) in [
        (
            404,
            1,
            "no published dev.thmsn.standards/cli for linux-amd64",
        ),
        (500, 1, "HTTP 500"),
        (503, 3, "can't reach the registry"),
    ] {
        let mut fake = Fake::new("1.0.0", b"");
        fake.status = status;
        let s = scratch(&fake.start());
        let r = run(&s.env, &["self-update", "--check"]);
        assert_eq!(r.code, code, "{status}: {}", r.err);
        assert!(r.err.contains(needle), "{status}: {}", r.err);
    }

    let s = scratch(&dead_host());
    let r = run(&s.env, &["self-update", "--check"]);
    assert_eq!(r.code, 3, "{}", r.err);

    let mut s = scratch(&Fake::new("2.0.0", b"x").start());
    s.env.current_exe = None;
    let r = run(&s.env, &["self-update"]);
    assert_eq!(r.code, 1);
    assert!(r.err.contains("can't locate the running binary"));
}
