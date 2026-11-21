// Three-way comparison of exact UTF-8 source. Remote and local may have changed independently.

/** Decide whether to acknowledge, pull, push, or preserve a conflict without overwriting either edit. */
export function fileDecision(base: string | undefined, local: string, remote: string) {
    if (local === remote) return 'ack';
    if (local === base) return 'pull';
    if (remote === base) return 'push';
    return 'conflict';
}
