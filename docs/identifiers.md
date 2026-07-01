# Identifiers

> This doc is about **reverse-domain names** (`dev.thmsn.*`) for products, components, grants,
> and error keys. *Entity* ids are elsewhere: internal `Id<T>` (UUIDv7) in
> [data-persistence.md](data-persistence.md), and user-facing **public short ids** in
> [public-ids.md](public-ids.md).

## Rule

Everything that needs a stable name uses a **reverse-domain identifier** rooted at
`dev.thmsn`. The grammar:

```
dev.thmsn.<product>                 # the root — names the product as a whole (its auth app id)
dev.thmsn.<product>.<component>     # a component within the product (see taxonomy below)
dev.thmsn.<product>.<grant>         # an auth permission on the product (a parallel axis)
```

The segment after `<product>` is a **component**, *not* just a platform. `<component>` and
`<grant>` occupy the same slot but live in different systems (one names a thing, the other a
permission), so they don't collide in practice.

## Components

A component is one deployable or installable unit of the product. Two families:

| Family | Component names | Used for |
|---|---|---|
| **Services / binaries** | `server`, `worker`, `cli`, or a specific `<service_name>` | Rust `product_name!`, OTel `service.name`, log/notification namespaces |
| **App surfaces** | `ios`, `macos`, `winui`, `web` | bundle identifiers, per-client namespacing |

Pick the name that describes *what the unit is*, not incidentally what OS it runs on — a
background job-runner that hangs off the main service is `worker`, the desktop app surface is
`macos`, the Windows app surface is `winui`, the browser app is `web`.

> **Component (`winui`) ≠ workflow surface (`windows`).** An identifier component names *what
> the app is* — the UI tech (`winui`, `web`). A workflow surface ([ci-cd.md](ci-cd.md) naming)
> names *what we build on / release for* — the OS/runner target (`windows`). They diverge for
> Windows on purpose: `dev.thmsn.<product>.winui` is the app identity; `windows.release.yml` is
> the workflow. Same surface, two different lenses.

### Examples

| Identifier | What it names |
|---|---|
| `dev.thmsn.someproduct.server` | the someproduct server binary (service component) |
| `dev.thmsn.someproduct.worker` | the someproduct worker binary (service component) |
| `dev.thmsn.someproduct.cli` | the someproduct CLI binary (service component) |
| `dev.thmsn.someproduct.ios` / `.macos` / `.winui` / `.web` | app-surface components (Apple bundle ids, etc.) |
| `dev.thmsn.someproduct` | the product as a whole (its auth app id) |
| `dev.thmsn.someproduct.use` / `.admin` | auth **grants** on the product (parallel axis) |
| `dev.thmsn.libsomeproduct` / `.libsomeproduct.proxy` | a library and its service variant |
| `dev.thmsn.apps` | the registry's AltStore source identifier |

## Grants (the parallel axis)

Auth grants hang off the **product root**, not a component: `dev.thmsn.<product>.<grant>`
(`use`/`read`/`write`/`admin`). They're declared with `#[derive(App)]` on the product's auth
app id and registered on boot — see [auth-integration.md](auth-integration.md). A grant
authorizes *access to the product*; it is not a component and is never a `product_name!`.

**Error keys** are another member of this reverse-domain family:
`dev.thmsn.<root>.<area>.error.<kind>` identifies a specific failure. See
[error-handling.md](error-handling.md).

### Segment rules

Each dot-separated segment:

- is non-empty,
- contains only lowercase ASCII letters, digits, and hyphens,
- does not start or end with a hyphen.

There must be at least two segments. (This is the `AppId` contract enforced in `auth`'s
`proto/src/app_id.rs`; reuse it wherever IDs are validated.)

## Don't confuse identifiers with hostnames

Two different namespaces, both reverse-ish — keep them straight:

- **Identifier** = `dev.thmsn.<product>...` — names a *thing* (binary, bundle, grant,
  source). Stable forever; existing installs depend on it.
- **Deployment hostname** = `<service>.dev.thmsn.dev` (e.g. `someproduct.dev.thmsn.dev`,
  `agentutil.dev.thmsn.dev`) — names a *running service* on the homelab. See
  [deployment.md](deployment.md).

A product typically has both: identifier `dev.thmsn.someproduct`, hostname `someproduct.dev.thmsn.dev`.

## How to set it

| Context | Mechanism |
|---|---|
| **Rust binary** (service component) | `libproduct`'s `product_name!("dev.thmsn.<product>.<component>")` in `main.rs` — e.g. `.server`, `.worker`, `.cli`. The canonical injection point; also becomes the OTel `service.name`. |
| **Apple** (`ios`/`macos`) | `project.yml` (XcodeGen): `options.bundleIdPrefix: dev.thmsn.<product>` and per-target `PRODUCT_BUNDLE_IDENTIFIER: dev.thmsn.<product>.{ios,macos}`. |
| **Windows / Web** (`winui`/`web`) | The WinUI app identity / web app config uses `dev.thmsn.<product>.{winui,web}`. |
| **Auth grants** | `#[derive(App)]` with `id = "dev.thmsn.<product>"` and a `Grants` enum (`Use`/`Read`/`Write`/`Admin` → `dev.thmsn.<product>.{use,read,write,admin}`), registered idempotently on startup. |
| **Notifications / channels / log service names** | Constants reusing the component string, e.g. `Notification.Name("dev.thmsn.someproduct.newJobRequested")`, OTel `service_name = dev.thmsn.<product>.server`. |
| **Registry source** | iOS source identifier `dev.thmsn.apps` (must never change). |

## Why

A single, validated namespace makes everything greppable and collision-free: a grant, a
bundle id, a log's `service_name`, and a notification name all derive from the same
`dev.thmsn.<product>` root, so tracing a signal from a client through auth to a log line
is a substring search. The strict segment rules keep IDs URL- and filename-safe.

## Checklist

- [ ] Product has one root identifier `dev.thmsn.<product>`.
- [ ] Each component is named `dev.thmsn.<product>.<component>` — a service (`server`,
      `worker`, `cli`, `<service_name>`) or an app surface (`ios`, `macos`, `winui`, `web`).
- [ ] Rust binaries set their component id via `libproduct`'s `product_name!()`.
- [ ] Apple targets derive bundle ids from `bundleIdPrefix: dev.thmsn.<product>` (`.ios`/`.macos`).
- [ ] Auth grants use `dev.thmsn.<product>.{use,read,write,admin}` (parallel to components).
- [ ] OTel `service_name`, notification names, etc. reuse the component string.
- [ ] Deployment hostname is `<service>.dev.thmsn.dev` (distinct from the identifier).
- [ ] Any identifier validation reuses the `AppId` segment rules.
