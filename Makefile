WASM := target/wasm32v1-none/release/ownable_timelock.wasm

default: build

build:
	stellar contract build --locked
	@ls -l $(WASM)
	@sha256sum $(WASM)

test:
	cargo test --locked

coverage:
	cargo llvm-cov --locked --summary-only

fmt:
	cargo fmt --all

clean:
	cargo clean

.PHONY: default build test coverage fmt clean
