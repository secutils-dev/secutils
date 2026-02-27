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

const MAX_VALUE_LENGTH = 10 * 1024;

export interface SecretEditModalProps {
  editingName?: string;
  onSave: (name: string, value: string) => Promise<void>;
  onClose: () => void;
}

const NAME_REGEX = /^[a-zA-Z][a-zA-Z0-9_-]*$/;

export function SecretEditModal({ editingName, onSave, onClose }: SecretEditModalProps) {
  const isEditing = editingName !== undefined;
  const [name, setName] = useState(editingName ?? '');
  const [value, setValue] = useState('');
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

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
    setError(null);
    try {
      await onSave(name, value);
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to save secret.');
    } finally {
      setSaving(false);
    }
  }, [name, value, onSave, onClose]);

  return (
    <EuiModal onClose={onClose} initialFocus="[name=secretName]">
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
        <EuiFormRow
          label="Value"
          isInvalid={valueTooLong || !!error}
          error={
            valueTooLong
              ? `Value is too long (${(value.length / 1024).toFixed(1)} KB). Maximum allowed size is 10 KB.`
              : (error ?? undefined)
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
