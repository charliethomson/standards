---
name: thmsn-standards-contribute
description: Make a change to the shared engineering standards from a consuming repo and push it upstream, genericized. Use when the user wants to improve/add/amend a standard, fix something in the standards docs or templates, or "contribute this back to the standards". Pairs with standards/bin/standards contribute.
---

# thmsn-standards-contribute — author & push a standards change

Edit the standards in place (the `standards/` submodule), keep them **product-neutral**, and
push the change upstream — then bump the pointer in this repo. The standards repo must never
contain a real product name.

## 1. Locate what to change

- The standards live in `standards/docs/` (the rules) and `standards/templates/` (copyable
  scaffolding). Find the right file — the index is `standards/docs/overview.md`.
- Read the doc first. Each standard follows one shape: **rule → why → implementation →
  checklist**. Match it; update the `## Checklist` if you change a rule. If a behaviour change
  also has a copyable artifact, update the matching file under `standards/templates/`.

## 2. Write it generically

- Use the placeholders, never this repo's real names: product → `someproduct` /
  `dev.thmsn.someproduct`, env prefix → `SOMEPRODUCT_`, a library → `libsomeproduct`. Template
  files use `{{PRODUCT}}` / `{{PRODUCT_UPPER}}`.
- Use **neutral example vocabulary** for sample code (`Order`/`Item`, `OrdersApi`,
  `Id<Order>`, a `payments` adapter) — not domain nouns from this product.
- **Keep real shared infrastructure real** — `auth`, `registry`, `agentutil`, Komodo, homelab,
  `apps.dev.thmsn.dev`, the `lib*` toolchain (`liberror`, `liblog`, `libpath`, …), `charliethomson`
  URLs. Those are the actual platform, not examples; do not anonymize them.

## 3. Verify, then push

```sh
standards/bin/standards lint        # confirms none of THIS repo's own identifiers leaked in
standards/bin/standards contribute -m "<concise change description>"
```

`contribute` auto-genericizes this repo's own identifiers in the files you changed, re-checks,
commits + pushes to the standards repo's `main`, and stages the submodule pointer bump here.

- If `lint`/`contribute` reports names it couldn't auto-genericize, they're likely **domain
  nouns** you introduced — replace them with neutral ones by hand and re-run.
- After it succeeds, commit the pointer bump in this repo:
  `git commit -m "chore: update standards"`.

## Notes

- The standards follow the same workflow they describe: commit straight to `main`.
- Don't add anything that lists real product names into the standards repo (no denylists,
  no "as used in <a real product>" references). Genericity is the contract.
- If the change is large or reshapes a standard's intent, draft the new rule + why + checklist
  and show the user the diff before pushing.
