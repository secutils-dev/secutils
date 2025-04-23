import axios from 'axios';
import { useState } from 'react';

import type { ContentSecurityPolicy } from './content_security_policy';
import { serializeContentSecurityPolicyDirectives } from './content_security_policy';
import { ContentSecurityPolicyForm } from './content_security_policy_form';
import type { AsyncData } from '../../../../../model';
import { getApiRequestConfig, getApiUrl, getErrorMessage, isClientError } from '../../../../../model';
import { EditorFlyout } from '../../../components/editor_flyout';
import { useWorkspaceContext } from '../../../hooks';

export interface Props {
  onClose: (success?: boolean) => void;
  policy?: ContentSecurityPolicy;
}

export function SaveContentSecurityPolicyFlyout({ onClose, policy }: Props) {
  const { addToast } = useWorkspaceContext();

  const [policyToSave, setPolicyToSave] = useState<ContentSecurityPolicy>(
    policy ?? { id: '', createdAt: 0, updatedAt: 0, name: '', directives: new Map() },
  );

  const [updatingStatus, setUpdatingStatus] = useState<AsyncData<void>>();

  return (
    <EditorFlyout
      title={`${policy ? 'Edit' : 'Add'} policy`}
      onClose={() => onClose()}
      onSave={() => {
        if (updatingStatus?.status === 'pending' || !policyToSave) {
          return;
        }

        setUpdatingStatus({ status: 'pending' });

        const [requestPromise, successMessage, errorMessage] = policyToSave.id
          ? [
              axios.put(
                getApiUrl(`/api/utils/web_security/csp/${policyToSave.id}`),
                {
                  name: policyToSave.name !== policy?.name ? policyToSave.name : null,
                  directives: serializeContentSecurityPolicyDirectives(policyToSave.directives),
                },
                getApiRequestConfig(),
              ),
              `Successfully updated "${policyToSave.name}" policy`,
              `Unable to update "${policyToSave.name}" policy, please try again later`,
            ]
          : [
              axios.post(
                getApiUrl('/api/utils/web_security/csp'),
                {
                  name: policyToSave.name,
                  content: {
                    type: 'directives',
                    value: serializeContentSecurityPolicyDirectives(policyToSave.directives),
                  },
                },
                getApiRequestConfig(),
              ),
              `Successfully saved "${policyToSave.name}" policy`,
              `Unable to save "${policyToSave.name}" policy, please try again later`,
            ];
        requestPromise.then(
          () => {
            setUpdatingStatus({ status: 'succeeded', data: undefined });

            addToast({
              id: `success-save-policy-${policyToSave.name}`,
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
              id: `failed-save-policy-${policyToSave.name}`,
              iconType: 'warning',
              color: 'danger',
              title: isClientError(err) ? remoteErrorMessage : errorMessage,
            });
          },
        );
      }}
      canSave={policyToSave.name.length > 0 && policyToSave.directives?.size > 0}
      saveInProgress={updatingStatus?.status === 'pending'}
    >
      <ContentSecurityPolicyForm policy={policyToSave} onChange={setPolicyToSave} />
    </EditorFlyout>
  );
}
