import {
  EuiBadge,
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

import { TagEditFlyout } from './tag_edit_flyout';
import { PageLoadingState } from '../../../../components';
import type { UserTag } from '../../../../model';
import { deleteUserTag, getUserTags } from '../../../../model';
import { useWorkspaceContext } from '../../hooks';

interface DeleteConfirmation {
  id: string;
  name: string;
}

export default function WorkspaceTags() {
  const { addToast, setTitleActions } = useWorkspaceContext();

  const [tags, setTags] = useState<UserTag[]>([]);
  const [loading, setLoading] = useState(true);
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

  const loadTags = useCallback(async () => {
    setLoading(true);
    try {
      const loaded = await getUserTags();
      setTags(loaded);
      setTitleActions(loaded.length === 0 ? null : createButton);
    } catch {
      addToast({ id: 'load-tags-error', color: 'danger', title: 'Failed to load tags' });
    } finally {
      setLoading(false);
    }
  }, [addToast, createButton, setTitleActions]);

  useEffect(() => {
    loadTags();
  }, [loadTags]);

  const handleDelete = useCallback(
    async (id: string, name: string) => {
      try {
        await deleteUserTag(id);
        addToast({ id: 'delete-tag', color: 'success', title: `Tag "${name}" deleted` });
        await loadTags();
      } catch {
        addToast({ id: 'delete-tag-error', color: 'danger', title: `Failed to delete tag "${name}"` });
      }
      setDeleteConfirm(null);
    },
    [loadTags, addToast],
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

  if (loading && tags.length === 0) {
    return <PageLoadingState />;
  }

  let content;
  if (tags.length === 0) {
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
      <EuiInMemoryTable
        items={tags}
        columns={columns}
        loading={loading}
        sorting={{ sort: { field: 'name', direction: 'asc' } }}
        pagination={{ pageSize: 10, showPerPageOptions: true }}
        search={{ box: { placeholder: 'Search tags…', incremental: true } }}
      />
    );
  }

  const editFlyout =
    tagToEdit !== null ? (
      <TagEditFlyout
        tag={tagToEdit}
        onClose={(success) => {
          if (success) {
            loadTags();
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
