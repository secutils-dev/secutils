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
import { useCallback, useEffect, useLayoutEffect, useMemo, useState } from 'react';

import { TagEditFlyout } from './tag_edit_flyout';
import { PageErrorState, PageLoadingState } from '../../../../components';
import type { UserTag } from '../../../../model';
import { deleteUserTag, getUserTagsPage } from '../../../../model';
import { FilteredEmptyState, ItemsTableFilter, useServerPaginatedItems } from '../../components/items_table_filter';
import { useWorkspaceContext } from '../../hooks';

interface DeleteConfirmation {
  id: string;
  name: string;
}

export default function WorkspaceTags() {
  const { addToast, setTitleActions } = useWorkspaceContext();

  const [initialized, setInitialized] = useState(false);
  const [tagToEdit, setTagToEdit] = useState<Partial<UserTag> | null | undefined>(null);
  const [deleteConfirm, setDeleteConfirm] = useState<DeleteConfirmation | null>(null);

  const createButton = useMemo(
    () => (
      <EuiButton iconType={'plusInCircle'} title="Create a new tag" fill onClick={() => setTagToEdit(undefined)}>
        Add tag
      </EuiButton>
    ),
    [],
  );

  const docsButton = (
    <EuiButtonEmpty
      iconType={'documentation'}
      title="Learn how to use tags"
      target={'_blank'}
      href={'/docs/guides/platform/tags'}
    >
      Learn how to
    </EuiButtonEmpty>
  );

  const {
    items: tags,
    total,
    loading,
    error,
    pagination,
    sorting,
    onTableChange,
    query,
    setQuery,
    hasPageFilters,
    hasActiveFilters,
    clearPageFilters,
    refresh,
  } = useServerPaginatedItems<UserTag>({
    fetcher: getUserTagsPage,
    defaultSortField: 'name',
    defaultSortDirection: 'asc',
    defaultPageSize: 10,
  });

  useEffect(() => {
    if (!loading) {
      setInitialized(true);
    }
  }, [loading]);

  const isEmpty = initialized && total === 0 && !hasActiveFilters;

  // Layout effect (not a passive effect) so the title-bar create button is
  // added/removed in the same commit as the empty-state prompt that renders its
  // own copy of the button. A passive effect lags one paint behind, briefly
  // showing two identical "create" buttons (and tripping Playwright strict mode).
  useLayoutEffect(() => {
    setTitleActions(isEmpty ? null : createButton);
  }, [isEmpty, createButton, setTitleActions]);

  const handleDelete = useCallback(
    async (id: string, name: string) => {
      try {
        await deleteUserTag(id);
        addToast({ id: 'delete-tag', color: 'success', title: `Tag "${name}" deleted` });
        refresh();
      } catch {
        addToast({ id: 'delete-tag-error', color: 'danger', title: `Failed to delete tag "${name}"` });
      }
      setDeleteConfirm(null);
    },
    [refresh, addToast],
  );

  const columns: Array<EuiBasicTableColumn<UserTag>> = [
    {
      field: 'name',
      name: 'Name',
      sortable: true,
      render: (_name: string, tag: UserTag) => <EuiBadge color={tag.color}>{tag.name}</EuiBadge>,
    },
    {
      field: 'color',
      name: 'Color',
      width: '80px',
      render: (color: string) => (
        <span
          style={{
            display: 'inline-block',
            width: 20,
            height: 20,
            borderRadius: 4,
            backgroundColor: color,
            verticalAlign: 'middle',
          }}
          title={color}
        />
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
          description: 'Edit tag',
          icon: 'pencil',
          type: 'icon',
          onClick: (tag: UserTag) => setTagToEdit(tag),
        },
        {
          name: 'Delete',
          description: 'Delete tag',
          icon: 'trash',
          color: 'danger',
          type: 'icon',
          onClick: (tag: UserTag) => setDeleteConfirm({ id: tag.id, name: tag.name }),
        },
      ],
    },
  ];

  if (!initialized && loading) {
    return <PageLoadingState />;
  }

  if (error && tags.length === 0) {
    return <PageErrorState title="Cannot load tags" content={<p>{error}</p>} />;
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
            icon={<EuiIcon type={'tag'} size={'xl'} />}
            title={<h2>No tags yet</h2>}
            titleSize="s"
            style={{ maxWidth: '60em', display: 'flex' }}
            body={
              <div>
                <p>Create tags to organize your responders, trackers, policies, and other items.</p>
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
        <ItemsTableFilter query={query} onQueryChange={setQuery} onRefresh={refresh} placeholder="Search tags…" />
        <EuiSpacer size="m" />
        <EuiBasicTable
          items={tags}
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

  const editFlyout =
    tagToEdit !== null ? (
      <TagEditFlyout
        tag={tagToEdit}
        onClose={(success) => {
          if (success) {
            refresh();
          }
          setTagToEdit(null);
        }}
      />
    ) : null;

  return (
    <>
      {content}
      {editFlyout}
      {deleteConfirm ? (
        <EuiConfirmModal
          title={`Delete tag "${deleteConfirm.name}"?`}
          onCancel={() => setDeleteConfirm(null)}
          onConfirm={() => handleDelete(deleteConfirm.id, deleteConfirm.name)}
          cancelButtonText="Cancel"
          confirmButtonText="Delete"
          buttonColor="danger"
        >
          This tag will be removed from all items that use it. This action cannot be undone.
        </EuiConfirmModal>
      ) : null}
    </>
  );
}
