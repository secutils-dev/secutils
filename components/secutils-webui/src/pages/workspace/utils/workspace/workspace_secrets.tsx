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
  EuiText,
} from '@elastic/eui';
import type { EuiBasicTableColumn } from '@elastic/eui';
import { unix } from 'moment/moment';
import { useCallback, useEffect, useMemo, useState } from 'react';

import { SecretEditFlyout } from './secret_edit_flyout';
import { PageLoadingState } from '../../../../components';
import { useUserTags } from '../../../../hooks';
import { deleteUserSecret, getUserSecrets } from '../../../../model';
import type { UserSecret } from '../../../../model';
import { EntityName } from '../../components/entity_name';
import {
  FilteredEmptyState,
  ItemsTableFilter,
  TagsFilter,
  useItemsTableFilter,
} from '../../components/items_table_filter';
import { useWorkspaceContext } from '../../hooks';

interface DeleteConfirmation {
  id: string;
  name: string;
}

export default function WorkspaceSecrets() {
  const { addToast, setTitleActions } = useWorkspaceContext();

  const [secrets, setSecrets] = useState<UserSecret[]>([]);
  const [loading, setLoading] = useState(true);
  const [secretToEdit, setSecretToEdit] = useState<null | {
    editingId?: string;
    editingName?: string;
    initialTagIds?: string[];
  }>(null);
  const [deleteConfirm, setDeleteConfirm] = useState<DeleteConfirmation | null>(null);
  const { allTags } = useUserTags();

  const createButton = useMemo(
    () => (
      <EuiButton iconType={'plusInCircle'} title="Create a new secret" fill onClick={() => setSecretToEdit({})}>
        Add secret
      </EuiButton>
    ),
    [],
  );

  const docsButton = (
    <EuiButtonEmpty
      iconType={'documentation'}
      title="Learn how to use secrets"
      target={'_blank'}
      href={'/docs/guides/platform/secrets'}
    >
      Learn how to
    </EuiButtonEmpty>
  );

  const loadSecrets = useCallback(async () => {
    setLoading(true);
    try {
      const loaded = await getUserSecrets();
      setSecrets(loaded);
      setTitleActions(loaded.length === 0 ? null : createButton);
    } catch {
      addToast({ id: 'load-secrets-error', color: 'danger', title: 'Failed to load secrets' });
    } finally {
      setLoading(false);
    }
  }, [addToast, createButton, setTitleActions]);

  useEffect(() => {
    loadSecrets();
  }, [loadSecrets]);

  const handleDelete = useCallback(
    async (id: string, name: string) => {
      try {
        await deleteUserSecret(id);
        addToast({ id: 'delete-secret', color: 'success', title: `Secret "${name}" deleted` });
        await loadSecrets();
      } catch {
        addToast({ id: 'delete-secret-error', color: 'danger', title: `Failed to delete secret "${name}"` });
      }
      setDeleteConfirm(null);
    },
    [loadSecrets, addToast],
  );

  const getSearchFields = useCallback((secret: UserSecret) => [secret.name, secret.id], []);
  const getItemTags = useCallback((secret: UserSecret) => secret.tags, []);
  const {
    filteredItems,
    query,
    setQuery,
    selectedTagIds,
    setSelectedTagIds,
    totalItems,
    hasPageFilters,
    clearPageFilters,
  } = useItemsTableFilter({
    items: secrets,
    allTags,
    getSearchFields,
    getItemTags,
  });

  const columns: Array<EuiBasicTableColumn<UserSecret>> = [
    {
      field: 'name',
      name: 'Name',
      sortable: true,
      render: (_: string, secret: UserSecret) => <EntityName name={secret.name} tags={secret.tags} />,
    },
    {
      field: 'updatedAt',
      name: 'Last updated',
      sortable: true,
      render: (updatedAt: number) => <EuiText size="s">{unix(updatedAt).format('ll LTS')}</EuiText>,
    },
    {
      name: 'Actions',
      width: '100px',
      actions: [
        {
          name: 'Edit',
          description: 'Update secret value',
          icon: 'pencil',
          type: 'icon',
          onClick: (secret: UserSecret) =>
            setSecretToEdit({
              editingId: secret.id,
              editingName: secret.name,
              initialTagIds: secret.tags?.map((t) => t.id),
            }),
        },
        {
          name: 'Delete',
          description: 'Delete secret',
          icon: 'trash',
          color: 'danger',
          type: 'icon',
          onClick: (secret: UserSecret) => setDeleteConfirm({ id: secret.id, name: secret.name }),
        },
      ],
    },
  ];

  if (loading && secrets.length === 0) {
    return <PageLoadingState />;
  }

  let content;
  if (secrets.length === 0) {
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
            icon={<EuiIcon type={'lock'} size={'xl'} />}
            title={<h2>No secrets yet</h2>}
            titleSize="s"
            style={{ maxWidth: '60em', display: 'flex' }}
            body={
              <div>
                <p>Add secrets to use in your responder scripts, tracker scripts, and responder templates.</p>
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
        <ItemsTableFilter query={query} onQueryChange={setQuery} onRefresh={loadSecrets} placeholder="Search secrets…">
          <TagsFilter tags={allTags} selectedTagIds={selectedTagIds} onSelectedTagIdsChange={setSelectedTagIds} />
        </ItemsTableFilter>
        <EuiSpacer size="m" />
        <EuiInMemoryTable
          items={filteredItems}
          columns={columns}
          loading={loading}
          sorting={{ sort: { field: 'updatedAt', direction: 'desc' } }}
          pagination={{ pageSize: 10, showPerPageOptions: true }}
          noItemsMessage={
            <FilteredEmptyState
              totalItems={totalItems}
              hasPageFilters={hasPageFilters}
              onClearFilters={clearPageFilters}
            />
          }
        />
      </>
    );
  }

  const editFlyout = secretToEdit ? (
    <SecretEditFlyout
      editingId={secretToEdit.editingId}
      editingName={secretToEdit.editingName}
      initialTagIds={secretToEdit.initialTagIds}
      onClose={(success) => {
        if (success) {
          loadSecrets();
        }
        setSecretToEdit(null);
      }}
    />
  ) : null;

  return (
    <>
      {content}
      {editFlyout}
      {deleteConfirm ? (
        <EuiConfirmModal
          title={`Delete secret "${deleteConfirm.name}"?`}
          onCancel={() => setDeleteConfirm(null)}
          onConfirm={() => handleDelete(deleteConfirm.id, deleteConfirm.name)}
          cancelButtonText="Cancel"
          confirmButtonText="Delete"
          buttonColor="danger"
        >
          This action cannot be undone.
        </EuiConfirmModal>
      ) : null}
    </>
  );
}
