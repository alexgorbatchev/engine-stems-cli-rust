---
created_on: 2026-10-14 16:19
last_modified: 2026-10-14 22:40
status: current
---

# engine-stems-cli

The `stems` CLI consumes the internal `crates/engine-stems` package in this repository's Cargo workspace; inherit `../AGENTS.md`.

## Commands

- Run: `just run file decrypt song.stems song.m4a`.
- Agent output: `just run-ai file encrypt song.m4a song.stems`.
- Check: `just check`.
- Release binary: `just build`, producing `bin/stems`.

## Contracts

- Keep conversion in the shared library and preserve its typed cancellation and atomic publication. Output-write failure can follow a successful publication; document that outcome.
- Classify cancellation by `engine_stems::Cancelled`, returning130. Report errors at the entry boundary once.
- Keep `skills/stems/SKILL.md` synchronized in the same change as commands, arguments, flags, defaults, environment, output, or side effects; update its `last_modified` and compare it against the live command tree.
- The source has no verified public GitHub skill destination. Keep a truthful local Skills CLI pointer until an actual remote is authorized and verified; never invent a repository or release URL.
- The native interrupt test uses Linux FIFOs to establish that the child has installed its signal handler before sending SIGINT. Preserve that synchronization rather than replacing it with a timed guess.

## Boundaries

- Any runtime behavior change requires a corresponding test-file change and at least90% line coverage; `scripts/` is excluded. Disable and restore the change to establish regression detection.
- Persist standing instructions only, updating rules in place; keep progress and task history outside this file.
- Never publish releases, tags, packages, or deployments without explicit user authorization.
