SQLITE_FORMATTER_VERSION ?= 0.7.1
SQLITE_FORMATTER ?= syntaqlite
SQL_FILES := $(filter-out migrations/0005_add_poster_ids.sql,$(wildcard migrations/*.sql))

.PHONY: format format-check install-formatters setup-formatting release release-check

release:
	@test -n "$(VERSION)" || { echo "VERSION is required (use: make release VERSION=x.y.z)" >&2; exit 1; }
	python scripts/release.py $(VERSION)

release-check:
	python scripts/release.py --check

format:
	cargo fmt --all
	$(SQLITE_FORMATTER) fmt --in-place $(SQL_FILES)

format-check:
	cargo fmt --all -- --check
	$(SQLITE_FORMATTER) fmt --check $(SQL_FILES)

release:
	@if test -z "$(VERSION)"; then echo "VERSION is required (e.g. make release VERSION=0.10.0)" >&2; exit 1; fi
	python scripts/release.py $(VERSION)

release-check:
	python scripts/release.py --check

install-formatters:
	rustup component add rustfmt
	python -m pip install --upgrade pre-commit "syntaqlite==$(SQLITE_FORMATTER_VERSION)"

setup-formatting: install-formatters
	python -m pre_commit install
