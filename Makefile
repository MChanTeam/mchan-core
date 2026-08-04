SQLITE_FORMATTER_VERSION ?= 0.7.1
SQLITE_FORMATTER ?= syntaqlite
SQL_GLOB := **/*.sql

.PHONY: format format-check install-formatters setup-formatting

format:
	cargo fmt --all
	$(SQLITE_FORMATTER) fmt --in-place "$(SQL_GLOB)"

format-check:
	cargo fmt --all -- --check
	$(SQLITE_FORMATTER) fmt --check "$(SQL_GLOB)"

install-formatters:
	rustup component add rustfmt
	python -m pip install --upgrade pre-commit "syntaqlite==$(SQLITE_FORMATTER_VERSION)"

setup-formatting: install-formatters
	python -m pre_commit install
