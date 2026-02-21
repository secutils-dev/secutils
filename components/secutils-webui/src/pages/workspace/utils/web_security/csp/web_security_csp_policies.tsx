import type { Criteria, Pagination, PropertySort } from '@elastic/eui';
import {
  EuiButton,
  EuiButtonEmpty,
  EuiConfirmModal,
  EuiEmptyPrompt,
  EuiFlexGroup,
  EuiFlexItem,
  EuiIcon,
  EuiInMemoryTable,
  EuiSpacer,
  EuiToolTip,
} from '@elastic/eui';
import { useCallback, useEffect, useMemo, useState } from 'react';

import type { ContentSecurityPolicy, SerializedContentSecurityPolicyDirectives } from './content_security_policy';
import { deserializeContentSecurityPolicyDirectives, getContentSecurityPolicyString } from './content_security_policy';
import { ContentSecurityPolicyCopyModal } from './content_security_policy_copy_modal';
import { ContentSecurityPolicyEditFlyout } from './content_security_policy_edit_flyout';
import { ContentSecurityPolicyImportModal } from './content_security_policy_import_modal';
import { ContentSecurityPolicyShareModal } from './content_security_policy_share_modal';
import { PageErrorState, PageLoadingState } from '../../../../../components';
import {
  type AsyncData,
  getApiRequestConfig,
  getApiUrl,
  getCopyName,
  getErrorMessage,
  ResponseError,
} from '../../../../../model';
import { ItemsTableFilter, useItemsTableFilter } from '../../../components/items_table_filter';
import { TimestampTableCell } from '../../../components/timestamp_table_cell';
import { useWorkspaceContext } from '../../../hooks';

export default function WebSecurityContentSecurityPolicies() {
  const { uiState, setTitleActions } = useWorkspaceContext();

  const [policyToCopy, setPolicyToCopy] = useState<ContentSecurityPolicy | null>(null);
  const [policyToShare, setPolicyToShare] = useState<ContentSecurityPolicy | null>(null);
  const [policyToRemove, setPolicyToRemove] = useState<ContentSecurityPolicy | null>(null);
  const [policyToEdit, setPolicyToEdit] = useState<Partial<ContentSecurityPolicy> | null | undefined>(null);
  const [isImportModalOpen, setIsImportModalOpen] = useState<boolean>(false);

  const [policies, setPolicies] = useState<AsyncData<ContentSecurityPolicy[]>>({ status: 'pending' });

  const createButton = useMemo(
    () => (
      <EuiFlexGroup responsive={false} gutterSize="s" alignItems="center" justifyContent={'center'}>
        <EuiFlexItem grow={false}>
          <EuiButton
            iconType={'importAction'}
            title="Import content security policy"
            onClick={() => setIsImportModalOpen(true)}
          >
            Import policy
          </EuiButton>
        </EuiFlexItem>
        <EuiFlexItem grow={false}>
          <EuiButton
            iconType={'plusInCircle'}
            fill
            title="Create new content security policy"
            onClick={() => setPolicyToEdit(undefined)}
          >
            Create policy
          </EuiButton>
        </EuiFlexItem>
      </EuiFlexGroup>
    ),
    [],
  );

  const docsButton = (
    <EuiButtonEmpty
      iconType={'documentation'}
      title="Learn how to create and use content security policies"
      target={'_blank'}
      href={'/docs/guides/web_security/csp'}
    >
      Learn how to
    </EuiButtonEmpty>
  );

  const loadPolicies = useCallback(() => {
    fetch(getApiUrl('/api/utils/web_security/csp'), getApiRequestConfig())
      .then(async (res) => {
        if (!res.ok) {
          throw await ResponseError.fromResponse(res);
        }

        const policies = (await res.json()) as ContentSecurityPolicy<SerializedContentSecurityPolicyDirectives>[];
        setPolicies({
          status: 'succeeded',
          data: policies.map((policy) => ({
            ...policy,
            directives: deserializeContentSecurityPolicyDirectives(policy.directives),
          })),
        });
        setTitleActions(policies.length === 0 ? null : createButton);
      })
      .catch((err) => setPolicies({ status: 'failed', error: getErrorMessage(err) }));
  }, [createButton, setTitleActions]);

  useEffect(() => {
    if (!uiState.synced) {
      return;
    }

    loadPolicies();
  }, [uiState, loadPolicies]);

  const editFlyout =
    policyToEdit !== null ? (
      <ContentSecurityPolicyEditFlyout
        onClose={(success) => {
          if (success) {
            loadPolicies();
          }
          setPolicyToEdit(null);
        }}
        policy={policyToEdit}
      />
    ) : null;

  const copyModal = policyToCopy ? (
    <ContentSecurityPolicyCopyModal onClose={() => setPolicyToCopy(null)} policy={policyToCopy} />
  ) : null;

  const shareModal = policyToShare ? (
    <ContentSecurityPolicyShareModal onClose={() => setPolicyToShare(null)} policy={policyToShare} />
  ) : null;

  const importModal = isImportModalOpen ? (
    <ContentSecurityPolicyImportModal
      onClose={(success) => {
        setIsImportModalOpen(false);
        if (success) {
          loadPolicies();
        }
      }}
    />
  ) : null;

  const removeConfirmModal = policyToRemove ? (
    <EuiConfirmModal
      title={`Remove "${policyToRemove.name}"?`}
      onCancel={() => setPolicyToRemove(null)}
      onConfirm={() => {
        setPolicyToRemove(null);

        fetch(
          getApiUrl(`/api/utils/web_security/csp/${encodeURIComponent(policyToRemove.id)}`),
          getApiRequestConfig('DELETE'),
        )
          .then(async (res) => {
            if (!res.ok) {
              throw await ResponseError.fromResponse(res);
            }

            loadPolicies();
          })
          .catch((err: Error) => console.error(`Failed to remove content security policy: ${getErrorMessage(err)}`));
      }}
      cancelButtonText="Cancel"
      confirmButtonText="Remove"
      buttonColor="danger"
    >
      The Content Security Policy template will be removed. Are you sure you want to proceed?
    </EuiConfirmModal>
  ) : null;

  // Filter configuration: search by name, ID, and policy content
  const getSearchFields = useCallback(
    (policy: ContentSecurityPolicy) => [policy.name, policy.id, getContentSecurityPolicyString(policy)],
    [],
  );
  const { filteredItems, query, setQuery } = useItemsTableFilter({
    items: policies.status === 'succeeded' ? policies.data : [],
    getSearchFields,
  });

  const [pagination, setPagination] = useState<Pagination>({
    pageIndex: 0,
    pageSize: 15,
    pageSizeOptions: [10, 15, 25, 50, 100],
    totalItemCount: 0,
  });
  const [sorting, setSorting] = useState<{ sort: PropertySort }>({ sort: { field: 'name', direction: 'asc' } });
  const onTableChange = useCallback(
    ({ page, sort }: Criteria<ContentSecurityPolicy>) => {
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

  if (policies.status === 'pending') {
    return <PageLoadingState />;
  }

  if (policies.status === 'failed') {
    return (
      <PageErrorState
        title="Cannot load content security policies"
        content={
          <p>
            Cannot load content security policies.
            <br />
            <br />
            <strong>{policies.error}</strong>.
          </p>
        }
      />
    );
  }

  let content;
  if (policies.data.length === 0) {
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
            icon={<EuiIcon type={'globe'} size={'xl'} />}
            title={<h2>You don&apos;t have any content security policies yet</h2>}
            titleSize="s"
            style={{ maxWidth: '60em', display: 'flex' }}
            body={
              <div>
                <p>Go ahead and create your first policy.</p>
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
    content = (
      <>
        <ItemsTableFilter
          query={query}
          onQueryChange={setQuery}
          onRefresh={loadPolicies}
          placeholder="Search by name, ID, or policy content..."
        />
        <EuiSpacer size="m" />
        <EuiInMemoryTable
          pagination={pagination}
          allowNeutralSort={false}
          sorting={sorting}
          onTableChange={onTableChange}
          items={filteredItems}
          itemId={(item) => item.id}
          tableLayout={'auto'}
          columns={[
            {
              name: (
                <EuiToolTip content="Content security policy name">
                  <span>
                    Name <EuiIcon size="s" color="subdued" type="question" className="eui-alignTop" />
                  </span>
                </EuiToolTip>
              ),
              field: 'name',
              sortable: true,
              textOnly: true,
              render: (_, item: ContentSecurityPolicy) => <span style={{ whiteSpace: 'nowrap' }}>{item.name}</span>,
            },
            {
              name: (
                <EuiToolTip content="Content security policy as it should appear in HTTP header or <meta> tag.">
                  <span>
                    Policy <EuiIcon size="s" color="subdued" type="question" className="eui-alignTop" />
                  </span>
                </EuiToolTip>
              ),
              field: 'directives',
              render: (_, policy: ContentSecurityPolicy) => getContentSecurityPolicyString(policy),
            },
            {
              name: 'Last updated',
              field: 'updatedAt',
              width: '160px',
              mobileOptions: { width: 'unset' },
              sortable: (policy) => policy.updatedAt,
              render: (_, policy: ContentSecurityPolicy) => <TimestampTableCell timestamp={policy.updatedAt} />,
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
                  onClick: ({ id }: ContentSecurityPolicy) => void navigator.clipboard.writeText(id),
                },
                {
                  name: 'Copy',
                  description: 'Copy policy',
                  icon: 'copyClipboard',
                  type: 'icon',
                  onClick: setPolicyToCopy,
                },
                {
                  name: 'Share',
                  description: 'Share policy',
                  icon: 'share',
                  type: 'icon',
                  onClick: setPolicyToShare,
                },
                {
                  name: 'Edit',
                  description: 'Edit policy',
                  icon: 'pencil',
                  type: 'icon',
                  isPrimary: true,
                  onClick: setPolicyToEdit,
                },
                {
                  name: 'Duplicate',
                  description: 'Duplicate policy',
                  icon: 'copy',
                  type: 'icon',
                  // eslint-disable-next-line @typescript-eslint/no-unused-vars
                  onClick: ({ id, createdAt, updatedAt, name, ...rest }: ContentSecurityPolicy) =>
                    setPolicyToEdit({ ...rest, name: getCopyName(name) }),
                },
                {
                  name: 'Remove',
                  description: 'Remove policy',
                  icon: 'trash',
                  color: 'danger',
                  type: 'icon',
                  isPrimary: true,
                  onClick: setPolicyToRemove,
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
      {copyModal}
      {shareModal}
      {removeConfirmModal}
      {importModal}
    </>
  );
}
