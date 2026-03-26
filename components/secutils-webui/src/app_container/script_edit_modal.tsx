import {
  EuiButton,
  EuiButtonEmpty,
  EuiFieldText,
  EuiFormRow,
  EuiModal,
  EuiModalBody,
  EuiModalFooter,
  EuiModalHeader,
  EuiModalHeaderTitle,
  EuiSelect,
  useEuiTheme,
} from '@elastic/eui';
import { useCallback, useEffect, useState } from 'react';

import { useUserTags } from '../hooks';
import { getUserScript, USER_SCRIPT_TYPE_OPTIONS } from '../model';
import type { UserScript, UserScriptType } from '../model';
import type { PageToast } from '../pages/page';
import { ScriptEditor } from '../pages/workspace/components/script_editor';
import { TagsComboBox } from '../pages/workspace/components/tags_combo_box';

const MAX_CONTENT_LENGTH = 50 * 1024;

export interface ScriptEditModalProps {
  editingId?: string;
  editingName?: string;
  duplicateFrom?: UserScript;
  duplicateSourceId?: string;
  duplicateSourceName?: string;
  onSave: (
    name: string,
    scriptType: UserScriptType,
    content: string,
    editingId?: string,
    tagIds?: string[],
  ) => Promise<void>;
  onClose: () => void;
  addToast?: (toast: PageToast) => void;
}

export function ScriptEditModal({
  editingId,
  editingName,
  duplicateFrom,
  duplicateSourceId,
  duplicateSourceName,
  onSave,
  onClose,
  addToast,
}: ScriptEditModalProps) {
  const { euiTheme } = useEuiTheme();
  const isEditing = editingName !== undefined;
  const isDuplicate = duplicateFrom !== undefined;
  const [name, setName] = useState(editingName ?? duplicateFrom?.name ?? '');
  const [scriptType, setScriptType] = useState<UserScriptType>(duplicateFrom?.scriptType ?? 'responder');
  const [content, setContent] = useState('');
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(isEditing || isDuplicate);
  const { allTags, setAllTags } = useUserTags();
  const [selectedTagIds, setSelectedTagIds] = useState<string[]>(duplicateFrom?.tags?.map((t) => t.id) ?? []);

  // Load script content when editing or duplicating
  useEffect(() => {
    if (!isEditing && !isDuplicate) {
      return;
    }

    async function loadScript() {
      try {
        const scriptId = isEditing ? editingId! : (duplicateSourceId ?? duplicateFrom!.id);
        const script = await getUserScript(scriptId);
        setScriptType(script.scriptType);
        setContent(script.content);
        setSelectedTagIds(script.tags?.map((t) => t.id) ?? []);
      } catch {
        if (isEditing) {
          setError('Failed to load script content.');
        } else if (isDuplicate && addToast) {
          addToast({
            id: 'duplicate-script-error',
            color: 'danger',
            title: `Failed to load script "${duplicateSourceName ?? duplicateFrom!.name}" to duplicate`,
          });
          onClose();
        }
      } finally {
        setLoading(false);
      }
    }

    loadScript();
  }, [isEditing, isDuplicate, editingId, duplicateFrom, duplicateSourceId, duplicateSourceName, addToast, onClose]);

  const nameValid = name.trim().length > 0 && name.length <= 128;
  const contentTooLong = content.length > MAX_CONTENT_LENGTH;
  const contentValid = content.length > 0 && !contentTooLong;

  const handleSave = useCallback(async () => {
    setSaving(true);
    setError(null);
    try {
      const normalizedName = name.trim();
      await onSave(normalizedName, scriptType, content, editingId, selectedTagIds);
      onClose();
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to save script.';
      addToast?.({ id: `failed-save-script-${name}`, iconType: 'warning', color: 'danger', title: message });
    } finally {
      setSaving(false);
    }
  }, [name, scriptType, content, editingId, selectedTagIds, onSave, onClose, addToast]);

  return (
    <EuiModal onClose={onClose} initialFocus="[name=scriptName]" style={{ width: 600, maxWidth: '90vw' }}>
      <EuiModalHeader>
        <EuiModalHeaderTitle>
          {isEditing ? 'Update script' : isDuplicate ? 'Duplicate script' : 'Add script'}
        </EuiModalHeaderTitle>
      </EuiModalHeader>
      <EuiModalBody>
        <EuiFormRow
          label="Name"
          helpText="Use any non-empty name (up to 128 characters)."
          isInvalid={name.length > 0 && !nameValid}
          error={
            name.length > 0 && !nameValid
              ? name.trim().length === 0
                ? 'Name cannot be empty.'
                : 'Name must be 128 characters or fewer.'
              : undefined
          }
          fullWidth
          style={{ minHeight: '90px' }}
        >
          <EuiFieldText
            name="scriptName"
            value={name}
            onChange={(e) => setName(e.target.value)}
            disabled={isEditing}
            maxLength={128}
            placeholder="MY_SCRIPT"
            fullWidth
          />
        </EuiFormRow>
        <TagsComboBox
          allTags={allTags}
          selectedTagIds={selectedTagIds}
          onChange={setSelectedTagIds}
          onTagCreated={(tag) => setAllTags((prev) => [...prev, tag])}
        />

        <EuiFormRow
          label="Type"
          helpText="Determines where this script can be imported."
          isInvalid={!isEditing && loading}
          fullWidth
        >
          <EuiSelect
            options={USER_SCRIPT_TYPE_OPTIONS}
            value={scriptType}
            onChange={(e) => setScriptType(e.target.value as UserScriptType)}
            disabled={isEditing || loading}
            fullWidth
          />
        </EuiFormRow>

        <EuiFormRow
          label="Script"
          isInvalid={contentTooLong || !!error}
          error={
            contentTooLong
              ? `Content is too long (${(content.length / 1024).toFixed(1)} KB). Maximum allowed size is 50 KB.`
              : (error ?? undefined)
          }
          fullWidth
          style={{ minHeight: '340px' }}
        >
          {loading ? (
            <div style={{ height: '300px', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
              Loading...
            </div>
          ) : (
            <ScriptEditor
              onChange={(value) => setContent(value ?? '')}
              defaultValue={content}
              overlayZIndex={Number(euiTheme.levels.modal) + 100}
            />
          )}
        </EuiFormRow>
      </EuiModalBody>
      <EuiModalFooter>
        <EuiButtonEmpty onClick={onClose}>Cancel</EuiButtonEmpty>
        <EuiButton
          fill
          onClick={handleSave}
          isLoading={saving}
          disabled={!nameValid || !contentValid || saving || loading}
        >
          {isEditing ? 'Update' : 'Create'}
        </EuiButton>
      </EuiModalFooter>
    </EuiModal>
  );
}
