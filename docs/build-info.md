# Build info

## Rule

A [version string](versioning.md) answers *"which commit is this?"*. **Build info answers
*"built from what, where, when, and was the tree clean?"*** Embed it via the
**`libbuildinfo`** crate **when feasible** — always for services and long-lived binaries;
optional (feature-gated) for libraries.

## What `libbuildinfo` captures

Embedded at compile time (MessagePack, via `include_bytes!`), so it travels inside the
binary with zero runtime cost:

- **git** — `commit_hash` (+ short), `branch`, `dirty` flag, commit timestamp/message,
  author, `tags`, `remote_url`, `describe`, `commit_count`.
- **package** — name, version, edition, description, repository, license, … (Cargo metadata).
- **agent** — build host: `hostname`, OS (name/version/kernel/arch), memory, CPU count.

The `dirty` flag and `commit_hash` are the high-value fields: they tell you instantly
whether a running binary was built from a clean, known commit.

## Integrate it (Rust)

Use the published crate from git — **not** the `buildinfo` repo (that's a minimal
package-only test fork; don't depend on it).

```toml
# Cargo.toml — needed in BOTH sections
[dependencies]
libbuildinfo = { git = "https://github.com/charliethomson/libbuildinfo" }

[build-dependencies]
libbuildinfo = { git = "https://github.com/charliethomson/libbuildinfo" }
```

```rust
// build.rs
fn main() {
    libbuildinfo::emit().expect("emit build info");
    // ... plus the version-deriving logic from templates/rust/build.rs
}
```

```rust
// main.rs
fn main() {
    let bi = libbuildinfo::load_build_info!().expect("load build info");
    // bi.git.commit_short_hash, bi.git.dirty, bi.package.version, bi.agent.hostname, ...
}
```

For libraries or crates whose `build.rs` may not run `emit()`, use the optional form so
the binary still builds:

```rust
let bi = libbuildinfo::load_build_info!(optional); // Option<BuildInfoResult<BuildInfo>>
```

## Expose it

Build info is for debugging, so make it reachable:

- **Services** — log a startup line with version + short hash + dirty flag; expose the
  full struct on a `/version` (or `/health`) endpoint.
- **Clients** — show version + short commit hash in the About/Settings screen.
- **Telemetry** — attach commit hash as a resource attribute alongside the
  `dev.thmsn.<product>` `service_name`.

A user reporting a bug can then read you the commit hash, and you know exactly what they're
running — including whether it was an uncommitted local build.

## Relationship to versioning

| Question | Answered by |
|---|---|
| Which release/commit ordinal? | version string `MAJOR.MINOR.<count>` ([versioning.md](versioning.md)) — **always present** |
| Exact commit hash, clean or dirty, built where/when? | `libbuildinfo` — **when feasible** |

Ship the version string everywhere; add `libbuildinfo` when the extra forensic detail
justifies the build-time dependency.

## Checklist

- [ ] Services and shipped binaries embed `libbuildinfo` (git dep, both `[dependencies]`
      and `[build-dependencies]`).
- [ ] `build.rs` calls `libbuildinfo::emit()`.
- [ ] Startup logs include short commit hash + dirty flag.
- [ ] A `/version` endpoint (services) or About screen (clients) exposes the full struct.
- [ ] Libraries use `load_build_info!(optional)` if they embed it at all.
- [ ] Depend on `libbuildinfo` (the crate), never the `buildinfo` test fork.
