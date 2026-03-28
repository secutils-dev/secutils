import { useCallback, useContext, useMemo } from 'react';
import { useSearchParams } from 'react-router';

import { filterItemsFuzzy } from './items_table_filter_utils';
import type { EntityTag } from '../../../../model';
import { WorkspaceContext } from '../../workspace_context';

export const FILTER_PARAM_QUERY = 'q';
export const FILTER_PARAM_TAGS = 'tags';

export interface UseItemsTableFilterOptions<T> {
  items: T[];
  allTags?: EntityTag[];
  getSearchFields: (item: T) => string[];
  getItemTags?: (item: T) => EntityTag[] | undefined;
}

export interface UseItemsTableFilterResult<T> {
  filteredItems: T[];
  query: string;
  setQuery: (query: string) => void;
  selectedTagIds: string[];
  setSelectedTagIds: (tagIds: string[]) => void;
  totalItems: number;
  hasActiveFilters: boolean;
  hasPageFilters: boolean;
  clearPageFilters: () => void;
}

export function useItemsTableFilter<T>({
  items,
  allTags,
  getSearchFields,
  getItemTags,
}: UseItemsTableFilterOptions<T>): UseItemsTableFilterResult<T> {
  const [searchParams, setSearchParams] = useSearchParams();
  const workspaceContext = useContext(WorkspaceContext);
  const globalScopeTagIds = useMemo(
    () => workspaceContext?.globalScopeTagIds ?? [],
    [workspaceContext?.globalScopeTagIds],
  );

  // Build name↔ID lookup maps for URL-friendly tag params.
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
  // URL stores tag names for readability; resolve to IDs for filtering.
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

  const setQuery = useCallback(
    (newQuery: string) => {
      setSearchParams(
        (prev) => {
          const newParams = new URLSearchParams(prev);
          if (newQuery) {
            newParams.set(FILTER_PARAM_QUERY, newQuery);
          } else {
            newParams.delete(FILTER_PARAM_QUERY);
          }
          return newParams;
        },
        { replace: true },
      );
    },
    [setSearchParams],
  );

  const setSelectedTagIds = useCallback(
    (tagIds: string[]) => {
      setSearchParams(
        (prev) => {
          const newParams = new URLSearchParams(prev);
          // Write tag names to URL for readability.
          const names = tagIds.map((id) => idToName.get(id)).filter((n): n is string => n != null);
          if (names.length > 0) {
            newParams.set(FILTER_PARAM_TAGS, names.join(','));
          } else {
            newParams.delete(FILTER_PARAM_TAGS);
          }
          return newParams;
        },
        { replace: true },
      );
    },
    [setSearchParams, idToName],
  );

  const filteredItems = useMemo(() => {
    let result = items;

    // Apply global scope filter (AND: item must have ALL selected global tags).
    if (globalScopeTagIds.length > 0 && getItemTags) {
      const globalIdSet = new Set(globalScopeTagIds);
      result = result.filter((item) => {
        const itemTags = getItemTags(item);
        if (!itemTags || itemTags.length === 0) {
          return false;
        }
        const itemTagIds = new Set(itemTags.map((tag) => tag.id));
        return Array.from(globalIdSet).every((id) => itemTagIds.has(id));
      });
    }

    // Apply page-level tag filter (OR: item has at least one selected tag).
    if (selectedTagIds.length > 0 && getItemTags) {
      const tagIdSet = new Set(selectedTagIds);
      result = result.filter((item) => {
        const itemTags = getItemTags(item);
        return itemTags?.some((tag) => tagIdSet.has(tag.id)) ?? false;
      });
    }

    // Apply fuzzy text search.
    if (query) {
      result = filterItemsFuzzy(result, query, getSearchFields);
    }

    return result;
  }, [items, query, selectedTagIds, globalScopeTagIds, getSearchFields, getItemTags]);

  const hasPageFilters = query.length > 0 || selectedTagIds.length > 0;
  const hasActiveFilters = hasPageFilters || globalScopeTagIds.length > 0;

  const clearPageFilters = useCallback(() => {
    setSearchParams(
      (prev) => {
        const newParams = new URLSearchParams(prev);
        newParams.delete(FILTER_PARAM_QUERY);
        newParams.delete(FILTER_PARAM_TAGS);
        return newParams;
      },
      { replace: true },
    );
  }, [setSearchParams]);

  return {
    filteredItems,
    query,
    setQuery,
    selectedTagIds,
    setSelectedTagIds,
    totalItems: items.length,
    hasActiveFilters,
    hasPageFilters,
    clearPageFilters,
  };
}
