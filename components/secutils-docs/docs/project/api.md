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

| Tag             | Base path                                                           | Description                                                        |
|-----------------|---------------------------------------------------------------------|--------------------------------------------------------------------|
| `webhooks`      | `/api/webhooks/responders`                                          | Create HTTP responders that capture and replay incoming requests   |
| `certificates`  | `/api/certificates/templates`, `/api/certificates/private_keys`     | Generate X.509 certificate templates and manage private keys       |
| `web_scraping`  | `/api/web_scraping/page_trackers`, `/api/web_scraping/api_trackers` | Track changes to web pages and API endpoints                       |
| `web_security`  | `/api/web_security/csp`                                             | Build, parse, and serialize Content Security Policy headers        |
| `api_keys`      | `/api/user/api_keys`                                                | Create and manage API keys for programmatic access                 |
| `tags`          | `/api/user/tags`                                                    | Organize resources with colored tags                               |
| `secrets`       | `/api/user/secrets`                                                 | Store encrypted secrets for use in scripts                         |
| `scripts`       | `/api/user/scripts`                                                 | Manage reusable JavaScript scripts for responders and trackers     |
| `settings`      | `/api/user/settings`, `/api/user/notification_email`                | Read and update user preferences, including the notification email |
| `data`          | `/api/user/data`                                                    | Export and import user data                                        |
| `notifications` | `/api/notifications/unsubscribe`                                    | Public endpoints for managing notification delivery (RFC 8058)     |

## Authentication

All API endpoints require authentication. The following methods are supported:

| Method             | Format                          | Description                                                                                                   |
|--------------------|---------------------------------|---------------------------------------------------------------------------------------------------------------|
| **Session cookie** | `id` cookie                     | Automatically set by the browser after login                                                                  |
| **API key**        | `Authorization: Bearer su_ak_…` | Opaque token for programmatic/agent access. Create via the API keys page or the `/api/user/api_keys` endpoint |
| **JWT**            | `Authorization: Bearer eyJ…`    | Service-account token (operator use only)                                                                     |

API keys are the recommended method for scripts, CI pipelines, and AI agents. They can have an optional expiration date and are independent of the browser session. The plaintext token is shown only once at creation - store it securely.

Shared resources can be accessed anonymously with the `x-secutils-share-id` header.
