# Platform UX

## Rule

**Native per OS, designed for the target OS — not ported across it.** Each client uses its
platform's native UI technology and respects that platform's conventions. An iOS app is
designed for iOS; a macOS app is designed for macOS; neither is the other with the serial
numbers filed off, and nothing is a web view in a native shell.

What's shared is **behaviour and data**, never the view layer — see the three sources of
truth in [archetypes/full-stack-product.md](archetypes/full-stack-product.md).

## Technology per platform

| Platform | UI tech | Shared code | Min target |
|---|---|---|---|
| **Web** | React + Vite + Tailwind + shadcn/ui | the typed API client | — |
| **iOS** | SwiftUI | SPM `<Product>Kit` (Models, API, Store, UI) | iOS 18 |
| **macOS** | SwiftUI (+ a macOS design system, e.g. Luminare) | same `<Product>Kit` | macOS 15 |
| **Windows** | WinUI 3 + MVVM | `<Product>Core` (testable) | net8/net10 |

- **Web is the reference UX.** Features are proven on web, then ported natively — taking the
  *behaviour*, not the pixels.
- **Apple:** iOS and macOS share `<Product>Kit` (models, networking, view models) but have
  **separate, platform-appropriate views**. macOS-only dependencies (e.g. the Luminare
  design system) live in a separate module (`<Product>MacUI`) kept **out of** the iOS
  umbrella, so iOS never pulls macOS-only code.
- **Windows:** a thin WinUI app over a multi-targeted, testable `<Product>Core` (so the core
  runs on Linux CI).
- **Data flow is identical everywhere:** the client is **generated from the OpenAPI spec**
  (TS: openapi-typescript; Swift: swift-openapi-generator; C#: Kiota), with a CI drift check.
  No platform reimplements business logic.

## What this rules out

- ❌ Wrapping the web app in a WKWebView/WebView2 and calling it the native app.
- ❌ Shipping the macOS UI on iOS unchanged (or vice-versa).
- ❌ A cross-platform UI framework that renders the same widgets everywhere.
- ❌ Reimplementing server behaviour in a client because "it's faster locally."

## Why

The whole point of going native is that each OS has interaction idioms users expect; a
ported or wrapped UI feels wrong on at least one platform. Sharing the *logic* (via the
generated client and the shared Kit/Core) keeps behaviour consistent without dragging one
platform's design onto another. Testing follows the same seam — [testing.md](testing.md)
covers the shared core, not the views.

## Checklist

- [ ] Each client uses its native UI tech (React / SwiftUI / WinUI 3).
- [ ] iOS and macOS designs are distinct and platform-appropriate; not ports of each other.
- [ ] No web-view-in-a-shell "native" apps.
- [ ] Shared logic via the OpenAPI-generated client + a shared Kit/Core, drift-checked.
- [ ] macOS-only deps isolated from the iOS module.
- [ ] No business logic reimplemented client-side.
