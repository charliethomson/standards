Program `standards-hosted`. Read `~/.local/state/thmsn/ultron/standards-hosted/PROGRAM.md` (LOCKED DECISIONS) and `SPECS.md` beside it before starting. Repo: charliethomson/standards, commit straight to main.

## Goal
Port bin/ultron to `standards ultron`, behaviour-identical (L16).

## Do
Every parsing rule in SPECS.md for ultron, same state path, same commands and output. Golden tests: copy a few real ~/.local/state/thmsn/ultron/*/PROGRAM.md files into fixtures and assert ls/status output matches the bash script's output. watch: alt screen, fingerprint redraw, non-TTY prints once. gc prompts. bin/ultron stays (deprecated header) until the fleet sweep (L17).

## Done when
Golden tests pass against ≥3 real programs. (The skill rewrite task owns SKILL.md edits.)
