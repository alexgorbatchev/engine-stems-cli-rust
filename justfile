mod engine_stems 'crates/engine-stems/justfile'

run *args:
    cargo run --package engine-stems-cli --bin stems -- {{ args }}

run-ai *args:
    AGENT=1 cargo run --package engine-stems-cli --bin stems -- {{ args }}

build:
    cargo build --package engine-stems-cli --release --bin stems
    mkdir -p bin
    cp target/release/stems bin/stems

[private]
_test:
    mkdir -p .tmp
    cargo test --package engine-stems-cli --all-targets

[private]
_lint:
    rustfmt --check --edition 2024 --config skip_children=true src/lib.rs src/main.rs tests/cli.rs
    cargo clippy --package engine-stems-cli --all-targets -- -D warnings

coverage:
    mkdir -p .tmp
    cargo llvm-cov --package engine-stems-cli --exclude-from-report engine-stems --all-targets --fail-under-lines 90

check: _lint _test coverage
    just engine_stems check

[private]
_fmt:
    cargo fmt --package engine-stems-cli -- --check

test: _test
    just engine_stems test

lint: _lint
    just engine_stems lint

fmt: _fmt
    just engine_stems fmt-check
