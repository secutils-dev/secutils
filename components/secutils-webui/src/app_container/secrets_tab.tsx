import {
  EuiButton,
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
import { useCallback, useEffect, useState } from 'react';

import { SecretEditModal } from './secret_edit_modal';
import { useUserTags } from '../hooks';
import { createUserSecret, deleteUserSecret, getUserSecrets, updateUserSecret } from '../model';
import type { UserSecret } from '../model';
import type { PageToast } from '../pages/page';
import { getTagsColumn } from '../pages/workspace/components/entity_tags_column';
import {
  FilteredEmptyState,
  ItemsTableFilter,
  TagsFilter,
  useItemsTableFilter,
} from '../pages/workspace/components/items_table_filter';

interface DeleteConfirmation {
  id: string;
  name: string;
}

export function SecretsTab({ addToast }: { addToast: (toast: PageToast) => void }) {
  const [secrets, setSecrets] = useState<UserSecret[]>([]);
  const [loading, setLoading] = useState(true);
  const [editModal, setEditModal] = useState<
    { visible: false } | { visible: true; editingId?: string; editingName?: string; initialTagIds?: string[] }
  >({
    visible: false,
  });
  const [deleteConfirm, setDeleteConfirm] = useState<DeleteConfirmation | null>(null);
  const { allTags } = useUserTags();

  const loadSecrets = useCallback(async () => {
    setLoading(true);
    try {
      setSecrets(await getUserSecrets());
    } catch {
      addToast({ id: 'load-secrets-error', color: 'danger', title: 'Failed to load secrets' });
    } finally {
      setLoading(false);
    }
  }, [addToast]);

  useEffect(() => {
    loadSecrets();
  }, [loadSecrets]);

  const handleSave = useCallback(
    async (name: string, value: string, editingId?: string, tagIds?: string[]) => {
      if (editingId !== undefined) {
        await updateUserSecret(editingId, value, tagIds);
        addToast({ id: 'update-secret', color: 'success', title: `Secret "${name}" updated` });
      } else {
        await createUserSecret(name, value, tagIds);
        addToast({ id: 'create-secret', color: 'success', title: `Secret "${name}" created` });
      }
      await loadSecrets();
    },
    [loadSecrets, addToast],
  );

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
      render: (name: string) => (
        <EuiText size="s">
          <strong>{name}</strong>
        </EuiText>
      ),
    },
    getTagsColumn(),
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
            setEditModal({
              visible: true,
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

  return (
    <>
      <EuiFlexGroup justifyContent="flexEnd">
        <EuiFlexItem grow={false}>
          <EuiButton
            iconType="plusInCircle"
            size="s"
            onClick={() => setEditModal({ visible: true })}
            disabled={loading}
          >
            Add secret
          </EuiButton>
        </EuiFlexItem>
      </EuiFlexGroup>
      <EuiSpacer size="m" />
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
          totalItems > 0 ? (
            <FilteredEmptyState
              totalItems={totalItems}
              hasPageFilters={hasPageFilters}
              onClearFilters={clearPageFilters}
            />
          ) : (
            <EuiEmptyPrompt
              icon={<EuiIcon type="lock" size="xl" />}
              title={<h3>No secrets yet</h3>}
              body="Add secrets to use in your responder scripts, tracker scripts, and responder templates."
            />
          )
        }
      />
      {editModal.visible ? (
        <SecretEditModal
          editingId={editModal.editingId}
          editingName={editModal.editingName}
          initialTagIds={editModal.initialTagIds}
          onSave={handleSave}
          onClose={() => setEditModal({ visible: false })}
          addToast={addToast}
        />
      ) : null}
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
