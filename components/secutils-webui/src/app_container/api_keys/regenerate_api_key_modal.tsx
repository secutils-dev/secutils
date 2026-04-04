import { EuiConfirmModal, EuiDatePicker, EuiFormRow } from '@elastic/eui';
import type { Moment } from 'moment/moment';
import moment from 'moment/moment';
import { useState } from 'react';

export interface RegenerateConfirmModalProps {
  name: string;
  saving: boolean;
  onCancel: () => void;
  onConfirm: (expiresAt?: number) => void;
}

export function RegenerateConfirmModal({ name, saving, onCancel, onConfirm }: RegenerateConfirmModalProps) {
  const [expiresDate, setExpiresDate] = useState<Moment | null>(null);
  const expiresValid = !expiresDate || expiresDate.isAfter(moment());

  return (
    <EuiConfirmModal
      title={`Regenerate API key "${name}"?`}
      onCancel={onCancel}
      onConfirm={() => onConfirm(expiresDate ? expiresDate.unix() : undefined)}
      cancelButtonText="Cancel"
      confirmButtonText="Regenerate"
      buttonColor="warning"
      confirmButtonDisabled={!expiresValid || saving}
      isLoading={saving}
    >
      <p>The current token will be immediately invalidated. A new token will be generated.</p>
      <EuiFormRow label="New expiration">
        <EuiDatePicker
          selected={expiresDate}
          onChange={setExpiresDate}
          minDate={moment()}
          showTimeSelect
          placeholder="Never"
          isInvalid={!!expiresDate && !expiresValid}
        />
      </EuiFormRow>
    </EuiConfirmModal>
  );
}
