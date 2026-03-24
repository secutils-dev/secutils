import { readdirSync, readFileSync } from 'fs';
import { join, relative, resolve } from 'path';

import { expect, test } from '@playwright/test';

import { ensureUserAndLogin } from '../helpers';

const SAMPLES_DIR = resolve(__dirname, '../../components/secutils-docs/static/samples');

/** Recursively find all *.secutils.json files under a directory. */
function findSampleFiles(dir: string): string[] {
  const results: string[] = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      results.push(...findSampleFiles(full));
    } else if (entry.name.endsWith('.secutils.json')) {
      results.push(relative(SAMPLES_DIR, full));
    }
  }
  return results.sort();
}

const sampleFiles = findSampleFiles(SAMPLES_DIR);

const ENTITY_TYPES = [
  'scripts',
  'secrets',
  'responders',
  'certificateTemplates',
  'privateKeys',
  'contentSecurityPolicies',
  'pageTrackers',
  'apiTrackers',
] as const;

test.describe('Sample import files', () => {
  test.beforeEach(async ({ request, page }) => {
    await ensureUserAndLogin(request, page);
  });

  for (const file of sampleFiles) {
    test(`"${file}" imports successfully`, async ({ page }) => {
      const raw = readFileSync(resolve(SAMPLES_DIR, file), 'utf-8');
      const data = JSON.parse(raw);

      // 1. Validate top-level structure.
      expect(data.version).toBe(1);
      expect(data.data).toBeDefined();

      // 2. Preview should succeed.
      const previewRes = await page.request.post('/api/user/data/_import_preview', {
        data: { data, mode: 'merge' },
      });
      if (!previewRes.ok()) {
        const errorBody = await previewRes.text();
        expect(previewRes.ok(), `Preview failed (${previewRes.status()}): ${errorBody}`).toBeTruthy();
      }
      const preview = await previewRes.json();
      expect(preview.valid).toBe(true);
      expect(preview.warnings).toHaveLength(0);

      // 3. Build selections from preview (import all items).
      const selections: Record<string, Array<{ sourceId: string; action: string }>> = {};
      for (const entityType of ENTITY_TYPES) {
        const summary = preview.summary[entityType];
        if (!summary || !('total' in summary) || summary.total === 0) {
          selections[entityType] = [];
          continue;
        }

        const items = (data.data[entityType] ?? []) as Array<{ id: string }>;
        selections[entityType] = items.map((item) => ({
          sourceId: item.id,
          action: 'import',
        }));
      }

      const importSettings = preview.summary.settings?.included ?? false;

      // 4. Execute import.
      const importRes = await page.request.post('/api/user/data/_import', {
        data: {
          data,
          mode: 'merge',
          selections: { ...selections, importSettings },
        },
      });
      expect(importRes.ok()).toBeTruthy();
      const result = await importRes.json();

      // 5. Verify no failures across all entity types.
      for (const [entityType, entityResult] of Object.entries(result.results) as Array<
        [string, { imported: number; failed: number; errors: string[] }]
      >) {
        expect(entityResult.failed, `${entityType} should have no failures`).toBe(0);
        expect(entityResult.errors, `${entityType} should have no errors`).toHaveLength(0);
      }

      // 6. Clean up imported entities.
      const entityApiPaths: Record<string, string> = {
        scripts: '/api/user/scripts',
        responders: '/api/utils/webhooks/responders',
        certificateTemplates: '/api/utils/certificates/templates',
        privateKeys: '/api/utils/certificates/private_keys',
        contentSecurityPolicies: '/api/utils/web_security/csp',
        pageTrackers: '/api/utils/web_scraping/page',
        apiTrackers: '/api/utils/web_scraping/api',
      };

      for (const [entityType, apiPath] of Object.entries(entityApiPaths)) {
        const items = (data.data[entityType] ?? []) as Array<{ id: string }>;
        if (items.length === 0) continue;

        const listRes = await page.request.get(apiPath);
        if (!listRes.ok()) continue;
        const existing = await listRes.json();
        for (const item of existing) {
          await page.request.delete(`${apiPath}/${encodeURIComponent(item.id)}`);
        }
      }
    });
  }
});
