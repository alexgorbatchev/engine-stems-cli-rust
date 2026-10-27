---
created_on: 2026-10-14 16:19
last_modified: 2026-10-14 16:19
status: current
---

# Dependency and format provenance

The source is the user's `projects/enginedj/golang` module, `github.com/agorbatchev/engine-stems` (Go 1.21, no external modules). Format evidence is the adjacent `ENGINE-STEMS-FILE-FORMAT.md` AES confirmation and recorded priming block, rather than its superseded TEA and whole-file hypotheses.

| Rust dependency | Native responsibility | License |
| :--- | :--- | :--- |
| `aes` 0.9.3, `ecb` 0.2.1 | AES-128 ECB with per-sample PKCS#7 padding | MIT OR Apache-2.0 |
| `mp4box` 0.14.0, edit feature | Container tree parsing and edits that preserve opaque data and resize ancestors | MIT |
| `mp4-atom` 0.16.2, strict feature | Native `stsz`, `stsc`, `stco`, `co64`, and media atom codecs | MIT OR Apache-2.0 |
| `same-file` 1.0.6 | Source/destination identity checks | MIT OR Unlicense |
| `tempfile` 3.27.0 | Atomic same-directory temporary-file publication | MIT OR Apache-2.0 |
| `anyhow` 1.0.104 | Contextual errors with downcastable cancellation | MIT OR Apache-2.0 |

Use the exact resolved versions in `Cargo.lock`. Consult [mp4box editing](https://docs.rs/mp4box/0.14.0/mp4box/edit/index.html), [mp4-atom](https://docs.rs/mp4-atom/0.16.2/mp4_atom/), and [RustCrypto ECB](https://docs.rs/ecb/0.2.1/ecb/) for the APIs. The installed `mp4box` source's `edit/fixup.rs` explicitly assigns replacement offset correctness to callers.

The Go writer assumes a suffix `moov`, concatenates only the first track's sample bytes, treats every sample-size table as variable, and handles only 32-bit offsets. Its decryptor accepts malformed lengths and padding. The Rust implementation validates these cases and preserves nonselected payloads. It intentionally rejects unsupported offset structures and 32-bit overflow rather than attempting an unverified conversion.
