BINARY := ttype
PREFIX ?= /usr/local
TARGET ?=

.PHONY: build test lint gate run generated man-install release-snapshot clean

build:
	cargo build --release $(if $(TARGET),--target $(TARGET))

test:
	cargo test --all-targets

lint:
	cargo clippy --all-targets -- -D warnings

gate:
	cargo fmt --all --check
	cargo clippy --all-targets -- -D warnings
	cargo test --all-targets

run:
	cargo run --

generated:
	cargo run --bin generate_artifacts -- contrib/generated
	cp contrib/generated/ttype.1 man/ttype.1

completions: generated

man-install:
	install -Dm644 man/$(BINARY).1 $(DESTDIR)$(PREFIX)/share/man/man1/$(BINARY).1

release-snapshot:
	TTYPE_RELEASE=1 cargo build --release --target x86_64-unknown-linux-musl
	TTYPE_RELEASE=1 cargo build --release --target aarch64-unknown-linux-musl

clean:
	cargo clean
