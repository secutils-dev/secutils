import moment from 'moment';
import { useState } from 'react';

import type { CertificateTemplate } from './certificate_template';
import type { CertificateTemplateProps } from './certificate_template_form';
import { CertificateTemplateForm } from './certificate_template_form';
import {
  type AsyncData,
  getApiRequestConfig,
  getApiUrl,
  getErrorMessage,
  isClientError,
  ResponseError,
} from '../../../../model';
import { EditorFlyout } from '../../components/editor_flyout';
import { useWorkspaceContext } from '../../hooks';

export interface CertificateTemplateEditFlyoutProps {
  template?: Partial<CertificateTemplate>;
  onClose: (success?: boolean) => void;
}

export function CertificateTemplateEditFlyout({ onClose, template }: CertificateTemplateEditFlyoutProps) {
  const { addToast } = useWorkspaceContext();

  const [templateToSave, setTemplateToSave] = useState<CertificateTemplateProps>({
    name: template?.name ?? '',
    attributes: template?.attributes ?? {
      commonName: 'CA Issuer',
      country: 'US',
      stateOrProvince: 'California',
      locality: 'San Francisco',
      organization: 'CA Issuer, Inc',
      keyAlgorithm: { keyType: 'ed25519' },
      signatureAlgorithm: 'ed25519',
      notValidBefore: moment().unix(),
      notValidAfter: moment().add(1, 'years').unix(),
      isCa: false,
    },
  });

  const [updatingStatus, setUpdatingStatus] = useState<AsyncData<void>>();
  return (
    <EditorFlyout
      title={`${template?.id ? 'Edit' : 'Add'} certificate template`}
      onClose={() => onClose()}
      onSave={() => {
        if (updatingStatus?.status === 'pending') {
          return;
        }

        setUpdatingStatus({ status: 'pending' });

        const [requestPromise, successMessage, errorMessage] = template?.id
          ? [
              fetch(getApiUrl(`/api/utils/certificates/templates/${template.id}`), {
                ...getApiRequestConfig('PUT'),
                body: JSON.stringify({
                  templateName: templateToSave.name !== template?.name ? templateToSave.name : null,
                  attributes: templateToSave.attributes,
                }),
              }),
              `Successfully updated "${templateToSave.name}" certificate template`,
              `Unable to update "${templateToSave.name}" certificate template, please try again later`,
            ]
          : [
              fetch(getApiUrl('/api/utils/certificates/templates'), {
                ...getApiRequestConfig('POST'),
                body: JSON.stringify({ templateName: templateToSave.name, attributes: templateToSave.attributes }),
              }),
              `Successfully saved "${templateToSave.name}" certificate template`,
              `Unable to save "${templateToSave.name}" certificate template, please try again later`,
            ];
        requestPromise
          .then(async (res) => {
            if (!res.ok) {
              throw await ResponseError.fromResponse(res);
            }
            setUpdatingStatus({ status: 'succeeded', data: undefined });

            addToast({
              id: `success-save-certificate-template-${templateToSave.name}`,
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
              id: `failed-save-certificate-template-${templateToSave.name}`,
              iconType: 'warning',
              color: 'danger',
              title: isClientError(err) ? remoteErrorMessage : errorMessage,
            });
          });
      }}
      canSave={templateToSave.name.length > 0}
      saveInProgress={updatingStatus?.status === 'pending'}
    >
      <CertificateTemplateForm template={templateToSave} onChange={setTemplateToSave} />
    </EditorFlyout>
  );
}
