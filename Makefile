ARGS ?=

.PHONY: run debug build env fmt lint test clean

run:
	cargo run --release -p fr_editor -- $(ARGS)

debug:
	cargo run -p fr_editor -- $(ARGS)

build:
	cargo build --workspace --all-targets

env:
	cargo install --path crates/fr_editor --locked

fmt:
	cargo fmt --all

lint:
	cargo fmt --all --check
	cargo clippy --workspace --all-targets -- -D warnings

test:
	cargo test --workspace

clean:
	cargo clean
