---
name: stems
description: Use when encrypting or decrypting Engine DJ stems samples with the stems CLI.
author: alexgorbatchev
metadata:
  created_on: 2026-10-14 16:19
  last_modified: 2026-10-14 16:19
  status: current
---

Use `AGENT=1 stems` for JSON conversion results and complete command help. Prepare a separate destination in an existing directory, then run the required conversion.

```sh
AGENT=1 stems file decrypt song.stems song.m4a
AGENT=1 stems file encrypt song.m4a song.stems
```

## Commands

| Command | Arguments and behavior |
| :--- | :--- |
| `stems` | Prints help when called without arguments. |
| `stems file` | Selects a required conversion. |
| `stems file encrypt <INPUT> <OUTPUT>` | Required path arguments; encrypts every first-track sample and replaces OUTPUT atomically on success. |
| `stems file decrypt <INPUT> <OUTPUT>` | Required path arguments; decrypts and validates every first-track sample, then replaces OUTPUT atomically. |
| `stems skill` | Prints local skill installation instructions and `skills/stems/SKILL.md`; accepts no arguments or command-specific flags. Run the printed install command from the local project directory. |
| `stems help [COMMAND]...` | Shows help for the selected command, including `stems help file encrypt`, `stems help file decrypt`, `stems help skill`, and `stems help help`. |
| `stems file help [COMMAND]...` | Shows group help, including `stems file help encrypt`, `stems file help decrypt`, and `stems file help help`. |

| Flag | Type and default | Behavior |
| :--- | :--- | :--- |
| `--help`, `-h` | bool, false | Available on each command; shows help without conversion. |
| `--version`, `-v` | bool, false | Root only, exclusive; prints the raw version and newline. |

## Output and outcomes

Read one JSON object on stdout in agent mode: `operation`, `input`, `output`, `samples`, `chunks`, `input_bytes`, `output_bytes`, `padding_bytes`. Paths use `~` for the home directory. `padding_bytes` counts bytes added when encrypting or removed when decrypting. Human stdout reports the same conversion as labeled lines. Errors appear once on stderr; runtime errors omit usage. Parse errors include usage.

Exit0 means success; exit1 means invalid usage, conversion failure, or output failure; exit130 means cancellation. If writing stdout fails after publication, the destination already contains the converted file. Inspect that destination before repeating a command.

Supply MP4 containers using `stsz`, `stsc`, and `stco` or `co64`. Constant-size tables, multiple media boxes, interleaved tracks, and `moov` before or after media are supported. Conversion preserves other tracks, gaps, and opaque metadata; rewritten headers and sample tables can change whole-file bytes even after a round trip. Input aliases of OUTPUT are rejected. Existing output survives conversion errors and cancellation before publication; new Unix output uses owner-only permissions.

Fragmented/indexed offset structures, compact `stz2` tables, overlapping samples, malformed sample tables, invalid cipher padding, and a32-bit chunk offset overflow cause errors. AES padding validity alone provides no authenticity guarantee. The first track is converted regardless of codec; ordinary MP4 conversion does not produce the four stems from a stereo recording.

Ctrl+C requests cancellation between samples and before destination publication. Blocking input or synchronization and individual container edits finish before cancellation is checked. Calls read whole files and hold intermediate copies in memory. No network, credentials, subprocesses, cache, completion command, or configuration file is used.

`AGENT` accepts `1`, `true`, or `yes` for agent mode; unset or other values select human output. Every agent help screen begins with the skill-reading alert. The `skill` pointer and raw version are identical in both modes.
