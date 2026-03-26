import { EuiComboBox, EuiFormRow } from '@elastic/eui';
import type { EuiComboBoxOptionOption } from '@elastic/eui';
import { useCallback, useMemo, useState } from 'react';

import type { UserTag } from '../../../model/user_tags';
import { createUserTag, TAG_COLOR_SWATCHES } from '../../../model/user_tags';

export interface TagsComboBoxProps {
  allTags: UserTag[];
  selectedTagIds: string[];
  onChange: (tagIds: string[]) => void;
  onTagCreated?: (tag: UserTag) => void;
}

export function TagsComboBox({ allTags, selectedTagIds, onChange, onTagCreated }: TagsComboBoxProps) {
  const [isCreating, setIsCreating] = useState(false);

  const options: EuiComboBoxOptionOption<string>[] = useMemo(
    () =>
      allTags.map((tag) => ({
        label: tag.name,
        value: tag.id,
        color: tag.color,
      })),
    [allTags],
  );

  const selectedOptions = useMemo(
    () => options.filter((opt) => selectedTagIds.includes(opt.value!)),
    [options, selectedTagIds],
  );

  const handleChange = useCallback(
    (selected: EuiComboBoxOptionOption<string>[]) => {
      onChange(selected.map((opt) => opt.value!));
    },
    [onChange],
  );

  const handleCreateOption = useCallback(
    (searchValue: string) => {
      const trimmed = searchValue.trim().toLowerCase();
      if (!trimmed || isCreating) {
        return;
      }

      const existing = allTags.find((t) => t.name === trimmed);
      if (existing) {
        if (!selectedTagIds.includes(existing.id)) {
          onChange([...selectedTagIds, existing.id]);
        }
        return;
      }

      setIsCreating(true);
      const color = TAG_COLOR_SWATCHES[allTags.length % TAG_COLOR_SWATCHES.length];
      createUserTag(trimmed, color)
        .then((newTag) => {
          onTagCreated?.(newTag);
          onChange([...selectedTagIds, newTag.id]);
        })
        .catch(() => {})
        .finally(() => setIsCreating(false));
    },
    [allTags, selectedTagIds, isCreating, onChange, onTagCreated],
  );

  return (
    <EuiFormRow label="Tags" helpText="Select existing tags or type to create a new one." fullWidth>
      <EuiComboBox
        placeholder="Select tags"
        options={options}
        selectedOptions={selectedOptions}
        onChange={handleChange}
        onCreateOption={handleCreateOption}
        isLoading={isCreating}
        isClearable
        fullWidth
      />
    </EuiFormRow>
  );
}
