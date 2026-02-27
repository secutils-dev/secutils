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

import type { UserSecret } from '../model';
import { createUserSecret, deleteUserSecret, getUserSecrets, updateUserSecret } from '../model';
import { SecretEditModal } from './secret_edit_modal';
import type { PageToast } from '../pages/page';

interface DeleteConfirmation {
  name: string;
}

export function SecretsTab({ addToast }: { addToast: (toast: PageToast) => void }) {
  const [secrets, setSecrets] = useState<UserSecret[]>([]);
  const [loading, setLoading] = useState(true);
  const [editModal, setEditModal] = useState<{ visible: false } | { visible: true; editingName?: string }>({
    visible: false,
  });
  const [deleteConfirm, setDeleteConfirm] = useState<DeleteConfirmation | null>(null);

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
    async (name: string, value: string) => {
      const isUpdate = secrets.some((s) => s.name === name);
      if (isUpdate) {
        await updateUserSecret(name, value);
        addToast({ id: 'update-secret', color: 'success', title: `Secret "${name}" updated` });
      } else {
        await createUserSecret(name, value);
        addToast({ id: 'create-secret', color: 'success', title: `Secret "${name}" created` });
      }
      await loadSecrets();
    },
    [secrets, loadSecrets, addToast],
  );

  const handleDelete = useCallback(
    async (name: string) => {
      try {
        await deleteUserSecret(name);
        addToast({ id: 'delete-secret', color: 'success', title: `Secret "${name}" deleted` });
        await loadSecrets();
      } catch {
        addToast({ id: 'delete-secret-error', color: 'danger', title: `Failed to delete secret "${name}"` });
      }
      setDeleteConfirm(null);
    },
    [loadSecrets, addToast],
  );

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
          onClick: (secret: UserSecret) => setEditModal({ visible: true, editingName: secret.name }),
        },
        {
          name: 'Delete',
          description: 'Delete secret',
          icon: 'trash',
          color: 'danger',
          type: 'icon',
          onClick: (secret: UserSecret) => setDeleteConfirm({ name: secret.name }),
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
      <EuiInMemoryTable
        items={secrets}
        columns={columns}
        loading={loading}
        search={{ box: { incremental: true, placeholder: 'Search secrets…' } }}
        sorting={{ sort: { field: 'name', direction: 'asc' } }}
        pagination={{ pageSize: 10, showPerPageOptions: true }}
        noItemsMessage={
          <EuiEmptyPrompt
            icon={<EuiIcon type="lock" size="xl" />}
            title={<h3>No secrets yet</h3>}
            body="Add secrets to use in your responder scripts, tracker scripts, and responder templates."
          />
        }
      />
      {editModal.visible ? (
        <SecretEditModal
          editingName={editModal.editingName}
          onSave={handleSave}
          onClose={() => setEditModal({ visible: false })}
        />
      ) : null}
      {deleteConfirm ? (
        <EuiConfirmModal
          title={`Delete secret "${deleteConfirm.name}"?`}
          onCancel={() => setDeleteConfirm(null)}
          onConfirm={() => handleDelete(deleteConfirm.name)}
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
