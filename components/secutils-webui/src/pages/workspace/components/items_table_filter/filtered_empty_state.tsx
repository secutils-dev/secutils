import { EuiButton, EuiEmptyPrompt, EuiIcon } from '@elastic/eui';

export interface FilteredEmptyStateProps {
  totalItems: number;
  hasPageFilters: boolean;
  onClearFilters: () => void;
}

export function FilteredEmptyState({ totalItems, hasPageFilters, onClearFilters }: FilteredEmptyStateProps) {
  return (
    <EuiEmptyPrompt
      icon={<EuiIcon type="magnify" size="xl" aria-hidden={true} />}
      title={<h3>No matching items</h3>}
      body={`${totalItems} ${totalItems === 1 ? 'item is' : 'items are'} hidden by active filters.`}
      actions={
        hasPageFilters ? (
          <EuiButton size="s" onClick={onClearFilters}>
            Clear filters
          </EuiButton>
        ) : undefined
      }
    />
  );
}
