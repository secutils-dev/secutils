import { loader } from '@monaco-editor/react';
import * as monaco from 'monaco-editor/editor';
// `monaco-editor/editor` is API-only since 0.56; editor contributions (context menu, find, folding,
// diff editor, …) are opt-in and none of them are reachable without this.
import 'monaco-editor/features/register.all';
import 'monaco-editor/languages/definitions/css/register';
import 'monaco-editor/languages/definitions/html/register';
import 'monaco-editor/languages/definitions/javascript/register';
import 'monaco-editor/languages/definitions/xml/register';
import 'monaco-editor/languages/features/css/register';
import 'monaco-editor/languages/features/html/register';
import 'monaco-editor/languages/features/json/register';
import * as monacoTypescript from 'monaco-editor/languages/features/typescript/register';

loader.config({ monaco });

(self as unknown as Record<string, unknown>).__test_monaco = monaco;

self.MonacoEnvironment = {
  getWorkerUrl: (_: string, label: string) => {
    if (label === 'javascript' || label === 'typescript') {
      return '/tools/monaco/ts.worker.js';
    }
    if (label === 'json') {
      return '/tools/monaco/json.worker.js';
    }
    if (label === 'html' || label === 'handlebars' || label === 'razor') {
      return '/tools/monaco/html.worker.js';
    }
    if (label === 'css' || label === 'scss' || label === 'less') {
      return '/tools/monaco/css.worker.js';
    }
    return '/tools/monaco/editor.worker.js';
  },
};

export { monaco, monacoTypescript };
