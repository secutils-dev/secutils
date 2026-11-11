// CodeMirror 6 source editor, lazy-imported from one embedded module graph.
// Falls back to a plain <textarea> if loading fails. The viewer talks to
// it through `window.__suEditorAPI`, `__suCreateBlockEditor`, and the change hooks.
//
// `__suEditorAPI` has the same shape for both editors:
// - `setText(text)` replaces the text and reports the change through `__suOnEditorChange`.
// - `applyRemote(text)` applies a remote update as one minimal change. It keeps the scroll
//   position and does not call `__suOnEditorChange`, so the viewer does not store it again.
// - `setReadOnly(value)`, `getText()`, and `focus()`.
// The viewer sets `__suOnEditorChange(text)` and `__suEditorReady()`, and this module calls them.
import editorRuntime from 'mars-runtime:editor';
import { sourceChange } from './source-change.ts';
let editorModule;
// Imports the embedded editor bundle once, from a Blob URL that is revoked after the import.
// A failed import clears the cached promise, so a later call starts again.
function editorLibrary() {
    if (!editorModule) {
        const url = URL.createObjectURL(new Blob([editorRuntime], { type: 'text/javascript' }));
        editorModule = import(url)
            .catch((error) => {
                editorModule = undefined;
                throw error;
            })
            .finally(() => URL.revokeObjectURL(url));
    }
    return editorModule;
}

// Installs the plain textarea editor in `mount` and publishes it as `__suEditorAPI`. It serves
// as the fallback when CodeMirror cannot load and as the editor for very large documents.
function textareaEditor(mount, text) {
    const ta = document.createElement('textarea');
    ta.className = 'editor-fallback';
    ta.spellcheck = false;
    ta.placeholder = 'Type or paste Markdown here...';
    ta.setAttribute('aria-label', 'Markdown source');
    ta.value = text;
    ta.addEventListener('input', () => window.__suOnEditorChange?.(ta.value));
    mount.replaceChildren(ta);
    window.__suEditorAPI = {
        kind: 'textarea',
        setText: (t) => {
            ta.value = t;
            window.__suOnEditorChange?.(t);
        },
        getText: () => ta.value,
        focus: () => ta.focus(),
        setReadOnly: (value) => {
            ta.readOnly = value;
        },
        applyRemote: (text) => {
            const change = sourceChange(ta.value, text);
            if (!change) return;
            // Keep the selection on the same text. An offset after the change shifts by the length
            // difference. An offset inside the replaced range moves to the end of the inserted
            // text.
            const map = (at) =>
                at <= change.from
                    ? at
                    : at >= change.to
                      ? at + change.insert.length - (change.to - change.from)
                      : change.from + change.insert.length;
            const start = map(ta.selectionStart),
                end = map(ta.selectionEnd),
                scroll = ta.scrollTop;
            ta.value = text;
            ta.setSelectionRange(start, end);
            ta.scrollTop = scroll;
        },
    };
}

// Creates the source editor in `mount` and publishes `__suEditorAPI`. With CodeMirror it also
// publishes `__suCreateBlockEditor(parent, text)` for the inline block editor of the preview.
// Never rejects: a load failure installs the textarea editor. Calls `__suEditorReady` at the end.
// The initial text comes from `window.__suInitialDoc`.
export async function loadEditor(mount) {
    try {
        // The bundle exports all three CodeMirror packages from one module.
        const cm = await editorLibrary(),
            cmMd = cm,
            state = cm;
        const permission = new state.Compartment();
        // True during `applyRemote`, so the update listener does not report a remote change as an edit.
        let applying = false;
        const view = new cm.EditorView({
            doc: window.__suInitialDoc || '',
            extensions: [
                cm.basicSetup,
                cmMd.markdown(),
                cm.EditorView.lineWrapping,
                cm.EditorView.contentAttributes.of({ 'aria-label': 'Markdown source' }),
                permission.of([state.EditorState.readOnly.of(false), cm.EditorView.editable.of(true)]),
                cm.EditorView.updateListener.of((u) => {
                    if (u.docChanged && !applying) window.__suOnEditorChange?.(u.state.doc.toString());
                }),
            ],
            parent: mount,
        });
        window.__suCreateBlockEditor = (parent, doc) => {
            const editor = new cm.EditorView({
                doc,
                extensions: [
                    cm.minimalSetup,
                    cmMd.markdown(),
                    cm.EditorView.lineWrapping,
                    cm.EditorView.contentAttributes.of({ 'aria-label': 'Edit Markdown block' }),
                ],
                parent,
            });
            return {
                getText: () => editor.state.doc.toString(),
                focus: () => {
                    editor.focus();
                    editor.dispatch({ selection: { anchor: editor.state.doc.length } });
                },
                destroy: () => editor.destroy(),
            };
        };
        window.__suEditorAPI = {
            kind: 'codemirror',
            setText: (t) => {
                // Above 1,000,000 characters the textarea replaces CodeMirror for this page.
                if (t.length > 1000000) {
                    view.destroy();
                    textareaEditor(mount, t);
                    window.__suOnEditorChange?.(t);
                } else view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: t } });
            },
            getText: () => view.state.doc.toString(),
            focus: () => view.focus(),
            setReadOnly: (value) =>
                view.dispatch({
                    effects: permission.reconfigure([
                        state.EditorState.readOnly.of(value),
                        cm.EditorView.editable.of(!value),
                    ]),
                }),
            applyRemote: (text) => {
                const change = sourceChange(view.state.doc.toString(), text);
                if (!change) return;
                const scroll = view.scrollDOM.scrollTop;
                applying = true;
                try {
                    // A remote change is not an edit of this reader. Keep it out of the undo history.
                    view.dispatch({
                        changes: { from: change.from, to: change.to, insert: change.insert },
                        annotations: state.Transaction.addToHistory.of(false),
                    });
                } finally {
                    applying = false;
                    view.scrollDOM.scrollTop = scroll;
                }
            },
        };
    } catch (err) {
        console.warn('CodeMirror failed to load, falling back to <textarea>:', err);
        textareaEditor(mount, window.__suInitialDoc || '');
    }
    window.__suEditorReady?.();
}
