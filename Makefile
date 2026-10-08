GAME ?= $(abspath ../../forge/demo)
ARGS ?=

.PHONY: run debug scene build fmt lint test clean

run:
	$(MAKE) -C "$(GAME)" run ARGS="$(ARGS)"

debug:
	$(MAKE) -C "$(GAME)" debug ARGS="$(ARGS)"

scene:
	cargo run --release -p fr_engine --example scene

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
