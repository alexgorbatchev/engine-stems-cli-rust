---
created_on: 2026-10-14 16:19
last_modified: 2026-10-14 22:40
status: current
---

# engine-stems

Synchronous first-track MP4 sample conversion for Engine DJ utilities. This internal workspace package belongs to `engine-stems-cli`; inherit `../../AGENTS.md` and run local recipes from this package directory.

## Commands

- Check: `just check`.
- Release library: `just build`.
- Explicit local reference: `ENGINE_STEMS_REFERENCE=/absolute/input.mp4 ENGINE_STEMS_EVIDENCE=.tmp/native-reference cargo test --test stems native_reference -- --ignored --nocapture`.

## Format contracts

- Use native AES/ECB padding and MP4 parser, atom codecs, and editor APIs. Reject invalid padding rather than returning garbage or silently passing ciphertext through.
- Preserve gaps and unselected tracks. Replacing a media box requires relocating every track's chunk offsets, including offsets inside that replacement. The native editor delegates replacements' offset integrity to the caller; keep the two-stage layout and equal-width offset replacement.
- Revalidate final sample tables. Reject unsupported independently indexed byte-offset structures rather than silently corrupting them.
- Preserve atomic same-directory publication, source alias rejection, and typed cancellation before publication.
- Keep recorded cipher vectors distinct from hardware verification; a synthetic or ordinary audio container does not establish device compatibility.

## Boundaries

- Any runtime behavior change requires a corresponding behavioral test-file change and at least 90% line coverage; `scripts/` is excluded. Temporarily disable the change to establish that the regression test detects it, then restore it exactly.
- Keep only standing instructions here, updating rules in place; keep progress and dated task history outside this file.
- Never publish releases, tags, packages, or deployments without explicit user authorization.
