COMPOSE_DEV          	:= dev/docker/docker-compose.yml
COMPOSE_DEBUG_SCRAPER	:= dev/docker/docker-compose.debug-scraper.yml
COMPOSE_E2E          	:= dev/docker/docker-compose.e2e.yml
ENV_FILE             	:= .env
CHROME_PATH          	?= /Applications/Google Chrome.app/Contents/MacOS/Google Chrome
RUNS                 	?= 10
E2E_LOOP_DIR         	:= /tmp/e2e-loop-results
AGENT_WORKSPACE     	?=

.PHONY: dev-up dev-down api webui docs e2e-up e2e-down e2e-test e2e-test-loop docs-screenshots docs-screenshots-loop agent-push agent-pull clean

## ---------- Development ----------

dev-up: ## Start dev infrastructure (DB, Kratos, Retrack). Use BUILD=1 to rebuild images.
	docker compose -f $(COMPOSE_DEV) --env-file $(ENV_FILE) up $(if $(BUILD),--build) -d

dev-down: ## Stop dev infrastructure and remove volumes.
	docker compose -f $(COMPOSE_DEV) --env-file $(ENV_FILE) down --volumes --remove-orphans

dev-logs: ## Tail logs from dev infrastructure.
	docker compose -f $(COMPOSE_DEV) logs -f

api: ## Run the Secutils API on the host.
	cargo run

webui: ## Run the Web UI dev server on the host.
	npm --prefix components/secutils-webui run watch

docs: ## Run the documentation dev server on the host.
	npm --prefix components/secutils-docs run watch

## ---------- Debug ----------

dev-debug-scraper: ## Start infra with web scraper routed to host. Use BUILD=1 to rebuild images.
	docker compose -f $(COMPOSE_DEV) -f $(COMPOSE_DEBUG_SCRAPER) --env-file $(ENV_FILE) up $(if $(BUILD),--build) -d

scraper-setup: ## Install web scraper npm dependencies.
	cd components/retrack && npm install

scraper: ## Run web scraper on host (headed browser).
	cd components/retrack && \
	RETRACK_WEB_SCRAPER_BROWSER_CHROMIUM_NO_HEADLESS=true \
	RETRACK_WEB_SCRAPER_BROWSER_CHROMIUM_EXECUTABLE_PATH="$(CHROME_PATH)" \
	npm run watch -w components/retrack-web-scraper

## ---------- End-to-End Testing ----------

e2e-up: ## Start the full e2e stack (all services in Docker). Use BUILD=1 to rebuild images.
	docker compose -f $(COMPOSE_DEV) -f $(COMPOSE_E2E) up $(if $(BUILD),--build) -d

e2e-down: ## Stop the e2e stack and remove volumes.
	docker compose -f $(COMPOSE_DEV) -f $(COMPOSE_E2E) down --volumes --remove-orphans

e2e-test: ## Run Playwright e2e tests (use ARGS for extra flags, e.g. make e2e-test ARGS="--ui").
	cd e2e && npx playwright test $(ARGS)

e2e-test-loop: ## Run e2e tests repeatedly (RUNS=N default 10, ARGS=...). Logs + failure screenshots → /tmp/e2e-loop-results/.
	@rm -rf $(E2E_LOOP_DIR) && mkdir -p $(E2E_LOOP_DIR); \
	pass=0; fail=0; \
	for i in $$(seq 1 $(RUNS)); do \
		echo "--- Run $$i/$(RUNS) ---"; \
		rm -rf e2e/test-results; \
		if (cd e2e && npx playwright test $(ARGS)) > $(E2E_LOOP_DIR)/run-$$i.log 2>&1; then \
			pass=$$((pass+1)); echo "PASS"; \
		else \
			fail=$$((fail+1)); echo "FAIL  →  $(E2E_LOOP_DIR)/run-$$i.log"; \
			[ -d e2e/test-results ] && cp -r e2e/test-results $(E2E_LOOP_DIR)/artifacts-run-$$i || true; \
		fi; \
	done; \
	echo "=== Results: $$pass/$(RUNS) passed, $$fail/$(RUNS) failed ==="; \
	echo "Logs and artifacts: $(E2E_LOOP_DIR)"

e2e-report: ## Open the Playwright HTML report.
	cd e2e && npx playwright show-report

e2e-logs: ## Tail logs from e2e stack.
	docker compose -f $(COMPOSE_DEV) -f $(COMPOSE_E2E) logs -f

## ---------- Documentation ----------

docs-screenshots: ## Regenerate doc screenshots (requires e2e stack running). Use ARGS for extra flags.
	cd e2e && npx playwright test --config playwright.docs.config.ts $(ARGS)

docs-screenshots-loop: ## Run docs screenshot tests repeatedly (RUNS=N default 10, ARGS=...). Logs + failure screenshots → /tmp/e2e-loop-results/.
	@rm -rf $(E2E_LOOP_DIR) && mkdir -p $(E2E_LOOP_DIR); \
	pass=0; fail=0; \
	for i in $$(seq 1 $(RUNS)); do \
		echo "--- Run $$i/$(RUNS) ---"; \
		rm -rf e2e/test-results; \
		if (cd e2e && npx playwright test --config playwright.docs.config.ts $(ARGS)) > $(E2E_LOOP_DIR)/run-$$i.log 2>&1; then \
			pass=$$((pass+1)); echo "PASS"; \
		else \
			fail=$$((fail+1)); echo "FAIL  →  $(E2E_LOOP_DIR)/run-$$i.log"; \
			[ -d e2e/test-results ] && cp -r e2e/test-results $(E2E_LOOP_DIR)/artifacts-run-$$i || true; \
		fi; \
	done; \
	echo "=== Results: $$pass/$(RUNS) passed, $$fail/$(RUNS) failed ==="; \
	echo "Logs and artifacts: $(E2E_LOOP_DIR)"

## ---------- Database ----------

db-reset: ## Drop, create, and migrate the dev database.
	sqlx database drop -y
	sqlx database create
	sqlx migrate run

## ---------- Docker Images ----------

docker-api: ## Build the Secutils API Docker image.
	docker build --tag secutils-api:latest .

docker-webui: ## Build the Web UI Docker image.
	docker build --tag secutils-webui:latest -f Dockerfile.webui .

docker-docs: ## Build the Docs Docker image.
	docker build --tag secutils-docs:latest -f Dockerfile.docs .

## ---------- Agent Workspace Sync ----------

agent-push: ## Push changes from this repo to the Agent workspace (excludes .git and gitignored files).
	@if [ -z "$(AGENT_WORKSPACE)" ]; then echo "Error: set AGENT_WORKSPACE to the Agent project path"; exit 1; fi
	rsync -av --delete --exclude='.git' --filter=':- .gitignore' ./ $(AGENT_WORKSPACE)/

agent-pull: ## Pull changes from the Agent workspace into this repo (excludes .git and gitignored files).
	@if [ -z "$(AGENT_WORKSPACE)" ]; then echo "Error: set AGENT_WORKSPACE to the Agent project path"; exit 1; fi
	rsync -av --delete --exclude='.git' --filter=':- .gitignore' $(AGENT_WORKSPACE)/ ./

## ---------- Misc ----------

clean: ## Remove build artifacts.
	cargo clean
	rm -rf e2e/test-results e2e/playwright-report

help: ## Show this help message.
	@grep -E '^[a-zA-Z0-9_-]+:.*## .*$$' $(MAKEFILE_LIST) | \
		awk 'BEGIN {FS = ":.*## "}; {printf "  \033[36m%-14s\033[0m %s\n", $$1, $$2}'
