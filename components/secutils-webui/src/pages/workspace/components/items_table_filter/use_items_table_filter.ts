import { useCallback, useContext, useMemo } from 'react';
import { useSearchParams } from 'react-router';

import { filterItemsFuzzy } from './items_table_filter_utils';
import type { EntityTag } from '../../../../model';
import { WorkspaceContext } from '../../workspace_context';

export const FILTER_PARAM_QUERY = 'q';
export const FILTER_PARAM_TAGS = 'tags';

export interface UseItemsTableFilterOptions<T> {
  items: T[];
  getSearchFields: (item: T) => string[];
  getItemTags?: (item: T) => EntityTag[] | undefined;
}

export interface UseItemsTableFilterResult<T> {
  filteredItems: T[];
  query: string;
  setQuery: (query: string) => void;
  selectedTagIds: string[];
  setSelectedTagIds: (tagIds: string[]) => void;
}

export function useItemsTableFilter<T>({
  items,
  getSearchFields,
  getItemTags,
}: UseItemsTableFilterOptions<T>): UseItemsTableFilterResult<T> {
  const [searchParams, setSearchParams] = useSearchParams();
  const workspaceContext = useContext(WorkspaceContext);
  const globalScopeTagIds = useMemo(
    () => workspaceContext?.globalScopeTagIds ?? [],
    [workspaceContext?.globalScopeTagIds],
  );

  const query = searchParams.get(FILTER_PARAM_QUERY) ?? '';
  const selectedTagIds = useMemo(() => {
    const raw = searchParams.get(FILTER_PARAM_TAGS);
    return raw ? raw.split(',').filter(Boolean) : [];
  }, [searchParams]);

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
          if (tagIds.length > 0) {
            newParams.set(FILTER_PARAM_TAGS, tagIds.join(','));
          } else {
            newParams.delete(FILTER_PARAM_TAGS);
          }
          return newParams;
        },
        { replace: true },
      );
    },
    [setSearchParams],
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

  return {
    filteredItems,
    query,
    setQuery,
    selectedTagIds,
    setSelectedTagIds,
  };
}
