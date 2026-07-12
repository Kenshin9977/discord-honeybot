SHELL := /usr/bin/env bash

# Prefer the rustup toolchain (~/.cargo/bin) over any Homebrew rustc that
# might be installed alongside it. Without this, `make run` may pick up an
# outdated Homebrew rustc and fail the MSRV check.
PATH := $(HOME)/.cargo/bin:$(PATH)
export PATH

.PHONY: help init run test ci docker docker-down clean

help: ## Show this help.
	@awk 'BEGIN{FS=":.*##";printf "Targets:\n"} /^[a-zA-Z_-]+:.*##/ {printf "  %-12s %s\n", $$1, $$2}' $(MAKEFILE_LIST)

init: ## Interactive first-time setup: prompts for the Discord token.
	@bash scripts/init.sh

run: ## Run the bot locally. Triggers `make init` first if there is no .env.
	@if [ ! -f .env ]; then $(MAKE) --no-print-directory init; fi
	@bash scripts/load-env-and-run.sh

test: ## Run all unit + handler tests (no Discord required).
	cargo test --workspace

ci: ## Mirror the GitHub Actions CI checks locally.
	cargo fmt --all -- --check
	cargo clippy --workspace --all-targets -- -D warnings
	cargo test --workspace

docker: ## Build the Docker image and start the stack with docker-compose.
	@if [ ! -f .env ]; then $(MAKE) --no-print-directory init; fi
	docker compose up --build

docker-down: ## Tear down the docker-compose stack.
	docker compose down

clean: ## Remove build artefacts and the local SQLite db.
	cargo clean
	rm -f *.db *.db-shm *.db-wal
