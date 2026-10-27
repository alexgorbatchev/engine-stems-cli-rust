`stems` encrypts and decrypts Engine DJ sample data in local MP4 containers for audio utilities and format investigation.

# What It Does

- **Per-sample conversion**: Encrypts or decrypts each sample in the first track.
- **Payload preservation**: Retains other tracks, gaps, and unknown metadata while adjusting sizes and offsets.
- **Atomic output**: Replaces the destination only after a complete, validated conversion.

# How It Works

- Supply an input file and a separate destination in an existing directory.
- Run `stems file encrypt` or `stems file decrypt`.
- Read the sample, chunk, byte-size, and padding summary, or select JSON results with `AGENT=1`.

# How it Really Works

- The first track is converted regardless of codec. Encrypting stereo audio preserves stereo audio; it does not separate vocals, drums, bass, and instruments.
- Source files remain untouched. Existing destinations are replaced on success; source aliases, including hard links, are rejected. New Unix output has owner-only permissions.
- Whole-file processing holds intermediate copies in memory. Ctrl+C requests cancellation between samples and before publication; blocking reads, synchronization, and individual container edits complete before the next check.
- Errors appear once on stderr. Exit0 means success, exit1 means usage/conversion/output failure, and exit130 means cancellation. A stdout failure after publication can leave a successfully converted destination; inspect it before retrying.
- Agent stdout contains one JSON object with operation, paths, sample/chunk counts, byte sizes, and padding bytes. Human stdout contains labeled lines. Displayed paths fold the home directory to `~`.
- Constant and variable sample-size tables, 32/64-bit offsets, multiple media boxes, and metadata before or after media are supported. Fragmented/indexed offset structures, malformed samples, compact sample tables, and 32-bit offset overflow fail explicitly.
- The tool makes no network requests, reads no credentials or configuration files, and invokes no external programs.

# Installation

This local migration provides `bin/stems` after contributor verification. No public Rust release or remote skill destination is published.

# Quick Start

```sh
stems file decrypt song.stems song.m4a
stems file encrypt song.m4a song.stems
AGENT=1 stems file decrypt song.stems song.m4a
stems skill
```

The `skill` command prints a local Skills CLI installation command and skill-file path. Run that installation command from the local project directory.

Sample Output from a local MP4 conversion:

```text
[OK] .tmp/reference.stems -> .tmp/reference-human.mp4
Samples: 12104
Chunks: 13
Bytes: 12831327 -> 12696947
Padding bytes removed: 134380
```

# Options & Flags

| Flag | Short | Default | Description |
| :--- | :--- | :--- | :--- |
| `--help` | `-h` | `false` | Show help for the selected command. |
| `--version` | `-v` | `false` | Root only; print the raw version. |

# Compatibility

The Engine AES key matches a recorded OfflineAnalyzer block. An ordinary local MP4 round trip preserves encoded samples and decoded PCM. Authentic eight-channel stems and Engine hardware remain unverified. Linux is exercised; macOS and Windows runtime behavior is unverified. The related Go CLI's top-level verbs are represented as `file encrypt` and `file decrypt`.

# License

[MIT](LICENSE).
