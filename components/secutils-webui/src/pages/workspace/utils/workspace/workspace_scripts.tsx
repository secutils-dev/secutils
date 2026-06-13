import {
  EuiBadge,
  EuiBasicTable,
  EuiButton,
  EuiButtonEmpty,
  EuiConfirmModal,
  EuiEmptyPrompt,
  EuiFlexGroup,
  EuiFlexItem,
  EuiIcon,
  EuiSpacer,
  EuiText,
} from '@elastic/eui';
import type { EuiBasicTableColumn } from '@elastic/eui';
import { unix } from 'moment/moment';
import { lazy, Suspense, useCallback, useEffect, useMemo, useState } from 'react';

const ScriptEditFlyout = lazy(() => import('./script_edit_flyout'));
import { PageErrorState, PageLoadingState } from '../../../../components';
import { useUserTags } from '../../../../hooks';
import { deleteUserScript, getCopyName, getUserScriptsPage, USER_SCRIPT_TYPE_LABELS } from '../../../../model';
import type { PaginationRequest, UserScript, UserScriptType } from '../../../../model';
import { EntityName } from '../../components/entity_name';
import {
  FilteredEmptyState,
  ItemsTableFilter,
  TagsFilter,
  useServerPaginatedItems,
} from '../../components/items_table_filter';
import { useWorkspaceContext } from '../../hooks';

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

export default function WorkspaceScripts() {
  const { addToast, setTitleActions } = useWorkspaceContext();

  const [initialized, setInitialized] = useState(false);
  const [scriptToEdit, setScriptToEdit] = useState<{
    editingId?: string;
    editingName?: string;
    duplicateFrom?: UserScript;
    duplicateSourceId?: string;
    duplicateSourceName?: string;
  } | null>(null);
  const [deleteConfirm, setDeleteConfirm] = useState<DeleteConfirmation | null>(null);
  const { allTags } = useUserTags();

  const createButton = useMemo(
    () => (
      <EuiButton iconType={'plusInCircle'} title="Create a new script" fill onClick={() => setScriptToEdit({})}>
        Add script
      </EuiButton>
    ),
    [],
  );

  const docsButton = (
    <EuiButtonEmpty
      iconType={'documentation'}
      title="Learn how to use scripts"
      target={'_blank'}
      href={'/docs/guides/platform/user_scripts'}
    >
      Learn how to
    </EuiButtonEmpty>
  );

  const fetcher = useCallback((request: PaginationRequest) => getUserScriptsPage(undefined, request), []);
  const {
    items: scripts,
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
  } = useServerPaginatedItems<UserScript>({
    fetcher,
    allTags,
    defaultSortField: 'updatedAt',
    defaultSortDirection: 'desc',
    defaultPageSize: 10,
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

  const handleDelete = useCallback(
    async (id: string, name: string) => {
      try {
        await deleteUserScript(id);
        addToast({ id: 'delete-script', color: 'success', title: `Script "${name}" deleted` });
        refresh();
      } catch {
        addToast({ id: 'delete-script-error', color: 'danger', title: `Failed to delete script "${name}"` });
      }
      setDeleteConfirm(null);
    },
    [refresh, addToast],
  );

  const columns: Array<EuiBasicTableColumn<UserScript>> = [
    {
      field: 'name',
      name: 'Name',
      sortable: true,
      render: (_: string, script: UserScript) => <EntityName name={script.name} tags={script.tags} />,
    },
    {
      field: 'scriptType',
      name: 'Type',
      sortable: true,
      render: (type: UserScriptType) => <EuiBadge color={TYPE_COLOR[type]}>{USER_SCRIPT_TYPE_LABELS[type]}</EuiBadge>,
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
          description: 'Update script content',
          icon: 'pencil',
          type: 'icon',
          isPrimary: true,
          onClick: (script: UserScript) => setScriptToEdit({ editingId: script.id, editingName: script.name }),
        },
        {
          name: 'Duplicate',
          description: 'Duplicate script',
          icon: 'copy',
          type: 'icon',
          onClick: (script: UserScript) =>
            setScriptToEdit({
              duplicateFrom: {
                ...script,
                name: getCopyName(
                  script.name,
                  scripts.map((s) => s.name),
                ),
              },
              duplicateSourceId: script.id,
              duplicateSourceName: script.name,
            }),
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

  if (!initialized && loading) {
    return <PageLoadingState />;
  }

  if (error && scripts.length === 0) {
    return <PageErrorState title="Cannot load scripts" content={<p>{error}</p>} />;
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
            icon={<EuiIcon type={'editorCodeBlock'} size={'xl'} />}
            title={<h2>No scripts yet</h2>}
            titleSize="s"
            style={{ maxWidth: '60em', display: 'flex' }}
            body={
              <div>
                <p>Add reusable scripts to use in your responders and trackers.</p>
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
        <ItemsTableFilter query={query} onQueryChange={setQuery} onRefresh={refresh} placeholder="Search scripts…">
          <TagsFilter tags={allTags} selectedTagIds={selectedTagIds} onSelectedTagIdsChange={setSelectedTagIds} />
        </ItemsTableFilter>
        <EuiSpacer size="m" />
        <EuiBasicTable
          items={scripts}
          columns={columns}
          loading={loading}
          sorting={sorting}
          pagination={pagination}
          onChange={onTableChange}
          noItemsMessage={
            <FilteredEmptyState totalItems={total} hasPageFilters={hasPageFilters} onClearFilters={clearPageFilters} />
          }
        />
      </>
    );
  }

  const editFlyout = scriptToEdit ? (
    <Suspense fallback={null}>
      <ScriptEditFlyout
        editingId={scriptToEdit.editingId}
        editingName={scriptToEdit.editingName}
        duplicateFrom={scriptToEdit.duplicateFrom}
        duplicateSourceId={scriptToEdit.duplicateSourceId}
        duplicateSourceName={scriptToEdit.duplicateSourceName}
        onClose={(success) => {
          if (success) {
            refresh();
          }
          setScriptToEdit(null);
        }}
      />
    </Suspense>
  ) : null;

  return (
    <>
      {content}
      {editFlyout}
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
