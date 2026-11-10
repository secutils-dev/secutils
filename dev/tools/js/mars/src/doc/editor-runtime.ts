// One module graph keeps CodeMirror's identity-sensitive extensions compatible.
export { basicSetup, minimalSetup, EditorView } from 'codemirror';
export { markdown } from '@codemirror/lang-markdown';
export { Compartment, EditorState, Transaction } from '@codemirror/state';
