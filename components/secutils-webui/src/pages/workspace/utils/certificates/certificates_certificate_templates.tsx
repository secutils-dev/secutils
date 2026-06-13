import {
  EuiBasicTable,
  EuiButton,
  EuiButtonEmpty,
  EuiCallOut,
  EuiConfirmModal,
  EuiEmptyPrompt,
  EuiFlexGroup,
  EuiFlexItem,
  EuiIcon,
  EuiLink,
  EuiSpacer,
  EuiToolTip,
} from '@elastic/eui';
import { unix } from 'moment';
import { useCallback, useEffect, useMemo, useState } from 'react';

import { UTIL_HANDLES } from '..';
import { certificateTypeString, getDistinguishedNameString, signatureAlgorithmString } from './certificate_attributes';
import type { CertificateTemplate } from './certificate_template';
import { CertificateTemplateEditFlyout } from './certificate_template_edit_flyout';
import { CertificateTemplateGenerateModal } from './certificate_template_generate_modal';
import { CertificateTemplateImportModal } from './certificate_template_import_modal';
import { CertificateTemplateShareModal } from './certificate_template_share_modal';
import { SELF_SIGNED_PROD_WARNING_USER_SETTINGS_KEY } from './consts';
import { privateKeyAlgString } from './private_key_alg';
import { PageErrorState, PageLoadingState } from '../../../../components';
import { useUserTags } from '../../../../hooks';
import type { Page, PaginationRequest } from '../../../../model';
import { apiFetch, buildPaginationQuery, getCopyName, ResponseError } from '../../../../model';
import { EntityName } from '../../components/entity_name';
import {
  FilteredEmptyState,
  ItemsTableFilter,
  TagsFilter,
  useServerPaginatedItems,
} from '../../components/items_table_filter';
import { useWorkspaceContext } from '../../hooks';
import { getWorkspaceEntityAbsoluteLink, getWorkspaceEntityLink } from '../workspace_links';

export default function CertificatesCertificateTemplates() {
  const { settings, setSettings, setTitleActions } = useWorkspaceContext();

  const [initialized, setInitialized] = useState(false);

  const [templateToGenerate, setTemplateToGenerate] = useState<CertificateTemplate | null>(null);
  const [templateToShare, setTemplateToShare] = useState<CertificateTemplate | null>(null);
  const [templateToEdit, setTemplateToEdit] = useState<Partial<CertificateTemplate> | null | undefined>(null);
  const [templateToRemove, setTemplateToRemove] = useState<CertificateTemplate | null>(null);
  const [showImportModal, setShowImportModal] = useState(false);
  const { allTags } = useUserTags();

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

  const importButton = useMemo(
    () => (
      <EuiButton iconType={'importAction'} title="Import certificate template" onClick={() => setShowImportModal(true)}>
        Import template
      </EuiButton>
    ),
    [],
  );

  const createButton = useMemo(
    () => (
      <EuiButton
        iconType={'plusInCircle'}
        title="Create a new certificate template"
        fill
        onClick={() => setTemplateToEdit(undefined)}
      >
        Create template
      </EuiButton>
    ),
    [],
  );

  const titleActions = useMemo(
    () => (
      <EuiFlexGroup gutterSize="s" responsive={false} alignItems="center" justifyContent="center">
        <EuiFlexItem grow={false}>{importButton}</EuiFlexItem>
        <EuiFlexItem grow={false}>{createButton}</EuiFlexItem>
      </EuiFlexGroup>
    ),
    [importButton, createButton],
  );

  const fetcher = useCallback(async (request: PaginationRequest): Promise<Page<CertificateTemplate>> => {
    const res = await apiFetch(`/api/certificates/templates${buildPaginationQuery(request)}`);
    if (!res.ok) {
      throw await ResponseError.fromResponse(res);
    }
    return (await res.json()) as Page<CertificateTemplate>;
  }, []);

  const {
    items: templates,
    total,
    loading,
    error,
    pagination,
    sorting,
    onTableChange,
    query,
    setQuery,
    selectedTagIds,
    setSelectedTagIds,
    hasPageFilters,
    hasActiveFilters,
    clearPageFilters,
    refresh,
  } = useServerPaginatedItems<CertificateTemplate>({
    fetcher,
    allTags,
    defaultSortField: 'updatedAt',
    defaultSortDirection: 'desc',
  });

  useEffect(() => {
    if (!loading) {
      setInitialized(true);
    }
  }, [loading]);

  const isEmpty = initialized && total === 0 && !hasActiveFilters;

  useEffect(() => {
    setTitleActions(isEmpty ? null : titleActions);
  }, [isEmpty, titleActions, setTitleActions]);

  const editFlyout =
    templateToEdit !== null ? (
      <CertificateTemplateEditFlyout
        onClose={(success) => {
          if (success) {
            refresh();
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
        apiFetch(`/api/certificates/templates/${encodeURIComponent(templateToRemove.id)}`, { method: 'DELETE' })
          .then(async (res) => {
            if (!res.ok) {
              throw await ResponseError.fromResponse(res);
            }
            refresh();
          })
          .catch((err: Error) => {
            console.error(`Failed to remove certificate template: ${err.message}`);
          });
      }}
      cancelButtonText="Cancel"
      confirmButtonText="Remove"
      buttonColor="danger"
    >
      The certificate template will be removed. Are you sure you want to proceed?
    </EuiConfirmModal>
  ) : null;

  if (!initialized && loading) {
    return <PageLoadingState />;
  }

  if (error && templates.length === 0) {
    return <PageErrorState title="Cannot load certificate templates" content={<p>{error}</p>} />;
  }

  let content;
  if (isEmpty) {
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
                <p>Go ahead and create your first certificate template or import one from an existing certificate.</p>
                <EuiFlexGroup gutterSize="s" justifyContent="center" responsive={false}>
                  <EuiFlexItem grow={false}>{importButton}</EuiFlexItem>
                  <EuiFlexItem grow={false}>{createButton}</EuiFlexItem>
                </EuiFlexGroup>
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
        <ItemsTableFilter
          query={query}
          onQueryChange={setQuery}
          onRefresh={refresh}
          placeholder="Search by name or ID..."
        >
          <TagsFilter tags={allTags} selectedTagIds={selectedTagIds} onSelectedTagIdsChange={setSelectedTagIds} />
        </ItemsTableFilter>
        <EuiSpacer size="m" />
        <EuiBasicTable
          loading={loading}
          pagination={pagination}
          noItemsMessage={
            <FilteredEmptyState totalItems={total} hasPageFilters={hasPageFilters} onClearFilters={clearPageFilters} />
          }
          sorting={sorting}
          onChange={onTableChange}
          items={templates}
          itemId={(template) => template.id}
          tableLayout={'auto'}
          columns={[
            {
              name: (
                <EuiToolTip content="A unique name of the certificate template">
                  <span>
                    Name <EuiIcon size="s" color="subdued" type="question" className="eui-alignTop" />
                  </span>
                </EuiToolTip>
              ),
              field: 'name',
              sortable: true,
              render: (_, template) => (
                <EntityName
                  name={template.name}
                  href={getWorkspaceEntityLink(UTIL_HANDLES.certificatesCertificateTemplates, template.id)}
                  tags={template.tags}
                />
              ),
            },
            {
              name: (
                <EuiToolTip content="Specifies whether the certificate can be used to sign other certificates (Certification Authority) or not.">
                  <span>
                    Type <EuiIcon size="s" color="subdued" type="question" className="eui-alignTop" />
                  </span>
                </EuiToolTip>
              ),
              field: 'isCa',
              textOnly: true,
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
              render: (_, template) => unix(template.attributes.notValidBefore).format('ll HH:mm'),
            },
            {
              name: 'Not valid after',
              field: 'notValidAfter',
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
                  name: 'Copy ID',
                  description: 'Copy ID to clipboard',
                  icon: 'tokenKey',
                  type: 'icon',
                  onClick: ({ id }: CertificateTemplate) => void navigator.clipboard.writeText(id),
                },
                {
                  name: 'Copy link',
                  description: 'Copy link to template in grid',
                  icon: 'link',
                  type: 'icon',
                  onClick: ({ id }: CertificateTemplate) =>
                    void navigator.clipboard.writeText(
                      getWorkspaceEntityAbsoluteLink(UTIL_HANDLES.certificatesCertificateTemplates, id),
                    ),
                },
                {
                  name: 'Generate',
                  description: 'Generate',
                  icon: 'download',
                  type: 'icon',
                  onClick: setTemplateToGenerate,
                },
                {
                  name: 'Share',
                  description: 'Share template',
                  icon: 'share',
                  type: 'icon',
                  onClick: setTemplateToShare,
                },
                {
                  name: 'Edit',
                  description: 'Edit template',
                  icon: 'pencil',
                  type: 'icon',
                  isPrimary: true,
                  onClick: setTemplateToEdit,
                },
                {
                  name: 'Duplicate',
                  description: 'Duplicate template',
                  icon: 'copy',
                  type: 'icon',
                  // eslint-disable-next-line @typescript-eslint/no-unused-vars
                  onClick: ({ id, createdAt, updatedAt, name, ...rest }: CertificateTemplate) =>
                    setTemplateToEdit({
                      ...rest,
                      name: getCopyName(
                        name,
                        templates.map((t) => t.name),
                      ),
                    }),
                },
                {
                  name: 'Remove',
                  description: 'Remove template',
                  icon: 'trash',
                  color: 'danger',
                  type: 'icon',
                  isPrimary: true,
                  onClick: setTemplateToRemove,
                },
              ],
            },
          ]}
        />
      </>
    );
  }

  const importModal = showImportModal ? (
    <CertificateTemplateImportModal
      onClose={(success) => {
        setShowImportModal(false);
        if (success) {
          refresh();
        }
      }}
    />
  ) : null;

  return (
    <>
      {content}
      {editFlyout}
      {generateModal}
      {shareModal}
      {removeConfirmModal}
      {importModal}
    </>
  );
}
