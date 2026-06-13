/**
 * Shared client-side helpers for the server-side, offset-based pagination
 * contract used by all entity list endpoints.
 *
 * Every paginated endpoint accepts the {@link PaginationRequest} query
 * parameters and returns a {@link Page} wrapper (`{ items, total }`).
 */

/** A single page of results returned by a paginated list endpoint. */
export interface Page<T> {
  items: T[];
  total: number;
}

export type SortDirection = 'asc' | 'desc';

/** Query parameters accepted by every paginated list endpoint. */
export interface PaginationRequest {
  /** Zero-based page index. */
  page?: number;
  /** Items per page (server clamps to a maximum of 100). */
  pageSize?: number;
  /** Field to sort by (entity-specific). */
  sort?: string;
  /** Sort direction. */
  order?: SortDirection;
  /** Free-text query matched against the entity name. */
  q?: string;
  /** Page-level tag filter (OR): tag IDs; items having ANY of these are returned. */
  tags?: string[];
  /** Global-scope tag filter (AND): tag IDs; only items having ALL of these are returned. */
  globalTags?: string[];
}

/** Maximum page size the server accepts; used when fetching every item. */
export const MAX_PAGE_SIZE = 100;

/**
 * Serializes a {@link PaginationRequest} into a URL query string (including the
 * leading `?`), omitting empty/undefined values. Tag lists are comma-joined.
 */
export function buildPaginationQuery(params: PaginationRequest = {}): string {
  const searchParams = new URLSearchParams();
  if (params.page != null) {
    searchParams.set('page', String(params.page));
  }
  if (params.pageSize != null) {
    searchParams.set('pageSize', String(params.pageSize));
  }
  if (params.sort) {
    searchParams.set('sort', params.sort);
  }
  if (params.order) {
    searchParams.set('order', params.order);
  }
  if (params.q) {
    searchParams.set('q', params.q);
  }
  if (params.tags && params.tags.length > 0) {
    searchParams.set('tags', params.tags.join(','));
  }
  if (params.globalTags && params.globalTags.length > 0) {
    searchParams.set('globalTags', params.globalTags.join(','));
  }
  const query = searchParams.toString();
  return query ? `?${query}` : '';
}

/**
 * Fetches every item across all pages by repeatedly requesting `MAX_PAGE_SIZE`
 * chunks. Used where the full dataset is required (export, tag selectors).
 */
export async function fetchAllItems<T>(fetchPage: (request: PaginationRequest) => Promise<Page<T>>): Promise<T[]> {
  const items: T[] = [];
  let page = 0;
  for (;;) {
    const result = await fetchPage({ page, pageSize: MAX_PAGE_SIZE });
    items.push(...result.items);
    if (result.items.length === 0 || items.length >= result.total) {
      break;
    }
    page += 1;
  }
  return items;
}
