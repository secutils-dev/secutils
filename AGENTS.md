# Secutils agent guide

Secutils.dev combines a Rust/actix-web API, React/EUI Web UI, Docusaurus docs,
PostgreSQL, Ory Kratos, and the Retrack tracking service.

## Scope and task guides

Paths and commands in this file are relative to the Secutils repository root.
Read each applicable guide before changes to its surface. A task can require several guides.

| Trigger                                                                                              | Read                                                                               |
|------------------------------------------------------------------------------------------------------|------------------------------------------------------------------------------------|
| Service topology, ports, or repository layout                                                        | [ARCHITECTURE.md](ARCHITECTURE.md)                                                 |
| Changes inside the Retrack submodule, including scraper or Camoufox work                             | [components/retrack/AGENTS.md](components/retrack/AGENTS.md)                       |
| Standalone tools, their bundles, agent surfaces, OG images, or `e2e/tools/` tests                    | [dev/tools/AGENTS.md](dev/tools/AGENTS.md)                                         |
| Dependency upgrades, runtime versions, Docker base images, or Retrack pointer upgrades               | [dev/agent-guides/dependency-upgrades.md](dev/agent-guides/dependency-upgrades.md) |
| Run, write, or debug application E2E or standalone tests                                             | [dev/agent-guides/e2e.md](dev/agent-guides/e2e.md)                                 |
| Generate docs screenshots, diagnose screenshot drift, or edit screenshot-based MDX guides            | [dev/agent-guides/docs-screenshots.md](dev/agent-guides/docs-screenshots.md)       |
| Add or change HTTP routes, request/response models, or OpenAPI metadata                              | [dev/agent-guides/http-routes.md](dev/agent-guides/http-routes.md)                 |
| Add entity lists/grids, paginate them, or change search, tags, or sorting                            | [dev/agent-guides/pagination.md](dev/agent-guides/pagination.md)                   |
| Measure JS runtime performance, change benchmark scenarios/history, or change Web UI bundle tracking | [dev/agent-guides/performance.md](dev/agent-guides/performance.md)                 |
| Change Docusaurus, rendered Markdown output, or docs Nginx MIME handling                             | [dev/agent-guides/docs-output.md](dev/agent-guides/docs-output.md)                 |

## Commands and verification

Use `make help` for targets and flags. Package scripts live in each `package.json`.
The host development loop uses `make dev-up`, then `make api`, `make webui`, or `make docs`.
`BUILD=1` rebuilds Compose images. `ONLY=` and `SKIP=` narrow the build.
`make dev-down`, `make e2e-down`, and `make db-reset` delete development data.

Use [.husky/pre-push](.husky/pre-push) for the local push gate.
Use [.github/workflows/ci.yml](.github/workflows/ci.yml) for CI checks and path exclusions.
Both Rust formatting gates require nightly because `rustfmt.toml` uses unstable options:

```bash
cargo +nightly fmt --all -- --check
```

Run the checks for the changed surface and any additional checks in its task guide.
Completion requires passing checks, reviewed generated artifacts, and a diff limited to the intended changes.
If a required check is blocked, report the command and blocker as incomplete verification.

## Repository boundaries

Retrack is a separate Git repository. Inspect its status before changes there.
Keep Retrack implementation guidance in that repository and product-wide coordination in the root dependency guide.
For root Rust tests, use `cargo test`, as CI does.
`cargo test --workspace` unifies `serde_json/arbitrary_precision` into Retrack snapshots and can produce unrelated number-format diffs.
Preserve Retrack snapshots against its own feature set.

When a commit is requested, use conventional commits.
Dependency, image, and submodule changes use `chore(deps)`, `chore(docker)`, and `chore(submodule)` respectively.
