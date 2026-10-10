# Entity pagination, filters, and sorting

Use the shared offset-based contract for entity lists and grids. Use Secrets as the implementation example.
Read [HTTP routes](http-routes.md) for route/schema changes and [E2E](e2e.md) for browser verification.

## Shared contract

Read `src/server/pagination.rs` for defaults, limits, request parsing, and SQL builders.
Read `components/secutils-webui/src/model/pagination.ts` for request serialization and bulk aggregation.

- `PaginationParams` supplies `page`, `pageSize`, `sort`, `order`, `q`, `tags`, and `globalTags`.
- `q` matches case-insensitive names or exact entity IDs. Preserve the ID branch for workspace `?q=<entity-id>` links.
- Page-level tags use OR. Global tags use AND. Preserve both when combined.
- `Page<T>` returns `{ items, total }`.
  Use `body = Page<Entity>` in the handler so utoipa auto-collects `Page_Entity`.
  Keep concrete pages out of `components(schemas(...))` and omit schema aliases.
- `resolve()` produces `ListParams` with escaped search text, tag IDs, limit, and offset.
- `sort_column()` maps client keys through a static allowlist and falls back to the default.
- `list_sql` and `count_sql` use runtime queries for dynamic filters and ordering.
  Bind `$1` user ID, `$2` nullable query, `$3` page tags, `$4` global tags, `$5` limit, `$6` offset.

## Backend sequence

1. Derive `sqlx::FromRow` on the raw entity row.
2. Define its `TagJunction` and paginated database method in `database_ext.rs`.
   Use the shared list/count builders and fixed bind order.
   Return `(Vec<Entity>, i64)`.
   Bind the query as `Option<&str>` and tags as slices.
   Select cheap placeholders for large fields that list views do not need, as Secrets does for its value.
3. For untagged entities, write equivalent SQL inline because the builders require a junction.
   For nondeterministic name collations, apply `COLLATE "C"` to the `ILIKE` branch.
   Preserve the exact-ID branch:

   ```sql
   name COLLATE "C" ILIKE ('%' || $2 || '%') ESCAPE '\' OR id::text = $2
   ```

4. Add the client-key/SQL-column allowlist and `list_…_page` method in `api_ext.rs`.
   Resolve params and sort, fetch the page, and hydrate tags for page IDs only.
5. Add `web::Query<PaginationParams>`, `params(PaginationParams)`, and `body = Page<Entity>` to the handler.
6. Review the generated `Page_Entity` schema snapshot.
   For full-path snapshots, use `spec_json` to avoid private-number serialization.
7. Add SQLx tests for page slicing, totals, asc/desc sorting, names, exact IDs, and literal `%`/`_` search.
   Cover tag OR, AND, and combined filters.
   Add API-layer coverage for search, sort fallback, and tag hydration.

Runtime queries do not require `.sqlx/` regeneration.

## Frontend sequence

1. Add `get<Entity>Page(params: PaginationRequest): Promise<Page<Entity>>` through `buildPaginationQuery`.
2. Preserve bulk `get<Entity>()` consumers through `fetchAllItems(get<Entity>Page)`.
   Export modals and selectors require the whole dataset.
3. Use `EuiBasicTable` with the shared `useServerPaginatedItems` hook in
   `components/secutils-webui/src/pages/workspace/components/items_table_filter/use_server_paginated_items.ts`.
   Supply fetcher, tags, default sort field, and direction.
   Bind hook pagination, sorting, and `onTableChange`.
   The hook handles URL filters, global tags, debounce integration, and stale responses.
4. Use `hasActiveFilters` to select the true-empty prompt or `FilteredEmptyState`.
5. Add Vitest coverage for query serialization, page parsing, and bulk aggregation across pages.

## Sort allowlist

Mark a column `sortable: true` only when its `field` matches a server allowlist key.
Use the boolean form on `EuiBasicTable`, rather than the in-memory callback form.
A missing allowlist key produces an active arrow while the server silently uses its default sort.
The `field` is the client sort key and need not be an entity property.
For example, responders render `lastRequestedAt` from stats.

Use actual relational columns or static SQL expressions in the allowlist.
Plain script `type`, key `alg`/`encrypted`, and tag `color` columns can support sorting.
Their stored order can differ from display-label order.
Leave opaque or service-owned fields unsortable until a schema/service change supplies a deterministic sort:

- Postcard blobs: responder `method` and certificate-template `attributes`.
- Retrack-owned fields: tracker URL, method, enabled state, scheduled time, and last-run time.
- Serialized/derived values: CSP directives.

### Computed values

For responders' last-request time, use the same `MAX(created_at)` aggregate as the displayed stats.
Wrap it in `COALESCE(…, to_timestamp(0))` so never-requested rows sort oldest rather than first under `DESC`.
Use a static correlated SQL expression, with no user input.
Read `RESPONDER_LAST_REQUESTED_AT_SQL` in the implementation as the example.
Add a SQLx test with distinct timestamps and a row without history, for both directions.

## Tracker merge and bounded lists

Paginate Secutils tracker rows first, then `bulk_get_trackers` for the page's `retrack_id`s.
Search, sorting, and tags use Secutils columns/tags.
Scope `_logs_summary` requests to visible page IDs.
Keep bounded tracker revisions and responder request history client-side.
Their existing server caps make a separate pagination contract unnecessary.

## Completion gate

Run the affected Rust tests, Web UI tests/build, and a cross-page E2E test.
Seed more than one page through the API.
Assert Next page, cross-page search, header sorting, and tag filtering.
Read `.items` from paginated GET responses.
Completion requires every sortable column to map to a tested allowlist key and every bulk consumer to retain all items.
