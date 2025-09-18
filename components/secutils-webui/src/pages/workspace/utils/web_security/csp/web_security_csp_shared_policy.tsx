import { EuiButton, EuiFlexGroup, EuiFlexItem } from '@elastic/eui';
import { useCallback, useEffect, useState } from 'react';

import type { ContentSecurityPolicy, SerializedContentSecurityPolicyDirectives } from './content_security_policy';
import { deserializeContentSecurityPolicyDirectives } from './content_security_policy';
import { ContentSecurityPolicyCopyModal } from './content_security_policy_copy_modal';
import { ContentSecurityPolicyForm } from './content_security_policy_form';
import { PageErrorState, PageLoadingState } from '../../../../../components';
import { type AsyncData, getApiRequestConfig, getApiUrl, getErrorMessage, ResponseError } from '../../../../../model';
import { useWorkspaceContext } from '../../../hooks';

type GetContentSecurityPolicyResponse = { policy?: ContentSecurityPolicy<SerializedContentSecurityPolicyDirectives> };

export default function WebSecuritySharedContentSecurityPolicy() {
  const { uiState, setTitle, setTitleActions } = useWorkspaceContext();

  const [policyToCopy, setPolicyToCopy] = useState<ContentSecurityPolicy | null>(null);
  const onToggleCopyModal = useCallback((policy?: ContentSecurityPolicy) => {
    setPolicyToCopy(policy ?? null);
  }, []);

  const [policy, setPolicy] = useState<AsyncData<ContentSecurityPolicy>>({ status: 'pending' });

  // Wait for user share status to be synced before trying to load policy.
  useEffect(() => {
    if (!uiState.synced) {
      setTitle(`Loading content security policy…`);
      return;
    }

    if (!uiState.userShare || uiState.userShare.resource.type !== 'contentSecurityPolicy') {
      setPolicy({ status: 'failed', error: 'Failed to load shared content security policy.' });
      return;
    }

    fetch(
      getApiUrl(`/api/utils/web_security/csp/${encodeURIComponent(uiState.userShare.resource.policyId)}`),
      getApiRequestConfig(),
    )
      .then(async (res) => {
        if (!res.ok) {
          throw await ResponseError.fromResponse(res);
        }

        const getPolicyResult = (await res.json()) as GetContentSecurityPolicyResponse;
        const loadedPolicy = getPolicyResult.policy
          ? {
              ...getPolicyResult.policy,
              directives: deserializeContentSecurityPolicyDirectives(getPolicyResult.policy.directives),
            }
          : null;
        if (loadedPolicy) {
          setTitle(`"${loadedPolicy.name}" content security policy`);
          setTitleActions(
            <EuiButton fill iconType={'copy'} title="Copy policy" onClick={() => setPolicyToCopy(loadedPolicy)}>
              Copy policy
            </EuiButton>,
          );
          setPolicy({ status: 'succeeded', data: loadedPolicy });
        } else {
          setPolicy({ status: 'failed', error: 'Failed to load shared content security policy.' });
        }
      })
      .catch((err: Error) => setPolicy({ status: 'failed', error: getErrorMessage(err) }));
  }, [uiState, setTitle, setTitleActions]);

  if (policy.status === 'pending') {
    return <PageLoadingState />;
  }

  if (policy.status === 'failed') {
    return (
      <PageErrorState
        title="Cannot load shared content security policy"
        content={
          <p>
            Cannot load shared content security policy
            <br />
            <br />
            <strong>{policy.error}</strong>.
          </p>
        }
      />
    );
  }

  const copyModal = policyToCopy ? (
    <ContentSecurityPolicyCopyModal onClose={() => onToggleCopyModal()} policy={policyToCopy} />
  ) : null;
  return (
    <>
      <EuiFlexGroup
        direction={'column'}
        gutterSize={'s'}
        justifyContent="center"
        alignItems="center"
        style={{ height: '100%' }}
      >
        <EuiFlexItem>
          <ContentSecurityPolicyForm policy={policy.data} isReadOnly />
        </EuiFlexItem>
      </EuiFlexGroup>
      {copyModal}
    </>
  );
}
