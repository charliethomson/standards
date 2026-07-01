# Branding

## Rule

Each product carries a **self-contained branding package**: one **code-as-truth generator**
at top-level **`branding/`** that owns the brand and emits **everything** — the icon
ladders for every platform *and* the color/design tokens for every platform — from
definitions held **in code**. Generated assets are committed; a CI drift check keeps them
honest. Never hand-edit a generated asset or token file.

This is the someproduct generator approach (Python, geometry-and-palette-in-code), extended to
cover all platforms' icons and tokens in one place.

## What the generator owns

`branding/build.py` is the single source of truth. It holds, in code:

- **The palette** — standardized brand colors (`primary`, `secondary`, `tertiary`) plus the
  semantic token set the UI needs (`background`, `foreground`, `surface`, `muted`, `border`,
  `input`, `ring`, `destructive`, and any reserved status colors), each with a **dark** and
  **light** value. Dark is the default scheme.
- **The mark geometry** — the icon artwork as code (gradient stops, shapes), so the icon is
  reproducible and tweakable by editing constants, not by editing pixels.
- **Type** — the font stacks (display/sans/mono).

From those it generates, into each platform's tree:

| Output | Platform | Form |
|---|---|---|
| App icon ladder | iOS (1024), macOS (16–1024), Windows | PNG + `Contents.json` / `.ico` |
| Favicons / PWA icons | Web | `favicon.svg`, `favicon-16/32.png`, `favicon.ico`, `apple-touch-icon.png`, `pwa-192/512.png` |
| Color tokens | Web | CSS custom properties (`:root` light + `.dark` overrides) |
| Color tokens | Apple | SwiftUI `Color` palette (dark/light) |
| Color tokens | Windows | WinUI `ResourceDictionary` with `ThemeDictionaries` |

Master SVGs are rendered with `rsvg-convert` (`magick` only for previews). The HSL palette
is converted to RGB for SwiftUI and hex for WinUI by the generator.

## Workflow

1. Edit constants in `branding/build.py` (a color value, a gradient stop, the geometry).
2. Run `python3 branding/build.py` from the repo root.
3. Commit the source **and** all regenerated outputs together.
4. CI drift check: re-run the generator and `git diff --exit-code` the generated icon/token
   files — a diff fails the build (same pattern as the OpenAPI codegen check in
   [ci-cd.md](ci-cd.md)).

## Conventions

- **Code wins.** If `docs`/design notes and the generator disagree, the generator is the
  source of truth. Document intent in `branding/README.md`; keep values in code.
- **Reserve status colors.** A color with a semantic meaning (e.g. a "live"/active-status
  accent) is used **only** for that meaning, never decoratively (e.g. reserve a coral accent
  for an active/live state, never for decoration).
- **Never hardcode surface/text colors in a client.** Consume the generated tokens so
  light/dark switches automatically. On web, the shared **`@thmsn/ui`** package ships the
  default token set ([web-architecture.md](web-architecture.md)); a product's generated
  `tokens.css` overrides it with the **same variable names** — so the generator's web output
  stays a drop-in swap, not a fork.
- **Standardized triad.** `primary`/`secondary`/`tertiary` are the brand seeds every product
  defines; the semantic tokens derive the rest of the system from them.

Template: [`templates/branding/build.py`](../templates/branding/build.py) — a starter
all-platform generator. Replace the palette, geometry, and output paths for the product.

## Checklist

- [ ] `branding/build.py` exists at top level and is the single source of truth.
- [ ] Palette (incl. `primary`/`secondary`/`tertiary`, dark+light) and geometry live in code.
- [ ] It emits icons for **every** platform the product ships.
- [ ] It emits color tokens for **every** platform (web CSS, Swift, WinUI XAML).
- [ ] Generated outputs are committed; a CI drift check guards them.
- [ ] Reserved status colors aren't used decoratively; clients consume tokens, not literals.
