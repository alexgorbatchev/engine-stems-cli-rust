`engine-stems` encrypts and decrypts Engine DJ sample data in MP4 containers for Rust audio utilities. It preserves the selected track's sample order and the other tracks' payloads.

# What It Does

- **Sample encryption**: Applies the Engine DJ AES-128 ECB key and PKCS#7 padding separately to each sample in the first track.
- **Container preservation**: Keeps gaps, unknown metadata, other tracks, and their relocated chunk offsets.
- **Atomic output**: Publishes a complete destination after validation and synchronization, leaving the input untouched.

# How It Works

1. Supply MP4 bytes or separate input and output paths, an operation, and a cancellation flag.
2. The library validates the media ranges and sample tables, converts the first track's samples, and adjusts sizes and offsets.
3. Receive the converted bytes or published file and sample, chunk, size, and padding statistics.

# How it Really Works

1. Calls are synchronous and hold the input, intermediate container, and output in memory. They read no environment variables, contact no services, and cache no state between calls.
2. Cancellation is checked between sample conversions and before publication. Blocking reads, synchronization, and an individual container edit finish before the next check. A cancellation error is identifiable as `Cancelled` through the returned error chain.
3. Files are written through a temporary file in the existing destination directory. Successful calls replace an existing destination; input aliases, including hard links, are rejected. On Unix, the new destination has owner-only permissions.
4. Supported containers use `stsz`, `stsc`, and either `stco` or `co64`. Constant-size tables, extended-size media boxes, media boxes extending to EOF, multiple media boxes, and `moov` before or after media are accepted. Tables are rewritten as variable sample sizes. A 32-bit offset overflow fails instead of promoting `stco` to `co64`.
5. Fragmented or independently indexed offset structures (`moof`, `sidx`, `iloc`, `saio`, `tfra`, `tfhd`), compact `stz2` tables, overlapping samples, and malformed encryption padding fail explicitly. Valid AES padding does not prove the input is an authentic Engine stems file.

# Installation

This implementation crate belongs to the `engine-stems-cli` repository; no Rust registry release is published. From a consumer beside that repository:

```sh
cargo add engine-stems --path ../engine-stems-cli/crates/engine-stems
```

# Quick Start

```rust
use engine_stems::{decrypt_sample, encrypt_sample};

fn main() -> anyhow::Result<()> {
    let source = b"audio sample";
    let encrypted = encrypt_sample(source);
    assert_eq!(decrypt_sample(&encrypted)?, source);
    Ok(())
}
```

# API

| Export | Description |
| :--- | :--- |
| `encrypt_sample(&[u8]) -> Vec<u8>` | Encrypts a sample, including a full padding block for aligned input. |
| `decrypt_sample(&[u8]) -> anyhow::Result<Vec<u8>>` | Decrypts a sample and validates block length and padding. |
| `transform(&[u8], Operation, &AtomicBool) -> anyhow::Result<Converted>` | Converts the first track in memory. |
| `transform_file(&Path, &Path, Operation, &AtomicBool) -> anyhow::Result<Stats>` | Converts and atomically publishes a file. |
| `Operation::{Encrypt, Decrypt}` | Selects the conversion. |
| `Converted` | Contains `bytes` and `stats`. |
| `Stats` | Reports sample/chunk counts, input/output bytes, and absolute padding bytes added or removed. |
| `Cancelled` | Typed cancellation cause. |

# Compatibility

The format key matches the recorded OfflineAnalyzer AAC block. A local stereo MP3-in-MP4 round trip preserves every encoded sample and its decoded PCM hash. Engine hardware and an authentic eight-channel encrypted stems file remain unverified.

# License

[MIT](LICENSE).
