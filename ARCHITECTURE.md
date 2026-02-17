# Secutils.dev Architecture

This document provides an overview of the Secutils.dev architecture.

## Table of Contents

- [Overview](#overview)
- [Architecture](#architecture)
- [Repository Structure](#repository-structure)
- [Dependencies](#dependencies)
- [Deployment](#deployment)

## Overview

Secutils.dev is an open-source security toolbox for engineers and researchers. It provides:

- **Webhook Responders**: Mock HTTP APIs and webhooks with custom JavaScript scripts
- **Digital Certificates**: Generate and manage X.509 certificates and private keys
- **Content Security Policy (CSP)**: Create, import, and manage CSP policies
- **Web Scraping**: Track and monitor web page content and API response changes

The backend is written in **Rust** using the **Actix-web** framework, with **PostgreSQL** for data persistence and **Ory Kratos** for identity management.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                                         Clients                                         │
│                                                                                         │
│      ┌──────────────┐    ┌──────────────┐    ┌──────────────┐    ┌──────────────┐       │
│      │   Web UI     │    │   REST API   │    │   Webhooks   │    │   CLI Tools  │       │
│      │  (React/TS)  │    │   Clients    │    │   Callers    │    │              │       │
│      └──────┬───────┘    └──────┬───────┘    └──────┬───────┘    └──────┬───────┘       │
│             │                   │                   │                   │               │
└─────────────┼───────────────────┼───────────────────┼───────────────────┼───────────────┘
              │                   │                   │                   │
              └───────────────────┴──────────┬────────┴───────────────────┘
                                             │
                                             ▼
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                                   Secutils API Server                                   │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐   │
│   │                              HTTP Server (Actix-web)                            │   │
│   │                                                                                 │   │
│   │     ┌───────────────┐  ┌─────────────┐  ┌──────────────┐  ┌───────────────┐     │   │
│   │     │ /api/status   │  │ /api/users  │  │ /api/utils   │  │ /api/webhooks │     │   │
│   │     └───────────────┘  └─────────────┘  └──────────────┘  └───────────────┘     │   │
│   └─────────────────────────────────────────────────────────────────────────────────┘   │
│                                            │                                            │
│   ┌────────────────────────────────────────┴────────────────────────────────────────┐   │
│   │                                   API Layer                                     │   │
│   │                                                                                 │   │
│   │  ┌────────────┐  ┌────────────┐  ┌────────────┐  ┌──────────┐  ┌─────────────┐  │   │
│   │  │   Users    │  │  Security  │  │   Utils    │  │  Search  │  │Notifications│  │   │
│   │  │  Manager   │  │   Manager  │  │  Manager   │  │  Index   │  │  Manager    │  │   │
│   │  └────────────┘  └────────────┘  └────────────┘  └──────────┘  └─────────────┘  │   │
│   └─────────────────────────────────────────────────────────────────────────────────┘   │
│                                            │                                            │
│   ┌────────────────────────────────────────┴─────────────────────────────────────────┐  │
│   │                              Core Services                                       │  │
│   │                                                                                  │  │
│   │          ┌────────────┐  ┌────────────┐  ┌────────────┐  ┌────────────┐          │  │
│   │          │ Scheduler  │  │ JS Runtime │  │  Network   │  │ Templates  │          │  │
│   │          │  (Cron)    │  │   (Deno)   │  │ (HTTP/DNS) │  │(Handlebars)│          │  │
│   │          └────────────┘  └────────────┘  └────────────┘  └────────────┘          │  │
│   └──────────────────────────────────────────────────────────────────────────────────┘  │
│                                            │                                            │
└────────────────────────────────────────────┼────────────────────────────────────────────┘
                                             │
                           ┌─────────────────┼─────────────────┐
                           │                 │                 │
                           ▼                 ▼                 ▼
                  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐
                  │  PostgreSQL  │  │  Ory Kratos  │  │   Retrack    │
                  │   Database   │  │   Identity   │  │   Service    │
                  │              │  │   Provider   │  │  (Optional)  │
                  └──────────────┘  └──────────────┘  └──────┬───────┘
                                                             │
                                                    ┌────────┴────────┐
                                                    │                 │
                                                    ▼                 ▼
                                           ┌──────────────┐  ┌──────────────┐
                                           │  Retrack API  │  │  Web Scraper │
                                           │  Port: 7676  │  │  Port: 7272  │
                                           └──────────────┘  └──────────────┘
```

## Repository Structure

```
secutils/
├── components/
│   ├── retrack/              # Git submodule: Retrack web page tracker
│   │   ├── components/
│   │   │   ├── retrack-types/       # Shared types (Cargo dependency)
│   │   │   └── retrack-web-scraper/ # Headless browser scraper (Node.js)
│   │   ├── Dockerfile               # Retrack API image
│   │   └── Dockerfile.web-scraper   # Web scraper image
│   ├── secutils-docs/        # Documentation site (Docusaurus)
│   ├── secutils-jwt-tools/   # JWT generation CLI (Cargo workspace member)
│   └── secutils-webui/       # Web UI (React/TypeScript, Parcel)
├── dev/
│   └── docker/               # Docker Compose files and configs
├── e2e/                      # Playwright end-to-end tests
├── migrations/               # SQLx database migrations
├── src/                      # Secutils API server (Rust)
├── Cargo.toml                # Rust workspace manifest
├── Dockerfile                # Secutils API image
├── Dockerfile.webui          # Web UI image (nginx)
├── Dockerfile.docs           # Docs image (nginx)
└── Makefile                  # Common development commands
```

## Dependencies

| Component              | Purpose                       | Technology       |
|------------------------|-------------------------------|------------------|
| **PostgreSQL**         | Primary data store            | SQL database     |
| **Ory Kratos**         | Identity & session management | OAuth2/OIDC      |
| **Retrack** (optional) | External web page tracking    | Headless browser |

**Retrack** is included as a git submodule at `components/retrack`. It provides:
- **Retrack API** (Rust, port 7676): Manages trackers, schedules, and revisions
- **Web Scraper** (Node.js + Chromium, port 7272): Renders pages and extracts content
- **retrack-types** (Rust crate): Shared type definitions used by the Secutils API

## Deployment

### Local development setup

```
┌──────────────────────────────────────────────────────────────────┐
│                      Docker Compose Network                      │
│              (docker compose -f dev/docker/docker-compose.yml)   │
│                                                                  │
│  ┌─────────────────┐                                             │
│  │  secutils_db    │  PostgreSQL 16                              │
│  │  Port: 5432     │  - Secutils data                            │
│  │                 │  - Kratos data (kratos schema)              │
│  │                 │  - Retrack data (retrack database)          │
│  └────────┬────────┘                                             │
│           │                                                      │
│     ┌─────┴──────────────┐                                       │
│     │                    │                                       │
│     ▼                    ▼                                       │
│  ┌─────────────────┐  ┌─────────────────┐                        │
│  │     kratos      │  │    retrack      │  Retrack API           │
│  │  Port: 4433     │  │  Port: 7676     │  - Tracker management  │
│  │  Port: 4434     │  └────────┬────────┘                        │
│  └─────────────────┘           │                                 │
│                                ▼                                 │
│                       ┌─────────────────┐                        │
│                       │ retrack_web_    │  Chromium + Node.js     │
│                       │ scraper         │  - Page rendering       │
│                       │ Port: 7272      │                        │
│                       └─────────────────┘                        │
└──────────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────────┐
│                           Host Machine                           │
│                                                                  │
│  ┌─────────────────┐                                             │
│  │  Secutils API   │  cargo run                                  │
│  │  Port: 7070     │  - Connects to PostgreSQL                   │
│  │                 │  - Connects to Kratos                       │
│  │                 │  - Connects to Retrack                      │
│  └─────────────────┘                                             │
│                                                                  │
│  ┌─────────────────┐                                             │
│  │  Secutils WebUI │  npm run watch (in components/secutils-webui)│
│  │  Port: 7171     │  - React SPA                                │
│  │                 │  - Proxies API, Kratos requests              │
│  └─────────────────┘                                             │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

### E2E testing setup

For end-to-end testing, all services run inside Docker using the e2e compose file
(`dev/docker/docker-compose.e2e.yml`). The Secutils API and Web UI are also containerized,
with nginx proxying API and Kratos requests. Playwright tests run on the host (or in CI)
against `http://localhost:7171`.

---

## Further Reading

- [Secutils.dev Documentation](https://secutils.dev/docs)
- [Retrack Documentation](https://github.com/secutils-dev/retrack)
- [Ory Kratos Documentation](https://www.ory.sh/docs/kratos)
- [Actix Web Documentation](https://actix.rs/docs)
