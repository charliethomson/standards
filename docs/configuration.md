# Configuration

## Rule

**`libconfig` is the fleet's one config tool** — tools, libraries, and services alike. It
applies the standard precedence below, and adds persistence (mtime tracking, atomic
write-back) and a `config!{}` macro. Pick the `Loader` source mode to match the consumer:

- **Service** → `Loader::path("/etc/<product>/config.toml")` (deploy-controlled file; read-only,
  no `mkdir`) or `Loader::pure_env()` (no file), plus `.shared_env([...])` for the fleet-shared
  unprefixed vars.
- **Tool / library** → `Loader::module("<name>")` (OS config dir, mtime-tracked write-back) or
  the `config!{}` macro — the desktop default.

Precedence (later wins):

```
built-in defaults  →  TOML file  →  shared (unprefixed) env  →  prefixed <PRODUCT>_ env
```

App-specific `<PRODUCT>_*` vars override everything; a small set of **bare, unprefixed** vars
(`AUTH_*`, `OTLP_ENDPOINT`, `PRODUCTION`, `SAMPLE_RATE`) are fleet-shared conventions, read via
`.shared_env([...])`.

## libconfig for a service

```rust
use libconfig::Loader;

// Deploy-controlled file (or Loader::pure_env() for none); never creates dirs,
// read-only by default. Config: Serialize + Deserialize + Default.
let cfg = Loader::path("/etc/someproduct/config.toml")
    .env_prefix("SOMEPRODUCT_")
    .shared_env(["AUTH_ADMIN_KEY", "AUTH_TCP_ADDR", "OTLP_ENDPOINT", "PRODUCTION", "SAMPLE_RATE"])
    .load::<Config>()?;
```

Or lazily via the macro (the `shared_env` field merges the bare vars; field order is
`module`, `env_prefix`, `shared_env`, `impl_trait`):

```rust
config! {
    pub static CONFIG: Config = {
        module: "someproduct",
        env_prefix: "SOMEPRODUCT_",
        shared_env: ["AUTH_ADMIN_KEY", "OTLP_ENDPOINT", "PRODUCTION", "SAMPLE_RATE"],
        impl_trait,
    }
}
```

For full container path control, `libpath` redirects every root (config **and** logs) under a
deploy base via the `LIBPATH_BASE_DIR` env var (or `libpath::set_base_override(...)`), and
`libpath::set_create_dirs(false)` disables forced `mkdir` globally — which also moves `liblog`'s
JSON log file off the OS user dir ([observability.md](observability.md)).

Provide defaults via `serde` defaults or `Default`, so a bare run with no file and no env still
boots.

## Naming

- **Prefixed per product:** `SOMEPRODUCT_BIND`, `AGENTUTIL_SYNC_INTERVAL_SECS`, `SOMEPRODUCT_SERVER_PORT`.
  The prefix is the product's short name, uppercased.
- **Bare, fleet-shared (no prefix):**
  - `AUTH_ADMIN_KEY`, `AUTH_TCP_ADDR` — the central auth connection, shared by every service.
  - `OTLP_ENDPOINT`, `PRODUCTION`, `SAMPLE_RATE` — telemetry knobs ([observability.md](observability.md)).
- **Bind default is loopback** (`127.0.0.1:8080`) for local dev; Docker sets `0.0.0.0:8080` via
  env. The internal Caddy proxy is the only thing that should bind publicly
  ([deployment.md](deployment.md)).

## Conventions

- **Hardcode sensible defaults in code**, not in committed config files
  (`DEFAULT_AUTH_TCP_ADDR = "192.168.0.193:7070"`, cache TTLs, ports). The TOML file is for
  overrides, not for carrying the baseline.
- **Secrets never come from a committed file** — they arrive as env (written by Komodo from UI
  Variables) or are sealed at rest ([security.md](security.md)). See the secrets split in
  [deployment.md](deployment.md).
- **Opt-in hardening** is config too: e.g. an `agent_host_allowlist` (CIDR/host/suffix) that
  accepts a comma/space list from env or a TOML array, empty = allow-any.

Prefer `libconfig` over hand-rolled `std::env::var` reads — it's testable, gives precedence for
free, and keeps the env table in one struct. The service-`Loader` template is
[`templates/rust/config.rs`](../templates/rust/config.rs); desktop tools use `Loader::module`
or the `config!{}` macro.

## Checklist

- [ ] Config loaded via `libconfig` — `Loader::path`/`pure_env` (services) or
      `Loader::module`/`config!{}` (tools); never hand-rolled `std::env::var`.
- [ ] `.shared_env([...])` reads the bare fleet vars (`AUTH_*`, `OTLP_ENDPOINT`, `PRODUCTION`,
      `SAMPLE_RATE`); app vars use the `<PRODUCT>_` prefix.
- [ ] Precedence defaults→TOML→shared→prefixed.
- [ ] Containers: `LIBPATH_BASE_DIR` (+ `set_create_dirs(false)`) for deploy-controlled paths.
- [ ] Defaults hardcoded in code; bind defaults to loopback locally.
- [ ] Secrets via env/sealed storage, never a committed file.
