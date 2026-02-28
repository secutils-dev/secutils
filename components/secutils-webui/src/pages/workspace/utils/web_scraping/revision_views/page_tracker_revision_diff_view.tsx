import {
  EuiButtonGroup,
  EuiEmptyPrompt,
  EuiFlexGroup,
  EuiFlexItem,
  EuiIcon,
  useEuiTheme,
  useIsWithinMaxBreakpoint,
} from '@elastic/eui';
import { DiffEditor } from '@monaco-editor/react';
import { useMemo, useState } from 'react';

import { createTheme } from '../../../components/script_editor';

export interface PageTrackerRevisionDiffViewProps {
  originalContent: string;
  modifiedContent: string;
  language: 'html' | 'json' | 'text';
}

type DiffLayout = 'sideBySide' | 'inline';

const DIFF_HEIGHT = '40vh';

export function PageTrackerRevisionDiffView({
  originalContent,
  modifiedContent,
  language,
}: PageTrackerRevisionDiffViewProps) {
  const euiTheme = useEuiTheme();
  const isMobile = useIsWithinMaxBreakpoint('s');
  const [layout, setLayout] = useState<DiffLayout>('sideBySide');
  const effectiveLayout = isMobile ? 'inline' : layout;

  const layoutOptions = useMemo(
    () => [
      { id: 'sideBySide' as const, label: 'Side by side' },
      { id: 'inline' as const, label: 'Inline' },
    ],
    [],
  );

  // Force a full remount of the DiffEditor when content or layout changes.
  // Monaco's DiffEditor reuses the editor instance when props update, which
  // causes hideUnchangedRegions to lose its collapsed state on model updates
  // and the fold widgets to disappear when toggling renderSideBySide.
  const [editorKey, setEditorKey] = useState(0);
  const [prevInputs, setPrevInputs] = useState({ effectiveLayout, originalContent, modifiedContent });
  if (
    prevInputs.effectiveLayout !== effectiveLayout ||
    prevInputs.originalContent !== originalContent ||
    prevInputs.modifiedContent !== modifiedContent
  ) {
    setEditorKey((k) => k + 1);
    setPrevInputs({ effectiveLayout, originalContent, modifiedContent });
  }

  if (originalContent === modifiedContent) {
    return (
      <EuiEmptyPrompt
        icon={<EuiIcon type="check" size="xl" color="success" />}
        title={<h3>No changes</h3>}
        body={<p>The content is identical between these two revisions.</p>}
        titleSize="xs"
      />
    );
  }

  const monacoLanguage = language === 'text' ? 'plaintext' : language;

  return (
    <EuiFlexGroup direction="column" gutterSize="s">
      {!isMobile && (
        <EuiFlexItem grow={false}>
          <EuiFlexGroup justifyContent="center" responsive={false} gutterSize="none">
            <EuiFlexItem grow={false}>
              <EuiButtonGroup
                legend="Diff layout"
                options={layoutOptions}
                idSelected={layout}
                onChange={(id) => setLayout(id as DiffLayout)}
                buttonSize="compressed"
              />
            </EuiFlexItem>
          </EuiFlexGroup>
        </EuiFlexItem>
      )}
      <EuiFlexItem>
        <DiffEditor
          key={editorKey}
          height={DIFF_HEIGHT}
          language={monacoLanguage}
          original={originalContent}
          modified={modifiedContent}
          theme="euiDiffTheme"
          options={{
            readOnly: true,
            renderSideBySide: effectiveLayout === 'sideBySide',
            renderSideBySideInlineBreakpoint: 0,
            minimap: { enabled: false },
            scrollBeyondLastLine: false,
            wordWrap: 'on',
            hideUnchangedRegions: {
              enabled: true,
              revealLineCount: 3,
              minimumLineCount: 3,
            },
            scrollbar: { verticalScrollbarSize: 14 },
            renderOverviewRuler: false,
            originalEditable: false,
          }}
          beforeMount={(monaco) => {
            monaco.editor.defineTheme('euiDiffTheme', createTheme(euiTheme));
          }}
        />
      </EuiFlexItem>
    </EuiFlexGroup>
  );
}
