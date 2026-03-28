import {
  EuiBadge,
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

import { ScriptEditModal } from './script_edit_modal';
import { useUserTags } from '../hooks';
import {
  createUserScript,
  deleteUserScript,
  getCopyName,
  getUserScripts,
  updateUserScript,
  USER_SCRIPT_TYPE_LABELS,
} from '../model';
import type { UserScript, UserScriptType } from '../model';
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

const TYPE_COLOR: Record<UserScriptType, string> = {
  responder: 'primary',
  api_configurator: 'warning',
  api_extractor: 'warning',
  page_extractor: 'success',
  universal: 'accent',
};

export default function ScriptsTab({ addToast }: { addToast: (toast: PageToast) => void }) {
  const [scripts, setScripts] = useState<UserScript[]>([]);
  const [loading, setLoading] = useState(true);
  const [editModal, setEditModal] = useState<
    | { visible: false }
    | {
        visible: true;
        editingId?: string;
        editingName?: string;
        duplicateFrom?: UserScript;
        duplicateSourceId?: string;
        duplicateSourceName?: string;
      }
  >({
    visible: false,
  });
  const [deleteConfirm, setDeleteConfirm] = useState<DeleteConfirmation | null>(null);
  const { allTags } = useUserTags();

  const loadScripts = useCallback(async () => {
    setLoading(true);
    try {
      setScripts(await getUserScripts());
    } catch {
      addToast({ id: 'load-scripts-error', color: 'danger', title: 'Failed to load scripts' });
    } finally {
      setLoading(false);
    }
  }, [addToast]);

  useEffect(() => {
    loadScripts();
  }, [loadScripts]);

  const handleSave = useCallback(
    async (name: string, scriptType: UserScriptType, content: string, editingId?: string, tagIds?: string[]) => {
      if (editingId !== undefined) {
        await updateUserScript(editingId, content, tagIds);
        addToast({ id: 'update-script', color: 'success', title: `Script "${name}" updated` });
      } else {
        await createUserScript(name, scriptType, content, tagIds);
        addToast({ id: 'create-script', color: 'success', title: `Script "${name}" created` });
      }
      await loadScripts();
    },
    [loadScripts, addToast],
  );

  const handleDuplicate = useCallback(
    (script: UserScript) => {
      setEditModal({
        visible: true,
        duplicateFrom: {
          ...script,
          name: getCopyName(script.name),
        },
        duplicateSourceId: script.id,
        duplicateSourceName: script.name,
      });
    },
    [setEditModal],
  );

  const handleDelete = useCallback(
    async (id: string, name: string) => {
      try {
        await deleteUserScript(id);
        addToast({ id: 'delete-script', color: 'success', title: `Script "${name}" deleted` });
        await loadScripts();
      } catch {
        addToast({ id: 'delete-script-error', color: 'danger', title: `Failed to delete script "${name}"` });
      }
      setDeleteConfirm(null);
    },
    [loadScripts, addToast],
  );

  const getSearchFields = useCallback(
    (script: UserScript) => [script.name, script.id, USER_SCRIPT_TYPE_LABELS[script.scriptType]],
    [],
  );
  const getItemTags = useCallback((script: UserScript) => script.tags, []);
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
    items: scripts,
    allTags,
    getSearchFields,
    getItemTags,
  });

  const columns: Array<EuiBasicTableColumn<UserScript>> = [
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
      field: 'scriptType',
      name: 'Type',
      sortable: true,
      render: (type: UserScriptType) => <EuiBadge color={TYPE_COLOR[type]}>{USER_SCRIPT_TYPE_LABELS[type]}</EuiBadge>,
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
          description: 'Update script content',
          icon: 'pencil',
          type: 'icon',
          isPrimary: true,
          onClick: (script: UserScript) =>
            setEditModal({ visible: true, editingId: script.id, editingName: script.name }),
        },
        {
          name: 'Duplicate',
          description: 'Duplicate script',
          icon: 'copy',
          type: 'icon',
          onClick: handleDuplicate,
        },
        {
          name: 'Delete',
          description: 'Delete script',
          icon: 'trash',
          color: 'danger',
          type: 'icon',
          isPrimary: true,
          onClick: (script: UserScript) => setDeleteConfirm({ id: script.id, name: script.name }),
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
            Add script
          </EuiButton>
        </EuiFlexItem>
      </EuiFlexGroup>
      <EuiSpacer size="m" />
      <ItemsTableFilter query={query} onQueryChange={setQuery} onRefresh={loadScripts} placeholder="Search scripts…">
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
              icon={<EuiIcon type="code" size="xl" />}
              title={<h3>No scripts yet</h3>}
              body="Add reusable scripts to use in your responders and trackers."
            />
          )
        }
      />
      {editModal.visible ? (
        <ScriptEditModal
          editingId={editModal.editingId}
          editingName={editModal.editingName}
          duplicateFrom={editModal.duplicateFrom}
          duplicateSourceId={editModal.duplicateSourceId}
          duplicateSourceName={editModal.duplicateSourceName}
          onSave={handleSave}
          onClose={() => setEditModal({ visible: false })}
          addToast={addToast}
        />
      ) : null}
      {deleteConfirm ? (
        <EuiConfirmModal
          title={`Delete script "${deleteConfirm.name}"?`}
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
