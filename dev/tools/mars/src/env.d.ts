// Ambient declarations: module forms that only the build resolves, and File System Access
// members that the TypeScript DOM library does not declare.

/** A stylesheet that esbuild bundles as its source text (`loader: { '.css': 'text' }`). */
declare module '*.css' {
    const css: string;
    export default css;
}

/**
 * A runtime that `build.mjs` compiles as a separate module graph (`reader`, `print`, `editor`).
 * The default export is the minified source text.
 */
declare module 'mars-runtime:*' {
    const source: string;
    export default source;
}

/** Permission methods of the File System Access API. Chromium only, so each call is optional. */
interface FileSystemHandle {
    queryPermission?(opts: { mode: 'read' | 'readwrite' }): Promise<PermissionState>;
    requestPermission?(opts: { mode: 'read' | 'readwrite' }): Promise<PermissionState>;
}
