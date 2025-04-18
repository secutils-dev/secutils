import type { ChangeEvent, MouseEventHandler } from 'react';
import { useCallback, useState } from 'react';

import {
  EuiButton,
  EuiButtonEmpty,
  EuiCallOut,
  EuiForm,
  EuiFormRow,
  EuiModal,
  EuiModalBody,
  EuiModalFooter,
  EuiModalHeader,
  EuiModalHeaderTitle,
  EuiSelect,
  EuiTitle,
} from '@elastic/eui';
import axios from 'axios';

import type { EncryptionMode } from './encryption_mode';
import { EncryptionModeSelector } from './encryption_mode_selector';
import type { PrivateKey } from './private_key';
import type { AsyncData } from '../../../../model';
import { getApiRequestConfig, getApiUrl, getErrorMessage } from '../../../../model';
import { Downloader } from '../../../../tools/downloader';

export interface PrivateKeyExportModalProps {
  privateKey: PrivateKey;
  onClose: () => void;
}

export function PrivateKeyExportModal({ privateKey, onClose }: PrivateKeyExportModalProps) {
  const [format, setFormat] = useState<string>('pkcs12');
  const onFormatChange = useCallback((e: ChangeEvent<HTMLSelectElement>) => {
    setFormat(e.target.value);
  }, []);

  const [exportEncryptionMode, setExportEncryptionMode] = useState<EncryptionMode>(
    privateKey.encrypted ? 'passphrase' : 'none',
  );
  const [exportPassphrase, setExportPassphrase] = useState<string | null>(privateKey.encrypted ? '' : null);
  const [currentPassphrase, setCurrentPassphrase] = useState<string>('');

  const [exportStatus, setExportStatus] = useState<AsyncData<undefined> | null>(null);
  const onPrivateKeyExport: MouseEventHandler<HTMLButtonElement> = useCallback(
    (e) => {
      e.preventDefault();

      if (exportStatus?.status === 'pending') {
        return;
      }

      setExportStatus({ status: 'pending' });

      axios
        .post<number[]>(
          getApiUrl(`/api/utils/certificates/private_keys/${encodeURIComponent(privateKey.id)}/export`),
          {
            format,
            passphrase: privateKey.encrypted ? currentPassphrase : null,
            exportPassphrase: exportEncryptionMode === 'passphrase' ? exportPassphrase : null,
          },
          getApiRequestConfig(),
        )
        .then(
          (response) => {
            const keyContent = new Uint8Array(response.data);
            if (format === 'pem') {
              Downloader.download(`${privateKey.name}.pem`, keyContent, 'application/x-pem-file');
            } else if (format === 'pkcs8') {
              Downloader.download(`${privateKey.name}.p8`, keyContent, 'application/pkcs8');
            } else {
              Downloader.download(`${privateKey.name}.pfx`, keyContent, 'application/x-pkcs12');
            }

            setExportStatus({ status: 'succeeded', data: undefined });

            onClose();
          },
          (err: Error) => {
            setExportStatus({ status: 'failed', error: getErrorMessage(err) });
          },
        );
    },
    [exportPassphrase, currentPassphrase, exportEncryptionMode, format, exportStatus],
  );

  const exportStatusCallout =
    exportStatus?.status === 'succeeded' ? (
      <EuiFormRow>
        <EuiCallOut size="s" title="Private key successfully exported." color="success" iconType="check" />
      </EuiFormRow>
    ) : exportStatus?.status === 'failed' ? (
      <EuiFormRow>
        <EuiCallOut
          size="s"
          title={exportStatus.error ?? 'An error occurred, please try again later'}
          color="danger"
          iconType="warning"
        />
      </EuiFormRow>
    ) : undefined;

  return (
    <EuiModal onClose={onClose}>
      <EuiModalHeader>
        <EuiModalHeaderTitle>
          <EuiTitle size={'s'}>
            <span>Export</span>
          </EuiTitle>
        </EuiModalHeaderTitle>
      </EuiModalHeader>
      <EuiModalBody>
        <EuiForm id="export-form" component="form">
          {exportStatusCallout}
          <EuiFormRow label="Format">
            <EuiSelect
              options={[
                { value: 'pem', text: 'PEM' },
                { value: 'pkcs8', text: 'PKCS#8' },
                { value: 'pkcs12', text: 'PKCS#12' },
              ]}
              value={format}
              onChange={onFormatChange}
            />
          </EuiFormRow>
          <EncryptionModeSelector
            initialMode={privateKey.encrypted ? 'passphrase' : 'none'}
            requireCurrentPassphrase={privateKey.encrypted}
            passphraseLabel={'Export passphrase'}
            repeatPassphraseLabel={'Repeat export passphrase'}
            onChange={(mode, passphrase, currentPassphrase) => {
              setExportEncryptionMode(mode);
              setExportPassphrase(passphrase);
              if (currentPassphrase !== undefined) {
                setCurrentPassphrase(currentPassphrase);
              }
            }}
          />
        </EuiForm>
      </EuiModalBody>
      <EuiModalFooter>
        <EuiButtonEmpty onClick={onClose}>Cancel</EuiButtonEmpty>
        <EuiButton
          type="submit"
          form="export-form"
          fill
          onClick={onPrivateKeyExport}
          isDisabled={exportEncryptionMode === 'passphrase' && exportPassphrase === null}
          isLoading={exportStatus?.status === 'pending'}
        >
          Export
        </EuiButton>
      </EuiModalFooter>
    </EuiModal>
  );
}
