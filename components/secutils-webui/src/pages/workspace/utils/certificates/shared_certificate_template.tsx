import { EuiButton, EuiFlexGroup, EuiFlexItem } from '@elastic/eui';
import axios from 'axios';
import { useEffect, useState } from 'react';

import type { CertificateTemplate } from './certificate_template';
import { CertificateTemplateForm } from './certificate_template_form';
import { CertificateTemplateGenerateModal } from './certificate_template_generate_modal';
import { PageErrorState, PageLoadingState } from '../../../../components';
import type { AsyncData } from '../../../../model';
import { getApiRequestConfig, getApiUrl, getErrorMessage } from '../../../../model';
import { useWorkspaceContext } from '../../hooks';

type GetTemplateResponse = { template?: CertificateTemplate };

export default function SharedCertificateTemplate() {
  const { uiState, setTitle, setTitleActions } = useWorkspaceContext();

  const [templateToGenerate, setTemplateToGenerate] = useState<CertificateTemplate | null>(null);
  const [template, setTemplate] = useState<AsyncData<CertificateTemplate>>({ status: 'pending' });

  // Wait for user share status to be synced before trying to load template.
  useEffect(() => {
    if (!uiState.synced) {
      setTitle(`Loading certificate template…`);
      return;
    }

    if (!uiState.userShare || uiState.userShare.resource.type !== 'certificateTemplate') {
      setTemplate({ status: 'failed', error: 'Failed to load shared certificate template.' });
      return;
    }

    axios
      .get<GetTemplateResponse>(
        getApiUrl(`/api/utils/certificates/templates/${encodeURIComponent(uiState.userShare.resource.templateId)}`),
        getApiRequestConfig(),
      )
      .then(
        (res) => {
          const loadedTemplate = res.data.template ?? null;
          if (loadedTemplate) {
            setTitle(`"${loadedTemplate.name}" certificate template`);
            setTitleActions(
              <EuiButton
                fill
                iconType={'download'}
                title="Generate private key and certificate"
                onClick={() => setTemplateToGenerate(loadedTemplate)}
              >
                Generate certificate
              </EuiButton>,
            );
            setTemplate({ status: 'succeeded', data: loadedTemplate });
          } else {
            setTemplate({ status: 'failed', error: 'Failed to load shared certificate template.' });
          }
        },
        (err: Error) => {
          setTemplate({ status: 'failed', error: getErrorMessage(err) });
        },
      );
  }, [uiState]);

  if (template.status === 'pending') {
    return <PageLoadingState />;
  }

  if (template.status === 'failed') {
    return (
      <PageErrorState
        title="Cannot load shared certificate template"
        content={
          <p>
            Cannot load shared certificate template
            <br />
            <br />
            <strong>{template.error}</strong>.
          </p>
        }
      />
    );
  }

  const generateModal = templateToGenerate ? (
    <CertificateTemplateGenerateModal onClose={() => setTemplateToGenerate(null)} template={templateToGenerate} />
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
          <CertificateTemplateForm template={template.data} isReadOnly />
        </EuiFlexItem>
      </EuiFlexGroup>
      {generateModal}
    </>
  );
}
