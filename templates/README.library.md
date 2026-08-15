<!--
  Skeleton README for a library or CLI repo. For a library, this file IS the API
  documentation — which raises the bar for the example, not the length.
  Convention and rationale: standards/docs/readmes.md

  Fill a section or delete it. Delete these comments as you go.
-->
# {{PRODUCT}}

**<One line. What problem this solves, and for whom.>**

<!--
  Two or three sentences. For a lib*, say which cross-cutting concern it owns and which
  standard it implements — the fleet's libraries exist to stop N repos re-solving one
  thing. A small table contrasting the old way with this one is often the fastest
  explanation.
-->

## Install

<!--
  HTTPS for a public repo, SSH for a private one (standards/docs/rust-conventions.md).
  For a CLI, the registry one-liner instead — and say that re-running it upgrades.
-->

```toml
[dependencies]
{{PRODUCT}} = { git = "https://github.com/charliethomson/{{PRODUCT}}" }
```

## Usage

<!--
  A minimal example that actually compiles and shows the real path through the API —
  not a signature dump. Point at examples/ for the walkthrough.
-->

```rust
```

## Features

<!-- Optional feature flags: what each turns on, and which are default. -->

## What stays in the consuming repo

<!--
  The boundary. What this library deliberately does NOT do, and what each consumer must
  still write for itself. Prevents both re-implementation and misuse.
-->

## Testing

<!-- The commands CI runs. Coverage config and floor, if there is one. -->

```sh
cargo test
cargo clippy --all-targets -- -D warnings
```

<!--
  Repo conventions and the shared standards live in AGENTS.md and the standards/
  submodule — keep the pointer, drop this comment.
-->
Conventions: [`AGENTS.md`](AGENTS.md) and the [`standards/`](standards) submodule.
