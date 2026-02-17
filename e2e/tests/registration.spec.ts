import { test, expect } from '@playwright/test';

test.describe('User Registration', () => {
  test('can navigate to the registration page', async ({ page }) => {
    await page.goto('/');

    // Look for a signup/register link or button and click it.
    const signupLink = page.locator('button:has-text("Create account")');
    await signupLink.click();
    await expect(page).toHaveURL(/signup/);
  });

  test('registration form has required fields', async ({ page }) => {
    // Navigate directly to the Kratos registration flow.
    await page.goto('/signup');

    // The registration form should have email and password fields.
    await expect(page.locator('input[name="traits.email"], input[type="email"]')).toBeVisible({
      timeout: 15000,
    });
  });
});
