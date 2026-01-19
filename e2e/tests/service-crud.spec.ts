import { test, expect, createService } from '../lib/fixtures.js';

test.describe('Service CRUD', () => {
  test('create service with minimal fields', async ({ page, server }) => {
    await page.goto(`${server.baseURL}/service/new`);

    // Fill only the required name field
    await page.fill('input[name="name"]', 'Minimal Service');
    await page.click('button[type="submit"]');

    // Should redirect to service detail
    await expect(page).toHaveURL(/\/service\/[0-9a-f-]+$/);
    await expect(page.locator('h1')).toContainText('Minimal Service');
  });

  test('create service with all fields', async ({ page, server }) => {
    await page.goto(`${server.baseURL}/service/new`);

    // Fill all fields
    await page.fill('input[name="name"]', 'Full Service');
    await page.fill('input[name="link"]', 'https://example.com');
    await page.fill('input[name="origins"]', 'https://example.com,https://www.example.com');

    // Privacy settings - uncheck defaults, check others
    await page.uncheck('input[name="respect_dnt"]');
    await page.check('input[name="ignore_robots"]');
    await page.uncheck('input[name="collect_ips"]');

    // Advanced settings
    await page.fill('input[name="ignored_ips"]', '192.168.1.0/24');
    await page.fill('input[name="hide_referrer_regex"]', '^https://internal\\..*');
    await page.fill('textarea[name="script_inject"]', 'console.log("injected");');

    await page.click('button[type="submit"]');

    // Should redirect to service detail
    await expect(page).toHaveURL(/\/service\/[0-9a-f-]+$/);
    await expect(page.locator('h1')).toContainText('Full Service');
    await expect(page.locator('a[href="https://example.com"]')).toBeVisible();
  });

  test('update service name and settings', async ({ page, server }) => {
    // Create a service first
    const serviceId = await createService(page, server.baseURL, 'Original Name');

    // Navigate to manage page
    await page.click('a:has-text("Manage")');
    await expect(page).toHaveURL(`${server.baseURL}/service/${serviceId}/manage`);

    // Update the name
    await page.fill('input[name="name"]', 'Updated Name');

    // Update privacy settings
    await page.check('input[name="ignore_robots"]');

    await page.click('button[type="submit"]');

    // Should redirect back to service detail with updated name
    await expect(page).toHaveURL(`${server.baseURL}/service/${serviceId}`);
    await expect(page.locator('h1')).toContainText('Updated Name');

    // Verify settings persisted by going back to manage
    await page.click('a:has-text("Manage")');
    await expect(page.locator('input[name="name"]')).toHaveValue('Updated Name');
    await expect(page.locator('input[name="ignore_robots"]')).toBeChecked();
  });

  test('delete service redirects to dashboard', async ({ page, server }) => {
    // Create a service first
    const serviceName = 'To Delete';
    await createService(page, server.baseURL, serviceName);

    // Navigate to manage, then delete
    await page.click('a:has-text("Manage")');
    await page.click('a:has-text("Delete Service")');

    // Should be on delete confirmation page
    await expect(page.locator('h1')).toContainText('Delete Service');
    await expect(page.locator(`text=${serviceName}`)).toBeVisible();

    // Confirm deletion
    await page.click('button:has-text("Delete Permanently")');

    // Should redirect to dashboard
    await expect(page).toHaveURL(server.baseURL + '/');

    // Service should be gone
    await expect(page.locator(`text=${serviceName}`)).not.toBeVisible();
  });

  test('cancel delete returns to manage page', async ({ page, server }) => {
    // Create a service first
    const serviceId = await createService(page, server.baseURL, 'Preserve Me');

    // Navigate to manage, then delete
    await page.click('a:has-text("Manage")');
    await page.click('a:has-text("Delete Service")');

    // Cancel
    await page.click('a:has-text("Cancel")');

    // Should be back at manage page
    await expect(page).toHaveURL(`${server.baseURL}/service/${serviceId}/manage`);
  });

  test('form validation requires name field', async ({ page, server }) => {
    await page.goto(`${server.baseURL}/service/new`);

    // Try to submit without filling name
    await page.click('button[type="submit"]');

    // Should still be on the create page (HTML5 validation prevents submission)
    await expect(page).toHaveURL(`${server.baseURL}/service/new`);

    // The name input should have required attribute
    const nameInput = page.locator('input[name="name"]');
    await expect(nameInput).toHaveAttribute('required', '');
  });
});
