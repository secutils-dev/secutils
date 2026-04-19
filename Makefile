COMPOSE_DEV          	:= dev/docker/docker-compose.yml
COMPOSE_DEBUG_SCRAPER	:= dev/docker/docker-compose.debug-scraper.yml
COMPOSE_E2E          	:= dev/docker/docker-compose.e2e.yml
COMPOSE_EXTERNAL_NET 	:= dev/docker/docker-compose.external-network.yml
ENV_FILE             	:= .env
CHROME_PATH          	?= /Applications/Google Chrome.app/Contents/MacOS/Google Chrome
RUNS                 	?= 10
E2E_LOOP_DIR         	:= /tmp/e2e-loop-results
AGENT_WORKSPACE     	?=

-include deploy.env

DEPLOY_REGISTRY     	?=
DEPLOY_PLATFORM     	?= linux/arm64
DEPLOY_DEV_TAG      	?= latest
DEPLOY_PROD_TAG     	?=
DEPLOY_CAMOUFOX_TAG 	?=

.PHONY: dev-up dev-down api webui webui-test docs e2e-up e2e-down e2e-test e2e-test-loop e2e-standalone-test docs-screenshots docs-screenshots-loop docs-screenshots-diff docs-screenshots-analyze agent-push agent-pull clean docker-df docker-prune docker-prune-images docker-prune-buildcache docker-pin-digests perf perf-analyze perf-report
.PHONY: deploy-dev deploy-dev-api deploy-dev-webui deploy-dev-docs deploy-dev-retrack-api deploy-dev-retrack-scraper
.PHONY: deploy-prod deploy-prod-api deploy-prod-webui deploy-prod-docs deploy-prod-retrack-api deploy-prod-retrack-scraper
.PHONY: deploy-camoufox
.PHONY: deploy-tools

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

webui-test: ## Run Web UI unit tests (use ARGS for extra flags, e.g. make webui-test ARGS="--watch").
	npm --prefix components/secutils-webui run test -- $(ARGS)

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

COMPOSE_E2E_FILES := -f $(COMPOSE_DEV) -f $(COMPOSE_E2E)
ifdef EXTERNAL_NETWORK
  COMPOSE_E2E_FILES += -f $(COMPOSE_EXTERNAL_NET)
endif

e2e-up: ## Start the full e2e stack (all services in Docker). Use BUILD=1 to rebuild images. Use EXTERNAL_NETWORK=<name> to join an external Docker network.
	docker compose $(COMPOSE_E2E_FILES) --env-file $(ENV_FILE) up $(if $(BUILD),--build) -d

e2e-down: ## Stop the e2e stack and remove volumes.
	docker compose $(COMPOSE_E2E_FILES) --env-file $(ENV_FILE) down --volumes --remove-orphans

e2e-test: ## Run Playwright e2e tests (use ARGS for extra flags, e.g. make e2e-test ARGS="--ui").
	cd e2e && npx playwright test $(ARGS)

e2e-standalone-test: ## Run standalone e2e tests (no Docker stack needed, e.g. codegen smoke tests).
	cd e2e && npx playwright test --config=playwright.standalone.config.ts $(ARGS)

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
	docker compose $(COMPOSE_E2E_FILES) logs -f

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

docs-screenshots-diff: ## Run docs screenshots twice and compare (use ARGS for extra flags).
	./e2e/scripts/compare-screenshots.sh $(ARGS)

docs-screenshots-analyze: ## Analyze diffs from docs-screenshots-diff (reads /tmp/screenshot-diff/).
	python3 e2e/scripts/analyze-screenshot-diffs.py

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

docker-pin-digests: ## Re-pin base images in root Dockerfiles to current SHA256 digests.
	./dev/scripts/docker-pin-digests.sh

## ---------- Deploy (build & push to Docker registry) ----------

define _buildx_push
	DOCKER_BUILDKIT=1 docker buildx build --progress=plain --push \
		--platform $(DEPLOY_PLATFORM) \
		--tag $(DEPLOY_REGISTRY)/$(1):$(2) $(3)
endef

_require-deploy-registry:
	@test -n "$(DEPLOY_REGISTRY)" || \
		{ echo "Error: DEPLOY_REGISTRY not set. Copy deploy.env.example to deploy.env and fill it in."; exit 1; }

_require-deploy-prod-tag: _require-deploy-registry
	@test -n "$(DEPLOY_PROD_TAG)" || \
		{ echo "Error: DEPLOY_PROD_TAG not set in deploy.env."; exit 1; }

_require-deploy-camoufox-tag: _require-deploy-registry
	@test -n "$(DEPLOY_CAMOUFOX_TAG)" || \
		{ echo "Error: DEPLOY_CAMOUFOX_TAG not set in deploy.env."; exit 1; }

deploy-dev: _require-deploy-registry ## Build & push all images for dev (DEV_TAG).
deploy-dev: deploy-dev-api deploy-dev-webui deploy-dev-docs deploy-dev-retrack-api deploy-dev-retrack-scraper

deploy-dev-api: _require-deploy-registry
	$(call _buildx_push,secutils-api,$(DEPLOY_DEV_TAG),.)

deploy-dev-webui: _require-deploy-registry
	$(call _buildx_push,secutils-webui,$(DEPLOY_DEV_TAG),-f Dockerfile.webui .)

deploy-dev-docs: _require-deploy-registry
	$(call _buildx_push,secutils-docs,$(DEPLOY_DEV_TAG),-f Dockerfile.docs .)

deploy-dev-retrack-api: _require-deploy-registry
	$(call _buildx_push,retrack-api,$(DEPLOY_DEV_TAG),-f components/retrack/Dockerfile components/retrack)

deploy-dev-retrack-scraper: _require-deploy-registry
	$(call _buildx_push,retrack-web-scraper,$(DEPLOY_DEV_TAG),-f components/retrack/Dockerfile.web-scraper components/retrack)

deploy-prod: _require-deploy-prod-tag ## Build & push all images for prod (PROD_TAG).
deploy-prod: deploy-prod-api deploy-prod-webui deploy-prod-docs deploy-prod-retrack-api deploy-prod-retrack-scraper

deploy-prod-api: _require-deploy-prod-tag
	$(call _buildx_push,secutils-api,$(DEPLOY_PROD_TAG),.)

deploy-prod-webui: _require-deploy-prod-tag
	$(call _buildx_push,secutils-webui,$(DEPLOY_PROD_TAG),-f Dockerfile.webui .)

deploy-prod-docs: _require-deploy-prod-tag
	$(call _buildx_push,secutils-docs,$(DEPLOY_PROD_TAG),-f Dockerfile.docs .)

deploy-prod-retrack-api: _require-deploy-prod-tag
	$(call _buildx_push,retrack-api,$(DEPLOY_PROD_TAG),-f components/retrack/Dockerfile components/retrack)

deploy-prod-retrack-scraper: _require-deploy-prod-tag
	$(call _buildx_push,retrack-web-scraper,$(DEPLOY_PROD_TAG),-f components/retrack/Dockerfile.web-scraper components/retrack)

deploy-camoufox: _require-deploy-camoufox-tag ## Build & push the Camoufox web scraper image.
	$(call _buildx_push,retrack-web-scraper-camoufox,$(DEPLOY_CAMOUFOX_TAG),-f components/retrack/Dockerfile.web-scraper-camoufox components/retrack)

## ---------- Agent Workspace Sync ----------

agent-push: ## Push changes from this repo to the Agent workspace (excludes .git and gitignored files).
	@if [ -z "$(AGENT_WORKSPACE)" ]; then echo "Error: set AGENT_WORKSPACE to the Agent project path"; exit 1; fi
	rsync -av --delete --exclude='.git' --filter=':- .gitignore' ./ $(AGENT_WORKSPACE)/

agent-pull: ## Pull changes from the Agent workspace into this repo (excludes .git and gitignored files).
	@if [ -z "$(AGENT_WORKSPACE)" ]; then echo "Error: set AGENT_WORKSPACE to the Agent project path"; exit 1; fi
	rsync -av --delete --exclude='.git' --filter=':- .gitignore' $(AGENT_WORKSPACE)/ ./

## ---------- JS Runtime Perf Harness ----------

PERF_OUTPUT ?= /tmp/perf.json
PERF_ITERATIONS ?= 500
PERF_WARMUP ?= 50
PERF_CONCURRENCY ?= 8
PERF_SCENARIOS ?= all

perf: ## Run the JS runtime perf harness. Use ANALYZE=1 to also print the comparison table and record to .perf/history.jsonl (ARGS='--scenarios cold_start_trivial --iterations 100').
	cargo run --release -p js-runtime-perf -- \
		--scenarios $(PERF_SCENARIOS) \
		--iterations $(PERF_ITERATIONS) \
		--warmup $(PERF_WARMUP) \
		--concurrency $(PERF_CONCURRENCY) \
		--output $(PERF_OUTPUT) $(ARGS) \
		$(if $(ANALYZE),&& node scripts/analyze-perf.ts $(PERF_OUTPUT))

perf-analyze: ## Analyze an existing $(PERF_OUTPUT) without rerunning the harness (equivalent to the ANALYZE=1 tail of `make perf`).
	node scripts/analyze-perf.ts $(PERF_OUTPUT)

perf-report: ## Open the HTML perf viewer. Load .perf/history.jsonl inside it.
	@open scripts/perf-report.html 2>/dev/null || \
		xdg-open scripts/perf-report.html 2>/dev/null || \
		echo 'Open scripts/perf-report.html in your browser'

## ---------- Tool Apps ----------

deploy-tools: ## Deploy dev/tools HTML apps to responders (ARGS="calc jwt-debugger" to deploy specific tools).
	node --env-file=.env dev/tools/deploy.ts $(ARGS)

## ---------- Docker Cleanup ----------

docker-df: ## Show Docker disk usage summary.
	docker system df

docker-prune: docker-prune-images docker-prune-buildcache ## Remove dangling images and build cache.

docker-prune-images: ## Remove dangling (untagged) Docker images.
	docker image prune -f

docker-prune-buildcache: ## Remove Docker BuildKit build cache.
	docker builder prune -f

## ---------- Misc ----------

clean: ## Remove build artifacts.
	cargo clean
	rm -rf e2e/test-results e2e/playwright-report

help: ## Show this help message.
	@grep -E '^[a-zA-Z0-9_-]+:.*## .*$$' $(MAKEFILE_LIST) | \
		awk 'BEGIN {FS = ":.*## "}; {printf "  \033[36m%-14s\033[0m %s\n", $$1, $$2}'
