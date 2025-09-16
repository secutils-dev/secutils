import type { UseEuiTheme } from '@elastic/eui';
import { useEuiTheme } from '@elastic/eui';
import { Editor, loader } from '@monaco-editor/react';
import * as monaco from 'monaco-editor';

loader.config({ monaco });

// See https://github.com/microsoft/monaco-editor/blob/main/docs/integrate-esm.md#using-parcel
// @ts-expect-error This doesn't exist on `window`.
self.MonacoEnvironment = {
  getWorkerUrl: (_: string, label: string) =>
    label === 'javascript' || label === 'typescript' ? '/tools/monaco/ts.worker.js' : '/tools/monaco/editor.worker.js',
};

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

export interface Props {
  onChange: (scriptContent?: string) => void;
  defaultValue?: string;
}

export function ScriptEditor({ onChange, defaultValue }: Props) {
  const euiTheme = useEuiTheme();

  return (
    <Editor
      height="25vh"
      defaultLanguage="javascript"
      options={{
        mouseWheelZoom: true,
        scrollbar: { verticalScrollbarSize: 14, horizontal: 'hidden' },
        wordWrap: 'on',
        minimap: { enabled: false },
      }}
      defaultValue={defaultValue}
      onChange={(value) => onChange(value)}
      theme={'euiTheme'}
      beforeMount={(monaco) => {
        monaco.editor.defineTheme('euiTheme', createTheme(euiTheme));
      }}
    />
  );
}
