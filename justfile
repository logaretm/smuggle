check: fmt lint build test

fix:
    cargo fmt
    cargo clippy --locked --fix --allow-dirty -- -D warnings

fmt:
    cargo fmt -- --check

lint:
    cargo clippy --locked -- -D warnings

build:
    cargo build --locked

test:
    cargo test --locked
