import type { UseEuiTheme } from '@elastic/eui';
import {
  EuiButton,
  EuiButtonIcon,
  EuiCodeBlock,
  EuiFlexGroup,
  EuiFlexItem,
  EuiFocusTrap,
  EuiModal,
  EuiModalBody,
  EuiModalFooter,
  EuiModalHeader,
  EuiModalHeaderTitle,
  EuiPanel,
  EuiSpacer,
  EuiText,
  EuiTextArea,
  useEuiTheme,
} from '@elastic/eui';
import { css } from '@emotion/react';
import { Editor } from '@monaco-editor/react';
import { useCallback, useEffect, useMemo, useState } from 'react';
import type { ChangeEvent, ReactNode } from 'react';
import { createPortal } from 'react-dom';

import type { monaco } from '../../../tools/monaco_setup';
import { monacoTypescript } from '../../../tools/monaco_setup';

export function createTheme({ euiTheme }: UseEuiTheme, backgroundColor?: string): monaco.editor.IStandaloneThemeData {
  return {
    base: 'vs',
    inherit: true,
    rules: [
      {
        token: '',
        foreground: euiTheme.colors.textParagraph,
        background: euiTheme.colors.backgroundBaseSubdued,
      },
      { token: 'invalid', foreground: euiTheme.colors.textAccent },
      { token: 'emphasis', fontStyle: 'italic' },
      { token: 'strong', fontStyle: 'bold' },

      { token: 'variable', foreground: euiTheme.colors.textPrimary },
      { token: 'variable.predefined', foreground: euiTheme.colors.textSuccess },
      { token: 'constant', foreground: euiTheme.colors.textAccent },
      { token: 'comment', foreground: euiTheme.colors.textSubdued },
      { token: 'number', foreground: euiTheme.colors.textAccent },
      { token: 'number.hex', foreground: euiTheme.colors.textAccent },
      { token: 'regexp', foreground: euiTheme.colors.textDanger },
      { token: 'annotation', foreground: euiTheme.colors.textSubdued },
      { token: 'type', foreground: euiTheme.colors.textSuccess },

      { token: 'delimiter', foreground: euiTheme.colors.textSubdued },
      { token: 'delimiter.html', foreground: euiTheme.colors.textParagraph },
      { token: 'delimiter.xml', foreground: euiTheme.colors.textPrimary },

      { token: 'tag', foreground: euiTheme.colors.textDanger },
      { token: 'metatag', foreground: euiTheme.colors.textSuccess },
      { token: 'metatag.content.html', foreground: euiTheme.colors.textDanger },
      { token: 'metatag.html', foreground: euiTheme.colors.textDanger },
      { token: 'metatag.xml', foreground: euiTheme.colors.textSubdued },

      { token: 'key', foreground: euiTheme.colors.textWarning },
      { token: 'string.key.json', foreground: euiTheme.colors.textDanger },
      { token: 'string.value.json', foreground: euiTheme.colors.textPrimary },

      { token: 'attribute.name', foreground: euiTheme.colors.textDanger },
      { token: 'attribute.name.css', foreground: euiTheme.colors.textSuccess },
      { token: 'attribute.value', foreground: euiTheme.colors.textPrimary },
      { token: 'attribute.value.number', foreground: euiTheme.colors.textWarning },
      { token: 'attribute.value.unit', foreground: euiTheme.colors.textWarning },
      { token: 'attribute.value.html', foreground: euiTheme.colors.textPrimary },
      { token: 'attribute.value.xml', foreground: euiTheme.colors.textPrimary },

      { token: 'string', foreground: euiTheme.colors.textDanger },
      { token: 'string.html', foreground: euiTheme.colors.textPrimary },

      { token: 'keyword', foreground: euiTheme.colors.textPrimary },
      { token: 'keyword.json', foreground: euiTheme.colors.textPrimary },
      { token: 'keyword.deprecated', foreground: euiTheme.colors.textAccent },

      { token: 'text', foreground: euiTheme.colors.textHeading },
      { token: 'label', foreground: euiTheme.colors.vis.euiColorVis9 },
    ],
    colors: {
      'editor.foreground': euiTheme.colors.textParagraph,
      'editor.background': backgroundColor ?? euiTheme.colors.backgroundBasePlain,
      'editorLineNumber.foreground': euiTheme.colors.textSubdued,
      'editorLineNumber.activeForeground': euiTheme.colors.textSubdued,
      'editorIndentGuide.background1': euiTheme.colors.lightShade,
      'editor.selectionBackground': euiTheme.colors.backgroundBaseInteractiveSelect,
      'editorWidget.border': euiTheme.colors.borderBasePlain,
      'editorWidget.background': euiTheme.colors.backgroundBaseSubdued,
      'editorCursor.foreground': euiTheme.colors.darkestShade,
      'editorSuggestWidget.selectedForeground': euiTheme.colors.darkestShade,
      'editorSuggestWidget.focusHighlightForeground': euiTheme.colors.primary,
      'editorSuggestWidget.selectedBackground': euiTheme.colors.lightShade,
      'list.hoverBackground': euiTheme.colors.backgroundBaseSubdued,
      'list.highlightForeground': euiTheme.colors.primary,
      'editor.lineHighlightBorder': euiTheme.colors.lightestShade,
      'editorHoverWidget.foreground': euiTheme.colors.darkestShade,
      'editorHoverWidget.background': euiTheme.colors.backgroundBaseSubdued,
    },
  };
}

export interface ExtraLib {
  content: string;
  filePath?: string;
}

export interface ScriptSnippet {
  id: string;
  label: string;
  template: string;
}

export interface ImportAction {
  id: string;
  label: string;
  description: ReactNode;
  transform: (input: string) => string;
}

let extraLibsConfigured = false;

function registerExtraLibs(extraLibs?: ExtraLib[]) {
  if (!extraLibs?.length) {
    return;
  }

  for (const lib of extraLibs) {
    monacoTypescript.javascriptDefaults.addExtraLib(lib.content, lib.filePath);
  }

  if (!extraLibsConfigured) {
    extraLibsConfigured = true;
    monacoTypescript.javascriptDefaults.setCompilerOptions({
      ...monacoTypescript.javascriptDefaults.getCompilerOptions(),
      allowJs: true,
      checkJs: true,
      target: monacoTypescript.ScriptTarget.ESNext,
      module: monacoTypescript.ModuleKind.ESNext,
    });
    monacoTypescript.javascriptDefaults.setDiagnosticsOptions({
      noSemanticValidation: true,
    });
  }
}

export interface Props {
  onChange: (scriptContent?: string) => void;
  defaultValue?: string;
  extraLibs?: ExtraLib[];
  language?: string;
  snippets?: ScriptSnippet[];
  importActions?: ImportAction[];
}

const EDITOR_OPTIONS: monaco.editor.IStandaloneEditorConstructionOptions = {
  mouseWheelZoom: true,
  scrollbar: { verticalScrollbarSize: 14, horizontal: 'hidden' },
  wordWrap: 'on',
  minimap: { enabled: false },
};

const FULLSCREEN_EDITOR_OPTIONS: monaco.editor.IStandaloneEditorConstructionOptions = {
  ...EDITOR_OPTIONS,
  minimap: { enabled: true },
};

function registerSnippetActions(editor: monaco.editor.IStandaloneCodeEditor, snippets?: ScriptSnippet[]) {
  (snippets ?? []).forEach((snippet, idx) => {
    editor.addAction({
      id: `insert-snippet-${snippet.id}`,
      label: snippet.label,
      contextMenuGroupId: '0_template',
      contextMenuOrder: idx,
      run: (ed) => {
        ed.setValue(snippet.template);
      },
    });
  });
}

function registerImportActions(
  editor: monaco.editor.IStandaloneCodeEditor,
  importActions: ImportAction[] | undefined,
  onRequestImport: (action: ImportAction) => void,
) {
  (importActions ?? []).forEach((action, idx) => {
    editor.addAction({
      id: `import-${action.id}`,
      label: action.label,
      contextMenuGroupId: '1_import',
      contextMenuOrder: idx,
      run: () => onRequestImport(action),
    });
  });
}

interface ScriptImportModalProps {
  action: ImportAction;
  onConfirm: (transformedScript: string) => void;
  onClose: () => void;
}

function ScriptImportModal({ action, onConfirm, onClose }: ScriptImportModalProps) {
  const [rawInput, setRawInput] = useState('');

  const { preview, error } = useMemo(() => {
    if (!rawInput.trim()) {
      return { preview: null, error: null };
    }
    try {
      return { preview: action.transform(rawInput), error: null };
    } catch (err) {
      return { preview: null, error: err instanceof Error ? err.message : String(err) };
    }
  }, [rawInput, action]);

  const handleInputChange = useCallback((e: ChangeEvent<HTMLTextAreaElement>) => {
    setRawInput(e.target.value);
  }, []);

  const handleConfirm = useCallback(() => {
    if (preview) {
      onConfirm(preview);
    }
  }, [preview, onConfirm]);

  return (
    <EuiModal onClose={onClose} style={{ maxWidth: 720 }}>
      <EuiModalHeader>
        <EuiModalHeaderTitle>{action.label}</EuiModalHeaderTitle>
      </EuiModalHeader>
      <EuiModalBody>
        <EuiText size="s">{action.description}</EuiText>
        <EuiSpacer size="s" />
        <EuiTextArea
          placeholder="Paste recorded script or JSON here…"
          value={rawInput}
          onChange={handleInputChange}
          rows={10}
          fullWidth
          compressed
        />
        {error ? (
          <>
            <EuiSpacer size="s" />
            <EuiText size="s" color="danger">
              <p>{error}</p>
            </EuiText>
          </>
        ) : null}
        {preview ? (
          <>
            <EuiSpacer size="m" />
            <EuiText size="xs">
              <strong>Preview</strong>
            </EuiText>
            <EuiSpacer size="xs" />
            <EuiCodeBlock language="javascript" fontSize="s" paddingSize="s" overflowHeight={200}>
              {preview}
            </EuiCodeBlock>
          </>
        ) : null}
      </EuiModalBody>
      <EuiModalFooter>
        <EuiButton onClick={onClose} color="text">
          Cancel
        </EuiButton>
        <EuiButton onClick={handleConfirm} fill disabled={!preview}>
          Import
        </EuiButton>
      </EuiModalFooter>
    </EuiModal>
  );
}

interface FullScreenEditorProps {
  value: string;
  onChange: (value?: string) => void;
  extraLibs?: ExtraLib[];
  snippets?: ScriptSnippet[];
  importActions?: ImportAction[];
  language: string;
  onClose: () => void;
}

function FullScreenEditor({
  value,
  onChange,
  extraLibs,
  snippets,
  importActions,
  language,
  onClose,
}: FullScreenEditorProps) {
  const euiTheme = useEuiTheme();
  const { euiTheme: theme } = euiTheme;
  const overlayZIndex = Number(theme.levels.mask) - 1;

  const [activeImportAction, setActiveImportAction] = useState<ImportAction | null>(null);

  const handleImportConfirm = useCallback(
    (transformedScript: string) => {
      onChange(transformedScript);
      setActiveImportAction(null);
    },
    [onChange],
  );

  const handleKeyDown = useCallback(
    (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !activeImportAction) {
        event.preventDefault();
        event.stopPropagation();
        onClose();
      }
    },
    [onClose, activeImportAction],
  );

  useEffect(() => {
    document.addEventListener('keydown', handleKeyDown);
    document.body.style.overflow = 'hidden';

    return () => {
      document.removeEventListener('keydown', handleKeyDown);
      document.body.style.overflow = '';
    };
  }, [handleKeyDown]);

  return createPortal(
    <EuiFocusTrap onClickOutside={activeImportAction ? undefined : onClose}>
      <div
        data-test-subj="scriptEditorFullScreen"
        css={css`
          animation: euiFullScreenOverlay 350ms cubic-bezier(0.34, 1.56, 0.64, 1);
          position: fixed;
          inset: 0;
          z-index: ${overlayZIndex};
          display: flex;
          flex-direction: column;
          background-color: ${theme.colors.body};

          @keyframes euiFullScreenOverlay {
            0% {
              opacity: 0;
              transform: translateY(16px);
            }
            100% {
              opacity: 1;
              transform: translateY(0);
            }
          }
        `}
      >
        <EuiPanel
          paddingSize="l"
          css={css`
            height: 100%;
            display: flex;
            flex-direction: column;
          `}
        >
          <EuiFlexGroup direction="column" gutterSize="none" style={{ height: '100%' }}>
            <EuiFlexItem grow={false}>
              <EuiFlexGroup justifyContent="flexEnd" gutterSize="none">
                <EuiFlexItem grow={false}>
                  <EuiButtonIcon
                    iconType="fullScreenExit"
                    aria-label="Exit full screen"
                    onClick={onClose}
                    color="text"
                  />
                </EuiFlexItem>
              </EuiFlexGroup>
            </EuiFlexItem>
            <EuiFlexItem>
              <Editor
                height="100%"
                language={language}
                options={FULLSCREEN_EDITOR_OPTIONS}
                value={value}
                onChange={onChange}
                theme="euiTheme"
                beforeMount={(m) => {
                  m.editor.defineTheme('euiTheme', createTheme(euiTheme));
                  registerExtraLibs(extraLibs);
                }}
                onMount={(editor) => {
                  registerSnippetActions(editor, snippets);
                  registerImportActions(editor, importActions, setActiveImportAction);
                }}
              />
            </EuiFlexItem>
          </EuiFlexGroup>
        </EuiPanel>
        {activeImportAction ? (
          <ScriptImportModal
            action={activeImportAction}
            onConfirm={handleImportConfirm}
            onClose={() => setActiveImportAction(null)}
          />
        ) : null}
      </div>
    </EuiFocusTrap>,
    document.body,
  );
}

export function ScriptEditor({
  onChange,
  defaultValue,
  extraLibs,
  language = 'javascript',
  snippets,
  importActions,
}: Props) {
  const euiTheme = useEuiTheme();
  const [isFullScreen, setIsFullScreen] = useState(false);
  const [currentValue, setCurrentValue] = useState(defaultValue ?? '');
  const [activeImportAction, setActiveImportAction] = useState<ImportAction | null>(null);

  const toggleFullScreen = useCallback(() => setIsFullScreen((prev) => !prev), []);

  const handleChange = useCallback(
    (value?: string) => {
      setCurrentValue(value ?? '');
      onChange(value);
    },
    [onChange],
  );

  const handleImportConfirm = useCallback(
    (transformedScript: string) => {
      setCurrentValue(transformedScript);
      onChange(transformedScript);
      setActiveImportAction(null);
    },
    [onChange],
  );

  return (
    <div
      css={css`
        position: relative;
      `}
    >
      <Editor
        height="25vh"
        language={language}
        options={EDITOR_OPTIONS}
        value={currentValue}
        onChange={handleChange}
        theme="euiTheme"
        beforeMount={(m) => {
          m.editor.defineTheme('euiTheme', createTheme(euiTheme));
          registerExtraLibs(extraLibs);
        }}
        onMount={(editor) => {
          registerSnippetActions(editor, snippets);
          registerImportActions(editor, importActions, setActiveImportAction);
        }}
      />
      <EuiButtonIcon
        iconType="fullScreen"
        aria-label="Enter full screen"
        onClick={toggleFullScreen}
        color="text"
        css={css`
          position: absolute;
          top: 4px;
          right: 18px;
          opacity: 0.4;
          transition: opacity 150ms ease-in-out;
          &:hover,
          &:focus {
            opacity: 1;
          }
        `}
      />
      {isFullScreen && (
        <FullScreenEditor
          value={currentValue}
          onChange={handleChange}
          extraLibs={extraLibs}
          snippets={snippets}
          importActions={importActions}
          language={language}
          onClose={toggleFullScreen}
        />
      )}
      {activeImportAction ? (
        <ScriptImportModal
          action={activeImportAction}
          onConfirm={handleImportConfirm}
          onClose={() => setActiveImportAction(null)}
        />
      ) : null}
    </div>
  );
}
