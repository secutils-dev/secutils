//! Shared, reusable building blocks for server-side, offset-based pagination of list/grid
//! endpoints.
//!
//! Every paginated list endpoint accepts the same [`PaginationParams`] query parameters (`page`,
//! `pageSize`, `sort`, `order`, `q`, `tags`, `globalTags`) and returns the same [`Page`] response
//! wrapper (`{ items, total }`).
//!
//! The actual SQL is composed at runtime (not via the compile-time `query_as!` macro) because
//! pagination requires dynamic `ORDER BY` and optional filter clauses. The `sort` column and order
//! keyword are always taken from a static allowlist (never user-controlled text), so the composed
//! SQL is injection-safe.

use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

/// Default page size used when the client does not provide one.
pub const DEFAULT_PAGE_SIZE: u32 = 15;
/// Maximum page size a client can request. Larger values are clamped.
pub const MAX_PAGE_SIZE: u32 = 100;

/// Sort direction for a paginated list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum SortOrder {
    #[default]
    Asc,
    Desc,
}

impl SortOrder {
    /// Returns the SQL keyword for this order. Safe to interpolate into SQL.
    pub fn as_sql(self) -> &'static str {
        match self {
            SortOrder::Asc => "ASC",
            SortOrder::Desc => "DESC",
        }
    }
}

/// Raw pagination query parameters as received from the client.
#[derive(Debug, Clone, Default, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub struct PaginationParams {
    /// Zero-based page index. Defaults to `0`.
    pub page: Option<u32>,
    /// Number of items per page. Defaults to 15, clamped to a maximum of 100.
    pub page_size: Option<u32>,
    /// Field to sort by. Entity-specific; falls back to the entity default when not in the
    /// allowlist.
    pub sort: Option<String>,
    /// Sort direction (`asc` or `desc`).
    pub order: Option<SortOrder>,
    /// Free-text query matched (case-insensitively) against the entity name, or matched verbatim
    /// against the entity id (used by "filter to a single entity" workspace links that navigate to
    /// `?q=<entity-id>`).
    pub q: Option<String>,
    /// Page-level tag filter (OR): a comma-separated list of tag IDs, items having ANY of these
    /// tags are returned.
    pub tags: Option<String>,
    /// Global-scope tag filter (AND): a comma-separated list of tag IDs, only items having ALL of
    /// these tags are returned.
    pub global_tags: Option<String>,
}

impl PaginationParams {
    /// Resolves the raw params into a normalized [`ListParams`] (clamped page size, computed
    /// offset/limit, parsed tag lists, escaped search query).
    pub fn resolve(&self) -> ListParams {
        let page = self.page.unwrap_or(0);
        let page_size = self
            .page_size
            .unwrap_or(DEFAULT_PAGE_SIZE)
            .clamp(1, MAX_PAGE_SIZE);
        ListParams {
            offset: i64::from(page) * i64::from(page_size),
            limit: i64::from(page_size),
            order: self.order.unwrap_or_default(),
            query: self.q.as_deref().and_then(|q| {
                let trimmed = q.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(escape_like(trimmed))
                }
            }),
            tags: parse_uuid_csv(self.tags.as_deref()),
            global_tags: parse_uuid_csv(self.global_tags.as_deref()),
        }
    }

    /// Resolves the requested `sort` field to a concrete SQL column using the provided allowlist,
    /// falling back to `default` when unknown/absent.
    pub fn sort_column(
        &self,
        allowed: &[(&str, &'static str)],
        default: &'static str,
    ) -> &'static str {
        match self.sort.as_deref() {
            Some(requested) => allowed
                .iter()
                .find(|(key, _)| *key == requested)
                .map(|(_, column)| *column)
                .unwrap_or(default),
            None => default,
        }
    }
}

/// Normalized pagination parameters ready to drive a SQL query.
#[derive(Debug, Clone)]
pub struct ListParams {
    pub offset: i64,
    pub limit: i64,
    pub order: SortOrder,
    /// `ILIKE`-escaped search query, `None` when no search is requested.
    pub query: Option<String>,
    /// Page-level (OR) tag IDs.
    pub tags: Vec<Uuid>,
    /// Global-scope (AND) tag IDs.
    pub global_tags: Vec<Uuid>,
}

/// A page of results returned by a paginated list endpoint.
///
/// utoipa 5 auto-collects the concrete schema (e.g. `Page<UserSecret>`) from the handler's
/// `responses(... body = Page<UserSecret>)` declaration, so there is no need to register the
/// instantiations in the `#[openapi]` `schemas(...)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Page<T: ToSchema> {
    /// Items on the current page.
    pub items: Vec<T>,
    /// Total number of items matching the filter across all pages.
    pub total: i64,
}

impl<T: ToSchema> Page<T> {
    pub fn new(items: Vec<T>, total: i64) -> Self {
        Self { items, total }
    }
}

/// Describes the junction table used to associate an entity with tags.
pub struct TagJunction {
    /// Junction table name, e.g. `user_data_secrets_tags`.
    pub table: &'static str,
    /// Column in the junction table referencing the entity id, e.g. `secret_id`.
    pub entity_col: &'static str,
}

/// Builds the dynamic `SELECT` SQL for a paginated, tag-aware list query.
///
/// Bind order: `$1` user_id (uuid), `$2` query (text, nullable), `$3` page-level tags (uuid[]),
/// `$4` global tags (uuid[]), `$5` limit (i8), `$6` offset (i8).
///
/// `sort_col` and `order` MUST originate from a static allowlist; they are
/// interpolated directly into the SQL.
pub fn list_sql(
    table: &str,
    columns: &str,
    name_col: &str,
    junction: &TagJunction,
    sort_col: &str,
    order: SortOrder,
) -> String {
    list_sql_with_filter(table, columns, name_col, junction, sort_col, order, None)
}

/// Like [`list_sql`], but appends an additional, caller-provided `WHERE` fragment (e.g.
/// `type IN ('responder','universal')`).
///
/// `extra_filter` MUST be composed exclusively from a static allowlist (no user-controlled text) -
/// it is interpolated directly into the SQL.
pub fn list_sql_with_filter(
    table: &str,
    columns: &str,
    name_col: &str,
    junction: &TagJunction,
    sort_col: &str,
    order: SortOrder,
    extra_filter: Option<&str>,
) -> String {
    format!(
        "SELECT {columns} FROM {table} \
         WHERE {where_clause} \
         ORDER BY {sort_col} {ord}, id {ord} \
         LIMIT $5 OFFSET $6",
        where_clause = where_clause(name_col, junction, extra_filter),
        ord = order.as_sql(),
    )
}

/// Builds the `COUNT(*)` SQL matching [`list_sql`]'s filter.
///
/// Bind order: `$1` user_id, `$2` query, `$3` tags, `$4` global tags.
pub fn count_sql(table: &str, name_col: &str, junction: &TagJunction) -> String {
    count_sql_with_filter(table, name_col, junction, None)
}

/// Like [`count_sql`], but appends the same `extra_filter` as [`list_sql_with_filter`]. See its
/// safety note.
pub fn count_sql_with_filter(
    table: &str,
    name_col: &str,
    junction: &TagJunction,
    extra_filter: Option<&str>,
) -> String {
    format!(
        "SELECT COUNT(*) FROM {table} WHERE {where_clause}",
        where_clause = where_clause(name_col, junction, extra_filter),
    )
}

/// Shared `WHERE` clause for both list and count queries.
fn where_clause(name_col: &str, junction: &TagJunction, extra_filter: Option<&str>) -> String {
    let jt = junction.table;
    let jc = junction.entity_col;
    let extra = extra_filter
        .map(|f| format!(" AND ({f})"))
        .unwrap_or_default();
    // The free-text query (`$2`) matches either the entity name (case-insensitively) or the
    // entity id verbatim. The id branch backs the "click an entity name to open a grid filtered
    // to that single entity" workspace links, which navigate to `?q=<entity-id>`, an exact id is
    // a stable, unambiguous filter even when names collide. Search strings are `ILIKE`-escaped, so
    // a uuid passes through unchanged and `id::text = $2` only ever matches a real id.
    //
    // `COLLATE "C"` forces a deterministic collation for the case-insensitive name search:
    // several entity name columns inherit the database's default (nondeterministic) ICU collation,
    // and Postgres rejects `ILIKE` against those with "nondeterministic collations are not
    // supported for ILIKE". The "C" collation is byte-wise and always deterministic, so it works
    // uniformly for every entity (the search is case-insensitive via `ILIKE`).
    format!(
        "user_id = $1 \
         AND ($2::text IS NULL OR {name_col} COLLATE \"C\" ILIKE ('%' || $2 || '%') ESCAPE '\\' OR id::text = $2) \
         AND (cardinality($3::uuid[]) = 0 OR id IN (SELECT {jc} FROM {jt} WHERE tag_id = ANY($3))) \
         AND (cardinality($4::uuid[]) = 0 OR id IN ( \
            SELECT {jc} FROM {jt} WHERE tag_id = ANY($4) \
            GROUP BY {jc} HAVING COUNT(DISTINCT tag_id) = cardinality($4::uuid[]) \
         )){extra}"
    )
}

/// Escapes `ILIKE` wildcard metacharacters in a user-supplied search string so they are matched
/// literally (paired with `ESCAPE '\'` in the SQL).
fn escape_like(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// Parses a comma-separated list of UUIDs, ignoring blank/invalid entries.
fn parse_uuid_csv(raw: Option<&str>) -> Vec<Uuid> {
    raw.map(|value| {
        value
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .filter_map(|part| Uuid::parse_str(part).ok())
            .collect()
    })
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_defaults() {
        let params = PaginationParams::default();
        let resolved = params.resolve();
        assert_eq!(resolved.offset, 0);
        assert_eq!(resolved.limit, i64::from(DEFAULT_PAGE_SIZE));
        assert_eq!(resolved.order, SortOrder::Asc);
        assert!(resolved.query.is_none());
        assert!(resolved.tags.is_empty());
        assert!(resolved.global_tags.is_empty());
    }

    #[test]
    fn clamps_page_size_and_computes_offset() {
        let params = PaginationParams {
            page: Some(3),
            page_size: Some(1_000),
            ..Default::default()
        };
        let resolved = params.resolve();
        assert_eq!(resolved.limit, i64::from(MAX_PAGE_SIZE));
        assert_eq!(resolved.offset, 3 * i64::from(MAX_PAGE_SIZE));

        let zero = PaginationParams {
            page_size: Some(0),
            ..Default::default()
        }
        .resolve();
        assert_eq!(zero.limit, 1);
    }

    #[test]
    fn escapes_search_query_and_trims() {
        let params = PaginationParams {
            q: Some("  50%_off\\now  ".to_string()),
            ..Default::default()
        };
        assert_eq!(
            params.resolve().query.as_deref(),
            Some("50\\%\\_off\\\\now")
        );

        let blank = PaginationParams {
            q: Some("   ".to_string()),
            ..Default::default()
        };
        assert!(blank.resolve().query.is_none());
    }

    #[test]
    fn parses_tag_csv_ignoring_invalid() {
        let valid = Uuid::now_v7();
        let params = PaginationParams {
            tags: Some(format!("{valid},not-a-uuid,")),
            global_tags: Some(String::new()),
            ..Default::default()
        };
        let resolved = params.resolve();
        assert_eq!(resolved.tags, vec![valid]);
        assert!(resolved.global_tags.is_empty());
    }

    #[test]
    fn resolves_sort_column_from_allowlist() {
        let allowed = &[("name", "name"), ("updatedAt", "updated_at")];
        let params = PaginationParams {
            sort: Some("updatedAt".to_string()),
            ..Default::default()
        };
        assert_eq!(params.sort_column(allowed, "name"), "updated_at");

        let unknown = PaginationParams {
            sort: Some("evil; DROP TABLE".to_string()),
            ..Default::default()
        };
        assert_eq!(unknown.sort_column(allowed, "name"), "name");
    }

    #[test]
    fn builds_injection_safe_sql() {
        let junction = TagJunction {
            table: "user_data_secrets_tags",
            entity_col: "secret_id",
        };
        let list = list_sql(
            "user_data_secrets",
            "id, name",
            "name",
            &junction,
            "name",
            SortOrder::Desc,
        );
        assert!(list.contains("ORDER BY name DESC, id DESC"));
        assert!(list.contains("LIMIT $5 OFFSET $6"));
        // The name search forces a deterministic collation so `ILIKE` works against entity name
        // columns that inherit the database's default nondeterministic collation.
        assert!(list.contains("name COLLATE \"C\" ILIKE ('%' || $2 || '%') ESCAPE"));
        // The query also matches an exact entity id (used by "filter to a single entity" links).
        assert!(list.contains("OR id::text = $2"));

        let count = count_sql("user_data_secrets", "name", &junction);
        assert!(count.starts_with("SELECT COUNT(*)"));
        assert!(count.contains("user_data_secrets_tags"));
    }
}
