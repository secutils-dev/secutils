import { EuiConfirmModal, EuiDatePicker, EuiFieldText, EuiFormRow } from '@elastic/eui';
import type { Moment } from 'moment/moment';
import moment from 'moment/moment';
import { useState } from 'react';

export const MAX_NAME_LENGTH = 128;

export interface CreateEditModalProps {
  mode: 'create' | 'edit';
  initialName?: string;
  saving: boolean;
  onSave: (name: string, expiresAt?: number) => void;
  onCancel: () => void;
}

export function CreateEditModal({ mode, initialName = '', saving, onSave, onCancel }: CreateEditModalProps) {
  const [name, setName] = useState(initialName);
  const [expiresDate, setExpiresDate] = useState<Moment | null>(null);

  const nameValid = name.trim().length > 0 && name.length <= MAX_NAME_LENGTH;
  const expiresValid = !expiresDate || expiresDate.isAfter(moment());
  const canSave = nameValid && expiresValid && !saving;

  return (
    <EuiConfirmModal
      title={mode === 'create' ? 'Create API key' : 'Rename API key'}
      onCancel={onCancel}
      onConfirm={() => onSave(name.trim(), expiresDate ? expiresDate.unix() : undefined)}
      cancelButtonText="Cancel"
      confirmButtonText={mode === 'create' ? 'Create' : 'Save'}
      confirmButtonDisabled={!canSave}
      isLoading={saving}
    >
      <EuiFormRow label="Name" fullWidth>
        <EuiFieldText
          placeholder="e.g. CI deployment key"
          value={name}
          maxLength={MAX_NAME_LENGTH}
          onChange={(e) => setName(e.target.value)}
          autoFocus
          fullWidth
        />
      </EuiFormRow>
      {mode === 'create' && (
        <EuiFormRow label="Expires" fullWidth>
          <EuiDatePicker
            selected={expiresDate}
            onChange={setExpiresDate}
            minDate={moment()}
            showTimeSelect
            timeFormat="HH:mm"
            placeholder="Never"
            fullWidth
            isInvalid={!!expiresDate && !expiresValid}
          />
        </EuiFormRow>
      )}
    </EuiConfirmModal>
  );
}
