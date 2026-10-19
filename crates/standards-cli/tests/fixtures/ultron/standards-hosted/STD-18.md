# STD-18 journal (worktree agent-a16da31646907e505, branch std-18)
PLAN
- [x] 1 read docs/templates/vision precedent
- [x] 2 vision comment: start + plan
- [x] 3 Dockerfile + .dockerignore; docker build + run + curl /manifest.json /health; rm image  (OK: healthy, etag=commit, user standards)
- [x] 4 deploy/ (compose, Caddyfile, komodo toml) + local override; compose up via caddy, curl, POST /mcp; down
- [ ] 5 scripts/ci from vision, .woodpecker/{rust.ci,server.build,audit.ci}.yml; lint
- [ ] 6 cli.release(.finalize).yml (wait for standards-cli on origin/main ≤15min)
- [ ] 7 README/crates README "what CI runs", CHANGELOG
- [ ] 8 commit(s), rebase, push; rm target/; vision finishing comment
STATE2 Rebased onto origin/main 74be252 (cli + html landed). Steps 5+6 written: scripts/ci/* (cli-release.sh rewritten, no codesign),
       .woodpecker/{rust.ci,server.build,audit.ci,cli.release,cli.release-finalize}.yml all lint --strict OK; shellcheck OK;
       audit gate local OK (2 high quick-xml RUSTSEC-2026-0194/0195, non-blocking); cli-release.sh PR path smoke OK on mac.
       crates/standards-cli/packaging/install.json added. Tarpaulin rerun in container std18-cov2 (image std18-cov-img; rm both after).
       First tarpaulin run failed only on commit_is_a_sha (no .git in copy) -> rerun sets RELEASE_COMMIT.
       Next: deploy/README.md, crates/README + README + CHANGELOG, commit, push, comment.
STATE  RESUMED. Steps 3+4 verified (compose via caddy :8103: health 200, manifest 200, /mcp initialize 200, bad Host 403; torn down).
       deploy/{compose/{prod,local}.compose.yml,Caddyfile},komodo/sync/{stacks,variables,servers}.toml written. deploy/README.md still TODO.
       docker-build.log in worktree root: DELETE before commit. Now step 5 (scripts/ci + pipelines).
OLD    PAUSED by ultron during step 3. Dockerfile (repo root) + .dockerignore (allowlist) WRITTEN, uncommitted, NOT yet built.
       Next: docker build --ssh default=$HOME/.ssh/id_ed25519 (ssh agent has no identities), with RELEASE_COMMIT etc.
       Get the SHA in a separate plain `git rev-parse HEAD` call first: the worktree guard rejects `$(git ...)` inside a docker command.
       Then run the container, curl /manifest.json and /health, and rm the image.
       Decisions so far: proxy loopback port 8103 (unused in homelab gateway/compose). Public ingress for standards.thmsn.dev is an STD-19 human step.
       Copying publish.sh conflicts with docs/registry-publishing.md:50; L20 overrides it, flag that in the finishing comment.
       Skip darwin codesign for the cli (it makes no keychain use). Coverage: lean to tarpaulin --engine llvm (fleet precedent), not yet tested.
TOUCHED Dockerfile, .dockerignore (uncommitted, worktree agent-a16da31646907e505, branch std-18)
VERIFY  (pending)
