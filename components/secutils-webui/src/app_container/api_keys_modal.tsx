import type { EuiBasicTableColumn } from '@elastic/eui';
import {
  EuiButton,
  EuiButtonEmpty,
  EuiCallOut,
  EuiCode,
  EuiConfirmModal,
  EuiCopy,
  EuiDatePicker,
  EuiFieldText,
  EuiFlexGroup,
  EuiFlexItem,
  EuiFormRow,
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
import type { Moment } from 'moment/moment';
import moment from 'moment/moment';
import type { ReactNode } from 'react';
import { useCallback, useEffect, useMemo, useState } from 'react';

import type { ApiKeyCreateResponse, UserApiKey } from '../model';
import { createUserApiKey, deleteUserApiKey, getUserApiKeys, regenerateUserApiKey, updateUserApiKey } from '../model';
import type { PageToast } from '../pages/page';
import { TimestampTableCell } from '../pages/workspace/components/timestamp_table_cell';

interface Props {
  addToast: (toast: PageToast) => void;
  onClose: () => void;
}

const NEW_KEY_SENTINEL = '__new__';
const MAX_NAME_LENGTH = 128;

function isExpired(expiresAt: number | null) {
  return expiresAt != null && expiresAt < Date.now() / 1000;
}

interface InlineFormProps {
  initialName?: string;
  showExpires?: boolean;
  saving: boolean;
  onSave: (name: string, expiresAt?: number) => void;
  onCancel: () => void;
}

function InlineForm({ initialName = '', showExpires, saving, onSave, onCancel }: InlineFormProps) {
  const [name, setName] = useState(initialName);
  const [expiresDate, setExpiresDate] = useState<Moment | null>(null);

  const nameValid = name.trim().length > 0 && name.length <= MAX_NAME_LENGTH;
  const expiresValid = !expiresDate || expiresDate.isAfter(moment());
  const canSave = nameValid && expiresValid && !saving;

  return (
    <div style={{ padding: '8px 0' }}>
      <EuiFlexGroup gutterSize="s" alignItems="flexEnd" responsive={false} wrap>
        <EuiFlexItem grow={2}>
          <EuiFormRow label="Name">
            <EuiFieldText
              compressed
              placeholder="e.g. CI deployment key"
              value={name}
              maxLength={MAX_NAME_LENGTH}
              onChange={(e) => setName(e.target.value)}
              autoFocus
            />
          </EuiFormRow>
        </EuiFlexItem>
        {showExpires && (
          <EuiFlexItem grow={2}>
            <EuiFormRow label="Expires">
              <EuiDatePicker
                selected={expiresDate}
                onChange={setExpiresDate}
                minDate={moment()}
                showTimeSelect
                compressed
                placeholder="Never"
                isInvalid={!!expiresDate && !expiresValid}
              />
            </EuiFormRow>
          </EuiFlexItem>
        )}
        <EuiFlexItem grow={false}>
          <EuiFormRow hasEmptyLabelSpace>
            <EuiFlexGroup gutterSize="s" responsive={false}>
              <EuiFlexItem grow={false}>
                <EuiButton
                  size="s"
                  fill
                  disabled={!canSave}
                  isLoading={saving}
                  onClick={() => {
                    onSave(name.trim(), expiresDate ? expiresDate.unix() : undefined);
                  }}
                >
                  Save
                </EuiButton>
              </EuiFlexItem>
              <EuiFlexItem grow={false}>
                <EuiButtonEmpty size="s" disabled={saving} onClick={onCancel}>
                  Cancel
                </EuiButtonEmpty>
              </EuiFlexItem>
            </EuiFlexGroup>
          </EuiFormRow>
        </EuiFlexItem>
      </EuiFlexGroup>
    </div>
  );
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

  const [expandedRowId, setExpandedRowId] = useState<string | null>(null);
  const [isCreating, setIsCreating] = useState(false);
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
        setIsCreating(false);
        setExpandedRowId(null);
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
        setExpandedRowId(null);
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

  const cancelEditing = useCallback(() => {
    setExpandedRowId(null);
    setIsCreating(false);
  }, []);

  const startCreate = useCallback(() => {
    setRevealedToken(null);
    setExpandedRowId(NEW_KEY_SENTINEL);
    setIsCreating(true);
  }, []);

  const startEdit = useCallback((key: UserApiKey) => {
    setRevealedToken(null);
    setIsCreating(false);
    setExpandedRowId(key.id);
  }, []);

  const expandedRowMap = useMemo(() => {
    const map: Record<string, ReactNode> = {};
    if (expandedRowId === NEW_KEY_SENTINEL && isCreating) {
      map[NEW_KEY_SENTINEL] = <InlineForm showExpires saving={saving} onSave={handleCreate} onCancel={cancelEditing} />;
    } else if (expandedRowId && !isCreating) {
      const key = apiKeys.find((k) => k.id === expandedRowId);
      if (key) {
        map[key.id] = (
          <InlineForm
            initialName={key.name}
            saving={saving}
            onSave={(name) => handleUpdate(key.id, name)}
            onCancel={cancelEditing}
          />
        );
      }
    }
    return map;
  }, [expandedRowId, isCreating, saving, apiKeys, handleCreate, handleUpdate, cancelEditing]);

  const items = useMemo(() => {
    if (isCreating) {
      const placeholder: UserApiKey = {
        id: NEW_KEY_SENTINEL,
        name: '',
        createdAt: 0,
        updatedAt: 0,
        expiresAt: null,
        lastUsedAt: null,
      };
      return [placeholder, ...apiKeys];
    }
    return apiKeys;
  }, [apiKeys, isCreating]);

  const columns: Array<EuiBasicTableColumn<UserApiKey>> = useMemo(
    () => [
      {
        field: 'name',
        name: 'Name',
        sortable: true,
        render: (name: string, key: UserApiKey) => {
          if (key.id === NEW_KEY_SENTINEL) {
            return (
              <EuiText size="s" color="subdued">
                <i>New API key</i>
              </EuiText>
            );
          }
          return <EuiText size="s">{name}</EuiText>;
        },
      },
      {
        field: 'expiresAt',
        name: 'Expires',
        sortable: true,
        render: (expiresAt: number | null, key: UserApiKey) => {
          if (key.id === NEW_KEY_SENTINEL) {
            return null;
          }
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
        render: (lastUsedAt: number | null, key: UserApiKey) => {
          if (key.id === NEW_KEY_SENTINEL) {
            return null;
          }
          return <TimestampTableCell timestamp={lastUsedAt} highlightRecent />;
        },
      },
      {
        field: 'updatedAt',
        name: 'Last updated',
        sortable: true,
        render: (updatedAt: number, key: UserApiKey) => {
          if (key.id === NEW_KEY_SENTINEL) {
            return null;
          }
          return <TimestampTableCell timestamp={updatedAt} />;
        },
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
            available: (key: UserApiKey) => key.id !== NEW_KEY_SENTINEL,
            onClick: startEdit,
          },
          {
            name: 'Regenerate',
            description: 'Regenerate API key token',
            icon: 'refresh',
            type: 'icon' as const,
            available: (key: UserApiKey) => key.id !== NEW_KEY_SENTINEL,
            onClick: (key: UserApiKey) => setRegenerateConfirm({ id: key.id, name: key.name }),
          },
          {
            name: 'Delete',
            description: 'Delete API key',
            icon: 'trash',
            color: 'danger' as const,
            type: 'icon' as const,
            isPrimary: true,
            available: (key: UserApiKey) => key.id !== NEW_KEY_SENTINEL,
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
                <EuiButton size="s" iconType="plusInCircle" disabled={isCreating} onClick={startCreate}>
                  Create API key
                </EuiButton>
              </EuiFlexItem>
            </EuiFlexGroup>
            <EuiSpacer size="s" />
            <EuiInMemoryTable
              items={items}
              itemId="id"
              columns={columns}
              loading={loading}
              sorting={{ sort: { field: 'updatedAt', direction: 'desc' } }}
              itemIdToExpandedRowMap={expandedRowMap}
              pagination={apiKeys.length > 10 ? { pageSize: 10, showPerPageOptions: false } : undefined}
              noItemsMessage="No API keys yet."
            />
          </>
        )}
      </EuiModalBody>
      <EuiModalFooter>
        <EuiButtonEmpty onClick={onClose}>Close</EuiButtonEmpty>
      </EuiModalFooter>

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

interface RegenerateConfirmModalProps {
  name: string;
  saving: boolean;
  onCancel: () => void;
  onConfirm: (expiresAt?: number) => void;
}

function RegenerateConfirmModal({ name, saving, onCancel, onConfirm }: RegenerateConfirmModalProps) {
  const [expiresDate, setExpiresDate] = useState<Moment | null>(null);
  const expiresValid = !expiresDate || expiresDate.isAfter(moment());

  return (
    <EuiConfirmModal
      title={`Regenerate API key "${name}"?`}
      onCancel={onCancel}
      onConfirm={() => onConfirm(expiresDate ? expiresDate.unix() : undefined)}
      cancelButtonText="Cancel"
      confirmButtonText="Regenerate"
      buttonColor="warning"
      confirmButtonDisabled={!expiresValid || saving}
      isLoading={saving}
    >
      <p>The current token will be immediately invalidated. A new token will be generated.</p>
      <EuiFormRow label="New expiration">
        <EuiDatePicker
          selected={expiresDate}
          onChange={setExpiresDate}
          minDate={moment()}
          showTimeSelect
          placeholder="Never"
          isInvalid={!!expiresDate && !expiresValid}
        />
      </EuiFormRow>
    </EuiConfirmModal>
  );
}
