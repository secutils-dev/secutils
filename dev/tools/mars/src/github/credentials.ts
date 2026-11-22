// Protect saved PATs with this browser profile's IndexedDB and a non-exportable local key.
// This does not protect against code running on the MARS origin or against profile compromise.

/** Structured-clone record. Neither the key nor bearer token enters a MARS export or URL. */
export interface GitHubCredential {
    key: CryptoKey;
    iv: Uint8Array<ArrayBuffer>;
    ciphertext: ArrayBuffer;
}

/** Encrypt a token before handing it to the browser's local storage. */
export async function sealGitHubToken(token: string, root: string): Promise<GitHubCredential> {
    const key = await crypto.subtle.generateKey({ name: 'AES-GCM', length: 256 }, false, ['encrypt', 'decrypt']);
    const iv = crypto.getRandomValues(new Uint8Array(12));
    const ciphertext = await crypto.subtle.encrypt(
        { name: 'AES-GCM', iv, additionalData: new TextEncoder().encode(root) },
        key,
        new TextEncoder().encode(token),
    );
    return { key, iv, ciphertext };
}

/** Recover the token for its original local repository attachment. */
export async function openGitHubToken(credential: GitHubCredential, root: string): Promise<string> {
    return new TextDecoder().decode(
        await crypto.subtle.decrypt(
            { name: 'AES-GCM', iv: credential.iv, additionalData: new TextEncoder().encode(root) },
            credential.key,
            credential.ciphertext,
        ),
    );
}
