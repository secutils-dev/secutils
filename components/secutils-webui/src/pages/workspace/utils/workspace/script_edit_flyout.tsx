import { EuiDescribedFormGroup, EuiFieldText, EuiForm, EuiFormRow, EuiSelect, useEuiTheme } from '@elastic/eui';
import { useCallback, useEffect, useState } from 'react';

import { useUserTags } from '../../../../hooks';
import { createUserScript, getUserScript, updateUserScript, USER_SCRIPT_TYPE_OPTIONS } from '../../../../model';
import type { AsyncData, UserScript, UserScriptType } from '../../../../model';
import { EditorFlyout } from '../../components/editor_flyout';
import { ScriptEditor } from '../../components/script_editor';
import { TagsComboBox } from '../../components/tags_combo_box';
import { useWorkspaceContext } from '../../hooks';

const MAX_CONTENT_LENGTH = 50 * 1024;

export interface ScriptEditFlyoutProps {
  editingId?: string;
  editingName?: string;
  duplicateFrom?: UserScript;
  duplicateSourceId?: string;
  duplicateSourceName?: string;
  onClose: (success?: boolean) => void;
}

export default function ScriptEditFlyout({
  editingId,
  editingName,
  duplicateFrom,
  duplicateSourceId,
  duplicateSourceName,
  onClose,
}: ScriptEditFlyoutProps) {
  const { addToast } = useWorkspaceContext();
  const { euiTheme } = useEuiTheme();

  const isEditing = editingName !== undefined;
  const isDuplicate = duplicateFrom !== undefined;
  const [name, setName] = useState(editingName ?? duplicateFrom?.name ?? '');
  const [scriptType, setScriptType] = useState<UserScriptType>(duplicateFrom?.scriptType ?? 'responder');
  const [content, setContent] = useState('');
  const [loading, setLoading] = useState(isEditing || isDuplicate);
  const { allTags, setAllTags } = useUserTags();
  const [selectedTagIds, setSelectedTagIds] = useState<string[]>(duplicateFrom?.tags?.map((t) => t.id) ?? []);
  const [updatingStatus, setUpdatingStatus] = useState<AsyncData<void>>();

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
        addToast({
          id: 'load-script-error',
          color: 'danger',
          title: `Failed to load script "${isEditing ? editingName : (duplicateSourceName ?? duplicateFrom!.name)}"`,
        });
        if (!isEditing) {
          onClose();
        }
      } finally {
        setLoading(false);
      }
    }

    loadScript();
  }, [
    isEditing,
    isDuplicate,
    editingId,
    editingName,
    duplicateFrom,
    duplicateSourceId,
    duplicateSourceName,
    addToast,
    onClose,
  ]);

  const nameValid = name.trim().length > 0 && name.length <= 128;
  const contentTooLong = content.length > MAX_CONTENT_LENGTH;
  const contentValid = content.length > 0 && !contentTooLong;

  const [formBaseline, setFormBaseline] = useState<string | null>(null);
  useEffect(() => {
    if (!loading && formBaseline === null) {
      setFormBaseline(JSON.stringify({ name, scriptType, content, selectedTagIds }));
    }
  }, [loading, formBaseline, name, scriptType, content, selectedTagIds]);
  const hasFormChanges =
    formBaseline !== null && JSON.stringify({ name, scriptType, content, selectedTagIds }) !== formBaseline;
  const hasChanges = isDuplicate || hasFormChanges;

  const handleSave = useCallback(() => {
    if (updatingStatus?.status === 'pending') {
      return;
    }

    setUpdatingStatus({ status: 'pending' });

    const trimmedName = name.trim();
    const promise = isEditing
      ? updateUserScript(editingId!, content, selectedTagIds)
      : createUserScript(trimmedName, scriptType, content, selectedTagIds);

    promise
      .then(() => {
        setUpdatingStatus({ status: 'succeeded', data: undefined });
        addToast({
          id: `success-save-script-${trimmedName}`,
          iconType: 'check',
          color: 'success',
          title: `Script "${trimmedName}" ${isEditing ? 'updated' : 'created'}`,
        });
        onClose(true);
      })
      .catch((err: Error) => {
        setUpdatingStatus({ status: 'failed', error: err.message });
        addToast({
          id: `failed-save-script-${trimmedName}`,
          iconType: 'warning',
          color: 'danger',
          title: err.message,
        });
      });
  }, [updatingStatus, name, scriptType, content, selectedTagIds, isEditing, editingId, addToast, onClose]);

  return (
    <EditorFlyout
      title={`${isEditing ? 'Edit' : isDuplicate ? 'Duplicate' : 'Add'} script`}
      onClose={() => onClose()}
      hasChanges={hasChanges}
      onSave={handleSave}
      canSave={nameValid && contentValid && !loading && (!isEditing || hasFormChanges)}
      saveInProgress={updatingStatus?.status === 'pending'}
    >
      <EuiForm id="update-form" component="form" fullWidth>
        <EuiDescribedFormGroup title={<h3>General</h3>} description="General properties of the script">
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
          >
            <EuiFieldText
              name="scriptName"
              value={name}
              onChange={(e) => setName(e.target.value)}
              disabled={isEditing}
              maxLength={128}
              placeholder="MY_SCRIPT"
            />
          </EuiFormRow>
          <EuiFormRow label="Type" helpText="Determines where this script can be imported.">
            <EuiSelect
              options={USER_SCRIPT_TYPE_OPTIONS}
              value={scriptType}
              onChange={(e) => setScriptType(e.target.value as UserScriptType)}
              disabled={isEditing || loading}
            />
          </EuiFormRow>
          <TagsComboBox
            allTags={allTags}
            selectedTagIds={selectedTagIds}
            onChange={setSelectedTagIds}
            onTagCreated={(tag) => setAllTags((prev) => [...prev, tag])}
          />
        </EuiDescribedFormGroup>
        <EuiDescribedFormGroup title={<h3>Content</h3>} description="Script source code">
          <EuiFormRow
            label="Script"
            isInvalid={contentTooLong}
            error={
              contentTooLong
                ? `Content is too long (${(content.length / 1024).toFixed(1)} KB). Maximum allowed size is 50 KB.`
                : undefined
            }
            fullWidth
          >
            {loading ? (
              <div style={{ height: '300px', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                Loading…
              </div>
            ) : (
              <ScriptEditor
                onChange={(val) => setContent(val ?? '')}
                defaultValue={content}
                overlayZIndex={Number(euiTheme.levels.flyout) + 100}
              />
            )}
          </EuiFormRow>
        </EuiDescribedFormGroup>
      </EuiForm>
    </EditorFlyout>
  );
}
