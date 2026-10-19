# STASH-12 — Move the stack from the fs box to dev

## PLAN
- [x] 1. Read task + STASH-3/STASH-1 comments; survey both repos for fs refs
- [x] 2. Read all target files (stash: deploy/*, DESIGN.md, README.md, AGENTS.md;
       homelab: gateway/Caddyfile, fs/caddy/Caddyfile, prometheus.yml, dns/*)
- [x] 3. Verify gateway + monitoring config_files/post_deploy in homelab/komodo/sync/stacks.toml
- [x] 4. Comment on STASH-12 with PLAN (start boundary)
- [x] 5. stash repo edits: deploy/komodo/sync/stacks.toml, servers.toml,
       deploy/compose/prod.compose.yml, stash.toml, Caddyfile, DESIGN.md,
       deploy/README.md, README.md, AGENTS.md
- [x] 6. homelab repo edits: gateway/Caddyfile (add vhost), fs/caddy/Caddyfile (remove block),
       monitoring/prometheus/config/prometheus.yml, dns/AdGuardHome.yaml.template, dns/README.md
- [x] 7. /thmsn-standards-review on stash diff; fix findings
- [x] 8. /thmsn-standards-review on homelab diff; fix findings
- [x] 9. Grep proof: no stash.fs.thmsn.dev, /nfs/files, /nfs/secure, server = "fs"
- [x] 10. Commit + push stash to main
- [x] 11. Commit + push homelab to main (stage ONLY my paths; leave
        M media/README.md and M standards untouched)
- [x] 12. Finishing comment on STASH-12 with SHAs, evidence, deferrals

## STATE
Steps 1-3 done. Findings: gateway has config_files=["Caddyfile"] + post_deploy
`docker compose restart caddy` (stacks.toml:~109). monitoring has
prometheus/config/prometheus.yml in config_files + post_deploy
`docker compose restart prometheus promtail alloy otelcol tempo grafana` (~420).
BOTH ALREADY COMPLIANT — nothing to add.
dns tracks only docker-compose.yml, no config_files: AdGuardHome.yaml.template is
NOT bind-mounted, it is seeded by a pre_deploy envsubst guarded by [ -f ], so it is
deliberately untracked and already documented as inert in dns/README.md. Not changing.

DECISION made in step 2: deploy/compose/stash.toml is ALSO bind-mounted by the
repo-root dev docker-compose.yml, so changing its `path` values forces the dev
compose's container-side paths to change too. Keeping the bind identity-mapped
(host==container) as the file already did, so the sentinel path in the runbook is
the same string on the host and in config.

Steps 4-5 DONE. stash repo edited (NOT committed yet), all files validated:
YAML + TOML parse, prod publishes 127.0.0.1:8085:80, roots are
/mnt/truenas/Files + /mnt/truenas/Secure, server="dev", tags=["dev"].
LEFT ALONE deliberately: server/storage/src/path.rs uses "/nfs/files" as a unit-test
fixture for path-join semantics. Not deploy, not docs; touching server/ would trigger
the cargo gates for no behaviour change. Report it.
DESIGN.md:297 still says "a stash.fs.thmsn.dev would have needed its own rewrite" —
deliberate counterfactual, keep.
Steps 6-9 DONE. Both repos edited, NEITHER COMMITTED YET.
Validated: caddy validate passes on gateway/Caddyfile, fs/caddy/Caddyfile and
stash's deploy/compose/Caddyfile (cloudflare tls blocks stubbed to `tls internal`).
prometheus.yml + AdGuardHome template parse as YAML; all TOMLs parse.
8085 appears only in gateway/Caddyfile and prometheus.yml across homelab's tracked
files. 4-space indent kept in gateway/Caddyfile (repo convention; `caddy fmt` wants
tabs and disagrees with the pre-existing file too).
Standards review: clean. The diff CLOSES two pre-existing deviations
(deployment.md server="dev" + <product>.dev.thmsn.dev, identifiers.md:116) and I
removed the now-false AGENTS.md "Runs on the fs box" override.
Steps 10-11 DONE. BOTH REPOS PUSHED.
  stash    ccf8bec  89193c8..ccf8bec  main -> main
  homelab  09ce2ae  922a722..09ce2ae  main -> main
homelab still shows ` M media/README.md` and ` M standards` UNSTAGED, untouched —
confirmed after the push. homelab had nothing else unpushed (ed92d36 and 922a722
were already on origin/main, so STASH-1's ordering constraint on the scrape commit
is moot).
TASK COMPLETE. Finishing comment posted; state set to in_review (all file work is
done and pushed, but the sentinels + the ss -ltnp check + running the sync are the
user s). Nothing half-applied.

## TOUCHED
stash @ ccf8bec (pushed): AGENTS.md, DESIGN.md, README.md, deploy/README.md,
  deploy/compose/{Caddyfile,prod.compose.yml,stash.toml},
  deploy/komodo/sync/{servers.toml,stacks.toml}, docker-compose.yml
homelab @ 09ce2ae (pushed): gateway/Caddyfile, fs/caddy/Caddyfile,
  monitoring/prometheus/config/prometheus.yml, dns/AdGuardHome.yaml.template,
  dns/README.md, komodo/sync/variables.toml
NOT touched on purpose: server/storage/src/path.rs (a `fn root()` unit-test fixture
  still says "/nfs/files"; resolve_lexical is pure string work so the constant is
  arbitrary — changing it would pull in the cargo gates for no behaviour change).

## VERIFY
Grep in /Users/charlesthomson/git/stash (excluding .git, standards/, server/*/tests fixtures):
  grep -rn 'stash\.fs\.thmsn\|/nfs/files\|/nfs/secure\|server = "fs"' .
Grep in /Users/charlesthomson/git/homelab:
  grep -rn 'stash\.fs\.thmsn' .
Both repos: git log origin/main..HEAD empty after push.
LAST RESULT (all pass):
 stash: only stash.fs.thmsn.dev left is DESIGN.md:297 (deliberate counterfactual);
   no server = "fs"; /nfs/files only in server/storage/src/path.rs test fixtures.
 homelab: zero stash.fs.thmsn.dev. stash.dev.thmsn.dev in gateway/Caddyfile:427,
   prometheus.yml:136, dns template comment. 8085 only in those two config files.
 caddy validate: Valid configuration on all three Caddyfiles (tls stubbed).
 git log origin/main..HEAD: empty in both repos.
