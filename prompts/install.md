# Prompt: install the standards into a repo

Copy everything in the block below to an agent working in the target repo.

```
Wire this repo to the shared engineering standards (vendored as a git submodule).

1. Add the submodule:
   git submodule add git@github.com:charliethomson/standards standards

2. Work out this repo's identity for the installer:
   - product slug — short lowercase product name → becomes dev.thmsn.<slug>. The product
     name can differ from the repo/dir name; if unsure, check the existing code's
     identifiers (product_name!(...), Apple bundle ids) and use that.
   - env prefix — the slug uppercased; pass --upper <ENV> only if it differs.
   - archetype — pick one: full-stack-product (Rust/Poem server + clients), library (a
     shared lib* crate), orchestration (infra/deploy repo), or cli-tool (a published
     binary). Infer it from the repo; if genuinely ambiguous, ask me.
   - lib name — only if this repo *is* a library: pass --lib <name>.

3. Run the installer (add --upper / --lib if needed):
   standards/bin/standards install --product <slug> --archetype <type>

4. Read standards/AGENTS.md (the entrypoint) and standards/docs/archetypes/<type>.md.
   Then briefly note how well this repo already follows the standards — flag gaps, don't
   fix them yet.

5. Commit everything the install produced:
   - .gitmodules and the standards submodule pointer
   - AGENTS.md, .standards.conf, .mcp.json
   - the .claude/skills/ symlinks the installer created (so the skills work on a fresh
     clone). If you'd rather not track them, add .claude/skills/ to .gitignore instead —
     don't leave them untracked.

After this: `standards/bin/standards sync` pulls future updates, and
`/thmsn-standards scan this repo and fix all findings` audits + fixes against the
standards. Don't hand-edit the generated AGENTS.md beyond its "repo-specific overrides"
section.
```

## Terser variant

```
Add the standards submodule and wire it up:
  git submodule add git@github.com:charliethomson/standards standards
  standards/bin/standards install --product <slug> --archetype <full-stack-product|library|orchestration|cli-tool>
Pick <slug> from the repo's real product identifier (product_name!/bundle id), not just
the dir name. Then read standards/AGENTS.md, and commit .gitmodules + the standards
pointer + AGENTS.md + .standards.conf + .mcp.json + the .claude/skills/ symlinks.
```
