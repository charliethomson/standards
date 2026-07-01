# Prompt: migrate a web app to `@thmsn/ui`

Paste the block below to an agent working in a fleet web app that currently vendors its own
shadcn/Radix components. It moves the app onto the shared **`@thmsn/ui`** library
incrementally, keeping the app green at every step. See
[`docs/web-architecture.md`](../docs/web-architecture.md) for the rule this enforces.

```
Migrate this app's UI primitives to the shared `@thmsn/ui` library
(https://npm.dev.thmsn.dev). Work INCREMENTALLY — one component per commit — and keep
`typecheck` + `build` green after every step. Do NOT do a big-bang replacement, and do not
fork upstream components into the app.

Context: `@thmsn/ui` is the fleet's shared shadcn/Radix + Tailwind v3 library (new-york
style, HSL CSS-variable tokens). It ships primitives (Button, Badge, Card, Input, Textarea,
Label, Checkbox, Switch, Separator, Skeleton, Spinner, StatusDot, Tooltip; Dialog,
AlertDialog/ConfirmDialog, DropdownMenu, Select, Tabs, ScrollArea, Toaster; ThemeToggle,
IconButton, EmptyState, Page, Avatar, Monogram), a Tailwind preset, and a default
`tokens.css`. Components are function/`forwardRef` with `data-slot` attributes; controlled
Radix Checkbox/Switch use `checked` / `onCheckedChange`; `cn` and a `useTheme` store are
exported too.

1. Install & wire the registry
   - Add / update the app `.npmrc` with the scope mapping (auth stays in ~/.npmrc via
     `npm login --registry=https://npm.dev.thmsn.dev/`; in CI, a THMSN_NPM_TOKEN):
         @thmsn:registry=https://npm.dev.thmsn.dev/
   - `bun add @thmsn/ui` and pin the current minor (`^0.x`).

2. Tailwind + tokens
   - In tailwind.config.*, add the preset and scan the package's classes:
         presets: [require('@thmsn/ui/tailwind-preset')]   // or the ESM import
         content: [..., './node_modules/@thmsn/ui/dist/**/*.js']
   - Import the tokens once at the app root, BEFORE your @tailwind directives:
         @import '@thmsn/ui/tokens.css';
     If the app has a generated branding tokens.css, import it AFTER (same variable names →
     it overrides the defaults). Reconcile any gaps (surface / success / warning / …).
   - Remove the app's now-duplicated shadcn colour/token config where it fully overlaps the
     preset. Keep only genuinely app-specific extensions.

3. Inventory the delta (REPORT before changing code)
   - List the app's local `components/ui/*` and map each to a `@thmsn/ui` export.
   - Flag components that DON'T exist upstream yet → candidates to contribute back, not fork.
   - Flag app-specific variants/props that differ from upstream (e.g. an extra Button
     variant, a different size scale) → decide per case: cover via `className`, or propose an
     upstream addition to `@thmsn/ui`.

4. Migrate component-by-component
   - Replace imports from `@/components/ui/<x>` (and the local `cn`) with `@thmsn/ui`, one
     component per commit.
   - Delete the local file once nothing imports it. Keep the app's local `cn` only if other
     code still imports it; otherwise use `cn` from `@thmsn/ui`.
   - Mind API drift: prop names, variant names, controlled vs uncontrolled, `asChild`.
   - After each component: `bun run typecheck && bun run build` (and lint/tests) must pass.

5. Theme
   - If the app hand-rolls a theme toggle/store, adopt `@thmsn/ui`'s `useTheme` +
     `ThemeToggle`, and call `initTheme()` once at startup (before first paint) to avoid a
     flash. The `Toaster` follows the store automatically.

6. Verify & report
   - Full typecheck + build + (Storybook / e2e if present). Visually spot-check dialogs,
     dropdowns, selects, and BOTH light and dark themes.
   - Summarise: components migrated, files deleted, token reconciliations, any upstream gaps
     to contribute to `@thmsn/ui`, and any intentional local overrides kept (with why).

If something is missing or wrong in `@thmsn/ui`, note it for a contribution to that repo
rather than diverging in the app.
```

For the reverse direction — improving `@thmsn/ui` itself — work in the `thmsn-ui` repo and
publish a new minor; consumers bump the pin.
