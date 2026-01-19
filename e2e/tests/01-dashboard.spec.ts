import { test, expect, createService } from '../lib/fixtures.js';

test.describe('Dashboard', () => {
  // Tests are ordered to respect shared server state (in-memory DB persists across tests)

  test('shows empty state when no services exist', async ({ page, server }) => {
    await page.goto(server.baseURL);

    // Check for empty state message
    await expect(page.locator('text=No services yet')).toBeVisible();
    await expect(page.locator('text=Create Service')).toBeVisible();

    // Verify "Create your first service" text is present
    await expect(page.locator('text=Create your first service')).toBeVisible();
  });

  test('can navigate to create service from empty state', async ({ page, server }) => {
    await page.goto(server.baseURL);

    // Click the Create Service button (must run before services are created)
    await page.click('a:has-text("Create Service")');

    // Should be on the create service page
    await expect(page).toHaveURL(`${server.baseURL}/service/new`);
    await expect(page.locator('h1')).toContainText('Create New Service');
  });

  test('shows service card after creating a service', async ({ page, server }) => {
    // Create a service first
    const serviceName = 'Dashboard Test Service';
    await createService(page, server.baseURL, serviceName);

    // Go back to dashboard
    await page.goto(server.baseURL);

    // Verify service card is displayed
    await expect(page.locator(`text=${serviceName}`)).toBeVisible();

    // Verify the card shows stats (even if 0)
    await expect(page.locator('text=Sessions (24h)')).toBeVisible();
    await expect(page.locator('text=Hits (24h)')).toBeVisible();

    // Verify the "No services yet" message is gone
    await expect(page.locator('text=No services yet')).not.toBeVisible();
  });

  test('clicking service card navigates to service detail', async ({ page, server }) => {
    // Create a service
    const serviceName = 'Clickable Service';
    const serviceId = await createService(page, server.baseURL, serviceName);

    // Go to dashboard
    await page.goto(server.baseURL);

    // Click on the service card
    await page.click(`a:has-text("${serviceName}")`);

    // Should navigate to service detail
    await expect(page).toHaveURL(`${server.baseURL}/service/${serviceId}`);
  });
});
