# Secutils.dev Architecture

This document provides an overview of the Secutils.dev architecture.

## Table of Contents

- [Overview](#overview)
- [Architecture](#architecture)
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
                  └──────────────┘  └──────────────┘  └──────────────┘
```

## Dependencies

| Component              | Purpose                       | Technology       |
|------------------------|-------------------------------|------------------|
| **PostgreSQL**         | Primary data store            | SQL database     |
| **Ory Kratos**         | Identity & session management | OAuth2/OIDC      |
| **Retrack** (optional) | External web page tracking    | Headless browser |

## Deployment

### Local development setup

```
┌──────────────────────────────────────────────────────────────────┐
│                      Docker Compose Network                      │
│                                                                  │
│  ┌─────────────────┐                                             │
│  │  secutils_db    │  PostgreSQL 16                              │
│  │  Port: 5432     │  - Secutils data                            │
│  │                 │  - Kratos data                              │
│  │                 │  - Scheduler jobs                           │
│  └────────┬────────┘                                             │
│           │                                                      │
│           ▼                                                      │
│  ┌─────────────────┐                                             │
│  │  kratos_migrate │  One-shot migration                         │
│  └────────┬────────┘                                             │
│           │                                                      │
│           ▼                                                      │
│  ┌─────────────────┐                                             │
│  │     kratos      │  Ory Kratos v25.x                           │
│  │  Port: 4433     │  - Public API (login, registration)         │
│  │  Port: 4434     │  - Admin API (identity management)          │
│  └─────────────────┘                                             │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────────┐
│                           Host Machine                           │
│                                                                  │
│  ┌─────────────────┐                                             │
│  │  Secutils API   │  cargo run                                  │
│  │  Port: 7070     │  - Connects to PostgreSQL                   │
│  │                 │  - Connects to Kratos                       │
│  └─────────────────┘                                             │
│                                                                  │
│  ┌─────────────────┐                                             │
│  │  Secutils WebUI │  npm start (in components/secutils-webui)   │
│  │  Port: 1234     │  - React SPA                                │
│  │                 │  - Proxies API requests                     │
│  └─────────────────┘                                             │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

---

## Further Reading

- [Secutils.dev Documentation](https://secutils.dev/docs)
- [Ory Kratos Documentation](https://www.ory.sh/docs/kratos)
- [Actix Web Documentation](https://actix.rs/docs)

