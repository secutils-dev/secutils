---
sidebar_position: 3
sidebar_label: API Reference
description: Interactive OpenAPI documentation for all Secutils.dev REST API endpoints.
---

# API Reference

Secutils.dev exposes a REST API for managing all resources programmatically. The full API is described by an [OpenAPI 3.1](https://spec.openapis.org/oas/v3.1.0) specification and can be explored interactively.

| Resource                | Description                                                                      |
|-------------------------|----------------------------------------------------------------------------------|
| **Interactive docs**    | [secutils.dev/api-docs](https://secutils.dev/api-docs)                           |
| **OpenAPI spec (JSON)** | [secutils.dev/api-docs/openapi.json](https://secutils.dev/api-docs/openapi.json) |

## Available API groups

| Tag            | Base path                                                           | Description                                                      |
|----------------|---------------------------------------------------------------------|------------------------------------------------------------------|
| `webhooks`     | `/api/webhooks/responders`                                          | Create HTTP responders that capture and replay incoming requests |
| `certificates` | `/api/certificates/templates`, `/api/certificates/private_keys`     | Generate X.509 certificate templates and manage private keys     |
| `web_scraping` | `/api/web_scraping/page_trackers`, `/api/web_scraping/api_trackers` | Track changes to web pages and API endpoints                     |
| `web_security` | `/api/web_security/csp`                                             | Build, parse, and serialize Content Security Policy headers      |
| `tags`         | `/api/user/tags`                                                    | Organize resources with colored tags                             |
| `secrets`      | `/api/user/secrets`                                                 | Store encrypted secrets for use in scripts                       |
| `scripts`      | `/api/user/scripts`                                                 | Manage reusable JavaScript scripts for responders and trackers   |
| `settings`     | `/api/user/settings`                                                | Read and update user preferences                                 |
| `data`         | `/api/user/data`                                                    | Export and import user data                                      |

## Authentication

All API endpoints require authentication via a session cookie (`ory_kratos_session`) or an API key passed as an `Authorization` header. Shared resources can be accessed anonymously with the `x-secutils-share-id` header.
