declare module '*.css' {
    const css: string;
    export default css;
}

declare module 'mars-runtime:*' {
    const source: string;
    export default source;
}

interface FileSystemHandle {
    queryPermission?(opts: { mode: 'read' | 'readwrite' }): Promise<PermissionState>;
    requestPermission?(opts: { mode: 'read' | 'readwrite' }): Promise<PermissionState>;
}
