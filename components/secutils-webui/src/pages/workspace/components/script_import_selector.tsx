import {
  EuiBadge,
  EuiButton,
  EuiButtonEmpty,
  EuiModal,
  EuiModalBody,
  EuiModalFooter,
  EuiModalHeader,
  EuiModalHeaderTitle,
  EuiSelectable,
  EuiSpacer,
  EuiText,
} from '@elastic/eui';
import { useCallback, useEffect, useState } from 'react';

import type { UserScript } from '../../../model';
import { getUserScript, getUserScripts, USER_SCRIPT_TYPE_LABELS } from '../../../model';

export type ScriptContext = 'responder' | 'api_tracker' | 'page_tracker';

interface ScriptOption {
  label: string;
  checked?: 'on' | undefined;
  scriptId: string;
}

interface ScriptImportSelectorProps {
  context: ScriptContext;
  onSelect: (content: string) => void;
  onClose: () => void;
}

export function ScriptImportSelector({ context, onSelect, onClose }: ScriptImportSelectorProps) {
  const [scripts, setScripts] = useState<UserScript[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selectedScriptId, setSelectedScriptId] = useState<string | null>(null);

  useEffect(() => {
    async function loadScripts() {
      try {
        setScripts(await getUserScripts(context));
      } catch {
        setError('Failed to load scripts.');
      } finally {
        setLoading(false);
      }
    }

    loadScripts();
  }, [context]);

  const handleSelect = useCallback((options: ScriptOption[]) => {
    const selected = options.find((o: ScriptOption) => o.checked === 'on');
    setSelectedScriptId(selected?.scriptId ?? null);
  }, []);

  const handleConfirm = useCallback(async () => {
    if (selectedScriptId) {
      try {
        const script = await getUserScript(selectedScriptId);
        onSelect(script.content);
      } catch {
        setError('Failed to load script content.');
      }
    }
  }, [selectedScriptId, onSelect]);

  const options: ScriptOption[] = scripts.map((script) => ({
    label: `${script.name} (${USER_SCRIPT_TYPE_LABELS[script.scriptType]})`,
    scriptId: script.id,
  }));

  return (
    <EuiModal onClose={onClose} style={{ width: 500 }}>
      <EuiModalHeader>
        <EuiModalHeaderTitle>Import from predefined scripts</EuiModalHeaderTitle>
      </EuiModalHeader>
      <EuiModalBody>
        {loading ? (
          <EuiText>Loading scripts...</EuiText>
        ) : error ? (
          <EuiText color="danger">{error}</EuiText>
        ) : scripts.length === 0 ? (
          <EuiText>
            No compatible scripts found.
            <EuiSpacer size="s" />
            <EuiText size="s" color="subdued">
              Go to <strong>Workspace → Scripts</strong> to create scripts that can be imported here.
            </EuiText>
          </EuiText>
        ) : (
          <>
            <EuiText size="s" color="subdued">
              Select a script to import. Only scripts compatible with this context are shown.
            </EuiText>
            <EuiSpacer size="s" />
            <EuiSelectable<ScriptOption>
              options={options}
              onChange={handleSelect}
              singleSelection
              listProps={{ bordered: true }}
            >
              {(list, search) => (
                <>
                  {search}
                  {list}
                </>
              )}
            </EuiSelectable>
            <EuiSpacer size="s" />
            <EuiText size="xs" color="subdued">
              Compatible types:{' '}
              {context === 'responder' && (
                <>
                  <EuiBadge color="primary">Responder</EuiBadge> <EuiBadge color="accent">Universal</EuiBadge>
                </>
              )}
              {context === 'api_tracker' && (
                <>
                  <EuiBadge color="warning">API Configurator</EuiBadge>{' '}
                  <EuiBadge color="warning">API Extractor</EuiBadge> <EuiBadge color="accent">Universal</EuiBadge>
                </>
              )}
              {context === 'page_tracker' && (
                <>
                  <EuiBadge color="success">Page Extractor</EuiBadge> <EuiBadge color="accent">Universal</EuiBadge>
                </>
              )}
            </EuiText>
          </>
        )}
      </EuiModalBody>
      <EuiModalFooter>
        <EuiButtonEmpty onClick={onClose}>Cancel</EuiButtonEmpty>
        <EuiButton fill onClick={handleConfirm} disabled={!selectedScriptId || loading}>
          Import
        </EuiButton>
      </EuiModalFooter>
    </EuiModal>
  );
}
