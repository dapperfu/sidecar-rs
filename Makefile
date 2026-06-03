.PHONY: clean build run test lint format format-fix check help

CARGO := cargo
export CARGO_TARGET_DIR ?= $(CURDIR)/target

help:
	@echo "Targets:"
	@echo "  make build       - Build release binaries"
	@echo "  make run         - Show sidecar CLI help"
	@echo "  make test        - Run workspace tests"
	@echo "  make lint        - Run clippy"
	@echo "  make format      - Check formatting"
	@echo "  make format-fix  - Apply rustfmt"
	@echo "  make check       - cargo check workspace"
	@echo "  make clean       - Remove build artifacts"

clean:
	$(CARGO) clean

build:
	$(CARGO) build --release

run: build
	./target/release/sidecar --help

test:
	$(CARGO) test --workspace

lint:
	$(CARGO) clippy --workspace -- -D warnings

format:
	$(CARGO) fmt --all -- --check

format-fix:
	$(CARGO) fmt --all

check:
	$(CARGO) check --workspace
