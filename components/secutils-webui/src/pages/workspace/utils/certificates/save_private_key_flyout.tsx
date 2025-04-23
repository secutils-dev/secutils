import { EuiDescribedFormGroup, EuiFieldText, EuiForm, EuiFormRow, EuiLink, EuiSelect } from '@elastic/eui';
import axios from 'axios';
import type { ChangeEvent } from 'react';
import { useCallback, useState } from 'react';

import type { EncryptionMode } from './encryption_mode';
import { EncryptionModeSelector } from './encryption_mode_selector';
import type { PrivateKey } from './private_key';
import type { PrivateKeyAlgorithm, PrivateKeyCurveName, PrivateKeySize } from './private_key_alg';
import { privateKeyCurveNameString } from './private_key_alg';
import type { AsyncData } from '../../../../model';
import { getApiRequestConfig, getApiUrl, getErrorMessage, isClientError } from '../../../../model';
import { EditorFlyout } from '../../components/editor_flyout';
import { useWorkspaceContext } from '../../hooks';

export interface SavePrivateKeyFlyoutProps {
  privateKey?: PrivateKey;
  onClose: (success?: boolean) => void;
}

export function SavePrivateKeyFlyout({ onClose, privateKey }: SavePrivateKeyFlyoutProps) {
  const { addToast } = useWorkspaceContext();

  const [name, setName] = useState<string>(privateKey?.name ?? '');
  const onNameChange = useCallback((e: ChangeEvent<HTMLInputElement>) => {
    setName(e.target.value);
  }, []);

  const [keyAlgorithm, setKeyAlgorithm] = useState<PrivateKeyAlgorithm>(
    privateKey?.alg && typeof privateKey?.alg === 'object' ? privateKey.alg : { keyType: 'ed25519' },
  );
  const onKeyAlgorithmChange = (e: ChangeEvent<HTMLSelectElement>) => {
    const keyType = e.target.value as PrivateKeyAlgorithm['keyType'];
    if (keyType === 'ed25519') {
      setKeyAlgorithm({ keyType });
    } else if (keyType === 'ecdsa') {
      setKeyAlgorithm({ keyType, curve: 'secp256r1' });
    } else {
      setKeyAlgorithm({ keyType, keySize: '2048' });
    }
  };

  const onKeySizeChange = (e: ChangeEvent<HTMLSelectElement>) => {
    setKeyAlgorithm((keyAlg) =>
      'keySize' in keyAlg ? { ...keyAlg, keySize: e.target.value as PrivateKeySize } : keyAlg,
    );
  };

  const onCurveChange = (e: ChangeEvent<HTMLSelectElement>) => {
    setKeyAlgorithm((keyAlg) =>
      'curve' in keyAlg ? { ...keyAlg, curve: e.target.value as PrivateKeyCurveName } : keyAlg,
    );
  };

  const [encryptionMode, setEncryptionMode] = useState<EncryptionMode>(
    !privateKey || privateKey.encrypted ? 'passphrase' : 'none',
  );
  const [passphrase, setPassphrase] = useState<string | null>('');
  const [currentPassphrase, setCurrentPassphrase] = useState<string>('');

  const [updatingStatus, setUpdatingStatus] = useState<AsyncData<void>>();

  const canSave = () => {
    if (name.trim().length === 0) {
      return false;
    }

    // If it's a new key, just make sure passphrase is either not needed or set correctly.
    if (!privateKey) {
      return encryptionMode === 'none' || passphrase !== null;
    }

    // If encryption was disabled, update is only possible if the key is still encrypted or name has changed.
    const nameChanged = privateKey.name !== name;
    if (encryptionMode === 'none') {
      return nameChanged || privateKey.encrypted;
    }

    // Otherwise, allow saving only if the name or encryption has changed.
    return passphrase !== null && (nameChanged || !privateKey.encrypted || currentPassphrase !== passphrase);
  };

  return (
    <EditorFlyout
      title={`${privateKey ? 'Edit' : 'Add'} private key`}
      onClose={() => onClose()}
      onSave={() => {
        if (updatingStatus?.status === 'pending') {
          return;
        }

        setUpdatingStatus({ status: 'pending' });

        // Only passphrase and name change are allowed for existing private keys.
        const newPassphraseToSend = encryptionMode === 'passphrase' ? passphrase : null;
        const currentPassphraseToSend = privateKey?.encrypted ? currentPassphrase : null;
        const [requestPromise, successMessage, errorMessage] = privateKey
          ? [
              axios.put(
                getApiUrl(`/api/utils/certificates/private_keys/${privateKey.id}`),
                {
                  keyName: privateKey.name !== name ? name.trim() : null,
                  ...(!privateKey.encrypted || newPassphraseToSend !== currentPassphraseToSend
                    ? { passphrase: currentPassphraseToSend, newPassphrase: newPassphraseToSend }
                    : {}),
                },
                getApiRequestConfig(),
              ),
              `Successfully updated "${name}" private key`,
              `Unable to update "${name}" private key, please try again later`,
            ]
          : [
              axios.post(
                getApiUrl('/api/utils/certificates/private_keys'),
                { keyName: name, alg: keyAlgorithm, passphrase: newPassphraseToSend },
                getApiRequestConfig(),
              ),
              `Successfully saved "${name}" private key`,
              `Unable to save "${name}" private key, please try again later`,
            ];
        requestPromise.then(
          () => {
            setUpdatingStatus({ status: 'succeeded', data: undefined });

            addToast({
              id: `success-save-private-key-${name}`,
              iconType: 'check',
              color: 'success',
              title: successMessage,
            });

            onClose(true);
          },
          (err: Error) => {
            const remoteErrorMessage = getErrorMessage(err);
            setUpdatingStatus({ status: 'failed', error: remoteErrorMessage });

            addToast({
              id: `failed-save-private-key-${name}`,
              iconType: 'warning',
              color: 'danger',
              title: isClientError(err) ? remoteErrorMessage : errorMessage,
            });
          },
        );
      }}
      canSave={canSave()}
      saveInProgress={updatingStatus?.status === 'pending'}
    >
      <EuiForm id="update-form" component="form" fullWidth>
        <EuiDescribedFormGroup title={<h3>General</h3>} description={'General properties of the private key'}>
          <EuiFormRow label="Name" helpText="Unique name of the private key.">
            <EuiFieldText value={name} required type={'text'} onChange={onNameChange} />
          </EuiFormRow>
          <EuiFormRow label="Key algorithm" helpText="Private key algorithm." isDisabled={!!privateKey}>
            <EuiSelect
              options={[
                { value: 'rsa', text: 'RSA' },
                { value: 'dsa', text: 'DSA' },
                { value: 'ecdsa', text: 'ECDSA' },
                { value: 'ed25519', text: 'Ed25519' },
              ]}
              value={keyAlgorithm.keyType}
              onChange={onKeyAlgorithmChange}
            />
          </EuiFormRow>
          {'keySize' in keyAlgorithm ? (
            <EuiFormRow label="Key size" helpText="Private key size." isDisabled={!!privateKey}>
              <EuiSelect
                options={[
                  { value: '1024', text: '1024 bit' },
                  { value: '2048', text: '2048 bit' },
                  { value: '4096', text: '4096 bit' },
                  { value: '8192', text: '8192 bit' },
                ]}
                value={keyAlgorithm.keySize}
                onChange={onKeySizeChange}
              />
            </EuiFormRow>
          ) : null}
          {'curve' in keyAlgorithm ? (
            <EuiFormRow
              label="Curve name"
              helpText={
                <span>
                  <EuiLink target="_blank" href="https://www.rfc-editor.org/rfc/rfc8422.html#section-5.1.1">
                    Elliptic curve
                  </EuiLink>{' '}
                  used for cryptographic operations.
                </span>
              }
              isDisabled={!!privateKey}
            >
              <EuiSelect
                options={[
                  { value: 'secp256r1', text: privateKeyCurveNameString('secp256r1') },
                  { value: 'secp384r1', text: privateKeyCurveNameString('secp384r1') },
                  { value: 'secp521r1', text: privateKeyCurveNameString('secp521r1') },
                ]}
                value={keyAlgorithm.curve}
                onChange={onCurveChange}
              />
            </EuiFormRow>
          ) : null}
        </EuiDescribedFormGroup>
        <EuiDescribedFormGroup title={<h3>Security</h3>} description={'Security properties of the private key'}>
          <EncryptionModeSelector
            initialMode={privateKey?.encrypted === false ? 'none' : 'passphrase'}
            requireCurrentPassphrase={!!privateKey?.encrypted}
            passphraseLabel={privateKey ? 'New passphrase' : undefined}
            repeatPassphraseLabel={privateKey ? 'Repeat new passphrase' : undefined}
            onChange={(mode, passphrase, currentPassphrase) => {
              setEncryptionMode(mode);
              setPassphrase(passphrase);
              if (currentPassphrase !== undefined) {
                setCurrentPassphrase(currentPassphrase);
              }
            }}
          />
        </EuiDescribedFormGroup>
      </EuiForm>
    </EditorFlyout>
  );
}
