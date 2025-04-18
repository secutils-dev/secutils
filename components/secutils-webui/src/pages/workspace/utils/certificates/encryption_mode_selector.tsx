import { type ChangeEvent, useState } from 'react';

import { EuiFieldText, EuiFormRow, EuiSelect } from '@elastic/eui';

import type { EncryptionMode } from './encryption_mode';

export interface EncryptionModeSelectorProps {
  initialMode?: EncryptionMode;
  requireCurrentPassphrase?: boolean;
  passphraseLabel?: string;
  repeatPassphraseLabel?: string;
  onChange(mode: EncryptionMode, passphrase: string | null, currentPassphrase?: string): void;
}

interface Passphrases {
  new: string;
  repeatNew: string;
  current?: string;
}

export function EncryptionModeSelector({
  initialMode,
  requireCurrentPassphrase,
  passphraseLabel = 'Passphrase',
  repeatPassphraseLabel = 'Repeat passphrase',
  onChange,
}: EncryptionModeSelectorProps) {
  const [mode, setMode] = useState<EncryptionMode>(initialMode ?? 'passphrase');
  const onModeChange = (e: ChangeEvent<HTMLSelectElement>) => {
    const newMode = e.target.value as EncryptionMode;
    setMode(newMode);
    setPassphrases((currentValue) => ({ ...currentValue, new: '', repeatNew: '' }));

    switch (newMode) {
      case 'passphrase':
        onChange(newMode, '', passphrases.current);
        break;
      case 'none':
        onChange(newMode, null, passphrases.current);
    }
  };

  const [passphrases, setPassphrases] = useState<Passphrases>({
    new: '',
    repeatNew: '',
    current: requireCurrentPassphrase ? '' : undefined,
  });
  const onPassphrasesChange = (passphrases: Passphrases) => {
    setPassphrases(passphrases);

    onChange(
      mode,
      mode === 'passphrase' && passphrases.new === passphrases.repeatNew ? passphrases.new : null,
      passphrases.current,
    );
  };

  // Display current passphrase field only when specifically requested and when the encryption mode either `passphrase`
  // or differs from the initial one.
  const currentPassphrase =
    requireCurrentPassphrase && (mode === 'passphrase' || mode !== initialMode) ? (
      <EuiFormRow label="Current passphrase">
        <EuiFieldText
          value={passphrases.current ?? ''}
          placeholder={'(Empty passphrase)'}
          type={'password'}
          onChange={(e) => onPassphrasesChange({ ...passphrases, current: e.target.value })}
        />
      </EuiFormRow>
    ) : null;

  return (
    <>
      <EuiFormRow
        label="Encryption"
        helpText="Specifies whether the private key should be encrypted with a passphrase or not."
      >
        <EuiSelect
          options={[
            { value: 'none', text: 'None' },
            { value: 'passphrase', text: 'Passphrase' },
          ]}
          value={mode}
          onChange={onModeChange}
        />
      </EuiFormRow>
      {currentPassphrase}
      {mode === 'passphrase' ? (
        <>
          <EuiFormRow label={passphraseLabel}>
            <EuiFieldText
              value={passphrases.new}
              type={'password'}
              placeholder={'(Empty passphrase)'}
              onChange={(e) => onPassphrasesChange({ ...passphrases, new: e.target.value })}
            />
          </EuiFormRow>
          <EuiFormRow label={repeatPassphraseLabel}>
            <EuiFieldText
              value={passphrases.repeatNew}
              isInvalid={passphrases.new !== passphrases.repeatNew}
              type={'password'}
              placeholder={'(Empty passphrase)'}
              onChange={(e) => onPassphrasesChange({ ...passphrases, repeatNew: e.target.value })}
            />
          </EuiFormRow>
        </>
      ) : null}
    </>
  );
}
