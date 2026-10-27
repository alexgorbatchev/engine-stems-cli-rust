---
created_on: 2026-10-14 16:19
last_modified: 2026-10-14 22:40
status: current
---

# Native CLI dependencies

The internal `crates/engine-stems` package owns the format conversion; consult its [dependency provenance](crates/engine-stems/DEPENDENCIES.md). `cli-command-kit` supplies native Clap argument parsing, command-tree help, agent alerts, path folding, and local skill output. `serde_json` 1.0.154 writes machine results. `signal-hook` 0.4.5 registers SIGINT directly into the shared atomic cancellation flag ([official API](https://docs.rs/signal-hook/0.4.5/signal_hook/flag/fn.register.html)); it does not run filesystem work in a signal handler. `anyhow` preserves typed causes.

Runtime dependencies carry MIT, Apache-2.0 alternatives, or compatible upstream licensing as recorded in their manifests; resolved versions are locked in `Cargo.lock`. Native regression fixtures use `mp4-atom`, `tempfile`, and Linux `rustix` FIFO/signal APIs.

The source has a private remote and no verified public GitHub repository. Accordingly the local skill pointer names the actual local project skill rather than fabricating a GitHub URL; [Skills CLI supports local paths](https://github.com/vercel-labs/skills#supported-sources). No release artifact or registry publication is claimed.
