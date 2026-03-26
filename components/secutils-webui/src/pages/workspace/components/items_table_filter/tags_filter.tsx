import { EuiBadge, EuiFilterButton, EuiFilterGroup, EuiFlexItem, EuiPopover, EuiSelectable } from '@elastic/eui';
import type { EuiSelectableOption } from '@elastic/eui';
import { useCallback, useMemo, useState } from 'react';

import type { EntityTag } from '../../../../model';

export interface TagsFilterProps {
  tags: EntityTag[];
  selectedTagIds: string[];
  onSelectedTagIdsChange: (tagIds: string[]) => void;
}

export function TagsFilter({ tags, selectedTagIds, onSelectedTagIdsChange }: TagsFilterProps) {
  const [isPopoverOpen, setIsPopoverOpen] = useState(false);

  const options: EuiSelectableOption[] = useMemo(
    () =>
      tags.map((tag) => ({
        label: tag.name,
        key: tag.id,
        checked: selectedTagIds.includes(tag.id) ? 'on' : undefined,
        prepend: <EuiBadge color={tag.color}>{'\u00A0'}</EuiBadge>,
      })),
    [tags, selectedTagIds],
  );

  const handleChange = useCallback(
    (newOptions: EuiSelectableOption[]) => {
      const newSelectedIds = newOptions
        .filter((opt) => opt.checked === 'on')
        .map((opt) => opt.key!)
        .filter(Boolean);
      onSelectedTagIdsChange(newSelectedIds);
    },
    [onSelectedTagIdsChange],
  );

  if (tags.length === 0) {
    return null;
  }

  const numActiveFilters = selectedTagIds.length;

  return (
    <EuiFlexItem grow={false}>
      <EuiFilterGroup>
        <EuiPopover
          button={
            <EuiFilterButton
              iconType="arrowDown"
              onClick={() => setIsPopoverOpen(!isPopoverOpen)}
              isSelected={isPopoverOpen}
              numFilters={tags.length}
              hasActiveFilters={numActiveFilters > 0}
              numActiveFilters={numActiveFilters}
            >
              Tags
            </EuiFilterButton>
          }
          isOpen={isPopoverOpen}
          closePopover={() => setIsPopoverOpen(false)}
          panelPaddingSize="none"
          anchorPosition="downLeft"
        >
          <EuiSelectable
            searchable
            searchProps={{ compressed: true, placeholder: 'Filter tags' }}
            options={options}
            onChange={handleChange}
          >
            {(list, search) => (
              <div style={{ width: 240 }}>
                {search}
                {list}
              </div>
            )}
          </EuiSelectable>
        </EuiPopover>
      </EuiFilterGroup>
    </EuiFlexItem>
  );
}
