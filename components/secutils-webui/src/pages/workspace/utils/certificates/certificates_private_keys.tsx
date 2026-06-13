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
  EuiText,
  EuiToolTip,
} from '@elastic/eui';
import { useCallback, useEffect, useMemo, useState } from 'react';

import { UTIL_HANDLES } from '..';
import { PRIVATE_KEYS_PROD_WARNING_USER_SETTINGS_KEY } from './consts';
import type { PrivateKey } from './private_key';
import { privateKeyAlgString } from './private_key_alg';
import { PrivateKeyEditFlyout } from './private_key_edit_flyout';
import { PrivateKeyExportModal } from './private_key_export_modal';
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
import { TimestampTableCell } from '../../components/timestamp_table_cell';
import { useWorkspaceContext } from '../../hooks';
import { getWorkspaceEntityAbsoluteLink, getWorkspaceEntityLink } from '../workspace_links';

export default function CertificatesPrivateKeys() {
  const { setTitleActions, setSettings, settings } = useWorkspaceContext();

  const [initialized, setInitialized] = useState(false);

  const [privateKeyToRemove, setPrivateKeyToRemove] = useState<PrivateKey | null>(null);
  const [privateKeyToExport, setPrivateKeyToExport] = useState<PrivateKey | null>(null);
  const [privateKeyToEdit, setPrivateKeyToEdit] = useState<Partial<PrivateKey> | null | undefined>(null);
  const { allTags } = useUserTags();

  const createButton = useMemo(
    () => (
      <EuiButton
        iconType={'plusInCircle'}
        title="Create a new private key"
        fill
        onClick={() => setPrivateKeyToEdit(undefined)}
      >
        Create private key
      </EuiButton>
    ),
    [],
  );

  const docsButton = (
    <EuiButtonEmpty
      iconType={'documentation'}
      title="Learn how to create and use private keys"
      target={'_blank'}
      href={'/docs/guides/digital_certificates/private_keys'}
    >
      Learn how to
    </EuiButtonEmpty>
  );

  const fetcher = useCallback(async (request: PaginationRequest): Promise<Page<PrivateKey>> => {
    const res = await apiFetch(`/api/certificates/private_keys${buildPaginationQuery(request)}`);
    if (!res.ok) {
      throw await ResponseError.fromResponse(res);
    }
    return (await res.json()) as Page<PrivateKey>;
  }, []);

  const {
    items: privateKeys,
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
  } = useServerPaginatedItems<PrivateKey>({
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
    setTitleActions(isEmpty ? null : createButton);
  }, [isEmpty, createButton, setTitleActions]);

  const editFlyout =
    privateKeyToEdit !== null ? (
      <PrivateKeyEditFlyout
        onClose={(success) => {
          if (success) {
            refresh();
          }
          setPrivateKeyToEdit(null);
        }}
        privateKey={privateKeyToEdit}
      />
    ) : null;

  const generateModal = privateKeyToExport ? (
    <PrivateKeyExportModal onClose={() => setPrivateKeyToExport(null)} privateKey={privateKeyToExport} />
  ) : null;

  const removeConfirmModal = privateKeyToRemove ? (
    <EuiConfirmModal
      title={`Remove "${privateKeyToRemove.name}"?`}
      onCancel={() => setPrivateKeyToRemove(null)}
      onConfirm={() => {
        setPrivateKeyToRemove(null);

        apiFetch(`/api/certificates/private_keys/${encodeURIComponent(privateKeyToRemove?.id)}`, { method: 'DELETE' })
          .then(async (res) => {
            if (!res.ok) {
              throw await ResponseError.fromResponse(res);
            }
            refresh();
          })
          .catch((err: Error) => {
            console.error(`Failed to remove private key: ${err.message}`);
          });
      }}
      cancelButtonText="Cancel"
      confirmButtonText="Remove"
      buttonColor="danger"
    >
      The private key will be removed. Are you sure you want to proceed?
    </EuiConfirmModal>
  ) : null;

  if (!initialized && loading) {
    return <PageLoadingState />;
  }

  if (error && privateKeys.length === 0) {
    return <PageErrorState title="Cannot load private keys" content={<p>{error}</p>} />;
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
            title={<h2>You don&apos;t have any private keys yet</h2>}
            titleSize="s"
            style={{ maxWidth: '60em', display: 'flex' }}
            body={
              <div>
                <p>Go ahead and create your first private key.</p>
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
    const privateKeysProdWarning =
      settings?.[PRIVATE_KEYS_PROD_WARNING_USER_SETTINGS_KEY] === true ? null : (
        <div>
          <EuiCallOut
            title="Don't use generated private keys in production environments"
            color="warning"
            iconType="warning"
          >
            <p>
              While private keys generated through Secutils.dev are encrypted at rest, they are intended for use in
              development and testing environments only. Please do not use these private keys in production environments
              unless you are running{' '}
              <EuiLink target="_blank" href="https://github.com/secutils-dev/secutils">
                your own version
              </EuiLink>{' '}
              of the Secutils.dev in a trusted and controlled environment.
            </p>
            <EuiButton
              color="accent"
              onClick={() => setSettings({ [PRIVATE_KEYS_PROD_WARNING_USER_SETTINGS_KEY]: true })}
            >
              Do not show again
            </EuiButton>
          </EuiCallOut>
          <EuiSpacer />
        </div>
      );

    content = (
      <>
        {privateKeysProdWarning}
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
          items={privateKeys}
          itemId={(item) => item.id}
          tableLayout={'auto'}
          columns={[
            {
              name: (
                <EuiToolTip content="A unique name of the private key">
                  <span>
                    Name <EuiIcon size="s" color="subdued" type="question" className="eui-alignTop" />
                  </span>
                </EuiToolTip>
              ),
              field: 'name',
              sortable: true,
              render: (_, privateKey: PrivateKey) => (
                <EntityName
                  name={privateKey.name}
                  href={getWorkspaceEntityLink(UTIL_HANDLES.certificatesPrivateKeys, privateKey.id)}
                  tags={privateKey.tags}
                />
              ),
            },
            {
              name: (
                <EuiToolTip content="Algorithm used to generate a private key.">
                  <span>
                    Type <EuiIcon size="s" color="subdued" type="question" className="eui-alignTop" />
                  </span>
                </EuiToolTip>
              ),
              field: 'alg',
              textOnly: true,
              width: '400px',
              mobileOptions: { width: 'unset' },
              render: (_, privateKey: PrivateKey) => privateKeyAlgString(privateKey.alg),
            },
            {
              name: (
                <EuiToolTip content="Indicates whether the private key is encrypted with a passphrase or not.">
                  <span>
                    Encryption <EuiIcon size="s" color="subdued" type="question" className="eui-alignTop" />
                  </span>
                </EuiToolTip>
              ),
              field: 'encrypted',
              textOnly: true,
              width: '110px',
              render: (_, privateKey: PrivateKey) => (
                <EuiText size={'s'} color={privateKey.encrypted ? 'success' : 'danger'}>
                  {privateKey.encrypted ? 'Passphrase' : <b>None</b>}
                </EuiText>
              ),
            },
            {
              name: 'Last updated',
              field: 'updatedAt',
              width: '160px',
              mobileOptions: { width: 'unset' },
              sortable: true,
              render: (_, privateKey: PrivateKey) => <TimestampTableCell timestamp={privateKey.updatedAt} />,
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
                  onClick: ({ id }: PrivateKey) => void navigator.clipboard.writeText(id),
                },
                {
                  name: 'Copy link',
                  description: 'Copy link to private key in grid',
                  icon: 'link',
                  type: 'icon',
                  onClick: ({ id }: PrivateKey) =>
                    void navigator.clipboard.writeText(
                      getWorkspaceEntityAbsoluteLink(UTIL_HANDLES.certificatesPrivateKeys, id),
                    ),
                },
                {
                  name: 'Export',
                  description: 'Export private key',
                  icon: 'download',
                  type: 'icon',
                  onClick: setPrivateKeyToExport,
                },
                {
                  name: 'Edit',
                  description: 'Edit private key details',
                  icon: 'pencil',
                  type: 'icon',
                  isPrimary: true,
                  onClick: setPrivateKeyToEdit,
                },
                {
                  name: 'Duplicate',
                  description: 'Duplicate private key',
                  icon: 'copy',
                  type: 'icon',
                  // eslint-disable-next-line @typescript-eslint/no-unused-vars
                  onClick: ({ id, createdAt, updatedAt, name, ...rest }: PrivateKey) =>
                    setPrivateKeyToEdit({
                      ...rest,
                      name: getCopyName(
                        name,
                        privateKeys.map((k) => k.name),
                      ),
                    }),
                },
                {
                  name: 'Remove',
                  description: 'Remove private key',
                  icon: 'trash',
                  color: 'danger',
                  type: 'icon',
                  isPrimary: true,
                  onClick: setPrivateKeyToRemove,
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
      {removeConfirmModal}
    </>
  );
}
