import type { Criteria, EuiTableSortingType } from '@elastic/eui';
import { useCallback, useContext, useEffect, useMemo, useRef, useState } from 'react';
import { useSearchParams } from 'react-router';

import { FILTER_PARAM_QUERY, FILTER_PARAM_TAGS } from './use_items_table_filter';
import type { EntityTag, Page, PaginationRequest, SortDirection } from '../../../../model';
import { getErrorMessage } from '../../../../model';
import { WorkspaceContext } from '../../workspace_context';

const DEFAULT_PAGE_SIZE = 15;
const DEFAULT_PAGE_SIZE_OPTIONS = [10, 15, 25, 50, 100];

/** Current page/sort state of a server-paginated table. */
export interface TableState {
  pageIndex: number;
  pageSize: number;
  sortField: string;
  sortDirection: SortDirection;
}

/**
 * Computes the next {@link TableState} from an EUI table `Criteria` change.
 *
 * EUI echoes the *current* sort back in the criteria on every change, including
 * pure page changes. We must therefore only treat it as a sort change (which
 * resets to the first page) when the field or direction actually differs from
 * the current state; otherwise a page change would be clobbered back to page 0.
 */
export function resolveTableChange<T>({ page, sort }: Criteria<T>, current: TableState): TableState {
  const sortChanged =
    sort != null && (String(sort.field) !== current.sortField || sort.direction !== current.sortDirection);

  if (sortChanged) {
    return {
      pageIndex: 0,
      pageSize: current.pageSize,
      sortField: String(sort.field),
      sortDirection: sort.direction,
    };
  }

  return {
    pageIndex: page ? page.index : current.pageIndex,
    pageSize: page ? page.size : current.pageSize,
    sortField: current.sortField,
    sortDirection: current.sortDirection,
  };
}

export interface UseServerPaginatedItemsOptions<T> {
  /** Fetches a single page from the server. */
  fetcher: (request: PaginationRequest) => Promise<Page<T>>;
  /** All known tags, used to map between URL-friendly tag names and tag IDs. */
  allTags?: EntityTag[];
  /** Default sort field (must match a server-side sort allowlist key). */
  defaultSortField: string;
  /** Default sort direction. */
  defaultSortDirection?: SortDirection;
  /** Default page size. */
  defaultPageSize?: number;
  /** Selectable page sizes. */
  pageSizeOptions?: number[];
}

export interface ServerPagination {
  pageIndex: number;
  pageSize: number;
  totalItemCount: number;
  pageSizeOptions: number[];
  showPerPageOptions: boolean;
}

export interface UseServerPaginatedItemsResult<T> {
  items: T[];
  total: number;
  loading: boolean;
  /** The error message from the most recent failed fetch, or `null` if the last fetch succeeded. */
  error: string | null;
  pagination: ServerPagination;
  sorting: EuiTableSortingType<T>;
  onTableChange: (criteria: Criteria<T>) => void;
  query: string;
  setQuery: (query: string) => void;
  selectedTagIds: string[];
  setSelectedTagIds: (tagIds: string[]) => void;
  hasPageFilters: boolean;
  hasActiveFilters: boolean;
  clearPageFilters: () => void;
  refresh: () => void;
}

/**
 * Drives a server-side paginated/sorted/filtered EUI table. Search text and
 * page-level tag IDs are synced to the URL (`?q=` / `?tags=`, the latter storing
 * tag names for readability), global-scope tags come from the workspace context,
 * and any change re-fetches the relevant page from the server.
 */
export function useServerPaginatedItems<T>({
  fetcher,
  allTags,
  defaultSortField,
  defaultSortDirection = 'asc',
  defaultPageSize = DEFAULT_PAGE_SIZE,
  pageSizeOptions = DEFAULT_PAGE_SIZE_OPTIONS,
}: UseServerPaginatedItemsOptions<T>): UseServerPaginatedItemsResult<T> {
  const [searchParams, setSearchParams] = useSearchParams();
  const workspaceContext = useContext(WorkspaceContext);
  const globalScopeTagIds = useMemo(
    () => workspaceContext?.globalScopeTagIds ?? [],
    [workspaceContext?.globalScopeTagIds],
  );

  const { nameToId, idToName } = useMemo(() => {
    const n2i = new Map<string, string>();
    const i2n = new Map<string, string>();
    for (const tag of allTags ?? []) {
      n2i.set(tag.name, tag.id);
      i2n.set(tag.id, tag.name);
    }
    return { nameToId: n2i, idToName: i2n };
  }, [allTags]);

  const query = searchParams.get(FILTER_PARAM_QUERY) ?? '';
  const selectedTagIds = useMemo(() => {
    const raw = searchParams.get(FILTER_PARAM_TAGS);
    if (!raw) {
      return [];
    }
    return raw
      .split(',')
      .filter(Boolean)
      .map((name) => nameToId.get(name))
      .filter((id): id is string => id != null);
  }, [searchParams, nameToId]);

  const [pageIndex, setPageIndex] = useState(0);
  const [pageSize, setPageSize] = useState(defaultPageSize);
  const [sortField, setSortField] = useState(defaultSortField);
  const [sortDirection, setSortDirection] = useState<SortDirection>(defaultSortDirection);
  const [refreshKey, setRefreshKey] = useState(0);

  const [items, setItems] = useState<T[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const setQuery = useCallback(
    (newQuery: string) => {
      setPageIndex(0);
      setSearchParams(
        (prev) => {
          const next = new URLSearchParams(prev);
          if (newQuery) {
            next.set(FILTER_PARAM_QUERY, newQuery);
          } else {
            next.delete(FILTER_PARAM_QUERY);
          }
          return next;
        },
        { replace: true },
      );
    },
    [setSearchParams],
  );

  const setSelectedTagIds = useCallback(
    (tagIds: string[]) => {
      setPageIndex(0);
      setSearchParams(
        (prev) => {
          const next = new URLSearchParams(prev);
          const names = tagIds.map((id) => idToName.get(id)).filter((name): name is string => name != null);
          if (names.length > 0) {
            next.set(FILTER_PARAM_TAGS, names.join(','));
          } else {
            next.delete(FILTER_PARAM_TAGS);
          }
          return next;
        },
        { replace: true },
      );
    },
    [setSearchParams, idToName],
  );

  const clearPageFilters = useCallback(() => {
    setPageIndex(0);
    setSearchParams(
      (prev) => {
        const next = new URLSearchParams(prev);
        next.delete(FILTER_PARAM_QUERY);
        next.delete(FILTER_PARAM_TAGS);
        return next;
      },
      { replace: true },
    );
  }, [setSearchParams]);

  const refresh = useCallback(() => setRefreshKey((key) => key + 1), []);

  const onTableChange = useCallback(
    (criteria: Criteria<T>) => {
      const next = resolveTableChange(criteria, { pageIndex, pageSize, sortField, sortDirection });
      setPageIndex(next.pageIndex);
      setPageSize(next.pageSize);
      setSortField(next.sortField);
      setSortDirection(next.sortDirection);
    },
    [pageIndex, pageSize, sortField, sortDirection],
  );

  // Serialize tag lists so the fetch effect re-runs on content (not identity) changes.
  const selectedTagsKey = selectedTagIds.join(',');
  const globalTagsKey = globalScopeTagIds.join(',');

  const requestIdRef = useRef(0);
  useEffect(() => {
    const requestId = ++requestIdRef.current;
    setLoading(true);
    setError(null);
    fetcher({
      page: pageIndex,
      pageSize,
      sort: sortField,
      order: sortDirection,
      q: query || undefined,
      tags: selectedTagIds.length > 0 ? selectedTagIds : undefined,
      globalTags: globalScopeTagIds.length > 0 ? globalScopeTagIds : undefined,
    })
      .then((result) => {
        if (requestId !== requestIdRef.current) {
          return;
        }
        setItems(result.items);
        setTotal(result.total);
      })
      .catch((err: unknown) => {
        if (requestId !== requestIdRef.current) {
          return;
        }
        setItems([]);
        setTotal(0);
        setError(getErrorMessage(err));
      })
      .finally(() => {
        if (requestId === requestIdRef.current) {
          setLoading(false);
        }
      });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [fetcher, pageIndex, pageSize, sortField, sortDirection, query, selectedTagsKey, globalTagsKey, refreshKey]);

  const hasPageFilters = query.length > 0 || selectedTagIds.length > 0;
  const hasActiveFilters = hasPageFilters || globalScopeTagIds.length > 0;

  return {
    items,
    total,
    loading,
    error,
    pagination: {
      pageIndex,
      pageSize,
      totalItemCount: total,
      pageSizeOptions,
      showPerPageOptions: true,
    },
    sorting: { sort: { field: sortField as keyof T, direction: sortDirection } },
    onTableChange,
    query,
    setQuery,
    selectedTagIds,
    setSelectedTagIds,
    hasPageFilters,
    hasActiveFilters,
    clearPageFilters,
    refresh,
  };
}
