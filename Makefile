EXAMPLE ?= demo
ARGS ?=

.PHONY: run debug build fmt lint test clean

run:
	cargo run --release -p fr_engine --example $(EXAMPLE) -- $(ARGS)

debug:
	cargo run -p fr_engine --example $(EXAMPLE) -- $(ARGS)

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
