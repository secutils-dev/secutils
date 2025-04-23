import {
  EuiButton,
  EuiButtonEmpty,
  EuiFieldText,
  EuiForm,
  EuiFormRow,
  EuiLink,
  EuiModal,
  EuiModalBody,
  EuiModalFooter,
  EuiModalHeader,
  EuiModalHeaderTitle,
  EuiSelect,
  EuiSpacer,
  EuiSwitch,
  EuiTab,
  EuiTabs,
  EuiTextArea,
  EuiTitle,
  htmlIdGenerator,
} from '@elastic/eui';
import axios from 'axios';
import { useState } from 'react';

import type { AsyncData } from '../../../../../model';
import { getApiRequestConfig, getApiUrl, getErrorMessage, isClientError } from '../../../../../model';
import { isValidURL } from '../../../../../tools/url';
import { useWorkspaceContext } from '../../../hooks';

export interface ContentSecurityPolicyImportModalProps {
  onClose: (success?: boolean) => void;
}

type ImportType = 'serialized' | 'remote';
type ImportSource = 'enforcingHeader' | 'reportOnlyHeader' | 'meta';

export function ContentSecurityPolicyImportModal({ onClose }: ContentSecurityPolicyImportModalProps) {
  const { uiState, addToast } = useWorkspaceContext();

  const [importType, setImportType] = useState<ImportType>('serialized');
  const [name, setName] = useState<string>('');
  const [serializedPolicy, setSerializedPolicy] = useState<string>('');
  const [remotePolicy, setRemotePolicy] = useState<{ url: string; followRedirects: boolean; source: ImportSource }>({
    url: '',
    followRedirects: true,
    source: 'enforcingHeader',
  });

  const canImport =
    name.trim().length > 0 &&
    (importType === 'serialized' ? serializedPolicy.length > 0 : isValidURL(remotePolicy.url));

  const [importStatus, setImportStatus] = useState<AsyncData<undefined> | null>(null);
  const serializedPolicyInput =
    importType === 'serialized' ? (
      <EuiFormRow
        label="Serialized policy"
        helpText={
          <span>
            <EuiLink target="_blank" href="https://www.w3.org/TR/CSP3/#parse-serialized-policy">
              Serialized
            </EuiLink>{' '}
            content security policy string
          </span>
        }
        fullWidth
      >
        <EuiTextArea
          fullWidth
          value={serializedPolicy}
          required
          placeholder={"E.g, default-src 'none'; script-src 'self'"}
          onChange={(e) => setSerializedPolicy(e.target.value)}
        />
      </EuiFormRow>
    ) : null;

  const urlInput =
    importType === 'remote' ? (
      <EuiFormRow label="URL" helpText="Web page URL to fetch the policy from" fullWidth>
        <EuiFieldText
          placeholder="E.g., https://secutils.dev"
          value={remotePolicy.url}
          type="url"
          required
          onChange={(e) => setRemotePolicy((parameters) => ({ ...parameters, url: e.target.value }))}
        />
      </EuiFormRow>
    ) : null;

  const sourceInput =
    importType === 'remote' ? (
      <EuiFormRow
        fullWidth
        label="Policy source"
        helpText={
          <span>
            Defines{' '}
            <EuiLink target="_blank" href="https://www.w3.org/TR/CSP3/#policy-delivery">
              the source
            </EuiLink>{' '}
            to extract the policy from
          </span>
        }
      >
        <EuiSelect
          fullWidth
          options={[
            { value: 'enforcingHeader', text: 'HTTP header (enforcing)' },
            { value: 'reportOnlyHeader', text: 'HTTP header (report only)' },
            { value: 'meta', text: 'HTML meta tag' },
          ]}
          value={remotePolicy.source}
          onChange={(e) => setRemotePolicy((parameters) => ({ ...parameters, source: e.target.value as ImportSource }))}
        />
      </EuiFormRow>
    ) : null;

  const followRedirectSwitch =
    importType === 'remote' ? (
      <EuiFormRow label="Follow redirects" fullWidth>
        <EuiSwitch
          showLabel={false}
          label="Follow redirects"
          checked={remotePolicy.followRedirects}
          onChange={(e) => setRemotePolicy((parameters) => ({ ...parameters, followRedirects: e.target.checked }))}
        />
      </EuiFormRow>
    ) : null;

  const policyNameInput = (
    <EuiFormRow label="Policy name" helpText="Arbitrary name to assign to an imported policy" fullWidth>
      <EuiFieldText value={name} required type={'text'} onChange={(e) => setName(e.target.value)} />
    </EuiFormRow>
  );

  return (
    <EuiModal onClose={() => onClose()} maxWidth={400}>
      <EuiModalHeader>
        <EuiModalHeaderTitle>
          <EuiTitle size={'s'}>
            <span>Import policy</span>
          </EuiTitle>
        </EuiModalHeaderTitle>
      </EuiModalHeader>
      <EuiModalBody>
        <EuiForm
          id="import-form"
          component="form"
          onSubmit={(e) => {
            e.preventDefault();

            if (!uiState.synced || importStatus?.status === 'pending') {
              return;
            }

            setImportStatus({ status: 'pending' });

            axios
              .post(
                getApiUrl('/api/utils/web_security/csp'),
                {
                  name,
                  content: { type: importType, value: importType === 'serialized' ? serializedPolicy : remotePolicy },
                },
                getApiRequestConfig(),
              )
              .then(
                () => {
                  addToast({
                    id: `success-import-policy-${name}`,
                    iconType: 'check',
                    color: 'success',
                    title: `Successfully imported "${name}" content security policy`,
                  });

                  setImportStatus({ status: 'succeeded', data: undefined });

                  onClose(true /** success **/);
                },
                (err: Error) => {
                  const remoteErrorMessage = getErrorMessage(err);
                  setImportStatus({ status: 'failed', error: remoteErrorMessage });

                  addToast({
                    id: htmlIdGenerator('failed-import-policy')(),
                    iconType: 'warning',
                    color: 'danger',
                    title: isClientError(err)
                      ? remoteErrorMessage
                      : `Unable to import "${name}" policy, please try again later`,
                  });
                },
              );
          }}
        >
          <EuiTabs>
            <EuiTab
              isSelected={importType === 'serialized'}
              title={'Import a new policy from a serialized policy string'}
              onClick={() => setImportType('serialized')}
            >
              Serialized policy
            </EuiTab>
            <EuiTab
              isSelected={importType === 'remote'}
              title={'Import a new policy from external URL'}
              onClick={() => setImportType('remote')}
            >
              URL
            </EuiTab>
          </EuiTabs>
          <EuiSpacer />
          {policyNameInput}
          {serializedPolicyInput}
          {urlInput}
          {followRedirectSwitch}
          {sourceInput}
        </EuiForm>
      </EuiModalBody>
      <EuiModalFooter>
        <EuiButtonEmpty onClick={() => onClose()}>Cancel</EuiButtonEmpty>
        <EuiButton
          type="submit"
          form="import-form"
          fill
          isLoading={importStatus?.status === 'pending'}
          isDisabled={!canImport}
        >
          Import
        </EuiButton>
      </EuiModalFooter>
    </EuiModal>
  );
}
