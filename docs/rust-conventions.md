# Rust conventions

## Rule

Every Rust crate in the fleet shares the same baseline: **edition 2024**, **clippy pedantic**
with a fixed allowlist, **workspace-inherited lints**, and **git-based distribution** (no
crates.io). These apply to products, libraries, and CLI tools alike.

## Workspace lints

Define lints once at the workspace root and inherit them in every member:

```toml
# workspace root Cargo.toml
[workspace.lints.clippy]
pedantic           = { level = "warn", priority = 0 }
missing_errors_doc = { level = "allow", priority = 1 }
panic              = { level = "warn", priority = 1 }
implicit_hasher    = { level = "allow", priority = 1 }

# each member Cargo.toml
[lints]
workspace = true
```

CI enforces it: `cargo clippy --workspace --all-targets -- -D warnings`
([ci-cd.md](ci-cd.md)). Template: [`templates/rust/Cargo-workspace-lints.toml`](../templates/rust/Cargo-workspace-lints.toml).

## Edition & toolchain

- **Edition 2024** everywhere.
- A `.cargo/config.toml` is required where the crate uses structured logging or fetches private
  deps:
  ```toml
  [build]
  rustflags = ["--cfg", "tracing_unstable"]   # enables tracing's `valuable` support
  [net]
  git-fetch-with-cli = true                    # reliable SSH fetch of private crates
  ```

## Dependency distribution

- **Git deps, not crates.io.** Consumers pin by git ref or use workspace members:
  ```toml
  liberror  = { git = "https://github.com/charliethomson/liberror" }   # public → HTTPS
  liblog    = { git = "ssh://git@github.com/charliethomson/liblog.git" } # private → SSH
  libsomeproduct   = { git = "ssh://git@github.com/charliethomson/libsomeproduct.git", default-features = false,
                features = ["tiktok", "twitch"] }
  ```
- **HTTPS for public** repos, **SSH for private** ones. CI loads the `ssh_key` deploy key into
  an `ssh-agent` and sets `CARGO_NET_GIT_FETCH_WITH_CLI=true`
  ([ci-cd.md](ci-cd.md#private-dependencies)).
- **kiln and `libci` are both private.** A repo's `.kiln/Cargo.toml` takes both over SSH, so
  the `.kiln/` build needs the deploy key and `git-fetch-with-cli` like any other private dep;
  on a CI agent the repo's `checkout_key` provides it:
  ```toml
  kiln  = { git = "ssh://git@github.com/charliethomson/kiln.git" }                              # private → SSH
  libci = { git = "ssh://git@github.com/charliethomson/libci.git", features = ["templates"] }  # private → SSH
  ```
- **Feature-gate optional weight** (platform backends, `buildinfo`, WAF bypass) so consumers
  opt in.

## Project shape

- Workspace-unified version placeholder; versioning is derived ([versioning.md](versioning.md)).
- Binaries declare their identifier with `product_name!("dev.thmsn.…")` and publish it with
  `set_global()` in `main` — the macro alone only declares it ([identifiers.md](identifiers.md)).
- Errors follow the [error-handling](error-handling.md) trinity; shared concerns come from the
  [`lib*` ecosystem](lib-ecosystem.md) rather than being re-solved.

## Checklist

- [ ] Edition 2024; `[lints] workspace = true` in every member.
- [ ] Workspace clippy `pedantic = warn` with the standard allowlist; CI runs `-D warnings`.
- [ ] `.cargo/config.toml` sets `tracing_unstable` and `git-fetch-with-cli` where needed.
- [ ] Deps are git (HTTPS public / SSH private), feature-gated where optional — including
      `.kiln/`: kiln and `libci` over `ssh://`.
- [ ] Cross-cutting concerns pulled from `lib*`, not reimplemented.
