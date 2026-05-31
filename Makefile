.PHONY: help test install dev-install build build-pyz clean lint format type-check
.PHONY: rust-build rust-release rust-clean rust-install all

help:
	@echo "scaff - AI Agent Project Scaffolder"
	@echo ""
	@echo "Available targets:"
	@echo "  test            - Run all Python tests"
	@echo "  install         - Install scaff (Python)"
	@echo "  dev-install     - Install scaff in development mode with test dependencies"
	@echo "  build           - Build wheel and source distribution (Python)"
	@echo "  build-pyz       - Build standalone Python executable (Unix only)"
	@echo "  clean           - Remove Python build artifacts and cache"
	@echo "  lint            - Run linter (flake8)"
	@echo "  format          - Format code (black)"
	@echo "  type-check      - Run type checker (mypy)"
	@echo "  rust-build      - Build Rust CLI (debug)"
	@echo "  rust-release    - Build Rust CLI (release, optimized)"
	@echo "  rust-clean      - Clean Rust build artifacts"
	@echo "  rust-install    - Install Rust CLI to ~/.cargo/bin"
	@echo "  all             - Run all checks and build both Python + Rust"

# Cross-platform python invocation
PYTHON := python

test:
	$(PYTHON) -m pytest tests/ -v --tb=short

test-cov:
	$(PYTHON) -m pytest tests/ -v --cov=scaff --cov-report=term-missing

install:
	pip install .

dev-install:
	pip install -e ".[dev]"
	pip install pytest pytest-cov black flake8 mypy

build: test
	$(PYTHON) build.py

build-pyz: build
	@echo "Standalone executable created at dist/scaff.pyz"
	@echo "Note: .pyz files require Python. On Windows use 'python dist/scaff.pyz'."

clean:
	$(PYTHON) -c "import shutil, pathlib; [shutil.rmtree(p, ignore_errors=True) for p in [pathlib.Path('build'), pathlib.Path('dist'), pathlib.Path('.pytest_cache')]]; [f.unlink() for f in pathlib.Path('.').rglob('*.pyc')]; [shutil.rmtree(d, ignore_errors=True) for d in pathlib.Path('.').rglob('__pycache__') if d.is_dir()]"
	$(PYTHON) -c "import shutil, pathlib; [shutil.rmtree(d, ignore_errors=True) for d in pathlib.Path('.').glob('*.egg-info')]"

lint:
	flake8 scaff tests --max-line-length=100

format:
	black scaff tests

type-check:
	mypy scaff --ignore-missing-imports

# --- Rust targets ---
rust-build:
	cargo build

rust-release:
	cargo build --release

rust-clean:
	cargo clean

rust-install: rust-release
	@echo "Installing Rust CLI to ~/.cargo/bin/scaff"
	cp target/release/scaff ~/.cargo/bin/

# --- Combined ---
all: clean lint type-check test build rust-release
	@echo "All checks passed and both builds complete!"
