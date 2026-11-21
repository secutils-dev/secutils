import { expect, test } from '@playwright/test';

const baseURL = process.env.MARS_BASE_URL ?? 'http://localhost:7373';
test.use({ baseURL });
test.skip(
  !process.env.MARS_BASE_URL ||
    !['localhost', '127.0.0.1', '[::1]', 'mars-dev.secutils.dev'].includes(new URL(baseURL).hostname),
  'GitHub preview is enabled only on an explicitly selected local or dev host',
);

test('GitHub preview imports nested files and sends one commit with the expected branch head', async ({ page }) => {
  const token = 'github_pat_synthetic_test';
  const oldHead = '1'.repeat(40),
    tree = '2'.repeat(40),
    blob = '3'.repeat(40);
  const bodies: any[] = [];
  const leaked: string[] = [];
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
      'access-control-allow-headers': 'authorization, content-type',
      'access-control-allow-methods': 'GET, POST, OPTIONS',
    };
    if (request.method() === 'OPTIONS') {
      await route.fulfill({ status: 204, headers });
      return;
    }
    expect(request.headers().authorization).toBe(`Bearer ${token}`);
    let response: unknown;
    if (path === '/repos/fake/notes') response = { full_name: 'fake/notes', default_branch: 'preview' };
    else if (path.endsWith('/branches/preview')) response = { commit: { sha: head, commit: { tree: { sha: tree } } } };
    else if (path.includes('/trees/'))
      response = {
        tree: [{ type: 'blob', mode: '100644', path: 'Nested/Welcome.md', sha: file, size: remoteText.length }],
      };
    else if (path.includes('/blobs/'))
      response = { sha: file, encoding: 'base64', content: Buffer.from(remoteText).toString('base64') };
    else if (path === '/graphql') {
      bodies.push(request.postDataJSON().variables.input);
      response = { data: { createCommitOnBranch: { commit: { oid: '4'.repeat(40) } } } };
    } else {
      await route.abort();
      return;
    }
    await route.fulfill({ json: response, headers });
  });
  await page.goto('/');
  await page.getByRole('button', { name: 'New document, folder, or import' }).click();
  await page.getByRole('menuitem', { name: 'Import from GitHub…' }).click();
  const dialog = page.getByRole('dialog', { name: 'Import from GitHub' });
  await dialog.getByLabel('Repository (required)').fill('fake/notes');
  await dialog.getByLabel('Fine-grained personal access token').fill(token);
  await dialog.getByLabel('Allow commits to the selected branch').check();
  await dialog.getByRole('button', { name: 'Import folder' }).click();
  await expect(dialog).not.toBeVisible();
  await page.getByRole('link', { name: 'Welcome', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'GitHub document' })).toBeVisible();
  await page.getByRole('button', { name: 'Source', exact: true }).click();
  await page.evaluate(() => (window as any).__suEditorAPI.setText('# Edited from MARS\n'));
  await page.getByRole('button', { name: 'Preview', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Edited from MARS' })).toBeVisible();
  await expect.poll(() => bodies.length, { timeout: 15_000 }).toBe(1);
  expect(bodies[0]).toMatchObject({
    expectedHeadOid: oldHead,
    branch: { repositoryNameWithOwner: 'fake/notes', refName: 'refs/heads/preview' },
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
  head = '5'.repeat(40);
  file = '6'.repeat(40);
  remoteText = '# Edited from GitHub\n';
  await page.getByRole('button', { name: 'Actions for Welcome', exact: true }).click();
  await page.getByRole('menuitem', { name: /Sync with GitHub/ }).click();
  await expect(page.getByRole('heading', { name: 'Edited from GitHub' })).toBeVisible();
  await page.getByRole('button', { name: 'Share', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Copy link', exact: true }).click();
  await page.getByRole('radio', { name: 'Short', exact: true }).check();
  await expect(page.getByText(/uploads an encrypted snapshot to Secutils/)).toBeVisible();
  expect(leaked).toEqual([]);
});
