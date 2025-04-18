import { useCallback, useEffect, useState } from 'react';

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
import axios from 'axios';
import { TimestampTableCell } from '../../../components/timestamp_table_cell';

import type { ContentSecurityPolicy, SerializedContentSecurityPolicyDirectives } from './content_security_policy';
import { deserializeContentSecurityPolicyDirectives, getContentSecurityPolicyString } from './content_security_policy';
import { ContentSecurityPolicyCopyModal } from './content_security_policy_copy_modal';
import { ContentSecurityPolicyImportModal } from './content_security_policy_import_modal';
import { ContentSecurityPolicyShareModal } from './content_security_policy_share_modal';
import { SaveContentSecurityPolicyFlyout } from './save_content_security_policy_flyout';
import { PageErrorState, PageLoadingState } from '../../../../../components';
import { type AsyncData, getApiRequestConfig, getApiUrl, getErrorMessage } from '../../../../../model';
import { useWorkspaceContext } from '../../../hooks';

export default function WebSecurityContentSecurityPolicies() {
  const { uiState, setTitleActions } = useWorkspaceContext();

  const [policyToCopy, setPolicyToCopy] = useState<ContentSecurityPolicy | null>(null);
  const [policyToShare, setPolicyToShare] = useState<ContentSecurityPolicy | null>(null);
  const [policyToRemove, setPolicyToRemove] = useState<ContentSecurityPolicy | null>(null);
  const [policyToEdit, setPolicyToEdit] = useState<ContentSecurityPolicy | null | undefined>(null);

  const [policies, setPolicies] = useState<AsyncData<ContentSecurityPolicy[]>>({ status: 'pending' });

  const loadPolicies = () => {
    axios
      .get<
        ContentSecurityPolicy<SerializedContentSecurityPolicyDirectives>[]
      >(getApiUrl('/api/utils/web_security/csp'), getApiRequestConfig())
      .then(
        (res) => {
          setPolicies({
            status: 'succeeded',
            data: res.data.map((policy) => ({
              ...policy,
              directives: deserializeContentSecurityPolicyDirectives(policy.directives),
            })),
          });
          setTitleActions(res.data.length === 0 ? null : createButton);
        },
        (err: Error) => {
          setPolicies({ status: 'failed', error: getErrorMessage(err) });
        },
      );
  };

  useEffect(() => {
    if (!uiState.synced) {
      return;
    }

    loadPolicies();
  }, [uiState]);

  const createButton = (
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

  const editFlyout =
    policyToEdit !== null ? (
      <SaveContentSecurityPolicyFlyout
        onClose={(success) => {
          if (success) {
            loadPolicies();
          }
          setPolicyToEdit(null);
        }}
        policy={policyToEdit}
      />
    ) : null;

  const [isImportModalOpen, setIsImportModalOpen] = useState<boolean>(false);

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

        axios
          .delete(
            getApiUrl(`/api/utils/web_security/csp/${encodeURIComponent(policyToRemove.id)}`),
            getApiRequestConfig(),
          )
          .then(
            () => loadPolicies(),
            (err: Error) => {
              console.error(`Failed to remove content security policy: ${getErrorMessage(err)}`);
            },
          );
      }}
      cancelButtonText="Cancel"
      confirmButtonText="Remove"
      buttonColor="danger"
    >
      The Content Security Policy template will be removed. Are you sure you want to proceed?
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
      <EuiInMemoryTable
        pagination={pagination}
        allowNeutralSort={false}
        sorting={sorting}
        onTableChange={onTableChange}
        items={policies.data}
        itemId={(item) => item.id}
        tableLayout={'auto'}
        columns={[
          {
            name: (
              <EuiToolTip content="Content security policy name">
                <span>
                  Name <EuiIcon size="s" color="subdued" type="questionInCircle" className="eui-alignTop" />
                </span>
              </EuiToolTip>
            ),
            field: 'name',
            sortable: true,
            textOnly: true,
            render: (_, item: ContentSecurityPolicy) => item.name,
          },
          {
            name: (
              <EuiToolTip content="Content security policy as it should appear in HTTP header or <meta> tag.">
                <span>
                  Policy <EuiIcon size="s" color="subdued" type="questionInCircle" className="eui-alignTop" />
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
                name: 'Copy policy',
                description: 'Copy policy',
                icon: 'copy',
                type: 'icon',
                isPrimary: true,
                onClick: setPolicyToCopy,
              },
              {
                name: 'Share policy',
                description: 'Share policy',
                icon: 'share',
                type: 'icon',
                onClick: setPolicyToShare,
              },
              {
                name: 'Edit policy',
                description: 'Edit policy',
                icon: 'pencil',
                type: 'icon',
                onClick: setPolicyToEdit,
              },
              {
                name: 'Remove policy',
                description: 'Remove policy',
                icon: 'minusInCircle',
                type: 'icon',
                isPrimary: true,
                onClick: setPolicyToRemove,
              },
            ],
          },
        ]}
      />
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
