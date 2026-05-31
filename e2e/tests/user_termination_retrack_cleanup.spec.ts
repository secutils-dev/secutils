import type { Page } from '@playwright/test';
import { expect, test } from '@playwright/test';

import { ensureUserAndLogin, OPERATOR_TOKEN } from '../helpers';

// Retrack is exposed on the host by the e2e docker-compose stack (see dev/docker/docker-compose.yml). The tests talk to
// it directly to assert the server-side state, independent of what Secutils reports.
const RETRACK_URL = process.env.RETRACK_URL ?? 'http://localhost:7676';

// Secutils tags every Retrack tracker it owns with `secutils:user:<userId>` (see src/retrack/tags.rs). The user id
// itself is never serialized back to the client, so we discover the full tag from a created tracker instead.
const USER_TAG_PREFIX = 'secutils:user:';

interface RetrackTracker {
  id: string;
  tags: string[];
}

async function getRetrackTracker(page: Page, id: string): Promise<RetrackTracker | null> {
  const response = await page.request.get(`${RETRACK_URL}/api/trackers/${id}`);
  if (response.status() === 404) {
    return null;
  }
  expect(response.ok()).toBeTruthy();
  return (await response.json()) as RetrackTracker;
}

async function listRetrackTrackersByTag(page: Page, tag: string): Promise<RetrackTracker[]> {
  const response = await page.request.get(`${RETRACK_URL}/api/trackers?tag=${encodeURIComponent(tag)}`);
  expect(response.ok()).toBeTruthy();
  return (await response.json()) as RetrackTracker[];
}

test.describe('User termination cleans up Retrack trackers', () => {
  test('trackers exist in Retrack while the user lives and are removed on termination', async ({ request, page }) => {
    const { email } = await ensureUserAndLogin(request, page);

    // Create a mix of page and API trackers via the Secutils API. Each creation provisions a backing Retrack tracker,
    // whose id is returned under `retrack`.
    const trackerDefs = [
      {
        endpoint: '/api/web_scraping/page_trackers',
        data: {
          name: 'Cleanup Page Tracker 1',
          config: { revisions: 3 },
          target: { extractor: 'export async function execute() { return "<p>one</p>"; }' },
        },
      },
      {
        endpoint: '/api/web_scraping/page_trackers',
        data: {
          name: 'Cleanup Page Tracker 2',
          config: { revisions: 3 },
          target: { extractor: 'export async function execute() { return "<p>two</p>"; }' },
        },
      },
      {
        endpoint: '/api/web_scraping/api_trackers',
        data: {
          name: 'Cleanup API Tracker 1',
          config: { revisions: 3 },
          target: { url: 'https://secutils.dev/' },
        },
      },
    ];

    const retrackIds: string[] = [];
    for (const def of trackerDefs) {
      const createResponse = await page.request.post(def.endpoint, { data: def.data });
      expect(createResponse.ok(), `failed to create tracker via ${def.endpoint}`).toBeTruthy();

      const tracker = await createResponse.json();
      expect(tracker.retrack?.id, 'created tracker must reference a Retrack tracker id').toBeTruthy();
      retrackIds.push(tracker.retrack.id);
    }
    expect(retrackIds).toHaveLength(trackerDefs.length);

    // Discover the per-user tag from one of the freshly created Retrack trackers.
    const sampleTracker = await getRetrackTracker(page, retrackIds[0]);
    expect(sampleTracker, 'the first created Retrack tracker must exist').not.toBeNull();
    const userTag = sampleTracker!.tags.find((tag) => tag.startsWith(USER_TAG_PREFIX));
    expect(userTag, 'created Retrack tracker must carry a Secutils user tag').toBeTruthy();

    // Every created tracker must exist in Retrack (validating they were provisioned).
    for (const id of retrackIds) {
      const tracker = await getRetrackTracker(page, id);
      expect(tracker, `Retrack tracker ${id} must exist before termination`).not.toBeNull();
      expect(tracker!.id).toBe(id);
    }

    // Listing Retrack directly by the user tag must return exactly our trackers.
    const trackersBefore = await listRetrackTrackersByTag(page, userTag!);
    expect(trackersBefore.map((tracker) => tracker.id).sort()).toEqual([...retrackIds].sort());

    // Terminate the user via the operator endpoint - this must bulk-remove the user's Retrack trackers before the
    // Secutils/Kratos records are deleted.
    const removeResponse = await request.post('/api/users/remove', {
      headers: { Authorization: `Bearer ${OPERATOR_TOKEN}` },
      data: { email },
    });
    expect(removeResponse.ok(), 'user removal must succeed').toBeTruthy();

    // Retrack must no longer report any trackers for the (now deleted) user.
    await expect.poll(async () => (await listRetrackTrackersByTag(page, userTag!)).length, { timeout: 15000 }).toBe(0);

    // And each individual tracker must be gone (404) rather than merely untagged.
    for (const id of retrackIds) {
      const response = await page.request.get(`${RETRACK_URL}/api/trackers/${id}`);
      expect(response.status(), `Retrack tracker ${id} must be removed after termination`).toBe(404);
    }
  });
});
