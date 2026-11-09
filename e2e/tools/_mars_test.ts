import { createHash, randomUUID } from 'node:crypto';

import { test as base, type BrowserContext } from '@playwright/test';

import { closeTestScope, installScopeFetch, openTestScope, type ScopeManifest } from '../mars-test-scopes';

export const test = base.extend<{ marsTestScope: { manifest: ScopeManifest; path: string } }>({
  marsTestScope: [
    async ({ browser, baseURL }, use, info) => {
      const origin = new URL(baseURL ?? process.env.MARS_BASE_URL ?? 'https://mars.secutils.dev').origin;
      const run = process.env.MARS_TEST_RUN_ID ?? randomUUID();
      const id = createHash('sha256')
        .update(`${info.testId}/${info.project.name}/${info.repeatEachIndex}/${info.retry}`)
        .digest('hex');
      const { manifest, path } = await openTestScope(origin, run, id, info.titlePath.join(' › '));
      const contexts: BrowserContext[] = [];
      const original = browser.newContext;
      browser.newContext = async (options) => {
        const context = await original.call(browser, options);
        contexts.push(context);
        await context.addInitScript(installScopeFetch, { origin, scope: manifest.scope, token: manifest.token });
        return context;
      };
      try {
        await use({ manifest, path });
      } finally {
        browser.newContext = original;
        await Promise.all(contexts.map((context) => context.close().catch(() => {})));
        const closed = await closeTestScope(path, origin);
        await info.attach('MARS test-scope cleanup', {
          contentType: 'application/json',
          body: Buffer.from(
            JSON.stringify({
              scope: closed.scope,
              run: closed.run,
              test: closed.test,
              state: closed.state,
              removed: closed.removed,
            }),
          ),
        });
      }
    },
    { auto: true },
  ],
});
