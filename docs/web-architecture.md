# Web & client architecture

## Rule

Web clients use **React + Vite + Tailwind + shadcn/Radix**, with a clean split: **server state
in TanStack Query**, **ephemeral UI state in Zustand**, and a **typed client generated from the
OpenAPI spec**. Native clients share a domain Kit and keep platform-specific design systems
isolated. This is the implementation detail beneath [platform-ux.md](platform-ux.md).

Shared UI primitives come from the fleet package **`@thmsn/ui`** (private registry), not a
per-app copy of shadcn — see [Shared component library](#shared-component-library-thmsnui).

## State split (the core rule)

- **Server state → TanStack Query.** One `QueryClient` (e.g. `staleTime: 10_000`, no refetch on
  focus). A **central query-key registry** (`qk.orders`, `qk.order(id)`,
  `qk.items(orderId)`) is the single source of keys so API reads, route-loader prefetch, and
  real-time invalidation all agree. Mutations invalidate the relevant keys on success.
- **UI state → Zustand.** One store per concern (theme, panes, in-flight draft). Ephemeral
  chrome only.
- **Never duplicate server state into Zustand.** Entities live in Query; Zustand holds only
  what the server doesn't own.

## API client

Generate from the committed OpenAPI spec ([contracts.md](contracts.md)): `openapi-typescript` →
`schema.d.ts`, consumed by **`openapi-fetch`** (`createClient<paths>({ baseUrl })`). A middleware
injects `Authorization: Bearer` on every request; asset/WS URLs thread the token as a query
param ([security.md](security.md)). Prefer this typed pipeline over hand-rolled `fetch` or
axios/orval.

## Real-time

Open the WebSocket (`/ws`), apply `{kind, data}` frames idempotently by id, and **invalidate
the matching `qk` keys** so Query refetches — the key registry is what keeps the socket and the
cache coherent. On a dropped frame, refetch via REST.

## Dev proxy (same-origin)

Vite proxies `/api` and `/ws` to the server so the browser is same-origin (no CORS):

```ts
server: { proxy: {
  '/api': { target: 'http://localhost:8080', changeOrigin: true },
  '/ws':  { target: 'http://localhost:8080', ws: true, changeOrigin: true },
}}
```

Production mirrors this via the internal Caddy router ([deployment.md](deployment.md)).

## Shared component library (`@thmsn/ui`)

Don't re-vendor shadcn primitives per app. The fleet ships them once as **`@thmsn/ui`**,
published to the private registry at **`https://npm.dev.thmsn.dev`** — shadcn/Radix on
Tailwind v3 (new-york style), with HSL CSS-variable tokens, a Tailwind **preset**, and a
default **`tokens.css`**. New apps depend on it; existing apps migrate to it (paste
[`prompts/migrate-to-thmsn-ui.md`](../prompts/migrate-to-thmsn-ui.md)).

- **Consume:** scope the registry in `.npmrc` (`@thmsn:registry=https://npm.dev.thmsn.dev/`;
  auth via `npm login`), `bun add @thmsn/ui`, add `@thmsn/ui/tailwind-preset` to `presets`
  (and `./node_modules/@thmsn/ui/dist/**/*.js` to Tailwind `content`), and
  `@import '@thmsn/ui/tokens.css'` at the app root.
- **Theme via tokens, not forks.** The primitives are token-driven; a product restyles them
  by shipping its generated `tokens.css` ([branding.md](branding.md)) *after* the default
  import — same variable names, different values. Never fork a component to change its colour.
- **Extend, don't fork.** Compose with `className` / `asChild` for app-specific needs; if a
  primitive is missing or wrong, contribute it upstream to `@thmsn/ui` rather than copying it
  back into the app. App-specific composites live in the app's `app-ui` layer.

## Monorepo packages

Web shared code is split into workspace packages when it's reused: **`@thmsn/ui`** provides the
shared primitives (from the registry — no local `ui` package to maintain), `app-ui` (higher
composites + the Zustand store) builds on it, `client` (API client), `types` (DTOs), plus
domain logic packages. A single-surface app can stay flat (`src/api`, `src/store`,
`src/router`).

## Native shared code

- **Apple:** an SPM `<Product>Kit` with separate products — `…API` (generated client +
  networking), `…Kit` (WS protocol, sync store, view models), `…UI` (shared SwiftUI). A
  macOS-only design system (e.g. Luminare) lives in a separate `…MacUI` product **kept out of
  the iOS umbrella**, so it never leaks into iOS builds.
- **Windows:** a testable `<Product>Core` (Api/Realtime/State/Protocol/ViewModels) under a thin
  WinUI app.
- Both center on a `ServerConfig { origin, token }` that derives the API base, asset URL (token
  in query), and WebSocket URL, plus a bearer auth provider. Credentials in the platform
  keychain ([security.md](security.md)).

## Checklist

- [ ] React + Vite + Tailwind + shadcn/Radix, with primitives from **`@thmsn/ui`** (preset +
      `tokens.css` wired), not re-vendored per app.
- [ ] Server state in TanStack Query with a central `qk` key registry; UI state in Zustand; no duplication.
- [ ] Typed client via `openapi-typescript` + `openapi-fetch`, generated from the committed spec.
- [ ] WebSocket frames invalidate `qk` keys; resync over REST on gaps.
- [ ] Same-origin Vite dev proxy for `/api` + `/ws`.
- [ ] Native: shared Kit (API/Kit/UI), platform design systems isolated, `ServerConfig` + keychain.
