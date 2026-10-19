# Migrating off the standards submodule

A repo used to vendor this repo as a `standards/` git submodule, symlink the shared skills
into `.claude/skills/`, and wire MCP servers from `.mcp.json`. Now the standards, skills and
templates come from the standards server, and the repo keeps only `.standards.conf` and one
`Standards:` line in `AGENTS.md` (see [skills-and-mcp](skills-and-mcp.md)).

This runbook moves one repo across. Do the machine setup once, then the per-repo steps for
each repo. Nothing here is destructive beyond the repo's own working tree, and `doctor
--fix` stages but never commits, so `git diff --staged` shows you everything before it lands.

## One-time machine setup

1. **Install the cli** from the registry:

   ```sh
   curl -fsSL https://standards.thmsn.dev/install | sh
   command -v standards
   ```

   `/install` redirects to the registry's installer for `dev.thmsn.standards`. Re-running
   it upgrades; so does `standards self-update`.

2. **Wire the machine:**

   ```sh
   standards doctor --fix
   ```

   Outside a git repo this runs the user-scope checks only. It:
   - registers the MCP server (`mcp.registered`):
     `claude mcp add -s user -t http standards https://standards.thmsn.dev/mcp`;
   - writes a skill stub per shared skill into `~/.claude/skills/<name>/`, each with a
     `.standards-managed` marker (`skills.synced`). Old symlinks there that point into a
     `standards/skills` checkout are replaced;
   - adds the SessionStart hook that runs `standards doctor --quiet` (`hook.session-start`);
   - removes `~/.local/bin/{standards,janitor,ultron}` symlinks into an old `standards/bin`
     (`legacy.path-links`). The cli now provides `standards janitor` and `standards ultron`.

3. **Verify:**

   ```sh
   standards doctor            # every user-scope check ok
   claude mcp list             # shows `standards` (http) as connected
   ls ~/.claude/skills/thmsn-standards/   # SKILL.md and .standards-managed
   ```

   Start a new Claude Code session and ask it to call the `standards` MCP `version` tool; it
   should answer with the server's version and commit.

If `doctor` exits 3, the server is unreachable from this machine; fix that first (it is
expected off the tailnet today).

## Per-repo runbook

Run from the repo root on a clean `main`. The examples assume the old layout; skip any step
whose leftover isn't there.

### 1. Run doctor

```sh
git pull
standards doctor            # read what it reports first
standards doctor --fix
git status
```

### What `doctor --fix` does

Repo-scope checks, by id:

| Id | Finds | `--fix` |
|---|---|---|
| `repo.conf` | `.standards.conf` missing, unparsable, or naming an unknown archetype | nothing; run `standards init` (it never overwrites) or edit the file |
| `repo.agents` | No `Standards:` line in `AGENTS.md`, or one that disagrees with `.standards.conf` | rewrites that line, or inserts it at the top |
| `legacy.submodule` | `standards` in `.gitmodules` | `git submodule deinit -f standards`, `git rm -f standards` (which also drops its `.gitmodules` entry), and removes `.git/modules/standards` |
| `legacy.skill-links` | `.claude/skills/thmsn-*` symlinks into `standards/` | `git rm`s tracked links, unlinks untracked ones |
| `legacy.mcp-json` | `.mcp.json` that is still the untouched template (a `//` key and no servers), or entries pointing at `standards/` paths | deletes the file, or just those entries |
| `legacy.ci-override` | `submodule_override: standards` in `.woodpecker/*`, `submodules: true` in `.github/workflows/*` | **warns only**, with `file:line`. doctor never rewrites CI YAML |

The `Standards:` line it writes is exactly:

```
Standards: archetype=<a> product=<p> — rules via the `standards` MCP (https://standards.thmsn.dev). Repo-specific overrides below.
```

### 2. Rewrite AGENTS.md around the line

The old `AGENTS.md` (from the previous `templates/consuming-repo-AGENTS.md`) told agents to
read `standards/AGENTS.md` and listed submodule commands. Delete that preamble and the
"Keeping standards current" section. Keep the `Standards:` line at the top and the
repo-specific overrides under it; that's all the file needs about standards. Compare with
the current template at https://standards.thmsn.dev/t/consuming-repo-AGENTS.md.

### 3. Remove CI submodule handling by hand

For each `legacy.ci-override` warning:

- **Woodpecker:** delete the `submodule_override:` entry for `standards` from the clone
  settings. Keep the rest of the `clone:` block; `partial: false` and `tags: true` are still
  what version derivation needs.

  ```yaml
  clone:
    git:
      image: woodpeckerci/plugin-git
      settings:
        partial: false
        tags: true
        submodule_override:                                            # remove
          standards: https://github.com/charliethomson/standards.git   # remove
  ```

  If the repo has other submodules, keep their `submodule_override` entries.
- **GitHub Actions:** remove `submodules: true` (or `recursive`) from `actions/checkout`
  steps, unless the repo has another submodule.

Re-run `standards doctor` until `legacy.ci-override` is clean.

### 4. Find leftover `standards/` paths

```sh
git grep -n 'standards/' -- ':!CHANGELOG.md'
```

Expect hits in places doctor doesn't own: comments like `See standards/docs/x.md`
(replace with `https://standards.thmsn.dev/d/x`), README layout rows and links to the
submodule (delete them; point at the `Standards:` line instead), scripts that call
`standards/bin/standards` (use the installed `standards` cli), and `.gitignore` entries.
Rewrite or delete each. `.standards.d/` overlays don't match and stay: they are the repo's own.

### 5. Commit

```sh
git diff --staged           # review what doctor staged plus your edits
git add -A
git commit -m "chore: consume standards via MCP"
git push
```

### 6. Verify

```sh
standards doctor --ci       # exits 0
```

- CI for the push is green; in particular the clone step works without the override.
- A fresh clone builds: `git clone <repo> /tmp/x && cd /tmp/x` has no `standards/`
  directory and nothing asks for one.
- In a new session, an agent in the repo can name its archetype from the `Standards:` line
  and fetch a checklist through the MCP.

## Checklist

- [ ] Machine: `standards` cli installed; `standards doctor` is clean at user scope.
- [ ] Machine: `claude mcp list` shows `standards` connected; skill stubs carry `.standards-managed`.
- [ ] Repo: `.standards.conf` parses and names the right archetype.
- [ ] Repo: `AGENTS.md` starts with the `Standards:` line; the old submodule preamble is gone.
- [ ] Repo: no `standards` submodule, `.git/modules/standards`, `thmsn-*` skill symlinks or
      template `.mcp.json`.
- [ ] Repo: no `submodule_override: standards` in Woodpecker and no needless
      `submodules: true` in GitHub workflows.
- [ ] Repo: `git grep 'standards/'` finds no path into the old submodule.
- [ ] Committed as `chore: consume standards via MCP`; CI green; `standards doctor --ci` exits 0.
