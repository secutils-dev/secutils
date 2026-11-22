// Reusable credentials live in browser storage, outside the review tree, exports, and snapshots.
import * as db from '../store/db.ts';
import { newId } from '../tree/model.ts';
import { openGitHubToken, sealGitHubToken, type GitHubCredential } from './credentials.ts';

/** Display metadata only. The owner is a picker hint, not a GitHub permission. */
export interface CredentialLabel {
    id: string;
    name: string;
    owner: string;
}

interface StoredCredential extends CredentialLabel {
    revision: string;
    secret: GitHubCredential;
}

/** The current authorization for one GitHub operation. Scope changes when the token changes. */
export interface GitHubAuthorization {
    token: string;
    scope?: string;
}

/** Error for an attachment that must never fall back to anonymous access. */
export const MISSING_GITHUB_CREDENTIAL = 'This GitHub credential is no longer available. Choose another in Sync.';

const credentialLock = <T>(id: string, mode: 'shared' | 'exclusive', run: () => Promise<T>) =>
    navigator.locks ? navigator.locks.request(`mars.github-credential:${id}`, { mode }, run) : run();

/** List labels without decrypting any PAT. */
export async function gitHubCredentials(): Promise<CredentialLabel[]> {
    const rows = await db.allSides<StoredCredential>('github-credential');
    return rows.map(({ id, name, owner }) => ({ id, name, owner })).sort((a, b) => a.name.localeCompare(b.name));
}

/** Test whether a saved credential still exists without decrypting it. */
export const hasGitHubCredential = async (id: string) =>
    !!(await db.getSide<StoredCredential>('github-credential', id));

/** Use exactly the selected PAT. Replacement and removal wait for active uses of this credential. */
export function withGitHubCredential<T>(id: string, run: (auth: GitHubAuthorization) => Promise<T>) {
    return credentialLock(id, 'shared', async () => {
        const item = await db.getSide<StoredCredential>('github-credential', id);
        if (!item) throw new Error(MISSING_GITHUB_CREDENTIAL);
        const token = await openGitHubToken(item.secret, id).catch(() => {
            throw new Error('Could not open this GitHub credential. Replace its token in Manage credentials.');
        });
        return run({ token, scope: `${id}:${item.revision}` });
    });
}

/** Add or update one credential. A blank token keeps the current secret when editing metadata. */
export async function saveGitHubCredential(label: { id?: string; name: string; owner: string }, secret: string) {
    if (db.isTemporaryStorage()) throw new Error('Open your browser library to save GitHub credentials.');
    const id = label.id ?? newId(),
        name = label.name.trim(),
        owner = label.owner.trim();
    const token = secret.trim();
    if (!name || name.length > 100) throw new Error('Enter a credential name of 1 to 100 characters.');
    if (owner && !/^[a-z\d](?:[a-z\d-]{0,38})$/i.test(owner))
        throw new Error('Enter a GitHub owner name without a URL.');
    if (token.length > 512 || /\s/.test(token)) throw new Error('Enter a GitHub token without spaces.');
    const sealed = token ? await sealGitHubToken(token, id) : undefined;
    return credentialLock(id, 'exclusive', async () => {
        const current = await db.getSide<StoredCredential>('github-credential', id);
        if (label.id && !current) throw new Error('This credential was removed in another tab.');
        if (!current && !sealed) throw new Error('Enter a GitHub token.');
        await db.putSide('github-credential', id, {
            id,
            name,
            owner,
            secret: sealed ?? current!.secret,
            revision: sealed ? newId() : current!.revision,
        } satisfies StoredCredential);
        return id;
    });
}

/** Remove only the local secret. Attachments keep their reference and stay paused. */
export function removeGitHubCredential(id: string) {
    return credentialLock(id, 'exclusive', () => db.deleteSide('github-credential', id));
}
