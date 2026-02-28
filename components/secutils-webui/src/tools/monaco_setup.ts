import { loader } from '@monaco-editor/react';
import type { typescript as MonacoTypescriptNs } from 'monaco-editor';
import * as monaco from 'monaco-editor/esm/vs/editor/editor.api';
import 'monaco-editor/esm/vs/basic-languages/css/css.contribution';
import 'monaco-editor/esm/vs/basic-languages/html/html.contribution';
import 'monaco-editor/esm/vs/basic-languages/javascript/javascript.contribution';
import 'monaco-editor/esm/vs/basic-languages/xml/xml.contribution';
import 'monaco-editor/esm/vs/language/css/monaco.contribution';
import 'monaco-editor/esm/vs/language/html/monaco.contribution';
import 'monaco-editor/esm/vs/language/json/monaco.contribution';
import * as _monacoTypescript from 'monaco-editor/esm/vs/language/typescript/monaco.contribution';

loader.config({ monaco });

self.MonacoEnvironment = {
  getWorkerUrl: (_: string, label: string) => {
    if (label === 'javascript' || label === 'typescript') return '/tools/monaco/ts.worker.js';
    if (label === 'json') return '/tools/monaco/json.worker.js';
    if (label === 'html' || label === 'handlebars' || label === 'razor') return '/tools/monaco/html.worker.js';
    if (label === 'css' || label === 'scss' || label === 'less') return '/tools/monaco/css.worker.js';
    return '/tools/monaco/editor.worker.js';
  },
};

const monacoTypescript = _monacoTypescript as unknown as typeof MonacoTypescriptNs;

export { monaco, monacoTypescript };
