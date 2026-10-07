EXAMPLE ?= ui

.PHONY: run debug build fmt lint test clean

run:
	cargo run --release -p fr_engine --example $(EXAMPLE)

debug:
	cargo run -p fr_engine --example $(EXAMPLE)

build:
	cargo build --workspace --all-targets

fmt:
	cargo fmt --all

lint:
	cargo fmt --all --check
	cargo clippy --workspace --all-targets -- -D warnings

test:
	cargo test --workspace

clean:
	cargo clean
