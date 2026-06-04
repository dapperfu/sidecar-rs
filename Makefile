.PHONY: clean build run test lint format format-fix check help \
        py-venv py-install py-build py-test py-wheel py-notebook py-notebook-run py-clean

CARGO := cargo
export CARGO_TARGET_DIR ?= $(CURDIR)/target

UV := uv
VENV := venv_sidecar-rs
VENV_ABS := $(CURDIR)/$(VENV)
PY := $(VENV)/bin/python

help:
	@echo "Targets:"
	@echo "  make build           - Build release binaries"
	@echo "  make run             - Show sidecar CLI help"
	@echo "  make test            - Run workspace tests"
	@echo "  make lint            - Run clippy"
	@echo "  make format          - Check formatting"
	@echo "  make format-fix      - Apply rustfmt"
	@echo "  make check           - cargo check workspace"
	@echo "  make clean           - Remove build artifacts"
	@echo "  make py-venv         - Create the Python venv ($(VENV))"
	@echo "  make py-install      - Install the package into the venv (uv pip install .)"
	@echo "  make py-build        - Build/install in editable dev mode (maturin develop)"
	@echo "  make py-test         - Run the Python test suite (pytest)"
	@echo "  make py-wheel        - Build a release wheel (maturin build --release)"
	@echo "  make py-notebook     - Launch the demo notebook in Jupyter"
	@echo "  make py-notebook-run - Execute the demo notebook headlessly"
	@echo "  make py-clean        - Remove the venv and Python build artifacts"

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

py-venv:
	$(UV) venv $(VENV)

py-install: py-venv
	VIRTUAL_ENV=$(VENV_ABS) $(UV) pip install .

py-build: py-venv
	VIRTUAL_ENV=$(VENV_ABS) $(UV) pip install maturin
	VIRTUAL_ENV=$(VENV_ABS) $(VENV)/bin/maturin develop

py-test: py-install
	VIRTUAL_ENV=$(VENV_ABS) $(UV) pip install pytest
	$(PY) -m pytest tests

py-wheel: py-venv
	VIRTUAL_ENV=$(VENV_ABS) $(UV) pip install maturin
	VIRTUAL_ENV=$(VENV_ABS) $(VENV)/bin/maturin build --release

py-notebook: py-install
	VIRTUAL_ENV=$(VENV_ABS) $(UV) pip install jupyter nbconvert
	$(PY) -m jupyter notebook notebooks/sidecar_demo.ipynb

py-notebook-run: py-install
	VIRTUAL_ENV=$(VENV_ABS) $(UV) pip install jupyter nbconvert
	$(PY) -m jupyter nbconvert --to notebook --execute --inplace notebooks/sidecar_demo.ipynb

py-clean:
	rm -rf $(VENV) dist *.egg-info python/*.egg-info
