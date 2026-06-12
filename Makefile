.PHONY: help build test fmt clippy run doctor clean install

BIN := target/release/scaff
ifeq ($(OS),Windows_NT)
	BIN := target/release/scaff.exe
endif

help:
	@echo "scaff - Harness Research Agent"
	@echo ""
	@echo "Targets:"
	@echo "  build       - cargo build --release"
	@echo "  test        - cargo test"
	@echo "  fmt         - cargo fmt --all"
	@echo "  clippy      - cargo clippy --all-targets -- -D warnings"
	@echo "  run         - run scaff with the bundled args (set QUERY=... to override)"
	@echo "  doctor      - run scaff doctor"
	@echo "  install     - copy the release binary to ~/.cargo/bin/scaff"
	@echo "  clean       - cargo clean"

build:
	cargo build --release

test:
	cargo test --all

fmt:
	cargo fmt --all

clippy:
	cargo clippy --all-targets -- -D warnings

run: build
	$(BIN) $(QUERY)

doctor: build
	$(BIN) doctor

install: build
	@mkdir -p $(HOME)/.cargo/bin
	@cp $(BIN) $(HOME)/.cargo/bin/scaff
	@echo "Installed to $(HOME)/.cargo/bin/scaff"

clean:
	cargo clean
