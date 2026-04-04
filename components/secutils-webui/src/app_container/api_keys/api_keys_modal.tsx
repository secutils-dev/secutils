import type { EuiBasicTableColumn } from '@elastic/eui';
import {
  EuiButton,
  EuiButtonEmpty,
  EuiCallOut,
  EuiCode,
  EuiConfirmModal,
  EuiCopy,
  EuiFlexGroup,
  EuiFlexItem,
  EuiInMemoryTable,
  EuiLoadingSpinner,
  EuiModal,
  EuiModalBody,
  EuiModalFooter,
  EuiModalHeader,
  EuiModalHeaderTitle,
  EuiSpacer,
  EuiText,
} from '@elastic/eui';
import { useCallback, useEffect, useMemo, useState } from 'react';

import { CreateEditModal } from './create_edit_api_key_modal';
import { RegenerateConfirmModal } from './regenerate_api_key_modal';
import type { ApiKeyCreateResponse, UserApiKey } from '../../model';
import {
  createUserApiKey,
  deleteUserApiKey,
  getUserApiKeys,
  regenerateUserApiKey,
  updateUserApiKey,
} from '../../model';
import type { PageToast } from '../../pages/page';
import { TimestampTableCell } from '../../pages/workspace/components/timestamp_table_cell';

interface Props {
  addToast: (toast: PageToast) => void;
  onClose: () => void;
}

function isExpired(expiresAt: number | null) {
  return expiresAt != null && expiresAt < Date.now() / 1000;
}

interface TokenRevealProps {
  token: string;
  isRegenerate?: boolean;
  onDismiss: () => void;
}

function TokenReveal({ token, isRegenerate, onDismiss }: TokenRevealProps) {
  return (
    <EuiCallOut
      title={isRegenerate ? 'API key regenerated' : 'API key created'}
      color={isRegenerate ? 'warning' : 'success'}
      iconType={isRegenerate ? 'refresh' : 'check'}
    >
      <p>This token will not be shown again. Copy it now.</p>
      <EuiFlexGroup gutterSize="s" alignItems="center" responsive={false} wrap>
        <EuiFlexItem grow={false} style={{ maxWidth: '100%', overflow: 'hidden' }}>
          <EuiCode style={{ wordBreak: 'break-all' }}>{token}</EuiCode>
        </EuiFlexItem>
        <EuiFlexItem grow={false}>
          <EuiCopy textToCopy={token}>
            {(copy) => (
              <EuiButtonEmpty iconType="copy" size="s" onClick={copy}>
                Copy
              </EuiButtonEmpty>
            )}
          </EuiCopy>
        </EuiFlexItem>
        <EuiFlexItem grow={false}>
          <EuiButtonEmpty size="s" onClick={onDismiss}>
            Dismiss
          </EuiButtonEmpty>
        </EuiFlexItem>
      </EuiFlexGroup>
    </EuiCallOut>
  );
}

export default function ApiKeysModal({ addToast, onClose }: Props) {
  const [apiKeys, setApiKeys] = useState<UserApiKey[]>([]);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);

  const [creatingKey, setCreatingKey] = useState(false);
  const [editingKey, setEditingKey] = useState<UserApiKey | null>(null);
  const [revealedToken, setRevealedToken] = useState<{ token: string; isRegenerate: boolean } | null>(null);
  const [deleteConfirm, setDeleteConfirm] = useState<{ id: string; name: string } | null>(null);
  const [regenerateConfirm, setRegenerateConfirm] = useState<{ id: string; name: string } | null>(null);

  const loadKeys = useCallback(async () => {
    setLoading(true);
    try {
      setApiKeys(await getUserApiKeys());
    } catch {
      addToast({ id: 'load-api-keys-error', color: 'danger', title: 'Failed to load API keys.' });
    } finally {
      setLoading(false);
    }
  }, [addToast]);

  useEffect(() => {
    loadKeys();
  }, [loadKeys]);

  const handleCreate = useCallback(
    async (name: string, expiresAt?: number) => {
      setSaving(true);
      try {
        const result: ApiKeyCreateResponse = await createUserApiKey(name, expiresAt);
        setRevealedToken({ token: result.token, isRegenerate: false });
        setCreatingKey(false);
        await loadKeys();
      } catch (err) {
        addToast({
          id: 'create-api-key-error',
          color: 'danger',
          title: err instanceof Error ? err.message : 'Failed to create API key.',
        });
      } finally {
        setSaving(false);
      }
    },
    [addToast, loadKeys],
  );

  const handleUpdate = useCallback(
    async (id: string, name: string) => {
      setSaving(true);
      try {
        await updateUserApiKey(id, name);
        setEditingKey(null);
        await loadKeys();
        addToast({ id: 'update-api-key', color: 'success', title: 'API key updated.' });
      } catch (err) {
        addToast({
          id: 'update-api-key-error',
          color: 'danger',
          title: err instanceof Error ? err.message : 'Failed to update API key.',
        });
      } finally {
        setSaving(false);
      }
    },
    [addToast, loadKeys],
  );

  const handleDelete = useCallback(
    async (id: string, name: string) => {
      try {
        await deleteUserApiKey(id);
        addToast({ id: 'delete-api-key', color: 'success', title: `API key "${name}" deleted.` });
        await loadKeys();
      } catch {
        addToast({ id: 'delete-api-key-error', color: 'danger', title: `Failed to delete API key "${name}".` });
      }
      setDeleteConfirm(null);
    },
    [addToast, loadKeys],
  );

  const handleRegenerate = useCallback(
    async (id: string, expiresAt?: number) => {
      setSaving(true);
      try {
        const result = await regenerateUserApiKey(id, expiresAt);
        setRevealedToken({ token: result.token, isRegenerate: true });
        setRegenerateConfirm(null);
        await loadKeys();
      } catch (err) {
        addToast({
          id: 'regenerate-api-key-error',
          color: 'danger',
          title: err instanceof Error ? err.message : 'Failed to regenerate API key.',
        });
      } finally {
        setSaving(false);
      }
    },
    [addToast, loadKeys],
  );

  const startCreate = useCallback(() => {
    setRevealedToken(null);
    setCreatingKey(true);
  }, []);

  const startEdit = useCallback((key: UserApiKey) => {
    setRevealedToken(null);
    setEditingKey(key);
  }, []);

  const columns: Array<EuiBasicTableColumn<UserApiKey>> = useMemo(
    () => [
      {
        field: 'name',
        name: 'Name',
        sortable: true,
        render: (name: string) => <EuiText size="s">{name}</EuiText>,
      },
      {
        field: 'expiresAt',
        name: 'Expires',
        sortable: true,
        render: (expiresAt: number | null) => {
          if (expiresAt == null) {
            return <EuiText size="s">Never</EuiText>;
          }
          return <TimestampTableCell timestamp={expiresAt} color={isExpired(expiresAt) ? 'danger' : undefined} />;
        },
      },
      {
        field: 'lastUsedAt',
        name: 'Last used',
        sortable: true,
        render: (lastUsedAt: number | null) => <TimestampTableCell timestamp={lastUsedAt} highlightRecent />,
      },
      {
        field: 'updatedAt',
        name: 'Last updated',
        sortable: true,
        render: (updatedAt: number) => <TimestampTableCell timestamp={updatedAt} />,
      },
      {
        name: 'Actions',
        width: '100px',
        actions: [
          {
            name: 'Edit',
            description: 'Rename API key',
            icon: 'pencil',
            type: 'icon' as const,
            isPrimary: true,
            onClick: startEdit,
          },
          {
            name: 'Regenerate',
            description: 'Regenerate API key token',
            icon: 'refresh',
            type: 'icon' as const,
            onClick: (key: UserApiKey) => setRegenerateConfirm({ id: key.id, name: key.name }),
          },
          {
            name: 'Delete',
            description: 'Delete API key',
            icon: 'trash',
            color: 'danger' as const,
            type: 'icon' as const,
            isPrimary: true,
            onClick: (key: UserApiKey) => setDeleteConfirm({ id: key.id, name: key.name }),
          },
        ],
      },
    ],
    [startEdit],
  );

  return (
    <EuiModal onClose={onClose} style={{ width: 800, minHeight: 480 }}>
      <EuiModalHeader>
        <EuiModalHeaderTitle>API keys</EuiModalHeaderTitle>
      </EuiModalHeader>
      <EuiModalBody>
        {revealedToken && (
          <>
            <TokenReveal
              token={revealedToken.token}
              isRegenerate={revealedToken.isRegenerate}
              onDismiss={() => setRevealedToken(null)}
            />
            <EuiSpacer size="m" />
          </>
        )}
        {loading && apiKeys.length === 0 ? (
          <EuiFlexGroup justifyContent="center">
            <EuiFlexItem grow={false}>
              <EuiLoadingSpinner size="l" />
            </EuiFlexItem>
          </EuiFlexGroup>
        ) : (
          <>
            <EuiFlexGroup justifyContent="flexEnd">
              <EuiFlexItem grow={false}>
                <EuiButton size="s" iconType="plusInCircle" onClick={startCreate}>
                  Create API key
                </EuiButton>
              </EuiFlexItem>
            </EuiFlexGroup>
            <EuiSpacer size="s" />
            <EuiInMemoryTable
              items={apiKeys}
              itemId="id"
              columns={columns}
              loading={loading}
              sorting={{ sort: { field: 'updatedAt', direction: 'desc' } }}
              pagination={apiKeys.length > 10 ? { pageSize: 10, showPerPageOptions: false } : undefined}
              noItemsMessage="No API keys yet."
            />
          </>
        )}
      </EuiModalBody>
      <EuiModalFooter>
        <EuiButtonEmpty onClick={onClose}>Close</EuiButtonEmpty>
      </EuiModalFooter>

      {creatingKey && (
        <CreateEditModal mode="create" saving={saving} onSave={handleCreate} onCancel={() => setCreatingKey(false)} />
      )}

      {editingKey && (
        <CreateEditModal
          mode="edit"
          initialName={editingKey.name}
          saving={saving}
          onSave={(name) => handleUpdate(editingKey.id, name)}
          onCancel={() => setEditingKey(null)}
        />
      )}

      {deleteConfirm && (
        <EuiConfirmModal
          title={`Delete API key "${deleteConfirm.name}"?`}
          onCancel={() => setDeleteConfirm(null)}
          onConfirm={() => handleDelete(deleteConfirm.id, deleteConfirm.name)}
          cancelButtonText="Cancel"
          confirmButtonText="Delete"
          buttonColor="danger"
        >
          This action cannot be undone. Any applications using this key will lose access.
        </EuiConfirmModal>
      )}

      {regenerateConfirm && (
        <RegenerateConfirmModal
          name={regenerateConfirm.name}
          saving={saving}
          onCancel={() => setRegenerateConfirm(null)}
          onConfirm={(expiresAt) => handleRegenerate(regenerateConfirm.id, expiresAt)}
        />
      )}
    </EuiModal>
  );
}
