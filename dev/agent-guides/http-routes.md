# HTTP routes and OpenAPI

Run commands from the Secutils root.
For entity list endpoints, also read [pagination](pagination.md) before implementation.
Use `src/server/handlers/private_keys.rs` as a current handler example.

## Implementation sequence

1. **Models.** Create or update typed request/response models in the feature directory.
   Use `ToSchema` and camelCase JSON fields.
   For `OffsetDateTime` with `time::serde::timestamp`, use `#[schema(value_type = i64)]`.
   Match schema visibility to serde's public representation.
   For composite responses, use a dedicated handler response struct, as in `CertificateTemplateGetResponse`.
2. **Request examples.** Add a realistic `#[schema(example = json!(...))]` to each body params type.
   Keep one params struct per `src/utils/{feature}/api_ext/{name}_{verb}_params.rs` file.
   Re-export it from the feature module.
   Add one sync-guard test per example through `crate::tests::schema_example::<T>()` in `src/main.rs`.
   Deserialize the generated example into the real type and assert at least one semantic property.
3. **Handler.** Add the actix handler in `src/server/handlers/{feature}.rs`.
   Declare full paths with `#[get]`, `#[post]`, `#[put]`, or `#[delete]`.
   Use `/api/{feature}/{resource}` and `/{id}/_action` or `/_action` for actions.
   Use `User` for authenticated handlers and typed `web::Path`/`web::Json` extractors.
   Add `utoipa::path` tags, responses, `params(PathStruct)` for path parameters, and `request_body` for JSON bodies.
   Path structs derive `Deserialize` and `IntoParams`.
   Use `Ok` for reads/updates, `Created` for creates, and `NoContent` for deletes.
4. **Registration.** Declare the handler module and all OpenAPI paths in `src/server/handlers.rs`.
   Register model schemas in `components(schemas(...))` where required.
   Let utoipa auto-collect concrete `Page<Entity>` schemas from response declarations, as described in the pagination guide.
   Add every handler's `.service()` registration in `src/server.rs`, grouped by feature.
   Completion requires every response/request `$ref` to resolve in the generated spec.
5. **Snapshots.** Update `openapi_spec_has_all_paths` and `openapi_spec_has_all_schemas` in alphabetical order.
   Review changes with `cargo insta review` after `cargo test`.
   For full-path snapshots with JSON numbers, use `assert_snapshot!(spec_json(&path), …)`.
   That helper avoids insta's private-number serialization under `serde_json/arbitrary_precision`.
6. **Consumers.** Update Web UI model URLs and relevant E2E interceptors/API calls/URL assertions.
   For route migrations, search for every old URL in Web UI `src/`, `e2e/tests/`, and `e2e/docs/`.
   Account for every occurrence and remove obsolete route references.
7. **Dev requests.** Update or add the feature's `.http` file under `dev/api/`.
8. **API documentation.** For new tags or changed group paths, update the OpenAPI tag description and docs API-groups table.
   The table lives in `components/secutils-docs/docs/project/api.md`.
   Build docs and inspect both the rendered page and `build/llms.txt`.

## Completion gate

```bash
cargo test
cargo clippy
npm run build --prefix components/secutils-webui
```

All three checks must pass. Review the generated spec, schema examples, and consumer changes.
If the route changes test behavior, run the affected E2E specs with [the changed stack](e2e.md).
For pagination, complete its database, frontend, and cross-page test coverage too.
