.PHONY: build release test test-e2e lint fmt check package verify clean install release-patch release-minor release-major

build:
	cargo build

release:
	cargo build --release

test:
	cargo nextest run

test-e2e: release
	QNAP_BIN=./target/release/qnap bash tests/e2e.sh

lint:
	cargo fmt -- --check
	cargo clippy -- -D warnings

fmt:
	cargo fmt

check:
	cargo check --locked

package:
	cargo publish --dry-run --locked --allow-dirty

verify: lint check test package

clean:
	cargo clean

install: release
	cp target/release/qnap ~/.local/bin/qnap

release-patch:
	vership bump patch

release-minor:
	vership bump minor

release-major:
	vership bump major
