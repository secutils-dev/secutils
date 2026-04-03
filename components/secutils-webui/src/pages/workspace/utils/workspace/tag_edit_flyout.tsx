import { EuiBadge, EuiColorPicker, EuiDescribedFormGroup, EuiFieldText, EuiForm, EuiFormRow } from '@elastic/eui';
import { useCallback, useState } from 'react';

import { useFormChanges } from '../../../../hooks';
import type { AsyncData, UserTag } from '../../../../model';
import { createUserTag, TAG_COLOR_SWATCHES, updateUserTag } from '../../../../model';
import { EditorFlyout } from '../../components/editor_flyout';
import { useWorkspaceContext } from '../../hooks';

export interface TagEditFlyoutProps {
  tag?: Partial<UserTag>;
  onClose: (success?: boolean) => void;
}

export function TagEditFlyout({ onClose, tag }: TagEditFlyoutProps) {
  const { addToast } = useWorkspaceContext();

  const [name, setName] = useState(tag?.name ?? '');
  const [color, setColor] = useState((tag?.color ?? TAG_COLOR_SWATCHES[0]).toLowerCase());
  const [colorIsValid, setColorIsValid] = useState(true);
  const [updatingStatus, setUpdatingStatus] = useState<AsyncData<void>>();

  const hasFormChanges = useFormChanges({ name, color });
  const isEditing = !!tag?.id;
  const nameValid = name.trim().length > 0 && name.trim().length <= 50;

  const handleSave = useCallback(() => {
    if (updatingStatus?.status === 'pending') {
      return;
    }

    setUpdatingStatus({ status: 'pending' });

    const trimmedName = name.trim().toLowerCase();
    const promise = isEditing
      ? updateUserTag(tag!.id!, { name: trimmedName, color })
      : createUserTag(trimmedName, color);

    promise
      .then(() => {
        setUpdatingStatus({ status: 'succeeded', data: undefined });
        addToast({
          id: `success-save-tag-${trimmedName}`,
          iconType: 'check',
          color: 'success',
          title: `Tag "${trimmedName}" ${isEditing ? 'updated' : 'created'}`,
        });
        onClose(true);
      })
      .catch((err: Error) => {
        setUpdatingStatus({ status: 'failed', error: err.message });
        addToast({
          id: `failed-save-tag-${trimmedName}`,
          iconType: 'warning',
          color: 'danger',
          title: err.message,
        });
      });
  }, [updatingStatus, name, color, isEditing, tag, addToast, onClose]);

  return (
    <EditorFlyout
      title={`${isEditing ? 'Edit' : 'Add'} tag`}
      onClose={() => onClose()}
      hasChanges={hasFormChanges}
      onSave={handleSave}
      canSave={nameValid && colorIsValid && hasFormChanges}
      saveInProgress={updatingStatus?.status === 'pending'}
    >
      <EuiForm id="update-form" component="form" fullWidth>
        <EuiDescribedFormGroup title={<h3>General</h3>} description="General properties of the tag">
          <EuiFormRow
            label="Name"
            helpText="Tag name (1–50 characters). Will be lowercased."
            isInvalid={name.length > 0 && !nameValid}
            error={name.length > 50 ? 'Name must be 50 characters or fewer.' : undefined}
          >
            <EuiFieldText
              name="tagName"
              value={name}
              onChange={(e) => setName(e.target.value)}
              maxLength={50}
              placeholder="e.g. production, staging, personal"
            />
          </EuiFormRow>
        </EuiDescribedFormGroup>
        <EuiDescribedFormGroup title={<h3>Appearance</h3>} description="Color and visual preview of the tag badge">
          <EuiFormRow label="Color">
            <EuiColorPicker
              color={color}
              onChange={(text, { isValid }) => {
                setColor(text.toLowerCase());
                setColorIsValid(isValid);
              }}
              swatches={TAG_COLOR_SWATCHES as unknown as string[]}
              isInvalid={!colorIsValid}
            />
          </EuiFormRow>
          {name.trim().length > 0 && colorIsValid ? (
            <EuiFormRow label="Preview">
              <EuiBadge color={color}>{name.trim().toLowerCase()}</EuiBadge>
            </EuiFormRow>
          ) : null}
        </EuiDescribedFormGroup>
      </EuiForm>
    </EditorFlyout>
  );
}
