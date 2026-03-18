import {
  EuiButton,
  EuiButtonEmpty,
  EuiFieldText,
  EuiFilePicker,
  EuiFormRow,
  EuiModal,
  EuiModalBody,
  EuiModalFooter,
  EuiModalHeader,
  EuiModalHeaderTitle,
  EuiTextArea,
} from '@elastic/eui';
import { useCallback, useState } from 'react';

import type { PageToast } from '../pages/page';

const MAX_VALUE_LENGTH = 10 * 1024;

export interface SecretEditModalProps {
  editingId?: string;
  editingName?: string;
  onSave: (name: string, value: string, editingId?: string) => Promise<void>;
  onClose: () => void;
  addToast?: (toast: PageToast) => void;
}

const NAME_REGEX = /^[a-zA-Z][a-zA-Z0-9_-]*$/;

export function SecretEditModal({ editingId, editingName, onSave, onClose, addToast }: SecretEditModalProps) {
  const isEditing = editingName !== undefined;
  const [name, setName] = useState(editingName ?? '');
  const [value, setValue] = useState('');
  const [saving, setSaving] = useState(false);

  const nameValid = NAME_REGEX.test(name) && name.length <= 128;
  const valueTooLong = value.length > MAX_VALUE_LENGTH;
  const valueValid = value.length > 0 && !valueTooLong;

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

  const handleSave = useCallback(async () => {
    setSaving(true);
    try {
      await onSave(name, value, editingId);
      onClose();
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to save secret.';
      addToast?.({ id: `failed-save-secret-${name}`, iconType: 'warning', color: 'danger', title: message });
    } finally {
      setSaving(false);
    }
  }, [name, value, editingId, onSave, onClose, addToast]);

  return (
    <EuiModal onClose={onClose} initialFocus="[name=secretName]" style={{ width: 600, maxWidth: '90vw' }}>
      <EuiModalHeader>
        <EuiModalHeaderTitle>{isEditing ? 'Update secret' : 'Add secret'}</EuiModalHeaderTitle>
      </EuiModalHeader>
      <EuiModalBody>
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
          fullWidth
          style={{ minHeight: '90px' }}
        >
          <EuiFieldText
            name="secretName"
            value={name}
            onChange={(e) => setName(e.target.value)}
            disabled={isEditing}
            maxLength={128}
            placeholder="MY_API_KEY"
            fullWidth
          />
        </EuiFormRow>
        <EuiFormRow
          label="Value"
          isInvalid={valueTooLong}
          error={
            valueTooLong
              ? `Value is too long (${(value.length / 1024).toFixed(1)} KB). Maximum allowed size is 10 KB.`
              : undefined
          }
          fullWidth
          style={{ minHeight: '190px' }}
        >
          <EuiTextArea
            value={value}
            onChange={(e) => setValue(e.target.value)}
            rows={6}
            placeholder="Enter secret value…"
            fullWidth
          />
        </EuiFormRow>
        <EuiFormRow label="Or upload from file" helpText="File content will replace the value above." fullWidth>
          <EuiFilePicker onChange={handleFilePick} accept="*/*" display="default" fullWidth />
        </EuiFormRow>
      </EuiModalBody>
      <EuiModalFooter>
        <EuiButtonEmpty onClick={onClose}>Cancel</EuiButtonEmpty>
        <EuiButton fill onClick={handleSave} isLoading={saving} disabled={!nameValid || !valueValid || saving}>
          {isEditing ? 'Update' : 'Create'}
        </EuiButton>
      </EuiModalFooter>
    </EuiModal>
  );
}
