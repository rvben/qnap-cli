.PHONY: build install release-patch release-minor release-major

build:
	cargo build --release

install: build
	cp target/release/qnap ~/.local/bin/qnap

release-patch:
	vership bump patch

release-minor:
	vership bump minor

release-major:
	vership bump major
