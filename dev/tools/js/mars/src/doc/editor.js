// CodeMirror 6 source editor, lazy-loaded from esm.sh (ported from md.html's module
// loader). Falls back to a plain <textarea> if any import fails. The viewer talks to
// it through `window.__suEditorAPI`, `__suCreateBlockEditor`, and the change hooks.
const CODEMIRROR_URL = 'https://esm.sh/codemirror@6.0.1';
const CM_MARKDOWN_URL = 'https://esm.sh/@codemirror/lang-markdown@6.3.0';

function textareaEditor(mount, text) {
    const ta = document.createElement('textarea');
    ta.className = 'editor-fallback';
    ta.spellcheck = false;
    ta.placeholder = 'Type or paste Markdown here...';
    ta.value = text;
    ta.addEventListener('input', () => window.__suOnEditorChange?.(ta.value));
    mount.replaceChildren(ta);
    window.__suEditorAPI = {
        kind: 'textarea',
        setText: (t) => { ta.value = t; window.__suOnEditorChange?.(t); },
        getText: () => ta.value,
        focus: () => ta.focus(),
    };
}

export async function loadEditor(mount) {
    try {
        // Unbundled, so both packages share one @codemirror/state instance;
        // separate ?bundle builds each inline their own and fail instanceof checks.
        const [cm, cmMd] = await Promise.all([import(CODEMIRROR_URL), import(CM_MARKDOWN_URL)]);
        const view = new cm.EditorView({
            doc: window.__suInitialDoc || '',
            extensions: [
                cm.basicSetup,
                cmMd.markdown(),
                cm.EditorView.lineWrapping,
                cm.EditorView.updateListener.of((u) => {
                    if (u.docChanged) window.__suOnEditorChange?.(u.state.doc.toString());
                }),
            ],
            parent: mount,
        });
        window.__suCreateBlockEditor = (parent, doc) => {
            const editor = new cm.EditorView({ doc, extensions: [cm.minimalSetup, cmMd.markdown(), cm.EditorView.lineWrapping], parent });
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
        };
    } catch (err) {
        console.warn('CodeMirror failed to load, falling back to <textarea>:', err);
        textareaEditor(mount, window.__suInitialDoc || '');
    }
    window.__suEditorReady?.();
}
