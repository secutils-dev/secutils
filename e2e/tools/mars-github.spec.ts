import { expect, type Locator, type Page, test } from '@playwright/test';

const baseURL = process.env.MARS_BASE_URL ?? 'http://localhost:7373';
test.use({ baseURL });
type EditorWindow = Window & { __suEditorAPI: { setText(text: string): void } };
test.skip(
  !process.env.MARS_BASE_URL ||
    !['localhost', '127.0.0.1', '[::1]', 'mars-dev.secutils.dev'].includes(new URL(baseURL).hostname),
  'GitHub preview is enabled only on an explicitly selected local or dev host',
);

async function addCredential(page: Page, parent: Locator, token: string, name = 'Test credential') {
  await parent.getByRole('button', { name: 'Manage credentials' }).click();
  const manager = page.getByRole('dialog', { name: 'GitHub credentials', exact: true });
  await manager.getByRole('button', { name: 'Add credential' }).click();
  const editor = page.getByRole('dialog', { name: 'Add GitHub credential' });
  await editor.getByLabel('Credential name').fill(name);
  await editor.getByLabel('Fine-grained personal access token').fill(token);
  await editor.getByRole('button', { name: 'Save credential' }).click();
  await expect(editor).not.toBeVisible();
  await manager.getByRole('button', { name: 'Close', exact: true }).click();
  await expect(parent.getByRole('combobox', { name: 'GitHub credential' }).locator('option:checked')).toContainText(
    name,
  );
}

test('GitHub preview imports nested files and sends one commit with the expected branch head', async ({ page }) => {
  const token = 'github_pat_synthetic_test';
  const oldHead = '1'.repeat(40),
    blob = '3'.repeat(40);
  let tree = '2'.repeat(40);
  const bodies: unknown[] = [];
  const leaked: string[] = [];
  const branchValidators: (string | undefined)[] = [];
  let trees = 0,
    blobs = 0;
  let releaseBlob!: () => void;
  const firstBlob = new Promise<void>((resolve) => {
    releaseBlob = resolve;
  });
  let head = oldHead,
    file = blob,
    remoteText = '# GitHub document\n';
  page.on('request', (request) => {
    if (
      !request.url().startsWith('https://api.github.com/') &&
      (request.url().includes(token) ||
        request.postData()?.includes(token) ||
        Object.values(request.headers()).some((value) => value.includes(token)))
    )
      leaked.push(request.url());
  });
  await page.route('https://api.github.com/**', async (route) => {
    const request = route.request(),
      path = new URL(request.url()).pathname;
    const headers = {
      'access-control-allow-origin': '*',
      'access-control-allow-headers': 'authorization, content-type, if-none-match',
      'access-control-allow-methods': 'GET, POST, OPTIONS',
      'access-control-expose-headers': 'etag',
      etag: `"${head}"`,
    };
    if (request.method() === 'OPTIONS') {
      await route.fulfill({ status: 204, headers });
      return;
    }
    expect(request.headers().authorization).toBe(`Bearer ${token}`);
    let response: unknown;
    if (path === '/repos/fake/notes') response = { full_name: 'fake/notes', default_branch: 'preview' };
    else if (path.endsWith('/branches/preview')) {
      const validator = request.headers()['if-none-match'];
      branchValidators.push(validator);
      if (validator === headers.etag) {
        await route.fulfill({ status: 304, headers });
        return;
      }
      response = { commit: { sha: head, commit: { tree: { sha: tree } } } };
    } else if (path.includes('/trees/')) {
      trees++;
      response = {
        tree: [{ type: 'blob', mode: '100644', path: 'Nested/Welcome.md', sha: file, size: remoteText.length }],
      };
    } else if (path.includes('/blobs/')) {
      blobs++;
      await firstBlob;
      response = { sha: file, encoding: 'base64', content: Buffer.from(remoteText).toString('base64') };
    } else if (path === '/graphql') {
      const input = request.postDataJSON().variables.input;
      expect(input.branch).toEqual({ repositoryNameWithOwner: 'fake/notes', branchName: 'preview' });
      bodies.push(input);
      response = { data: { createCommitOnBranch: { commit: { oid: '4'.repeat(40) } } } };
    } else {
      await route.abort();
      return;
    }
    await route.fulfill({ json: response, headers });
  });
  await page.goto('/');
  await page.getByRole('button', { name: 'New document, folder, or import' }).click();
  const importItem = page.getByRole('menuitem', { name: 'Import from GitHub…' });
  await expect(importItem.locator('[data-icon="logoGithub"] svg')).toBeVisible();
  await importItem.click();
  const dialog = page.getByRole('dialog', { name: 'Import from GitHub' });
  await dialog.getByLabel('Repository (required)').fill('fake/notes');
  const picker = dialog.getByRole('combobox', { name: 'GitHub credential' });
  const write = dialog.getByLabel('Sync my edits back to GitHub');
  await expect(picker.locator('option:checked')).toHaveText('None (public read-only)');
  await expect(dialog.locator('input[type="password"]')).toHaveCount(0);
  const manage = dialog.getByRole('button', { name: 'Manage credentials' });
  await expect(manage.locator('svg[aria-hidden="true"]')).toBeVisible();
  const selectBox = (await picker.boundingBox())!;
  const buttonBox = (await manage.boundingBox())!;
  expect(buttonBox.x).toBeCloseTo(selectBox.x + selectBox.width, 0);
  expect(buttonBox.y).toBe(selectBox.y);
  expect(buttonBox.height).toBe(selectBox.height);
  await expect(write).toBeDisabled();
  await expect(write).not.toBeChecked();
  await addCredential(page, dialog, token);
  await expect(manage).toBeFocused();
  await expect(write).toBeEnabled();
  await write.check();
  await dialog.getByRole('button', { name: 'Import folder' }).click();
  await expect(page.getByRole('progressbar', { name: 'GitHub download' })).toHaveAttribute('max', '1');
  releaseBlob();
  await expect(dialog).not.toBeVisible();
  await page.getByRole('link', { name: 'Welcome', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'GitHub document' })).toBeVisible();
  await expect(page.locator('#documentAccess')).toBeHidden();
  await page.getByRole('button', { name: 'Source', exact: true }).click();
  await page.evaluate(() => (window as EditorWindow).__suEditorAPI.setText('# Edited from MARS\n'));
  await page.getByRole('button', { name: 'Preview', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Edited from MARS' })).toBeVisible();
  await expect.poll(() => bodies.length, { timeout: 15_000 }).toBe(1);
  expect(branchValidators).toContain(`"${oldHead}"`);
  expect(bodies[0]).toMatchObject({
    expectedHeadOid: oldHead,
    message: { headline: 'docs(mars): sync document edits' },
    branch: { repositoryNameWithOwner: 'fake/notes', branchName: 'preview' },
    fileChanges: {
      additions: [{ path: 'Nested/Welcome.md', contents: Buffer.from('# Edited from MARS\n').toString('base64') }],
    },
  });
  await expect
    .poll(async () =>
      page.evaluate(() => {
        const open = indexedDB.open('mars');
        return new Promise<boolean>((resolve, reject) => {
          open.onerror = () => reject(open.error);
          open.onsuccess = () => {
            const db = open.result,
              read = db.transaction('nodes').objectStore('nodes').getAll();
            read.onerror = () => reject(read.error);
            read.onsuccess = () => {
              db.close();
              resolve(
                read.result.some(
                  (item) => item.githubFile?.path === 'Nested/Welcome.md' && item.githubFile.sha !== '3'.repeat(40),
                ),
              );
            };
          };
        });
      }),
    )
    .toBe(true);
  const checksBeforeReload = branchValidators.length;
  await page.reload();
  await expect(page.getByRole('heading', { name: 'Edited from MARS' })).toBeVisible();
  expect(branchValidators).toHaveLength(checksBeforeReload);
  head = '4'.repeat(40);
  await page.getByRole('button', { name: 'Actions for Welcome', exact: true }).click();
  await expect(page.getByRole('menuitem', { name: /Reconnect GitHub/ })).toHaveCount(0);
  await expect(page.getByText(/Never expires/)).toHaveCount(0);
  await expect(page.getByRole('menu').getByText('fake/notes', { exact: false })).toHaveCount(0);
  await page.getByRole('menuitem', { name: 'Sync', exact: true }).click();
  const settings = page.getByRole('dialog', { name: 'Sync', exact: true });
  await expect(settings.getByText('fake/notes', { exact: true })).toBeVisible();
  await expect(settings.getByRole('combobox').locator('option:checked')).toHaveText('Test credential · fake');
  await expect(settings.locator('input[type="password"]')).toHaveCount(0);
  await expect(settings.getByLabel('Sync my edits back to GitHub')).toBeEnabled();
  await settings.getByRole('button', { name: 'Sync now' }).click();
  await expect(settings).not.toBeVisible();
  await expect(page.getByText('Checked GitHub.', { exact: true })).toHaveCount(0);
  expect(trees).toBe(1);
  expect(blobs).toBe(1);
  head = '5'.repeat(40);
  file = '6'.repeat(40);
  tree = '7'.repeat(40);
  remoteText = '# Edited from GitHub\n';
  await page.getByRole('button', { name: 'Actions for Welcome', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Sync', exact: true }).click();
  await settings.getByRole('button', { name: 'Sync now' }).click();
  await expect(page.getByRole('heading', { name: 'Edited from GitHub' })).toBeVisible();
  expect(trees).toBe(2);
  expect(blobs).toBe(2);
  await page.getByRole('button', { name: 'Actions for Welcome', exact: true }).click();
  await expect(page.getByRole('menu').getByRole('menuitem', { name: /share a live copy/i })).toHaveCount(0);
  await page.keyboard.press('Escape');
  await page.getByRole('button', { name: 'Share', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Copy link', exact: true }).click();
  const sharing = page.getByRole('dialog', { name: 'Copy link', exact: true });
  await expect(sharing.getByRole('radio', { name: 'Live', exact: true, includeHidden: true })).toHaveCount(0);
  await expect(sharing.getByRole('button', { name: 'Live links', exact: true, includeHidden: true })).toHaveCount(0);
  await expect(sharing.getByRole('radio', { name: 'Full', exact: true })).toBeChecked();
  await sharing.getByRole('button', { name: 'Copy link', exact: true }).click();
  await expect(sharing.getByRole('textbox', { name: 'Link', exact: true })).toHaveValue(/#(?:name|b)=/);
  await sharing.getByRole('radio', { name: 'Short', exact: true }).check();
  await expect(page.getByText(/Short links upload an encrypted copy to Secutils/)).toBeVisible();
  await page.getByRole('button', { name: 'Cancel', exact: true }).click();
  await page.getByRole('button', { name: 'Actions for notes', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Delete', exact: true }).click();
  const deletion = page.getByRole('dialog', { name: 'Delete "notes"?' });
  await expect(deletion.getByText(/GitHub repository and its files will not be deleted/)).toBeVisible();
  await deletion.getByRole('button', { name: 'Delete folder' }).click();
  await expect(page.getByRole('link', { name: 'Welcome', exact: true })).toHaveCount(0);
  expect(bodies).toHaveLength(1);
  expect(leaked).toEqual([]);
});

test('named credentials are explicit per repository, rotate across tabs, and pause on removal', async ({
  page,
  context,
}) => {
  const tokens = {
    acme: 'github_pat_synthetic_acme',
    other: 'github_pat_synthetic_other',
    replacement: 'github_pat_synthetic_rotated',
  };
  const accepted = new Map([
    ['acme', tokens.acme],
    ['other', tokens.other],
  ]);
  const traffic: { repo: string; auth: string }[] = [];
  const leaks: string[] = [];
  context.on('request', (request) => {
    if (
      !request.url().startsWith('https://api.github.com/') &&
      Object.values(tokens).some(
        (token) =>
          request.url().includes(token) ||
          request.postData()?.includes(token) ||
          Object.values(request.headers()).some((value) => value.includes(token)),
      )
    )
      leaks.push(request.url());
  });
  await context.route('https://api.github.com/**', async (route) => {
    const request = route.request();
    const [, , owner, name, ...path] = new URL(request.url()).pathname.split('/');
    const repo = `${owner}/${name}`;
    const headers = {
      'access-control-allow-origin': '*',
      'access-control-allow-headers': 'authorization, content-type, if-none-match',
      'access-control-allow-methods': 'GET, POST, OPTIONS',
    };
    if (request.method() === 'OPTIONS') return route.fulfill({ status: 204, headers });
    expect(request.method()).toBe('GET');
    const auth = request.headers().authorization ?? '';
    traffic.push({ repo, auth });
    if (!accepted.has(owner) || auth !== `Bearer ${accepted.get(owner)}`)
      return route.fulfill({ status: 404, headers });
    const text = `# ${name}\n`;
    const sha = '3'.repeat(40);
    let json: unknown;
    if (!path.length) json = { full_name: repo, default_branch: 'main', private: true };
    else if (path[0] === 'branches')
      json = { commit: { sha: '1'.repeat(40), commit: { tree: { sha: '2'.repeat(40) } } } };
    else if (path[1] === 'trees')
      json = { tree: [{ path: `About-${name}.md`, mode: '100644', type: 'blob', sha, size: Buffer.byteLength(text) }] };
    else if (path[1] === 'blobs') json = { sha, encoding: 'base64', content: Buffer.from(text).toString('base64') };
    else return route.abort();
    return route.fulfill({ json, headers });
  });
  const open = async (repo: string) => {
    await page.getByRole('button', { name: 'New document, folder, or import' }).click();
    await page.getByRole('menuitem', { name: 'Import from GitHub…' }).click();
    const dialog = page.getByRole('dialog', { name: 'Import from GitHub' });
    await dialog.getByLabel('Repository (required)').fill(repo);
    return dialog;
  };
  const settings = async (tab: typeof page, repo: string) => {
    await tab
      .getByRole('treeitem', { name: `${repo} Actions for ${repo}`, exact: true })
      .getByRole('button', { name: `Actions for ${repo}`, exact: true })
      .click();
    await tab.getByRole('menuitem', { name: 'Sync', exact: true }).click();
    return tab.getByRole('dialog', { name: 'Sync', exact: true });
  };
  await page.goto('/');
  let dialog = await open('acme/handbook');
  await addCredential(page, dialog, tokens.acme, 'Acme docs');
  await expect(dialog.getByLabel('Sync my edits back to GitHub')).not.toBeChecked();
  expect(traffic).toHaveLength(0);
  await dialog.getByRole('button', { name: 'Import folder' }).click();
  await expect(dialog).not.toBeVisible();
  dialog = await open('other/manuals');
  await addCredential(page, dialog, tokens.other, 'Other docs');
  await dialog.getByRole('button', { name: 'Import folder' }).click();
  await expect(dialog).not.toBeVisible();
  dialog = await open('acme/guides');
  const picker = dialog.getByRole('combobox', { name: 'GitHub credential' });
  await expect(picker.locator('optgroup[label="Suggested for acme"]')).toContainText('Acme docs');
  await expect(picker).toHaveValue('public');
  await picker.selectOption({ label: 'Other docs · other' });
  const beforeDenied = traffic.length;
  await dialog.getByRole('button', { name: 'Import folder' }).click();
  await expect(dialog.getByRole('alert')).toContainText('whether your token has access');
  expect(traffic.slice(beforeDenied)).toEqual([{ repo: 'acme/guides', auth: `Bearer ${tokens.other}` }]);
  await picker.selectOption({ label: 'Acme docs · acme' });
  await dialog.getByLabel('Sync my edits back to GitHub').check();
  await dialog.getByRole('button', { name: 'Import folder' }).click();
  await expect(dialog).not.toBeVisible();
  dialog = await open('acme/guides');
  await expect(picker.locator('option:checked')).toHaveText('Acme docs · acme');
  await expect(dialog.getByLabel('Sync my edits back to GitHub')).not.toBeChecked();
  await dialog.getByRole('button', { name: 'Close', exact: true }).click();
  const otherTab = await context.newPage();
  await otherTab.goto('/');
  let remoteSettings = await settings(otherTab, 'guides');
  await expect(remoteSettings.getByRole('combobox').locator('option:checked')).toHaveText('Acme docs · acme');
  const localSettings = await settings(page, 'handbook');
  await localSettings.getByRole('button', { name: 'Manage credentials' }).click();
  const manager = page.getByRole('dialog', { name: 'GitHub credentials', exact: true });
  await expect(manager.getByText('acme · 2 folders')).toBeVisible();
  const editButton = manager.getByRole('button', { name: 'Edit Acme docs' });
  const removeButton = manager.getByRole('button', { name: 'Remove Acme docs' });
  for (const action of [editButton, removeButton]) {
    await expect(action.locator('svg[aria-hidden="true"]')).toBeVisible();
    await expect(action).toHaveText('');
  }
  await editButton.click();
  const edit = page.getByRole('dialog', { name: 'Edit GitHub credential' });
  const replacement = edit.getByLabel('Replacement token (optional)');
  await expect(replacement).toHaveValue('');
  await replacement.fill(tokens.replacement);
  await edit.getByRole('button', { name: 'Save credential' }).click();
  await expect(edit).not.toBeVisible();
  accepted.set('acme', tokens.replacement);
  await manager.getByRole('button', { name: 'Close', exact: true }).click();
  const beforeRotation = traffic.length;
  await remoteSettings.getByRole('button', { name: 'Sync now' }).click();
  await expect(remoteSettings).not.toBeVisible();
  expect(traffic.slice(beforeRotation).filter(({ repo }) => repo === 'acme/guides')).not.toHaveLength(0);
  expect(
    traffic
      .slice(beforeRotation)
      .filter(({ repo }) => repo === 'acme/guides')
      .every(({ auth }) => auth === `Bearer ${tokens.replacement}`),
  ).toBe(true);
  remoteSettings = await settings(otherTab, 'guides');
  await localSettings.getByRole('button', { name: 'Manage credentials' }).click();
  await manager.getByRole('button', { name: 'Remove Acme docs' }).click();
  await page
    .getByRole('dialog', { name: 'Remove Acme docs?' })
    .getByRole('button', { name: 'Remove credential' })
    .click();
  await expect(manager.getByText('Acme docs', { exact: true })).toHaveCount(0);
  await expect(remoteSettings.getByRole('combobox').locator('option:checked')).toHaveText('Credential unavailable');
  const beforePause = traffic.length;
  await remoteSettings.getByRole('button', { name: 'Sync now' }).click();
  await expect(remoteSettings.getByRole('alert')).toContainText('no longer available');
  expect(traffic.slice(beforePause).filter(({ repo }) => repo === 'acme/guides')).toHaveLength(0);
  await remoteSettings.getByRole('button', { name: 'Close', exact: true }).click();
  await otherTab.getByRole('link', { name: 'About-guides', exact: true }).click();
  await expect(otherTab.locator('#previewArticle')).toContainText('guides');
  await otherTab.getByRole('button', { name: 'Source', exact: true }).click();
  await otherTab.evaluate(() => (window as EditorWindow).__suEditorAPI.setText('# Preserved draft\n'));
  await otherTab.getByRole('button', { name: 'Preview', exact: true }).click();
  await expect(otherTab.locator('#previewArticle')).toContainText('Preserved draft');
  await expect(otherTab.locator('#saveState')).toHaveText('Saved');
  await otherTab.reload();
  await expect(otherTab.locator('#previewArticle')).toContainText('Preserved draft');
  expect(traffic.slice(beforePause).filter(({ repo }) => repo === 'acme/guides')).toHaveLength(0);

  remoteSettings = await settings(otherTab, 'guides');
  await addCredential(otherTab, remoteSettings, tokens.replacement, 'Acme guides read-only');
  await expect(remoteSettings.getByRole('combobox').locator('option:checked')).toHaveText(
    'Acme guides read-only · acme',
  );
  await remoteSettings.getByLabel('Sync my edits back to GitHub').uncheck();
  await remoteSettings.getByRole('button', { name: 'Sync now' }).click();
  await expect(remoteSettings).not.toBeVisible();
  await expect(otherTab.locator('#previewArticle')).toContainText('Preserved draft');
  remoteSettings = await settings(otherTab, 'guides');
  await expect(remoteSettings.getByRole('combobox').locator('option:checked')).toHaveText(
    'Acme guides read-only · acme',
  );
  await expect(remoteSettings.getByLabel('Sync my edits back to GitHub')).not.toBeChecked();
  expect(leaks).toEqual([]);
});

test('Sync keeps a legacy folder token selected and lets you save it from the manager', async ({ page }) => {
  const token = 'github_pat_legacy_fixture';
  const root = 'legacy-test-root';
  const sha = '4'.repeat(40);
  const text = '# Existing draft\n';
  let requests = 0;
  await page.route('https://api.github.com/**', async (route) => {
    const headers = { 'access-control-allow-origin': '*', 'access-control-allow-headers': 'authorization' };
    if (route.request().method() === 'OPTIONS') return route.fulfill({ status: 204, headers });
    requests++;
    expect(route.request().headers().authorization).toBe(`Bearer ${token}`);
    const path = new URL(route.request().url()).pathname;
    if (path.endsWith('/branches/main'))
      return route.fulfill({
        headers,
        json: { commit: { sha: '1'.repeat(40), commit: { tree: { sha: '2'.repeat(40) } } } },
      });
    if (path.includes('/trees/'))
      return route.fulfill({
        headers,
        json: { tree: [{ path: 'Draft.md', type: 'blob', mode: '100644', sha, size: text.length }] },
      });
    return route.abort();
  });
  await page.goto('/');
  await page.getByRole('button', { name: 'New document, folder, or import' }).waitFor();
  await page.evaluate(
    async ({ root, token, sha, text }) => {
      const key = await crypto.subtle.generateKey({ name: 'AES-GCM', length: 256 }, false, ['encrypt', 'decrypt']);
      const iv = crypto.getRandomValues(new Uint8Array(12));
      const ciphertext = await crypto.subtle.encrypt(
        { name: 'AES-GCM', iv, additionalData: new TextEncoder().encode(root) },
        key,
        new TextEncoder().encode(token),
      );
      await new Promise<void>((resolve, reject) => {
        const request = indexedDB.open('mars');
        request.onerror = () => reject(request.error);
        request.onsuccess = () => {
          const db = request.result,
            tx = db.transaction(['nodes', 'docs'], 'readwrite');
          const fields = { order: 0, createdAt: 1, updatedAt: 1 };
          tx.objectStore('nodes').put({
            ...fields,
            id: root,
            kind: 'folder',
            name: 'Legacy notes',
            open: true,
            parentId: null,
            github: { repository: 'fake/notes', branch: 'main', write: true, authenticated: true },
          });
          tx.objectStore('nodes').put({
            ...fields,
            id: 'legacy-draft',
            kind: 'doc',
            name: 'Draft',
            parentId: root,
            githubFile: { root, path: 'Draft.md', sha },
          });
          tx.objectStore('docs').put({ key, iv, ciphertext }, `github-auth:${root}`);
          tx.objectStore('docs').put(text, 'legacy-draft');
          tx.objectStore('docs').put(text, 'github-base:legacy-draft');
          tx.oncomplete = () => {
            db.close();
            resolve();
          };
          tx.onerror = () => reject(tx.error);
        };
      });
    },
    { root, token, sha, text },
  );
  await page.reload();
  await expect.poll(() => requests).toBeGreaterThanOrEqual(2);
  await page.getByRole('button', { name: 'Actions for Legacy notes', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Sync', exact: true }).click();
  const settings = page.getByRole('dialog', { name: 'Sync', exact: true });
  const picker = settings.getByRole('combobox', { name: 'GitHub credential' });
  await expect(picker.locator('option:checked')).toHaveText('This folder’s token');
  await settings.getByRole('button', { name: 'Manage credentials' }).click();
  const manager = page.getByRole('dialog', { name: 'GitHub credentials', exact: true });
  await manager.getByRole('button', { name: 'Save as reusable…' }).click();
  const editor = page.getByRole('dialog', { name: 'Save a reusable credential' });
  await expect(editor.locator('input[type="password"]')).toHaveCount(0);
  await editor.getByLabel('Credential name').fill('Existing notes credential');
  const before = requests;
  await editor.getByRole('button', { name: 'Save credential' }).click();
  await manager.getByRole('button', { name: 'Close', exact: true }).click();
  await expect(picker.locator('option:checked')).toHaveText('Existing notes credential · fake');
  await expect(settings.getByLabel('Sync my edits back to GitHub')).toBeChecked();
  expect(requests).toBe(before);
});

test('a read-only GitHub document can be copied or disconnected without modifying GitHub', async ({ page }) => {
  const commit = '1'.repeat(40),
    tree = '2'.repeat(40),
    blob = '3'.repeat(40);
  const text = '# Public notes\n';
  const nested = '# Nested notes\n';
  const nestedBlob = '4'.repeat(40);
  let networkCalls = 0;
  const headers = { 'access-control-allow-origin': '*' };
  await page.route('https://api.github.com/**', (route) => {
    networkCalls++;
    expect(route.request().method()).toBe('GET');
    const path = new URL(route.request().url()).pathname;
    if (path === '/repos/fake/notes')
      return route.fulfill({ json: { full_name: 'fake/notes', default_branch: 'main', private: false }, headers });
    if (path.endsWith('/branches/main'))
      return route.fulfill({ json: { commit: { sha: commit, commit: { tree: { sha: tree } } } }, headers });
    if (path.includes('/trees/'))
      return route.fulfill({
        json: {
          tree: [
            { type: 'blob', mode: '100644', path: 'Notes.md', sha: blob, size: text.length },
            { type: 'blob', mode: '100644', path: 'Nested/Second.md', sha: nestedBlob, size: nested.length },
          ],
        },
        headers,
      });
    return route.abort();
  });
  await page.route(`https://raw.githubusercontent.com/fake/notes/${commit}/Notes.md`, (route) =>
    route.fulfill({ body: text, headers }),
  );
  await page.route(`https://raw.githubusercontent.com/fake/notes/${commit}/Nested/Second.md`, (route) =>
    route.fulfill({ body: nested, headers }),
  );
  await page.goto('/');
  await page.getByRole('button', { name: 'New document, folder, or import' }).click();
  await page.getByRole('menuitem', { name: 'Import from GitHub…' }).click();
  const importer = page.getByRole('dialog', { name: 'Import from GitHub' });
  await importer.getByLabel('Repository (required)').fill('fake/notes');
  await importer.getByRole('button', { name: 'Import folder' }).click();
  await expect(importer).not.toBeVisible();
  const root = page
    .getByRole('treeitem', { name: 'notes', exact: false })
    .filter({ has: page.getByRole('button', { name: 'Actions for notes', exact: true }) });
  await expect(root.locator('.tree-github-lock')).toBeVisible();
  await expect(root).toHaveAttribute('aria-description', /GitHub.*Read-only/);
  await page.getByRole('link', { name: 'Notes', exact: true }).click();
  const badge = page.locator('#documentAccess');
  await expect(badge).toBeVisible();
  await expect(badge).toHaveText('Read-only');
  await expect(badge).toHaveAttribute('data-tooltip', /enable write access in Sync/);
  const copy = page.getByRole('button', { name: 'Make a copy', exact: true });
  await copy.click();
  await expect(page.getByRole('link', { name: 'Notes (copy)', exact: true })).toBeVisible();
  await expect(badge).toBeHidden();
  await expect(copy).toBeHidden();
  await page.evaluate(() => (window as EditorWindow).__suEditorAPI.setText('# My editable copy\n'));
  await page.getByRole('button', { name: 'Preview', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'My editable copy' })).toBeVisible();
  await page.getByRole('link', { name: 'Notes', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Public notes' })).toBeVisible();
  await page.getByRole('button', { name: 'Source', exact: true }).click();
  await expect(badge).toBeVisible();
  const callsBeforeDisconnect = networkCalls;
  await page.getByRole('button', { name: 'Actions for Second', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Exclude from Sync', exact: true }).click();
  await expect(page.getByRole('link', { name: 'Second', exact: true })).toHaveCount(0);
  await page.getByRole('button', { name: 'Actions for Notes', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Sync', exact: true }).click();
  const settings = page.getByRole('dialog', { name: 'Sync', exact: true });
  await expect(settings.getByText('Read-only', { exact: true }).first()).toBeVisible();
  await settings.getByRole('button', { name: 'Disconnect this device' }).click();
  let confirmation = page.getByRole('dialog', { name: 'Disconnect this device?' });
  await confirmation.getByRole('button', { name: 'Cancel' }).click();
  await expect(settings).toBeVisible();
  await settings.getByRole('button', { name: 'Disconnect this device' }).click();
  confirmation = page.getByRole('dialog', { name: 'Disconnect this device?' });
  await expect(confirmation.getByText(/GitHub repository and other devices are unchanged/)).toBeVisible();
  await confirmation.getByRole('button', { name: 'Disconnect', exact: true }).click();
  await expect(settings).not.toBeVisible();
  await expect(badge).toBeHidden();
  await expect(page.getByRole('link', { name: 'Second', exact: true })).toBeVisible();
  await page.evaluate(() => (window as EditorWindow).__suEditorAPI.setText('# Disconnected notes\n'));
  await page.getByRole('button', { name: 'Preview', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Disconnected notes' })).toBeVisible();
  await page.getByRole('link', { name: 'Second', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Nested notes' })).toBeVisible();
  await page.reload();
  await expect(page.getByRole('heading', { name: 'Nested notes' })).toBeVisible();
  await page.getByRole('link', { name: 'Notes', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Disconnected notes' })).toBeVisible();
  expect(networkCalls).toBe(callsBeforeDisconnect);
});

test('a writable GitHub folder can add, duplicate, rename and delete files without changing the repository root', async ({
  page,
}) => {
  test.setTimeout(60_000);
  const { createHash } = await import('node:crypto');
  const oid = (n: number) => n.toString(16).repeat(40);
  const sha = (text: string) =>
    createHash('sha1')
      .update(`blob ${Buffer.byteLength(text)}\0${text}`)
      .digest('hex');
  let version = 1;
  const remote = new Map([
    ['Notes.md', '# Original notes\n'],
    ['keep.js', 'unchanged'],
  ]);
  const requests: { additions: { path: string; contents: string }[]; deletions?: { path: string }[] }[] = [];
  let gate: Promise<void> | null = null;
  await page.route('https://api.github.com/**', async (route) => {
    const request = route.request(),
      path = new URL(request.url()).pathname;
    const headers = {
      'access-control-allow-origin': '*',
      'access-control-allow-headers': 'authorization, content-type, if-none-match',
      'access-control-allow-methods': 'GET, POST, OPTIONS',
    };
    if (request.method() === 'OPTIONS') return route.fulfill({ status: 204, headers });
    let json: unknown;
    if (path === '/repos/fake/notes') json = { full_name: 'fake/notes', default_branch: 'main' };
    else if (path.endsWith('/branches/main')) {
      await gate;
      json = { commit: { sha: oid(version), commit: { tree: { sha: oid(version + 8) } } } };
    } else if (path.includes('/trees/'))
      json = {
        tree: [...remote].map(([name, text]) => ({
          path: name,
          mode: '100644',
          type: 'blob',
          sha: sha(text),
          size: Buffer.byteLength(text),
        })),
      };
    else if (path.includes('/blobs/')) {
      const text = [...remote.values()].find((value) => path.endsWith(sha(value)))!;
      json = { sha: sha(text), encoding: 'base64', content: Buffer.from(text).toString('base64') };
    } else if (path === '/graphql') {
      const { input } = request.postDataJSON().variables;
      expect(input.expectedHeadOid).toBe(oid(version));
      requests.push(input.fileChanges);
      for (const deletion of input.fileChanges.deletions ?? []) expect(remote.delete(deletion.path)).toBe(true);
      for (const item of input.fileChanges.additions)
        remote.set(item.path, Buffer.from(item.contents, 'base64').toString());
      json = { data: { createCommitOnBranch: { commit: { oid: oid(++version) } } } };
    } else return route.abort();
    return route.fulfill({ json, headers });
  });
  const action = async (item: string, entry: string) => {
    await page.getByRole('button', { name: `Actions for ${item}`, exact: true }).click();
    await page.getByRole('menuitem', { name: entry, exact: true }).click();
  };
  await page.goto('/');
  await page.getByRole('button', { name: 'New document, folder, or import' }).click();
  await page.getByRole('menuitem', { name: 'Import from GitHub…' }).click();
  const importer = page.getByRole('dialog', { name: 'Import from GitHub' });
  await importer.getByLabel('Repository (required)').fill('fake/notes');
  await addCredential(page, importer, 'github_pat_synthetic_test');
  await importer.getByLabel('Sync my edits back to GitHub').check();
  await importer.getByRole('button', { name: 'Import folder' }).click();
  await expect(importer).not.toBeVisible();
  await action('notes', 'New document');
  const create = page.getByRole('dialog', { name: 'New document' });
  await create.getByLabel('Document name').fill('Draft');
  await create.getByRole('button', { name: 'Create' }).click();
  await expect(page.getByRole('link', { name: 'Draft', exact: true })).toBeVisible();
  expect(remote.has('Draft.md')).toBe(true);
  await action('Notes', 'Duplicate');
  await expect(page.getByRole('link', { name: 'Notes (copy)', exact: true })).toBeVisible();
  expect(remote.get('Notes (copy).md')).toBe('# Original notes\n');
  await action('Notes (copy)', 'Rename');
  const rename = page.locator('.tree-rename');
  await rename.fill('Archive');
  let release!: () => void;
  gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  await rename.press('Enter');
  await expect(page.getByRole('link', { name: 'Archive', exact: true })).toBeVisible();
  await expect(
    page
      .getByRole('treeitem')
      .filter({ has: page.getByRole('link', { name: 'Archive', exact: true }) })
      .getByRole('button', { name: 'Pending Sync' }),
  ).toBeVisible();
  expect(remote.has('Notes (copy).md')).toBe(true);
  expect(remote.has('Archive.md')).toBe(false);
  remote.set('Archive.md', '# Existing upstream\n');
  version++;
  release();
  await expect(page.getByRole('button', { name: 'Rename blocked', exact: true })).toBeVisible({ timeout: 12_000 });
  expect(remote.get('Archive.md')).toBe('# Existing upstream\n');
  await action('Archive', 'Exclude from Sync');
  await expect(page.getByText(/These rules match a pending rename or deletion/)).toBeVisible();
  await expect(page.getByRole('link', { name: 'Archive', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Rename blocked', exact: true }).click();
  await page.getByRole('button', { name: 'Choose another name', exact: true }).click();
  const choose = page.getByRole('dialog', { name: 'Choose another name' });
  await choose.getByLabel('Name', { exact: true }).fill('Renamed notes');
  await choose.getByRole('button', { name: 'Rename', exact: true }).click();
  await page
    .getByRole('dialog', { name: 'Review pending changes' })
    .locator('header')
    .getByRole('button', { name: 'Close', exact: true })
    .click();
  await expect(page.getByRole('link', { name: 'Renamed notes', exact: true })).toBeVisible();
  await expect.poll(() => remote.has('Renamed notes.md'), { timeout: 12_000 }).toBe(true);
  expect(remote.has('Notes (copy).md')).toBe(false);
  expect(remote.get('Renamed notes.md')).toBe('# Original notes\n');
  await expect(page.getByRole('button', { name: 'Pending Sync', exact: true })).toHaveCount(0);
  await action('Renamed notes', 'Delete');
  const remove = page.getByRole('dialog', { name: 'Delete "Renamed notes"?' });
  await expect(remove.getByText(/deletes the selected files from the GitHub branch/)).toBeVisible();
  gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  await remove.getByRole('button', { name: 'Delete document' }).click();
  await expect(page.getByRole('link', { name: 'Renamed notes', exact: true })).toHaveCount(0);
  expect(remote.get('Renamed notes.md')).toBe('# Original notes\n');
  remote.set('Renamed notes.md', '# Edited upstream\n');
  version++;
  release();
  await expect(page.getByRole('button', { name: 'GitHub changes blocked', exact: true })).toBeVisible({
    timeout: 12_000,
  });
  await page.reload();
  await expect(page.getByRole('link', { name: 'Renamed notes', exact: true })).toHaveCount(0);
  await action('notes', 'Sync');
  await page.getByRole('button', { name: 'Review pending changes', exact: true }).click();
  const review = page.getByRole('dialog', { name: 'Review pending changes' });
  await expect(review.getByText('Delete blocked', { exact: true })).toBeVisible();
  await expect(review.getByRole('button', { name: 'Delete the latest version' })).toBeDisabled();
  await review.getByRole('button', { name: 'Compare changes' }).click();
  const compare = page.getByRole('dialog', { name: 'Compare changes' });
  await expect(compare.getByText('Local copy', { exact: true })).toBeVisible();
  await expect(compare.getByText('Edited upstream', { exact: false })).toBeVisible();
  await compare.getByRole('button', { name: 'Close', exact: true }).click();
  await review.getByRole('button', { name: 'Delete the latest version' }).click();
  const confirm = page.getByRole('dialog', { name: 'Delete the latest version?' });
  await confirm.getByRole('button', { name: 'Delete the latest version', exact: true }).click();
  await expect.poll(() => remote.has('Renamed notes.md'), { timeout: 12_000 }).toBe(false);
  expect(remote.get('Archive.md')).toBe('# Existing upstream\n');
  expect(remote.get('Notes.md')).toBe('# Original notes\n');
  expect(remote.get('keep.js')).toBe('unchanged');
  expect(requests).toHaveLength(4);
});

test('different names for a GitHub folder are resolved without a duplicate', async ({ page }) => {
  const { createHash } = await import('node:crypto');
  const text = '# Guide\n';
  const sha = createHash('sha1')
    .update(`blob ${Buffer.byteLength(text)}\0${text}`)
    .digest('hex');
  let name = 'agent-guides';
  let version = 1;
  let writes = 0;
  const oid = (n: number) => n.toString(16).repeat(40);
  await page.route('https://api.github.com/**', async (route) => {
    const path = new URL(route.request().url()).pathname;
    const headers = {
      'access-control-allow-origin': '*',
      'access-control-allow-headers': 'authorization, content-type, if-none-match',
      'access-control-allow-methods': 'GET, POST, OPTIONS',
    };
    if (route.request().method() === 'OPTIONS') return route.fulfill({ status: 204, headers });
    let json: unknown;
    if (path === '/repos/fake/notes') json = { full_name: 'fake/notes', default_branch: 'main' };
    else if (path.endsWith('/branches/main'))
      json = { commit: { sha: oid(version), commit: { tree: { sha: oid(version + 8) } } } };
    else if (path.includes('/trees/'))
      json = { tree: [{ path: `dev/${name}/Guide.md`, mode: '100644', type: 'blob', sha, size: text.length }] };
    else if (path.includes('/blobs/'))
      json = { sha, encoding: 'base64', content: Buffer.from(text).toString('base64') };
    else if (path === '/graphql') {
      writes++;
      return route.abort();
    } else return route.abort();
    return route.fulfill({ json, headers });
  });
  const menu = async (row: string, label: string) => {
    await page.getByRole('button', { name: `Actions for ${row}`, exact: true }).click();
    await page.getByRole('menuitem', { name: label, exact: true }).click();
  };
  await page.goto('/');
  await page.getByRole('button', { name: 'New document, folder, or import' }).click();
  await page.getByRole('menuitem', { name: 'Import from GitHub…' }).click();
  const importer = page.getByRole('dialog', { name: 'Import from GitHub' });
  await importer.getByLabel('Repository (required)').fill('fake/notes');
  await addCredential(page, importer, 'github_pat_synthetic_test');
  await importer.getByLabel('Sync my edits back to GitHub').check();
  await importer.getByRole('button', { name: 'Import folder' }).click();
  await expect(importer).not.toBeVisible();
  const guide = page.getByRole('link', { name: 'Guide', exact: true });
  const id = await guide.getAttribute('href');
  await menu('agent-guides', 'Rename');
  const input = page.locator('.tree-rename');
  await input.fill('agent-guides-mars');
  await input.press('Enter');
  name = 'agent-guides-repo';
  version++;
  await menu('notes', 'Sync');
  await page.getByRole('button', { name: 'Sync now', exact: true }).click();
  await page.getByRole('button', { name: 'Rename blocked', exact: true }).first().click();
  const review = page.getByRole('dialog', { name: 'Review pending changes' });
  await expect(review.getByText('Different names for the same item')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Actions for agent-guides-repo', exact: true })).toHaveCount(0);
  await review.getByRole('button', { name: 'Keep GitHub’s name', exact: true }).click();
  await expect(review).not.toBeVisible();
  await expect(page.getByRole('button', { name: 'Actions for agent-guides-repo', exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Actions for agent-guides-mars', exact: true })).toHaveCount(0);
  await expect(guide).toHaveCount(1);
  await expect(guide).toHaveAttribute('href', id!);
  expect(writes).toBe(0);
});

test('GitHub exclusions hide files and subtrees, skip sync, and restore cached drafts', async ({ page }) => {
  const { createHash } = await import('node:crypto');
  const remote = new Map([
    ['Draft.md', '# Draft\n\nOriginal draft.\n'],
    ['docs/archive/Old.md', '# Archived note\n'],
    ['docs/Guide.md', '# Guide\n'],
    ['docs/Keep.txt', 'Keep this.\n'],
    ['docs/Transcript.txt', 'Transcript.\n'],
  ]);
  const oid = (n: number) => n.toString(16).repeat(40);
  const blob = (text: string) =>
    createHash('sha1')
      .update(`blob ${Buffer.byteLength(text)}\0${text}`)
      .digest('hex');
  const downloaded: string[] = [];
  const writes: unknown[] = [];
  let version = 1;
  await page.route('https://api.github.com/**', async (route) => {
    const request = route.request();
    const path = new URL(request.url()).pathname;
    const headers = {
      'access-control-allow-origin': '*',
      'access-control-allow-headers': 'authorization, content-type, if-none-match',
      'access-control-allow-methods': 'GET, POST, OPTIONS',
    };
    if (request.method() === 'OPTIONS') return route.fulfill({ status: 204, headers });
    let json: unknown;
    if (path === '/repos/fake/notes') json = { full_name: 'fake/notes', default_branch: 'main', private: true };
    else if (path.endsWith('/branches/main'))
      json = { commit: { sha: oid(version), commit: { tree: { sha: oid(version + 8) } } } };
    else if (path.includes('/trees/'))
      json = {
        tree: [...remote].map(([path, text]) => ({
          type: 'blob',
          mode: '100644',
          path,
          sha: blob(text),
          size: Buffer.byteLength(text),
        })),
      };
    else if (path.includes('/blobs/')) {
      const sha = path.split('/').at(-1)!;
      const [name, text] = [...remote].find(([, value]) => blob(value) === sha)!;
      downloaded.push(name);
      json = { sha, encoding: 'base64', content: Buffer.from(text).toString('base64') };
    } else if (path === '/graphql') {
      const input = request.postDataJSON().variables?.input;
      if (!input) return route.fulfill({ status: 400, headers });
      writes.push(input);
      return route.abort();
    } else if (path.includes('/compare/')) json = { status: 'diverged' };
    else return route.abort();
    return route.fulfill({ json, headers });
  });
  const menu = async (row: string, label: string) => {
    await page.getByRole('button', { name: `Actions for ${row}`, exact: true }).click();
    await page.getByRole('menuitem', { name: label, exact: true }).click();
  };
  const settings = page.getByRole('dialog', { name: 'Sync', exact: true });
  const exclusions = page.getByRole('dialog', { name: 'Manage exclusions' });
  const rules = exclusions.getByLabel('Exclusion rules');
  const manage = async () => {
    await menu('notes', 'Sync');
    await settings.getByRole('button', { name: 'Manage exclusions' }).click();
  };
  const saveRules = async (text: string) => {
    await rules.fill(text);
    await exclusions.getByRole('button', { name: 'Save exclusions' }).click();
    await expect(exclusions).not.toBeVisible();
  };
  const link = (name: string) => page.getByRole('link', { name, exact: true });
  const archive = page.getByRole('button', { name: 'Actions for archive', exact: true });

  await page.goto('/');
  await page.getByRole('button', { name: 'New document, folder, or import' }).click();
  await page.getByRole('menuitem', { name: 'Import from GitHub…' }).click();
  const importer = page.getByRole('dialog', { name: 'Import from GitHub' });
  await importer.getByLabel('Repository (required)').fill('fake/notes');
  await addCredential(page, importer, 'github_pat_synthetic_test');
  await importer.getByLabel('Sync my edits back to GitHub').check();
  await importer.getByRole('button', { name: 'Import folder' }).click();
  await expect(importer).not.toBeVisible();
  const draftId = (await link('Draft').getAttribute('href'))!;
  await link('Draft').click();
  await page.getByRole('button', { name: 'Source', exact: true }).click();
  await page.evaluate(() => (window as EditorWindow).__suEditorAPI.setText('# Draft\n\nUnsynced local text.\n'));
  await page.getByRole('button', { name: 'Preview', exact: true }).click();
  await menu('Draft', 'Exclude from Sync');
  await expect(link('Draft')).toHaveCount(0);
  await expect(page).not.toHaveURL(new RegExp(`${draftId}$`));
  await menu('archive', 'Exclude from Sync');
  await expect(archive).toHaveCount(0);
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await expect(archive).toBeVisible();
  await expect(link('Old')).toBeVisible();
  await menu('archive', 'Exclude from Sync');
  await expect(archive).toHaveCount(0);
  await expect(link('Old')).toHaveCount(0);
  await expect(link('Guide')).toBeVisible();

  await manage();
  const exactRules = '/Draft.md\n/docs/archive/';
  await expect(rules).toHaveValue(exactRules);
  await rules.fill(`${exactRules}\n**/*.txt\n!/docs/Keep.txt`);
  await exclusions.getByRole('button', { name: 'Preview matches' }).click();
  await expect(exclusions.getByText('3 known files match.')).toBeVisible();
  await expect(exclusions.getByText('docs/archive/Old.md', { exact: true })).toBeVisible();
  const combined = await rules.inputValue();
  await saveRules(combined);
  await settings.getByRole('button', { name: 'Close', exact: true }).click();
  await expect(link('Keep')).toBeVisible();
  await expect(link('Transcript')).toHaveCount(0);

  downloaded.length = 0;
  remote.set('docs/archive/New.md', '# Hidden new file\n');
  remote.set('docs/New.txt', 'Hidden new transcript.\n');
  remote.set('docs/Moved.md', remote.get('docs/archive/Old.md')!);
  remote.delete('docs/archive/Old.md');
  remote.set('docs/Guide.md', '# Guide updated on GitHub\n');
  version++;
  await menu('notes', 'Sync');
  await settings.getByRole('button', { name: 'Sync now' }).click();
  await expect(settings).not.toBeVisible();
  await expect(link('Moved')).toBeVisible();
  expect(downloaded.sort()).toEqual(['docs/Guide.md', 'docs/Moved.md']);
  expect(writes).toHaveLength(0);
  await page.reload();
  await expect(link('Moved')).toBeVisible();
  await expect(link('Draft')).toHaveCount(0);
  await expect(archive).toHaveCount(0);
  await manage();
  await expect(rules).toHaveValue(combined);
  await saveRules(exactRules);
  await settings.getByRole('button', { name: 'Sync now' }).click();
  await expect(settings).not.toBeVisible();
  await expect(link('New')).toBeVisible();
  await expect(link('Transcript')).toBeVisible();
  expect(writes).toHaveLength(0);

  await manage();
  await saveRules('/docs/archive/');
  await settings.getByRole('button', { name: 'Close', exact: true }).click();
  await expect(link('Draft')).toHaveAttribute('href', draftId);
  await expect(link('Transcript')).toBeVisible();
  await link('Draft').click();
  await expect(page.locator('#previewArticle')).toContainText('Unsynced local text.');
  expect(writes).toHaveLength(0);

  const other = await page.context().newPage();
  await other.goto(draftId);
  await expect(other.locator('#previewArticle')).toContainText('Unsynced local text.');
  await menu('Draft', 'Exclude from Sync');
  await expect(other.getByRole('link', { name: 'Draft', exact: true })).toHaveCount(0);
  await expect(other).not.toHaveURL(new RegExp(`${draftId}$`));
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  const restored = other.getByRole('link', { name: 'Draft', exact: true });
  await expect(restored).toHaveAttribute('href', draftId);
  await restored.click();
  await expect(other.locator('#previewArticle')).toContainText('Unsynced local text.');
});

test('GitHub not-found errors distinguish public import from token access', async ({ page }) => {
  await page.route('https://api.github.com/**', (route) =>
    route.fulfill({ status: 404, headers: { 'access-control-allow-origin': '*' } }),
  );
  await page.goto('/');
  await page.getByRole('button', { name: 'New document, folder, or import' }).click();
  await page.getByRole('menuitem', { name: 'Import from GitHub…' }).click();
  const dialog = page.getByRole('dialog', { name: 'Import from GitHub' });
  await dialog.getByLabel('Repository (required)').fill('fake/notes');
  await dialog.getByRole('button', { name: 'Import folder' }).click();
  await expect(dialog.getByRole('alert')).toContainText('could not find a public repository or branch');
  await expect(dialog.getByRole('alert')).not.toContainText('token');
  await addCredential(page, dialog, 'github_pat_synthetic_test');
  await dialog.getByRole('button', { name: 'Import folder' }).click();
  await expect(dialog.getByRole('alert')).toContainText('whether your token has access');
});

test('GitHub tree follows edited renames and deletions after history is rewritten', async ({ page }) => {
  const { createHash } = await import('node:crypto');
  const sha = (text: string) =>
    createHash('sha1')
      .update(`blob ${Buffer.byteLength(text)}\0${text}`)
      .digest('hex');
  const exact = '# Notes\n\nThe exact file moved unchanged.\n';
  const before = '# Second\n\nInstall the required tools.\nUse the workspace directory.\nOld line.\n';
  const after = before.replace('Old line.', 'New line.');
  let current = 'a'.repeat(40);
  let tree = 'b'.repeat(40);
  let previousUnavailable = false;
  const remote = new Map([
    ['Old/Notes.md', exact],
    ['Old/Second.md', before],
    ['Remove.md', '# Deleted upstream\n'],
    ['Retain.md', '# Retain local\n'],
  ]);
  await page.route('https://api.github.com/**', async (route) => {
    const request = route.request(),
      path = new URL(request.url()).pathname;
    const headers = {
      'access-control-allow-origin': '*',
      'access-control-allow-headers': 'authorization, content-type, if-none-match',
      'access-control-allow-methods': 'GET, POST, OPTIONS',
      'access-control-expose-headers': 'etag',
      etag: `"${current}"`,
    };
    if (request.method() === 'OPTIONS') return route.fulfill({ status: 204, headers });
    let json: unknown;
    if (path === '/repos/fake/notes') json = { full_name: 'fake/notes', default_branch: 'main' };
    else if (path.endsWith('/branches/main')) json = { commit: { sha: current, commit: { tree: { sha: tree } } } };
    else if (path.includes('/compare/')) {
      previousUnavailable = true;
      return route.fulfill({ status: 404, headers, json: { message: 'Not Found' } });
    } else if (path.includes('/trees/')) {
      json = {
        tree: [...remote].map(([name, text]) => ({
          path: name,
          mode: '100644',
          type: 'blob',
          sha: sha(text),
          size: Buffer.byteLength(text),
        })),
      };
    } else if (path.includes('/blobs/')) {
      const text = [...remote.values()].find((value) => path.endsWith(sha(value)))!;
      json = { sha: sha(text), encoding: 'base64', content: Buffer.from(text).toString('base64') };
    } else if (path === '/graphql') {
      // GraphQL optionally batches private blob reads. REST fallback is also supported.
      const body = request.postDataJSON();
      expect(body.query).toContain('query MarsBlobs');
      const blobs = Object.fromEntries(
        Object.entries(body.variables)
          .filter(([key]) => key.startsWith('oid'))
          .map(([key, id]) => {
            const text = [...remote.values()].find((value) => sha(value) === id)!;
            return [`b${key.slice(3)}`, { oid: id, text, isBinary: false, isTruncated: false }];
          }),
      );
      json = { data: { repository: blobs } };
    } else return route.abort();
    return route.fulfill({ json, headers });
  });
  await page.goto('/');
  await page.getByRole('button', { name: 'New document, folder, or import' }).click();
  await page.getByRole('menuitem', { name: 'Import from GitHub…' }).click();
  const importer = page.getByRole('dialog', { name: 'Import from GitHub' });
  await importer.getByLabel('Repository (required)').fill('fake/notes');
  await addCredential(page, importer, 'github_pat_synthetic_test');
  await importer.getByLabel('Sync my edits back to GitHub').check();
  await importer.getByRole('button', { name: 'Import folder' }).click();
  await expect(importer).not.toBeVisible();
  const originalNotes = await page.getByRole('link', { name: 'Notes', exact: true }).getAttribute('href');
  const originalSecond = await page.getByRole('link', { name: 'Second', exact: true }).getAttribute('href');
  await page.getByRole('link', { name: 'Retain', exact: true }).click();
  await page.getByRole('button', { name: 'Source', exact: true }).click();
  await page.evaluate(() => (window as EditorWindow).__suEditorAPI.setText('# My unsynced edit\n'));
  await page.getByRole('button', { name: 'Preview', exact: true }).click();
  current = 'c'.repeat(40);
  tree = 'd'.repeat(40);
  remote.clear();
  remote.set('New/Notes.md', exact);
  remote.set('New/Renamed.md', after);
  await page.getByRole('button', { name: 'Actions for notes', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Sync', exact: true }).click();
  const sync = page.getByRole('dialog', { name: 'Sync', exact: true });
  await sync.getByRole('button', { name: 'Sync now' }).click();
  await expect(sync).not.toBeVisible();
  expect(previousUnavailable).toBe(true);
  await expect(page.getByRole('button', { name: 'Actions for Old', exact: true })).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Actions for New', exact: true })).toBeVisible();
  await expect(page.getByRole('link', { name: 'Notes', exact: true })).toHaveAttribute('href', originalNotes!);
  await expect(page.getByRole('link', { name: 'Renamed', exact: true })).toHaveAttribute('href', originalSecond!);
  await expect(page.getByRole('link', { name: 'Remove', exact: true })).toHaveCount(0);
  await expect(page.getByRole('link', { name: 'Second', exact: true })).toHaveCount(0);
  await expect(page.getByRole('link', { name: 'Retain (local edits)', exact: true })).toBeVisible();
  await expect(page.getByRole('heading', { name: /^My unsynced edit/ })).toBeVisible();
  await page.getByRole('link', { name: 'Renamed', exact: true }).click();
  await expect(page.locator('#previewArticle')).toContainText('New line.');
});

test('a rejected automatic push tells the user that their local edit is not synced', async ({ page }) => {
  const commit = '1'.repeat(40),
    tree = '2'.repeat(40),
    blob = '3'.repeat(40);
  const text = '# Original\n';
  await page.route('https://api.github.com/**', async (route) => {
    const path = new URL(route.request().url()).pathname;
    const headers = {
      'access-control-allow-origin': '*',
      'access-control-allow-headers': 'authorization, content-type',
      'access-control-allow-methods': 'GET, POST, OPTIONS',
    };
    if (route.request().method() === 'OPTIONS') return route.fulfill({ status: 204, headers });
    let json: unknown;
    if (path === '/repos/fake/notes') json = { full_name: 'fake/notes', default_branch: 'test' };
    else if (path.endsWith('/branches/test')) json = { commit: { sha: commit, commit: { tree: { sha: tree } } } };
    else if (path.includes('/trees/'))
      json = { tree: [{ type: 'blob', mode: '100644', path: 'Notes.md', sha: blob, size: text.length }] };
    else if (path.includes('/blobs/'))
      json = { sha: blob, encoding: 'base64', content: Buffer.from(text).toString('base64') };
    else if (path === '/graphql') {
      expect(route.request().postDataJSON().variables.input.branch).toEqual({
        repositoryNameWithOwner: 'fake/notes',
        branchName: 'test',
      });
      json = { errors: [{ type: 'FORBIDDEN', message: 'Raw GitHub details' }] };
    } else return route.abort();
    return route.fulfill({ json, headers });
  });
  await page.goto('/');
  await page.getByRole('button', { name: 'New document, folder, or import' }).click();
  await page.getByRole('menuitem', { name: 'Import from GitHub…' }).click();
  const importer = page.getByRole('dialog', { name: 'Import from GitHub' });
  await importer.getByLabel('Repository (required)').fill('fake/notes');
  await addCredential(page, importer, 'github_pat_synthetic_test');
  await importer.getByLabel('Sync my edits back to GitHub').check();
  await importer.getByRole('button', { name: 'Import folder' }).click();
  await expect(importer).not.toBeVisible();
  await page.getByRole('link', { name: 'Notes', exact: true }).click();
  await page.getByRole('button', { name: 'Source', exact: true }).click();
  await page.evaluate(() => (window as EditorWindow).__suEditorAPI.setText('# Still local\n'));
  await page.getByRole('button', { name: 'Preview', exact: true }).click();
  await expect(
    page.getByText(/Your edit is saved in MARS but not synced to GitHub.*Contents: Read and write/),
  ).toBeVisible({ timeout: 15_000 });
  await expect(page.getByText('Raw GitHub details')).toHaveCount(0);
  await page.reload();
  await expect(page.getByRole('heading', { name: 'Still local' })).toBeVisible();
});

test('GitHub sync merges independent edits and resolves an overlap after reload', async ({ page }) => {
  const token = 'github_pat_synthetic_test';
  let version = 1;
  let remote = '# Notes\n\nFirst original.\n\nMiddle original.\n\nLast original.\n';
  const writes: string[] = [];
  const oid = (n: number) => n.toString(16).repeat(40);
  await page.route('https://api.github.com/**', async (route) => {
    const request = route.request();
    const path = new URL(request.url()).pathname;
    const headers = {
      'access-control-allow-origin': '*',
      'access-control-allow-headers': 'authorization, content-type, if-none-match',
      'access-control-allow-methods': 'GET, POST, OPTIONS',
    };
    if (request.method() === 'OPTIONS') return route.fulfill({ status: 204, headers });
    let json: unknown;
    if (path === '/repos/fake/notes') json = { full_name: 'fake/notes', default_branch: 'main' };
    else if (path.endsWith('/branches/main'))
      json = { commit: { sha: oid(version), commit: { tree: { sha: oid(version + 5) } } } };
    else if (path.includes('/trees/'))
      json = {
        tree: [
          { type: 'blob', mode: '100644', path: 'Notes.md', sha: oid(version + 9), size: Buffer.byteLength(remote) },
        ],
      };
    else if (path.includes('/blobs/'))
      json = { sha: oid(version + 9), encoding: 'base64', content: Buffer.from(remote).toString('base64') };
    else if (path === '/graphql') {
      const body = request.postDataJSON().variables.input;
      expect(body.branch).toEqual({ repositoryNameWithOwner: 'fake/notes', branchName: 'main' });
      expect(body.expectedHeadOid).toBe(oid(version));
      remote = Buffer.from(body.fileChanges.additions[0].contents, 'base64').toString();
      writes.push(remote);
      json = { data: { createCommitOnBranch: { commit: { oid: oid(++version) } } } };
    } else return route.abort();
    await route.fulfill({ json, headers });
  });
  await page.goto('/');
  await page.getByRole('button', { name: 'New document, folder, or import' }).click();
  await page.getByRole('menuitem', { name: 'Import from GitHub…' }).click();
  const importer = page.getByRole('dialog', { name: 'Import from GitHub' });
  await importer.getByLabel('Repository (required)').fill('fake/notes');
  await addCredential(page, importer, token);
  await importer.getByLabel('Sync my edits back to GitHub').check();
  await importer.getByRole('button', { name: 'Import folder' }).click();
  await expect(importer).not.toBeVisible();
  await page.getByRole('link', { name: 'Notes', exact: true }).click();
  const edit = async (text: string) => {
    await page.getByRole('button', { name: 'Source', exact: true }).click();
    await page.evaluate((value) => (window as EditorWindow).__suEditorAPI.setText(value), text);
    await page.getByRole('button', { name: 'Preview', exact: true }).click();
  };
  const sync = async () => {
    await page.getByRole('button', { name: 'Actions for Notes', exact: true }).click();
    await page.getByRole('menuitem', { name: 'Sync', exact: true }).click();
    const settings = page.getByRole('dialog', { name: 'Sync', exact: true });
    await settings.getByRole('button', { name: 'Sync now' }).click();
    await expect(settings).not.toBeVisible();
  };
  await edit(remote.replace('First original.', 'Local first.'));
  remote = remote.replace('Last original.', 'GitHub last.');
  version++;
  await sync();
  expect(writes).toHaveLength(1);
  expect(writes[0]).toContain('Local first.');
  expect(writes[0]).toContain('GitHub last.');
  const originalViewport = page.viewportSize()!;
  for (const width of [1100, 320]) {
    await page.setViewportSize({ width, height: originalViewport.height });
    const changes = page.getByRole('group', { name: 'Changes since your last review' });
    await expect(changes).toBeVisible();
    for (const name of ['Compare changes', 'Mark reviewed']) {
      const action = changes.getByRole('button', { name });
      await expect(action).toBeInViewport();
      await expect(action).toHaveText('');
      await expect(action.locator('svg[aria-hidden="true"]')).toBeVisible();
    }
    const layout = await changes.evaluate((bar) => {
      const rect = bar.getBoundingClientRect();
      const toolbar = document.querySelector('.toolbar')!.getBoundingClientRect();
      const preview = document.querySelector('#previewPane')!.getBoundingClientRect();
      const controls = Array.from(bar.querySelectorAll('button:not([hidden])')).map((element) =>
        element.getBoundingClientRect(),
      );
      return {
        belowToolbar: rect.top >= toolbar.bottom,
        abovePreview: rect.bottom <= preview.top,
        controlsFit: controls.every((control) => control.left >= rect.left && control.right <= rect.right),
        centered: Math.abs(rect.left + rect.width / 2 - (preview.left + preview.width / 2)) < 1,
      };
    });
    expect(layout).toEqual({ belowToolbar: true, abovePreview: true, controlsFit: true, centered: true });
  }
  await page.setViewportSize(originalViewport);
  const compareChanges = page.getByRole('button', { name: 'Compare changes' });
  await compareChanges.hover();
  await expect(page.locator('#marsTooltip')).toHaveText('Compare changes');
  await compareChanges.click();
  const review = page.getByRole('dialog', { name: 'Compare changes' });
  await expect(review.getByText('Before your last review', { exact: true })).toBeVisible();
  await expect(review.getByText('Change 1 of 1', { exact: true })).toBeVisible();
  await review.getByRole('button', { name: 'Close', exact: true }).click();
  await edit(remote.replace('Middle original.', 'Mine.'));
  remote = remote.replace('Middle original.', 'Theirs.');
  version++;
  await sync();
  expect(writes).toHaveLength(1);
  await page.reload();
  await expect(page.locator('#previewArticle')).toContainText('Mine.');
  const banner = page.locator('#githubConflictBar');
  expect(
    await banner.evaluate(
      (bar) => bar.getBoundingClientRect().top >= document.querySelector('.toolbar')!.getBoundingClientRect().bottom,
    ),
  ).toBe(true);
  await page.getByRole('button', { name: 'Resolve conflicts', exact: true }).click();
  const resolver = page.getByRole('dialog', { name: 'Resolve conflicts' });
  await expect(resolver.getByText('Conflict 1 of 1 · 1 left')).toBeVisible();
  await expect(resolver.getByRole('button', { name: 'Apply changes' })).toBeDisabled();
  const result = resolver.locator('.cm-content[aria-label="Result"]');
  await expect(result).toContainText('GitHub last.');
  await expect(resolver.locator('.cm-content[aria-label="Local"]')).toContainText('Mine.');
  await expect(resolver.locator('.cm-content[aria-label="GitHub"]')).toContainText('Theirs.');
  await resolver.getByRole('button', { name: '← Use GitHub', exact: true }).click();
  await expect(result).toContainText('Theirs.');
  await resolver.getByRole('button', { name: 'Use local →', exact: true }).click();
  await expect(result).toContainText('Mine.');
  await result.fill(remote.replace('Theirs.', 'Manually resolved.'));
  await resolver.getByRole('button', { name: 'Mark resolved', exact: true }).click();
  await resolver.getByRole('button', { name: 'Apply changes' }).click();
  await expect(resolver).not.toBeVisible();
  await expect(page.locator('#previewArticle')).toContainText('Manually resolved.');
  await sync();
  expect(writes).toHaveLength(2);
  expect(writes[1]).toContain('Manually resolved.');
  expect(writes[1]).toContain('GitHub last.');
});

test('GitHub import exposes anonymous limits using the shared error callout', async ({ page }) => {
  await page.route('https://api.github.com/**', (route) =>
    route.fulfill({
      status: 403,
      headers: {
        'access-control-allow-origin': '*',
        'access-control-expose-headers': 'x-ratelimit-remaining, x-ratelimit-reset',
        'x-ratelimit-remaining': '0',
        'x-ratelimit-reset': String(Math.floor(Date.now() / 1000) + 300),
      },
    }),
  );
  await page.goto('/');
  await page.getByRole('button', { name: 'New document, folder, or import' }).click();
  await page.getByRole('menuitem', { name: 'Import from GitHub…' }).click();
  const dialog = page.getByRole('dialog', { name: 'Import from GitHub' });
  await dialog.getByLabel('Repository (required)').fill('fake/notes');
  await dialog.getByRole('button', { name: 'Import folder' }).click();
  await expect(dialog.getByRole('alert')).toContainText('anonymous limit for this IP address');
  await expect(dialog.getByRole('alert')).toContainText('Retry after');
  await expect(dialog.getByRole('button', { name: /ZIP/ })).toHaveCount(0);
});
