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
(`AUTH_ADMIN_KEY`, `AUTH_MODE`, `OTLP_ENDPOINT`, `PRODUCTION`, `SAMPLE_RATE`) are fleet-shared
conventions, read via `.shared_env([...])`. Note this is **not** all of `AUTH_*` — see
[Naming](#naming).

## libconfig for a service

```rust
use libconfig::Loader;

// Deploy-controlled file (or Loader::pure_env() for none); never creates dirs,
// read-only by default. Config: Serialize + Deserialize + Default.
let cfg = Loader::path("/etc/someproduct/config.toml")
    .env_prefix("SOMEPRODUCT_")
    .shared_env(["AUTH_ADMIN_KEY", "AUTH_MODE", "OTLP_ENDPOINT", "PRODUCTION", "SAMPLE_RATE"])
    // NB: no AUTH_TCP_* here — those are this service's own prefixed fields. See Naming.
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
  - `AUTH_ADMIN_KEY`, `AUTH_MODE` — read out of the environment by the auth SDK itself, so they
    keep these exact names in every service.
  - `OTLP_ENDPOINT`, `PRODUCTION`, `SAMPLE_RATE` — telemetry knobs ([observability.md](observability.md)).
- **`AUTH_TCP_ADDR` / `AUTH_TCP_TLS` / `AUTH_TCP_DOMAIN` are prefixed, not shared.** They look
  fleet-wide but they are *the consuming service's own* config fields, so under `libconfig` they
  take that service's prefix — `SOMEPRODUCT_AUTH_TCP_ADDR`, **not** `AUTH_TCP_ADDR`. Putting them
  in `.shared_env([...])` fails **silently**: the bare names are ignored, the built-in defaults
  apply, and the service speaks plaintext to a TLS-only listener. Deploy env must set the
  prefixed names. This is the single most common auth-wiring mistake — see
  [auth-integration.md](auth-integration.md).
- **Bind default is loopback** (`127.0.0.1:8080`) for local dev; Docker sets `0.0.0.0:8080` via
  env. The internal Caddy proxy is the only thing that should bind publicly
  ([deployment.md](deployment.md)).

## Conventions

- **Hardcode sensible defaults in code**, not in committed config files
  (`DEFAULT_AUTH_TCP_ADDR = "tcp.auth.dev.thmsn.dev:7070"`, cache TTLs, ports). The TOML file is
  for overrides, not for carrying the baseline. Default the auth address to a **name, not a LAN
  IP** — the TLS leg verifies the certificate's hostname
  ([auth-integration.md](auth-integration.md)).
- **Write `Default` by hand; don't `#[derive(Default)]` on a config struct.** `libconfig`'s
  lowest layer is `Serialized::defaults(Config::default())`, i.e. it *serializes your `Default`
  impl*. A derived `Default` supplies `""`/`0`/`None` for every field, so that layer covers them
  all and your `#[serde(default = "…")]` functions **never run** — the `DEFAULT_*` consts go
  dead and a field like `bind` silently becomes `""`. This is invisible until something fails to
  bind. Delegate the hand-written impl to the same `default_*()` functions.
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
- [ ] `.shared_env([...])` reads the bare fleet vars (`AUTH_ADMIN_KEY`, `AUTH_MODE`,
      `OTLP_ENDPOINT`, `PRODUCTION`, `SAMPLE_RATE`); app vars use the `<PRODUCT>_` prefix.
- [ ] `AUTH_TCP_ADDR`/`_TLS`/`_DOMAIN` are **prefixed** (`<PRODUCT>_AUTH_TCP_ADDR`) and are
      *not* in `shared_env` — deploy env sets the prefixed names.
- [ ] `Default` is hand-written, not derived — otherwise `#[serde(default = "…")]` never runs.
- [ ] Precedence defaults→TOML→shared→prefixed.
- [ ] Containers: `LIBPATH_BASE_DIR` (+ `set_create_dirs(false)`) for deploy-controlled paths.
- [ ] Defaults hardcoded in code; bind defaults to loopback locally.
- [ ] Secrets via env/sealed storage, never a committed file.
