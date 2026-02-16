import { EuiButton, EuiFieldSearch, EuiFlexGroup, EuiFlexItem } from '@elastic/eui';
import type { ChangeEvent, ReactNode } from 'react';
import { useCallback, useRef, useState } from 'react';

export interface ItemsTableFilterProps {
  query: string;
  onQueryChange: (query: string) => void;
  onRefresh?: () => void;
  placeholder?: string;
  children?: ReactNode;
}

const SEARCH_DEBOUNCE_MS = 150;

export function ItemsTableFilter({
  query,
  onQueryChange,
  onRefresh,
  placeholder = 'Search by name or ID...',
  children,
}: ItemsTableFilterProps) {
  const [localQuery, setLocalQuery] = useState(query);
  const debounceRef = useRef<number | null>(null);

  const handleQueryChange = useCallback(
    (e: ChangeEvent<HTMLInputElement>) => {
      const newQuery = e.target.value;
      setLocalQuery(newQuery);

      if (debounceRef.current) {
        window.clearTimeout(debounceRef.current);
      }

      debounceRef.current = window.setTimeout(() => {
        onQueryChange(newQuery);
      }, SEARCH_DEBOUNCE_MS);
    },
    [onQueryChange],
  );

  return (
    <EuiFlexGroup gutterSize="s" alignItems="center" responsive={false}>
      <EuiFlexItem grow={true}>
        <EuiFieldSearch
          placeholder={placeholder}
          value={localQuery}
          onChange={handleQueryChange}
          isClearable
          aria-label="Search"
          fullWidth
        />
      </EuiFlexItem>
      {children}
      {onRefresh && (
        <EuiFlexItem grow={false}>
          <EuiButton iconType="refresh" aria-label="Refresh" onClick={onRefresh}>
            Refresh
          </EuiButton>
        </EuiFlexItem>
      )}
    </EuiFlexGroup>
  );
}
