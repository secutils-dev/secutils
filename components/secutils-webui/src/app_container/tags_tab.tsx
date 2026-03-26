import {
  EuiBadge,
  EuiButton,
  EuiColorPicker,
  EuiConfirmModal,
  EuiEmptyPrompt,
  EuiFieldText,
  EuiFlexGroup,
  EuiFlexItem,
  EuiFormRow,
  EuiIcon,
  EuiInMemoryTable,
  EuiModal,
  EuiModalBody,
  EuiModalFooter,
  EuiModalHeader,
  EuiModalHeaderTitle,
  EuiSpacer,
  EuiText,
} from '@elastic/eui';
import type { EuiBasicTableColumn } from '@elastic/eui';
import { unix } from 'moment/moment';
import { useCallback, useEffect, useState } from 'react';

import type { UserTag } from '../model/user_tags';
import { createUserTag, deleteUserTag, getUserTags, TAG_COLOR_SWATCHES, updateUserTag } from '../model/user_tags';
import type { PageToast } from '../pages/page';

interface EditingTag {
  id?: string;
  name: string;
  color: string;
}

interface DeleteConfirmation {
  id: string;
  name: string;
}

export function TagsTab({ addToast }: { addToast: (toast: PageToast) => void }) {
  const [tags, setTags] = useState<UserTag[]>([]);
  const [loading, setLoading] = useState(true);
  const [editModal, setEditModal] = useState<{ visible: false } | { visible: true; tag: EditingTag }>({
    visible: false,
  });
  const [deleteConfirm, setDeleteConfirm] = useState<DeleteConfirmation | null>(null);
  const [saving, setSaving] = useState(false);

  const loadTags = useCallback(async () => {
    setLoading(true);
    try {
      setTags(await getUserTags());
    } catch {
      addToast({ id: 'load-tags-error', color: 'danger', title: 'Failed to load tags' });
    } finally {
      setLoading(false);
    }
  }, [addToast]);

  useEffect(() => {
    loadTags();
  }, [loadTags]);

  const handleSave = useCallback(
    async (tag: EditingTag) => {
      setSaving(true);
      try {
        if (tag.id) {
          await updateUserTag(tag.id, { name: tag.name.trim().toLowerCase(), color: tag.color });
          addToast({ id: 'update-tag', color: 'success', title: `Tag "${tag.name}" updated` });
        } else {
          await createUserTag(tag.name.trim().toLowerCase(), tag.color);
          addToast({ id: 'create-tag', color: 'success', title: `Tag "${tag.name}" created` });
        }
        await loadTags();
        setEditModal({ visible: false });
      } catch {
        addToast({
          id: `save-tag-error-${tag.name}`,
          color: 'danger',
          title: `Failed to ${tag.id ? 'update' : 'create'} tag "${tag.name}"`,
        });
      } finally {
        setSaving(false);
      }
    },
    [loadTags, addToast],
  );

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
      sortable: true,
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
          onClick: (tag: UserTag) =>
            setEditModal({ visible: true, tag: { id: tag.id, name: tag.name, color: tag.color } }),
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

  return (
    <>
      <EuiFlexGroup justifyContent="flexEnd">
        <EuiFlexItem grow={false}>
          <EuiButton
            iconType="plusInCircle"
            size="s"
            onClick={() => setEditModal({ visible: true, tag: { name: '', color: TAG_COLOR_SWATCHES[0] } })}
            disabled={loading}
          >
            Add tag
          </EuiButton>
        </EuiFlexItem>
      </EuiFlexGroup>
      <EuiSpacer size="m" />
      <EuiInMemoryTable
        items={tags}
        columns={columns}
        loading={loading}
        sorting={{ sort: { field: 'name', direction: 'asc' } }}
        pagination={{ pageSize: 10, showPerPageOptions: true }}
        search={{ box: { placeholder: 'Search tags…', incremental: true } }}
        noItemsMessage={
          <EuiEmptyPrompt
            icon={<EuiIcon type="tag" size="xl" />}
            title={<h3>No tags yet</h3>}
            body="Create tags to organize your responders, trackers, policies, and other items."
          />
        }
      />
      {editModal.visible ? (
        <TagEditModal
          tag={editModal.tag}
          saving={saving}
          onSave={handleSave}
          onClose={() => setEditModal({ visible: false })}
        />
      ) : null}
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

function TagEditModal({
  tag,
  saving,
  onSave,
  onClose,
}: {
  tag: EditingTag;
  saving: boolean;
  onSave: (tag: EditingTag) => void;
  onClose: () => void;
}) {
  const [name, setName] = useState(tag.name);
  const [color, setColor] = useState(tag.color);
  const [colorIsValid, setColorIsValid] = useState(true);
  const isEditing = !!tag.id;
  const nameValid = name.trim().length > 0 && name.trim().length <= 50;

  return (
    <EuiModal onClose={onClose} initialFocus="[name=tagName]" style={{ width: 450, maxWidth: '90vw' }}>
      <EuiModalHeader>
        <EuiModalHeaderTitle>{isEditing ? 'Edit tag' : 'Add tag'}</EuiModalHeaderTitle>
      </EuiModalHeader>
      <EuiModalBody>
        <EuiFormRow
          label="Name"
          helpText="Tag name (1-50 characters). Will be lowercased."
          isInvalid={name.length > 0 && !nameValid}
          error={name.length > 50 ? 'Name must be 50 characters or fewer.' : undefined}
          fullWidth
        >
          <EuiFieldText
            name="tagName"
            value={name}
            onChange={(e) => setName(e.target.value)}
            maxLength={50}
            placeholder="e.g. production, staging, personal"
            fullWidth
          />
        </EuiFormRow>
        <EuiFormRow label="Color" fullWidth>
          <EuiColorPicker
            color={color}
            onChange={(text, { isValid }) => {
              setColor(text);
              setColorIsValid(isValid);
            }}
            swatches={TAG_COLOR_SWATCHES as unknown as string[]}
            isInvalid={!colorIsValid}
          />
        </EuiFormRow>
      </EuiModalBody>
      <EuiModalFooter>
        <EuiFlexGroup justifyContent="flexEnd">
          <EuiFlexItem grow={false}>
            <EuiButton onClick={onClose} color="text">
              Cancel
            </EuiButton>
          </EuiFlexItem>
          <EuiFlexItem grow={false}>
            <EuiButton
              fill
              onClick={() => onSave({ ...tag, name, color })}
              isLoading={saving}
              disabled={!nameValid || !colorIsValid || saving}
            >
              {isEditing ? 'Update' : 'Create'}
            </EuiButton>
          </EuiFlexItem>
        </EuiFlexGroup>
      </EuiModalFooter>
    </EuiModal>
  );
}
