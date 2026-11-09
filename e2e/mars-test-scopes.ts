import { createHash, randomUUID } from 'node:crypto';
import { chmodSync, existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { parseEnv } from 'node:util';

export interface ScopeManifest {
  v: 1;
  origin: string;
  scope: string;
  run: string;
  test: string;
  title: string;
  token: string;
  createdAt: string;
  state: 'opening' | 'active' | 'closing' | 'closed';
  removed: number;
  snapshots?: { id: string; token: string }[];
}
export interface ScopeAudit {
  scope: string;
  run: string;
  test: string;
  title: string;
  createdAt: number;
  expiresAt: number;
  state: 'active' | 'closing' | 'closed';
  removed: number;
}
// Find the checkout from root, e2e/, or the MARS package (Playwright uses CommonJS).
const repositoryRoot = () => {
  let directory = process.cwd();
  while (
    !existsSync(resolve(directory, 'e2e/package.json')) ||
    !existsSync(resolve(directory, 'dev/tools/mars.html'))
  ) {
    const parent = dirname(directory);
    if (parent === directory) throw new Error('Run MARS test tools from the Secutils checkout.');
    directory = parent;
  }
  return directory;
};
const root = repositoryRoot();
export const testScopeManifestPath = (run: string, scope: string) =>
  resolve(root, 'e2e/.cache/mars-test-scopes', run, `${scope}.json`);
const validId = (value: unknown) =>
  typeof value === 'string' && /^[a-f0-9]{8}-[a-f0-9]{4}-[1-8][a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}$/.test(value);
const environment = () => ({
  ...(existsSync(resolve(root, '.env')) ? parseEnv(readFileSync(resolve(root, '.env'), 'utf8')) : {}),
  ...process.env,
});
export function validateTestOrigin(origin: string) {
  const address = new URL(origin),
    env = environment();
  const local = ['localhost', '127.0.0.1', '[::1]'].includes(address.hostname);
  const configured = new URL(env.MARS_BASE_URL ?? `https://${env.SECUTILS_MARS_PUBLIC_HOST ?? 'mars.secutils.dev'}`)
    .origin;
  if (
    address.origin !== origin ||
    !['http:', 'https:'].includes(address.protocol) ||
    (!local && (address.protocol !== 'https:' || origin !== configured))
  ) {
    throw new Error('MARS cleanup origin is not the configured MARS host or a loopback preview. No request was sent.');
  }
  return local;
}
export function operatorKey(origin: string) {
  const local = validateTestOrigin(origin),
    env = environment();
  return env.MARS_TEST_OPERATOR_KEY ?? (local ? 'local-operator-secret' : env.SECUTILS_HTML_APP_API_KEY);
}
const save = (path: string, manifest: ScopeManifest) => {
  mkdirSync(dirname(path), { recursive: true, mode: 0o700 });
  writeFileSync(path, JSON.stringify(manifest, null, 2), { mode: 0o600 });
  chmodSync(path, 0o600);
};
export async function scopeRequest(origin: string, body: Record<string, unknown>) {
  const key = operatorKey(origin);
  if (!key)
    throw new Error(
      'MARS production checks require MARS_TEST_OPERATOR_KEY or SECUTILS_HTML_APP_API_KEY for isolated cleanup.',
    );
  const response = await fetch(`${origin}/sync/testing`, {
    method: 'POST',
    redirect: 'error',
    signal: AbortSignal.timeout(20000),
    headers: { Authorization: `Bearer ${key}`, 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
  if (!response.ok) throw new Error(`MARS test-scope operation failed (HTTP ${response.status}).`);
  return response.json() as Promise<{
    complete?: boolean;
    removed?: number;
    expiresAt?: number;
    scopes?: ScopeAudit[];
    cursor?: string | null;
  }>;
}
export async function openTestScope(origin: string, run: string, test: string, title: string) {
  validateTestOrigin(origin);
  if (!validId(run) || !/^[a-f0-9]{64}$/.test(test))
    throw new Error('Invalid MARS test identity. No request was sent.');
  const manifest: ScopeManifest = {
    v: 1,
    origin,
    scope: randomUUID(),
    run,
    test,
    title: title.slice(0, 256),
    token: Buffer.from(crypto.getRandomValues(new Uint8Array(32))).toString('base64url'),
    createdAt: new Date().toISOString(),
    state: 'opening',
    removed: 0,
  };
  const path = testScopeManifestPath(run, manifest.scope);
  // Persist the identity before the remote request, including an ambiguous/lost acknowledgement.
  save(path, manifest);
  try {
    await scopeRequest(origin, {
      operation: 'open',
      scope: manifest.scope,
      run,
      test,
      title: manifest.title,
      tokenHash: createHash('sha256').update(manifest.token).digest('hex'),
    });
    manifest.state = 'active';
    save(path, manifest);
    return { manifest, path };
  } catch (error) {
    throw new Error(`Could not open an isolated MARS test scope. Recovery manifest: ${path}`, { cause: error });
  }
}
export async function closeTestScope(path: string, expectedOrigin?: string) {
  const manifest = JSON.parse(readFileSync(path, 'utf8')) as ScopeManifest;
  if (
    manifest.v !== 1 ||
    !/^https?:\/\//.test(manifest.origin) ||
    new URL(manifest.origin).origin !== manifest.origin ||
    (expectedOrigin && manifest.origin !== expectedOrigin) ||
    ![manifest.scope, manifest.run].every(validId) ||
    !/^[a-f0-9]{64}$/.test(manifest.test)
  ) {
    throw new Error('Invalid MARS cleanup manifest. No deletion was attempted.');
  }
  manifest.state = 'closing';
  save(path, manifest);
  const snapshots = manifest.snapshots ?? [];
  if (
    !Array.isArray(snapshots) ||
    snapshots.length > 32 ||
    !snapshots.every(
      (snapshot) => /^[A-Za-z0-9_-]{16}$/.test(snapshot.id) && /^[A-Za-z0-9_-]{22}$/.test(snapshot.token),
    )
  ) {
    throw new Error('Invalid snapshot cleanup capability. No snapshot deletion was attempted.');
  }
  const cleanupCanaries = async () => {
    for (const snapshot of snapshots) {
      validateTestOrigin(manifest.origin);
      const response = await fetch(`${manifest.origin}/?s=${snapshot.id}`, {
        method: 'DELETE',
        redirect: 'error',
        signal: AbortSignal.timeout(20000),
        headers: { 'x-su-delete-token': snapshot.token },
      });
      if (![204, 404].includes(response.status))
        throw new Error(`Snapshot canary cleanup failed (HTTP ${response.status}).`);
    }
  };
  for (let page = 0; page < 2000; page++) {
    const result = await scopeRequest(manifest.origin, {
      operation: 'close',
      scope: manifest.scope,
      run: manifest.run,
      test: manifest.test,
    });
    manifest.removed = result.removed ?? manifest.removed;
    save(path, manifest);
    if (result.complete) {
      await cleanupCanaries();
      manifest.state = 'closed';
      save(path, manifest);
      return manifest;
    }
  }
  throw new Error(`MARS cleanup did not finish. Retry the private manifest: ${path}`);
}

// Preservation tests create one ordinary canary. Record its narrow delete capability before creation.
export function recordSnapshotCanary(path: string, id: string, token: string) {
  if (!/^[A-Za-z0-9_-]{16}$/.test(id) || !/^[A-Za-z0-9_-]{22}$/.test(token))
    throw new Error('Invalid snapshot canary identity.');
  const manifest = JSON.parse(readFileSync(path, 'utf8')) as ScopeManifest;
  if (manifest.v !== 1 || manifest.state !== 'active' || !validId(manifest.scope) || !validId(manifest.run))
    throw new Error('Invalid test manifest.');
  validateTestOrigin(manifest.origin);
  manifest.snapshots = [...(manifest.snapshots ?? []), { id, token }];
  save(path, manifest);
}

// Only the per-scope capability reaches the synthetic browser, never the operator credential.
export function installScopeFetch({ origin, scope, token }: Pick<ScopeManifest, 'origin' | 'scope' | 'token'>) {
  const native = window.fetch.bind(window);
  window.fetch = (input, options = {}) => {
    const url = new URL(input instanceof Request ? input.url : String(input), location.href);
    if (url.origin !== origin) return native(input, options);
    const headers = new Headers(options.headers ?? (input instanceof Request ? input.headers : undefined));
    headers.set('x-mars-test-scope', scope);
    headers.set('x-mars-test-token', token);
    return native(input, { ...options, headers });
  };
}
