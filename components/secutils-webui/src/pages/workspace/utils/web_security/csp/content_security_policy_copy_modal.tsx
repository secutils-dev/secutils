import {
  EuiButtonEmpty,
  EuiCallOut,
  EuiCodeBlock,
  EuiForm,
  EuiFormRow,
  EuiLink,
  EuiModal,
  EuiModalBody,
  EuiModalFooter,
  EuiModalHeader,
  EuiModalHeaderTitle,
  EuiSelect,
  EuiTitle,
} from '@elastic/eui';
import axios from 'axios';
import type { ChangeEvent } from 'react';
import { useCallback, useEffect, useState } from 'react';

import type { ContentSecurityPolicy } from './content_security_policy';
import type { AsyncData } from '../../../../../model';
import { getApiRequestConfig, getApiUrl, getErrorMessage } from '../../../../../model';
import { useWorkspaceContext } from '../../../hooks';

export interface ContentSecurityPolicyCopyModalProps {
  policy: ContentSecurityPolicy;
  onClose: () => void;
}

export function ContentSecurityPolicyCopyModal({ policy, onClose }: ContentSecurityPolicyCopyModalProps) {
  const { uiState } = useWorkspaceContext();

  const [source, setSource] = useState<string>('enforcingHeader');
  const onSourceChange = useCallback((e: ChangeEvent<HTMLSelectElement>) => {
    setSource(e.target.value);
    onSerializePolicy(e.target.value);
  }, []);

  const [snippet, setSnippet] = useState<string>('');

  const [serializingStatus, setSerializingStatus] = useState<AsyncData<undefined> | null>(null);
  const onSerializePolicy = useCallback(
    (currentSource?: string) => {
      if (serializingStatus?.status === 'pending') {
        return;
      }

      setSerializingStatus({ status: 'pending' });

      const sourceToUse = currentSource ?? source;
      axios
        .post<string>(
          getApiUrl(`/api/utils/web_security/csp/${encodeURIComponent(policy.id)}/serialize`),
          { source: sourceToUse },
          getApiRequestConfig(),
        )
        .then(
          (res) => {
            if (sourceToUse === 'meta') {
              setSnippet(`<meta http-equiv="Content-Security-Policy" content="${res.data}">`);
            } else {
              const endpointGroup = policy.directives.get('report-to')?.[0];
              const reportToHeader = endpointGroup
                ? `## Define reporting endpoints
Reporting-Endpoints: default="https://secutils.dev/csp_reports/default

`
                : '';

              setSnippet(
                `${reportToHeader}## Policy header
${sourceToUse === 'enforcingHeader' ? 'Content-Security-Policy' : 'Content-Security-Policy-Report-Only'}: ${res.data}`,
              );
            }

            setSerializingStatus({ status: 'succeeded', data: undefined });
          },
          (err: Error) => {
            setSerializingStatus({ status: 'failed', error: getErrorMessage(err) });
          },
        );
    },
    [source, policy, serializingStatus],
  );

  useEffect(() => {
    if (!uiState.synced) {
      return;
    }

    onSerializePolicy();
  }, [uiState]);

  const copyStatusCallout =
    serializingStatus?.status === 'failed' ? (
      <EuiFormRow>
        <EuiCallOut
          size="s"
          title={serializingStatus.error ?? 'An error occurred, please try again later'}
          color="danger"
          iconType="warning"
        />
      </EuiFormRow>
    ) : undefined;

  return (
    <EuiModal onClose={onClose} maxWidth={450}>
      <EuiModalHeader>
        <EuiModalHeaderTitle>
          <EuiTitle size={'s'}>
            <span>Copy policy</span>
          </EuiTitle>
        </EuiModalHeaderTitle>
      </EuiModalHeader>
      <EuiModalBody>
        <EuiForm id="copy-form" component="form">
          {copyStatusCallout}
          <EuiFormRow
            fullWidth
            label="Policy source"
            helpText={
              <span>
                Defines how the policy should be{' '}
                <EuiLink target="_blank" href="https://www.w3.org/TR/CSP3/#policy-delivery">
                  delivered
                </EuiLink>
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
              value={source}
              onChange={onSourceChange}
            />
          </EuiFormRow>
          <EuiFormRow label="Snippet" fullWidth>
            <EuiCodeBlock language={source === 'meta' ? 'html' : 'http'} fontSize="m" paddingSize="m" isCopyable>
              {snippet}
            </EuiCodeBlock>
          </EuiFormRow>
        </EuiForm>
      </EuiModalBody>
      <EuiModalFooter>
        <EuiButtonEmpty onClick={onClose}>Close</EuiButtonEmpty>
      </EuiModalFooter>
    </EuiModal>
  );
}
