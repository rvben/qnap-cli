.PHONY: build release test lint fmt check package verify clean install release-patch release-minor release-major

build:
	cargo build

release:
	cargo build --release

test:
	cargo test

lint:
	cargo fmt -- --check
	cargo clippy --all-targets --all-features -- -D warnings

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
