SHELL := /usr/bin/env bash

.PHONY: help run test ci docker docker-down clean

help: ## Show this help.
	@awk 'BEGIN{FS=":.*##";printf "Targets:\n"} /^[a-zA-Z_-]+:.*##/ {printf "  %-12s %s\n", $$1, $$2}' $(MAKEFILE_LIST)

run: ## Run the bot locally; sources .env first.
	@test -f .env || (echo "no .env — copy .env.example and fill in DISCORD_TOKEN"; exit 1)
	@set -a && source .env && set +a && cargo run

test: ## Run all unit + handler tests (no Discord required).
	cargo test --workspace

ci: ## Mirror the GitHub Actions CI checks locally.
	cargo fmt --all -- --check
	cargo clippy --workspace --all-targets -- -D warnings
	cargo test --workspace

docker: ## Build the Docker image and start the stack with docker-compose.
	docker compose up --build

docker-down: ## Tear down the docker-compose stack.
	docker compose down

clean: ## Remove build artefacts and the local SQLite db.
	cargo clean
	rm -f *.db *.db-shm *.db-wal
