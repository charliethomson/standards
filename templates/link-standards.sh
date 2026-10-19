#!/usr/bin/env bash
# DEPRECATED: legacy submodule mechanism, kept only so unmigrated repos keep working
# until the fleet sweep. Skills and MCP now come from https://standards.thmsn.dev via the
# `standards` cli (`standards doctor --fix`). Don't run this in a new repo; to move a repo
# off the submodule follow https://standards.thmsn.dev/d/migration.
#
# Wire the standards submodule into a consuming repo:
#   - symlink every shared skill into .claude/skills/
#   - scaffold .mcp.json from the template if absent
#
# Run from the repo root:  ./standards/templates/link-standards.sh
# Idempotent — safe to re-run after `git submodule update --remote`.
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

if [ ! -d standards/skills ]; then
  echo "error: standards/ submodule not found (run from repo root)." >&2
  exit 1
fi

# --- skills: relative symlinks into .claude/skills/ ---
mkdir -p .claude/skills
for skill in standards/skills/*/; do
  name="$(basename "$skill")"
  [ "$name" = "README.md" ] && continue
  [ -f "$skill/SKILL.md" ] || continue
  ln -sfn "../../standards/skills/$name" ".claude/skills/$name"
  echo "linked skill: $name"
done

# --- mcp: scaffold .mcp.json once (never clobber an existing one) ---
if [ ! -f .mcp.json ] && [ -f standards/templates/mcp.json ]; then
  cp standards/templates/mcp.json .mcp.json
  echo "scaffolded .mcp.json (edit to enable the servers you want)"
fi

echo "done."
