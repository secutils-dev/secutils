// CodeMirror 6 source editor, lazy-imported from one embedded module graph.
// Falls back to a plain <textarea> if loading fails. The viewer talks to
// it through `window.__suEditorAPI`, `__suCreateBlockEditor`, and the change hooks.
import editorRuntime from 'mars-runtime:editor';
import { sourceChange } from './source-change.ts';
let editorModule;
function editorLibrary() {
    if (!editorModule) {
        const url = URL.createObjectURL(new Blob([editorRuntime], { type: 'text/javascript' }));
        editorModule = import(url).catch(error => { editorModule = undefined; throw error; }).finally(() => URL.revokeObjectURL(url));
    }
    return editorModule;
}

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
        setText: (t) => { ta.value = t; window.__suOnEditorChange?.(t); },
        getText: () => ta.value,
        focus: () => ta.focus(),
        setReadOnly: (value) => { ta.readOnly = value; },
        applyRemote: (text) => {
            const change = sourceChange(ta.value, text); if (!change) return;
            const map = (at) => at <= change.from ? at : at >= change.to ? at + change.insert.length - (change.to - change.from) : change.from + change.insert.length;
            const start = map(ta.selectionStart), end = map(ta.selectionEnd), scroll = ta.scrollTop;
            ta.value = text; ta.setSelectionRange(start, end); ta.scrollTop = scroll;
        },
    };
}

export async function loadEditor(mount) {
    try {
        const cm = await editorLibrary(), cmMd = cm, state = cm;
        const permission = new state.Compartment();
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
            const editor = new cm.EditorView({ doc, extensions: [cm.minimalSetup, cmMd.markdown(), cm.EditorView.lineWrapping, cm.EditorView.contentAttributes.of({ 'aria-label': 'Edit Markdown block' })], parent });
            return {
                getText: () => editor.state.doc.toString(),
                focus: () => { editor.focus(); editor.dispatch({ selection: { anchor: editor.state.doc.length } }); },
                destroy: () => editor.destroy(),
            };
        };
        window.__suEditorAPI = {
            kind: 'codemirror',
            setText: (t) => {
                if (t.length > 1000000) {
                    view.destroy();
                    textareaEditor(mount, t);
                    window.__suOnEditorChange?.(t);
                } else view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: t } });
            },
            getText: () => view.state.doc.toString(),
            focus: () => view.focus(),
            setReadOnly: (value) => view.dispatch({ effects: permission.reconfigure([state.EditorState.readOnly.of(value), cm.EditorView.editable.of(!value)]) }),
            applyRemote: (text) => {
                const change = sourceChange(view.state.doc.toString(), text); if (!change) return;
                const scroll = view.scrollDOM.scrollTop;
                applying = true;
                try { view.dispatch({ changes: { from: change.from, to: change.to, insert: change.insert }, annotations: state.Transaction.addToHistory.of(false) }); }
                finally { applying = false; view.scrollDOM.scrollTop = scroll; }
            },
        };
    } catch (err) {
        console.warn('CodeMirror failed to load, falling back to <textarea>:', err);
        textareaEditor(mount, window.__suInitialDoc || '');
    }
    window.__suEditorReady?.();
}
