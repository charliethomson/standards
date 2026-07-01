# Prompt: validate a standards install

Copy the block below to an agent working in the target repo. It checks the standards submodule
is wired up correctly and reports PASS / WARN / FAIL with fixes.

```
Validate that this repo is correctly wired to the shared engineering standards submodule.
Run each check, report PASS / WARN / FAIL with the evidence, then offer to fix the easy ones.

1. Submodule registered & populated
   - `cat .gitmodules` includes a [submodule "standards"] entry → path = standards.
   - the submodule `url` uses the `ssh://git@github.com/...` scheme, NOT the scp-style
     `git@github.com:...`. Git accepts both, but cargo's submodule URL parser rejects the
     scp form ("relative URL without a base"), which breaks any Rust repo that is pulled in
     as a git dependency. WARN if scp-style → fix: rewrite to `ssh://git@github.com/<owner>/standards`
     and `git submodule sync`.
   - `git submodule status` — a LEADING SPACE before the hash = initialized & clean.
     A leading `-` = not initialized → fix: `git submodule update --init standards`.
     A leading `+` = checked-out commit differs from the pinned one.
   - `test -f standards/bin/standards` → the submodule is populated (not an empty dir).

2. Identity (.standards.conf)
   - `cat .standards.conf` exists with PRODUCT, PRODUCT_UPPER, ARCHETYPE.
   - PRODUCT must be the repo's real PRODUCT identifier, which can differ from the dir name.
     Cross-check against the code: `grep -rn 'product_name!' .` and any Apple bundle id.
     WARN if PRODUCT looks like the directory name but the code says otherwise.
   - ARCHETYPE is one of: full-stack-product | library | orchestration | cli-tool | vendored-app.

3. Entrypoint (AGENTS.md)
   - Root AGENTS.md exists, points to `standards/AGENTS.md`, and its Archetype + Identifier
     (`dev.thmsn.<product>`) match .standards.conf.

4. MCP scaffold
   - `.mcp.json` exists (scaffolded by install).

5. Shared skills
   - `ls -l .claude/skills` shows symlinks `thmsn-*` → `../../standards/skills/thmsn-*`,
     and each resolves (not broken).
   - `git status --short .claude/skills` is EMPTY → the symlinks are committed (or the dir is
     in .gitignore). If they show as untracked (`??`), that's a WARN → fix: commit them
     (`git add .claude/skills`) or add `.claude/skills/` to .gitignore. Don't leave untracked.

6. Everything committed
   - `git ls-files --error-unmatch .gitmodules AGENTS.md .standards.conf .mcp.json` all succeed
     (i.e. tracked). FAIL if any is untracked or ignored.

7. Up to date with upstream
   - `git -C standards fetch -q origin main`
   - pinned = `git rev-parse HEAD:standards`; upstream = `git -C standards rev-parse origin/main`.
   - Equal → PASS. Behind → WARN: run `standards/bin/standards sync` to update (then commit the
     pointer bump + any newly-linked skills).

Summary: list each check's status. Offer to fix WARN/FAIL items (init the submodule, commit the
skill symlinks, sync to upstream). Don't change PRODUCT/archetype without confirming with me.
```

For the ongoing audit of whether the repo's *code* follows the standards (not just that the
submodule is wired up), use `/thmsn-standards` instead.
