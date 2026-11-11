// Entry point of the editor bundle. The build compiles this file to one ES module source string
// (`mars-runtime:editor` in build.mjs). `editor.js` imports that string from a Blob URL on first
// use. The file only re-exports what `editor.js` reads. One module graph keeps CodeMirror's
// identity-sensitive extensions compatible.
export { basicSetup, minimalSetup, EditorView } from 'codemirror';
export { markdown } from '@codemirror/lang-markdown';
export { Compartment, EditorState, Transaction } from '@codemirror/state';
