import { useCallback, useMemo } from 'react';
import { useSearchParams } from 'react-router';

import { filterItemsFuzzy } from './items_table_filter_utils';

export const FILTER_PARAM_QUERY = 'q';

export interface UseItemsTableFilterOptions<T> {
  items: T[];
  getSearchFields: (item: T) => string[];
}

export interface UseItemsTableFilterResult<T> {
  filteredItems: T[];
  query: string;
  setQuery: (query: string) => void;
}

export function useItemsTableFilter<T>({
  items,
  getSearchFields,
}: UseItemsTableFilterOptions<T>): UseItemsTableFilterResult<T> {
  const [searchParams, setSearchParams] = useSearchParams();

  const query = searchParams.get(FILTER_PARAM_QUERY) ?? '';
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

  const filteredItems = useMemo(
    () => (query ? filterItemsFuzzy(items, query, getSearchFields) : items),
    [items, query, getSearchFields],
  );

  return {
    filteredItems,
    query,
    setQuery,
  };
}
