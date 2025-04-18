import { useCallback, useEffect, useState } from 'react';

import type { Criteria, Pagination, PropertySort } from '@elastic/eui';
import {
  EuiButton,
  EuiButtonEmpty,
  EuiCallOut,
  EuiConfirmModal,
  EuiEmptyPrompt,
  EuiFlexGroup,
  EuiFlexItem,
  EuiIcon,
  EuiInMemoryTable,
  EuiLink,
  EuiSpacer,
  EuiToolTip,
} from '@elastic/eui';
import axios from 'axios';
import { unix } from 'moment';

import { certificateTypeString, getDistinguishedNameString, signatureAlgorithmString } from './certificate_attributes';
import type { CertificateTemplate } from './certificate_template';
import { CertificateTemplateGenerateModal } from './certificate_template_generate_modal';
import { CertificateTemplateShareModal } from './certificate_template_share_modal';
import { SELF_SIGNED_PROD_WARNING_USER_SETTINGS_KEY } from './consts';
import { privateKeyAlgString } from './private_key_alg';
import { SaveCertificateTemplateFlyout } from './save_certificate_template_flyout';
import { PageErrorState, PageLoadingState } from '../../../../components';
import { type AsyncData, getApiRequestConfig, getApiUrl, getErrorMessage } from '../../../../model';
import { useWorkspaceContext } from '../../hooks';

export default function CertificatesCertificateTemplates() {
  const { uiState, settings, setSettings, setTitleActions } = useWorkspaceContext();

  const [templates, setTemplates] = useState<AsyncData<CertificateTemplate[]>>({ status: 'pending' });

  const [templateToGenerate, setTemplateToGenerate] = useState<CertificateTemplate | null>(null);
  const [templateToShare, setTemplateToShare] = useState<CertificateTemplate | null>(null);
  const [templateToEdit, setTemplateToEdit] = useState<CertificateTemplate | null | undefined>(null);
  const [templateToRemove, setTemplateToRemove] = useState<CertificateTemplate | null>(null);

  const loadCertificateTemplates = () => {
    axios.get<CertificateTemplate[]>(getApiUrl('/api/utils/certificates/templates'), getApiRequestConfig()).then(
      (res) => {
        setTemplates({ status: 'succeeded', data: res.data });
        setTitleActions(res.data.length === 0 ? null : createButton);
      },
      (err: Error) => {
        setTemplates({ status: 'failed', error: getErrorMessage(err) });
      },
    );
  };

  useEffect(() => {
    if (!uiState.synced) {
      return;
    }

    loadCertificateTemplates();
  }, [uiState]);

  const createButton = (
    <EuiButton
      iconType={'plusInCircle'}
      title="Create a new certificate template"
      fill
      onClick={() => setTemplateToEdit(undefined)}
    >
      Create certificate template
    </EuiButton>
  );

  const docsButton = (
    <EuiButtonEmpty
      iconType={'documentation'}
      title="Learn how to create and use certificate templates"
      target={'_blank'}
      href={'/docs/guides/digital_certificates/certificate_templates'}
    >
      Learn how to
    </EuiButtonEmpty>
  );

  const editFlyout =
    templateToEdit !== null ? (
      <SaveCertificateTemplateFlyout
        onClose={(success) => {
          if (success) {
            loadCertificateTemplates();
          }
          setTemplateToEdit(null);
        }}
        template={templateToEdit}
      />
    ) : null;

  const generateModal = templateToGenerate ? (
    <CertificateTemplateGenerateModal onClose={() => setTemplateToGenerate(null)} template={templateToGenerate} />
  ) : null;

  const shareModal = templateToShare ? (
    <CertificateTemplateShareModal onClose={() => setTemplateToShare(null)} template={templateToShare} />
  ) : null;

  const removeConfirmModal = templateToRemove ? (
    <EuiConfirmModal
      title={`Remove "${templateToRemove.name}"?`}
      onCancel={() => setTemplateToRemove(null)}
      onConfirm={() => {
        setTemplateToRemove(null);
        axios
          .delete(
            getApiUrl(`/api/utils/certificates/templates/${encodeURIComponent(templateToRemove.id)}`),
            getApiRequestConfig(),
          )
          .then(
            () => loadCertificateTemplates(),
            (err: Error) => {
              console.error(`Failed to remove certificate template: ${getErrorMessage(err)}`);
            },
          );
      }}
      cancelButtonText="Cancel"
      confirmButtonText="Remove"
      buttonColor="danger"
    >
      The certificate template will be removed. Are you sure you want to proceed?
    </EuiConfirmModal>
  ) : null;

  const [pagination, setPagination] = useState<Pagination>({
    pageIndex: 0,
    pageSize: 15,
    pageSizeOptions: [10, 15, 25, 50, 100],
    totalItemCount: 0,
  });
  const [sorting, setSorting] = useState<{ sort: PropertySort }>({ sort: { field: 'name', direction: 'asc' } });
  const onTableChange = useCallback(
    ({ page, sort }: Criteria<CertificateTemplate>) => {
      setPagination({
        ...pagination,
        pageIndex: page?.index ?? 0,
        pageSize: page?.size ?? 15,
      });

      if (sort?.field) {
        setSorting({ sort });
      }
    },
    [pagination],
  );

  if (templates.status === 'pending') {
    return <PageLoadingState />;
  }

  if (templates.status === 'failed') {
    return (
      <PageErrorState
        title="Cannot load certificate templates"
        content={
          <p>
            Cannot load certificate templates.
            <br />
            <br />
            <strong>{templates.error}</strong>.
          </p>
        }
      />
    );
  }

  let content;
  if (templates.data.length === 0) {
    content = (
      <EuiFlexGroup
        direction={'column'}
        gutterSize={'s'}
        justifyContent="center"
        alignItems="center"
        style={{ height: '100%' }}
      >
        <EuiFlexItem>
          <EuiEmptyPrompt
            icon={<EuiIcon type={'securityApp'} size={'xl'} />}
            title={<h2>You don&apos;t have any certificate templates yet</h2>}
            titleSize="s"
            style={{ maxWidth: '60em', display: 'flex' }}
            body={
              <div>
                <p>Go ahead and create your first certificate template.</p>
                {createButton}
                <EuiSpacer size={'s'} />
                {docsButton}
              </div>
            }
          />
        </EuiFlexItem>
      </EuiFlexGroup>
    );
  } else {
    const selfSignedCertificatesProdWarning =
      settings?.[SELF_SIGNED_PROD_WARNING_USER_SETTINGS_KEY] === true ? null : (
        <div>
          <EuiCallOut
            title="Don't use self-signed certificates in production environments"
            color="warning"
            iconType="warning"
          >
            <p>
              Self-signed certificates generated through Secutils.dev are intended for use in development and testing
              environments only. Please do not use these certificates in production environments unless you are running{' '}
              <EuiLink target="_blank" href="https://github.com/secutils-dev/secutils">
                your own version
              </EuiLink>{' '}
              of the Secutils.dev in a trusted and controlled environment.
            </p>
            <EuiButton
              color="accent"
              onClick={() => setSettings({ [SELF_SIGNED_PROD_WARNING_USER_SETTINGS_KEY]: true })}
            >
              Do not show again
            </EuiButton>
          </EuiCallOut>{' '}
          <EuiSpacer />
        </div>
      );

    content = (
      <>
        {selfSignedCertificatesProdWarning}
        <EuiInMemoryTable
          pagination={pagination}
          allowNeutralSort={false}
          sorting={sorting}
          onTableChange={onTableChange}
          items={templates.data}
          itemId={(template) => template.id}
          tableLayout={'auto'}
          columns={[
            {
              name: (
                <EuiToolTip content="A unique name of the certificate template">
                  <span>
                    Name <EuiIcon size="s" color="subdued" type="questionInCircle" className="eui-alignTop" />
                  </span>
                </EuiToolTip>
              ),
              field: 'name',
              textOnly: true,
              sortable: true,
              render: (_, template) => template.name,
            },
            {
              name: (
                <EuiToolTip content="Specifies whether the certificate can be used to sign other certificates (Certification Authority) or not.">
                  <span>
                    Type <EuiIcon size="s" color="subdued" type="questionInCircle" className="eui-alignTop" />
                  </span>
                </EuiToolTip>
              ),
              field: 'isCa',
              textOnly: true,
              sortable: true,
              render: (_, template) => certificateTypeString(template.attributes),
            },
            {
              name: 'Distinguished name (DN)',
              field: 'commonName',
              render: (_, template) => getDistinguishedNameString(template.attributes),
            },
            {
              name: 'Not valid before',
              field: 'notValidBefore',
              sortable: true,
              render: (_, template) => unix(template.attributes.notValidBefore).format('ll HH:mm'),
            },
            {
              name: 'Not valid after',
              field: 'notValidAfter',
              sortable: true,
              render: (_, template) => unix(template.attributes.notValidAfter).format('ll HH:mm'),
            },
            {
              name: 'Key algorithm',
              field: 'keyAlgorithm',
              mobileOptions: { only: true },
              render: (_, template) => privateKeyAlgString(template.attributes.keyAlgorithm),
            },
            {
              name: 'Signature algorithm',
              field: 'signatureAlgorithm',
              mobileOptions: { only: true },
              render: (_, template) => signatureAlgorithmString(template.attributes),
            },
            {
              name: 'Actions',
              field: 'headers',
              width: '105px',
              actions: [
                {
                  name: 'Generate',
                  description: 'Generate',
                  icon: 'download',
                  type: 'icon',
                  isPrimary: true,
                  onClick: setTemplateToGenerate,
                },
                {
                  name: 'Share template',
                  description: 'Share template',
                  icon: 'share',
                  type: 'icon',
                  onClick: setTemplateToShare,
                },
                {
                  name: 'Edit template',
                  description: 'Edit template',
                  icon: 'pencil',
                  type: 'icon',
                  isPrimary: true,
                  onClick: setTemplateToEdit,
                },
                {
                  name: 'Remove template',
                  description: 'Remove template',
                  icon: 'minusInCircle',
                  type: 'icon',
                  onClick: setTemplateToRemove,
                },
              ],
            },
          ]}
        />
      </>
    );
  }

  return (
    <>
      {content}
      {editFlyout}
      {generateModal}
      {shareModal}
      {removeConfirmModal}
    </>
  );
}
