import { EuiDescribedFormGroup, EuiFieldText, EuiFilePicker, EuiForm, EuiFormRow, EuiTextArea } from '@elastic/eui';
import { useCallback, useState } from 'react';

import { useFormChanges, useUserTags } from '../../../../hooks';
import { createUserSecret, updateUserSecret } from '../../../../model';
import type { AsyncData } from '../../../../model';
import { EditorFlyout } from '../../components/editor_flyout';
import { TagsComboBox } from '../../components/tags_combo_box';
import { useWorkspaceContext } from '../../hooks';

const MAX_VALUE_LENGTH = 10 * 1024;
const NAME_REGEX = /^[a-zA-Z][a-zA-Z0-9_-]*$/;

export interface SecretEditFlyoutProps {
  editingId?: string;
  editingName?: string;
  initialTagIds?: string[];
  onClose: (success?: boolean) => void;
}

export function SecretEditFlyout({ editingId, editingName, initialTagIds, onClose }: SecretEditFlyoutProps) {
  const { addToast } = useWorkspaceContext();

  const isEditing = editingName !== undefined;
  const [name, setName] = useState(editingName ?? '');
  const [value, setValue] = useState('');
  const { allTags, setAllTags } = useUserTags();
  const [selectedTagIds, setSelectedTagIds] = useState<string[]>(initialTagIds ?? []);
  const [updatingStatus, setUpdatingStatus] = useState<AsyncData<void>>();

  const nameValid = NAME_REGEX.test(name) && name.length <= 128;
  const valueTooLong = value.length > MAX_VALUE_LENGTH;
  const valueValid = value.length > 0 && !valueTooLong;

  const hasFormChanges = useFormChanges({ name, value, selectedTagIds });

  const handleFilePick = useCallback((files: FileList | null) => {
    if (!files || files.length === 0) {
      return;
    }
    const reader = new FileReader();
    reader.onload = () => {
      if (typeof reader.result === 'string') {
        setValue(reader.result);
      }
    };
    reader.readAsText(files[0]);
  }, []);

  const handleSave = useCallback(() => {
    if (updatingStatus?.status === 'pending') {
      return;
    }

    setUpdatingStatus({ status: 'pending' });

    const promise = isEditing
      ? updateUserSecret(editingId!, value.length > 0 ? value : undefined, selectedTagIds)
      : createUserSecret(name, value, selectedTagIds);

    promise
      .then(() => {
        setUpdatingStatus({ status: 'succeeded', data: undefined });
        addToast({
          id: `success-save-secret-${name}`,
          iconType: 'check',
          color: 'success',
          title: `Secret "${name}" ${isEditing ? 'updated' : 'created'}`,
        });
        onClose(true);
      })
      .catch((err: Error) => {
        setUpdatingStatus({ status: 'failed', error: err.message });
        addToast({
          id: `failed-save-secret-${name}`,
          iconType: 'warning',
          color: 'danger',
          title: err.message,
        });
      });
  }, [updatingStatus, name, value, selectedTagIds, isEditing, editingId, addToast, onClose]);

  return (
    <EditorFlyout
      title={`${isEditing ? 'Update' : 'Add'} secret`}
      onClose={() => onClose()}
      hasChanges={hasFormChanges}
      onSave={handleSave}
      canSave={isEditing ? hasFormChanges && !valueTooLong : nameValid && valueValid}
      saveInProgress={updatingStatus?.status === 'pending'}
    >
      <EuiForm id="update-form" component="form" fullWidth>
        <EuiDescribedFormGroup title={<h3>General</h3>} description="General properties of the secret">
          <EuiFormRow
            label="Name"
            helpText="Starts with a letter. Letters, digits, underscores, and hyphens only."
            isInvalid={name.length > 0 && !nameValid}
            error={
              name.length > 0 && !nameValid
                ? name.length > 128
                  ? 'Name must be 128 characters or fewer.'
                  : 'Must start with a letter and contain only letters, digits, underscores, and hyphens.'
                : undefined
            }
          >
            <EuiFieldText
              name="secretName"
              value={name}
              onChange={(e) => setName(e.target.value)}
              disabled={isEditing}
              maxLength={128}
              placeholder="MY_API_KEY"
            />
          </EuiFormRow>
          <TagsComboBox
            allTags={allTags}
            selectedTagIds={selectedTagIds}
            onChange={setSelectedTagIds}
            onTagCreated={(tag) => setAllTags((prev) => [...prev, tag])}
          />
        </EuiDescribedFormGroup>
        <EuiDescribedFormGroup
          title={<h3>Value</h3>}
          description="The secret value is write-only and cannot be retrieved after saving."
        >
          <EuiFormRow
            label="Value"
            isInvalid={valueTooLong}
            error={
              valueTooLong
                ? `Value is too long (${(value.length / 1024).toFixed(1)} KB). Maximum allowed size is 10 KB.`
                : undefined
            }
          >
            <EuiTextArea
              value={value}
              onChange={(e) => setValue(e.target.value)}
              rows={6}
              placeholder="Enter secret value…"
            />
          </EuiFormRow>
          <EuiFormRow label="Or upload from file" helpText="File content will replace the value above.">
            <EuiFilePicker onChange={handleFilePick} accept="*/*" display="default" />
          </EuiFormRow>
        </EuiDescribedFormGroup>
      </EuiForm>
    </EditorFlyout>
  );
}
