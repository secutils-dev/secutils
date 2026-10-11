import { EuiButton, EuiEmptyPrompt, EuiIcon } from '@elastic/eui';

export interface FilteredEmptyStateProps {
  hasPageFilters: boolean;
  onClearFilters: () => void;
}

export function FilteredEmptyState({ hasPageFilters, onClearFilters }: FilteredEmptyStateProps) {
  return (
    <EuiEmptyPrompt
      icon={<EuiIcon type="magnify" size="xl" aria-hidden={true} />}
      title={<h3>No matching items</h3>}
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
