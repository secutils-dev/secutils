import { useState } from 'react';

import type { ContentSecurityPolicy } from './content_security_policy';
import { serializeContentSecurityPolicyDirectives } from './content_security_policy';
import type { ContentSecurityPolicyProps } from './content_security_policy_form';
import { ContentSecurityPolicyForm } from './content_security_policy_form';
import {
  type AsyncData,
  getApiRequestConfig,
  getApiUrl,
  getErrorMessage,
  isClientError,
  ResponseError,
} from '../../../../../model';
import { EditorFlyout } from '../../../components/editor_flyout';
import { useWorkspaceContext } from '../../../hooks';

export interface ContentSecurityPolicyEditFlyoutProps {
  policy?: Partial<ContentSecurityPolicy>;
  onClose: (success?: boolean) => void;
}

export function ContentSecurityPolicyEditFlyout({ onClose, policy }: ContentSecurityPolicyEditFlyoutProps) {
  const { addToast } = useWorkspaceContext();

  const [policyToSave, setPolicyToSave] = useState<ContentSecurityPolicyProps>({
    name: policy?.name ?? '',
    directives: policy?.directives ?? new Map(),
  });

  const [updatingStatus, setUpdatingStatus] = useState<AsyncData<void>>();

  return (
    <EditorFlyout
      title={`${policy?.id ? 'Edit' : 'Add'} policy`}
      onClose={() => onClose()}
      onSave={() => {
        if (updatingStatus?.status === 'pending') {
          return;
        }

        setUpdatingStatus({ status: 'pending' });

        const [requestPromise, successMessage, errorMessage] = policy?.id
          ? [
              fetch(getApiUrl(`/api/utils/web_security/csp/${policy.id}`), {
                ...getApiRequestConfig('PUT'),
                body: JSON.stringify({
                  name: policyToSave.name !== policy?.name ? policyToSave.name : null,
                  directives: serializeContentSecurityPolicyDirectives(policyToSave.directives),
                }),
              }),
              `Successfully updated "${policyToSave.name}" policy`,
              `Unable to update "${policyToSave.name}" policy, please try again later`,
            ]
          : [
              fetch(getApiUrl('/api/utils/web_security/csp'), {
                ...getApiRequestConfig('POST'),
                body: JSON.stringify({
                  name: policyToSave.name,
                  content: {
                    type: 'directives',
                    value: serializeContentSecurityPolicyDirectives(policyToSave.directives),
                  },
                }),
              }),
              `Successfully saved "${policyToSave.name}" policy`,
              `Unable to save "${policyToSave.name}" policy, please try again later`,
            ];
        requestPromise
          .then(async (res) => {
            if (!res.ok) {
              throw await ResponseError.fromResponse(res);
            }

            setUpdatingStatus({ status: 'succeeded', data: undefined });

            addToast({
              id: `success-save-policy-${policyToSave.name}`,
              iconType: 'check',
              color: 'success',
              title: successMessage,
            });

            onClose(true);
          })
          .catch((err: Error) => {
            const remoteErrorMessage = getErrorMessage(err);
            setUpdatingStatus({ status: 'failed', error: remoteErrorMessage });

            addToast({
              id: `failed-save-policy-${policyToSave.name}`,
              iconType: 'warning',
              color: 'danger',
              title: isClientError(err) ? remoteErrorMessage : errorMessage,
            });
          });
      }}
      canSave={policyToSave.name.length > 0 && policyToSave.directives?.size > 0}
      saveInProgress={updatingStatus?.status === 'pending'}
    >
      <ContentSecurityPolicyForm policy={policyToSave} onChange={setPolicyToSave} />
    </EditorFlyout>
  );
}
