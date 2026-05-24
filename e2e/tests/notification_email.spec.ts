import { expect, test } from '@playwright/test';

import { ensureUserAndLogin } from '../helpers';

test.describe('Notification email', () => {
  test('set, pending, resend cooldown, clear', async ({ request, page }) => {
    await ensureUserAndLogin(request, page);

    const NOTIFICATION_ADDRESS = 'alerts@example.com';

    // No notification email yet.
    const initial = await page.request.get('/api/user/notification_email');
    expect(initial.status()).toBe(200);
    const initialBody = await initial.text();
    expect(initialBody === '' || initialBody === 'null' || initialBody === '{}').toBeTruthy();

    // Self-pointing must be rejected.
    const stateResponse = await page.request.get('/api/ui/state');
    expect(stateResponse.ok()).toBeTruthy();
    const state = await stateResponse.json();
    const loginEmail = state.user.email as string;

    const selfPointing = await page.request.put('/api/user/notification_email', {
      data: { email: loginEmail },
    });
    expect(selfPointing.status()).toBe(400);

    // Set a custom address; the response describes the pending row.
    const setResponse = await page.request.put('/api/user/notification_email', {
      data: { email: NOTIFICATION_ADDRESS },
    });
    expect(setResponse.ok()).toBeTruthy();
    const record = await setResponse.json();
    expect(record.address).toBe(NOTIFICATION_ADDRESS);
    expect(record.kind).toBe('email');
    // Server returns timestamps, not booleans. A row freshly created by `set` has no
    // `verifiedAt`, no `unsubscribedAt`, and a `verificationExpiresAt` that is still in
    // the future.
    expect(record.verifiedAt).toBeUndefined();
    expect(record.unsubscribedAt).toBeUndefined();
    expect(typeof record.verificationExpiresAt).toBe('number');
    expect(record.verificationExpiresAt * 1000).toBeGreaterThan(Date.now());

    // The pending row is reflected in UiState.
    const stateAfterSet = await (await page.request.get('/api/ui/state')).json();
    expect(stateAfterSet.notificationEmail).toBeDefined();
    expect(stateAfterSet.notificationEmail.address).toBe(NOTIFICATION_ADDRESS);
    expect(stateAfterSet.notificationEmail.verifiedAt).toBeUndefined();
    expect(stateAfterSet.notificationEmail.unsubscribedAt).toBeUndefined();
    expect(typeof stateAfterSet.notificationEmail.verificationExpiresAt).toBe('number');

    // Immediate re-send is blocked by the 1-minute cooldown.
    const resend = await page.request.post('/api/user/notification_email/_resend');
    expect(resend.status()).toBe(400);

    // A wrong code increments the attempt counter and yields 400.
    const verify = await page.request.post('/api/user/notification_email/_verify', {
      data: { code: '000000' },
    });
    expect(verify.status()).toBe(400);

    // Clearing wipes the row immediately.
    const del = await page.request.delete('/api/user/notification_email');
    expect(del.ok()).toBeTruthy();
    const stateAfterClear = await (await page.request.get('/api/ui/state')).json();
    expect(stateAfterClear.notificationEmail).toBeUndefined();
  });

  test('public unsubscribe endpoint accepts unknown tokens silently', async ({ request, page }) => {
    await ensureUserAndLogin(request, page);

    // POST with a bogus token must succeed (we never leak whether the token exists).
    const post = await request.post('/api/notifications/unsubscribe', {
      data: { token: 'definitely-not-a-real-token' },
    });
    expect(post.ok()).toBeTruthy();

    // GET form (one-click) must also succeed.
    const get = await request.get('/api/notifications/unsubscribe?token=definitely-not-a-real-token');
    expect(get.ok()).toBeTruthy();

    // Missing token returns 400 on POST.
    const missing = await page.request.post('/api/notifications/unsubscribe', { data: {} });
    expect(missing.status()).toBe(400);
  });
});
