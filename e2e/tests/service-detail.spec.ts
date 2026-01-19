import { test, expect, createService, createServiceWithTrackingId, sendPixelTrack } from '../lib/fixtures.js';

test.describe('Service Detail', () => {
  test('shows tracker setup instructions for empty service', async ({ page, server }) => {
    // Create a service
    await createService(page, server.baseURL, 'Empty Service');

    // Service detail page should show setup instructions
    await expect(page.locator('h2:has-text("Get Started")')).toBeVisible();
    await expect(page.locator('text=Add this script to your website')).toBeVisible();
  });

  test('displays JavaScript tracker snippet', async ({ page, server }) => {
    const { trackingId } = await createServiceWithTrackingId(page, server.baseURL, 'Snippet Test');

    // Check for script snippet (app_{tracking_id}.js format)
    const scriptSnippet = page.locator('pre:has-text("app_")');
    await expect(scriptSnippet).toBeVisible();

    // Verify the snippet contains the tracking ID
    await expect(scriptSnippet).toContainText(trackingId);
    await expect(scriptSnippet).toContainText('.js');
  });

  test('displays pixel tracker snippet', async ({ page, server }) => {
    const { trackingId } = await createServiceWithTrackingId(page, server.baseURL, 'Pixel Test');

    // Check for pixel snippet (/trace/px_{tracking_id}.gif format)
    const pixelSnippet = page.locator('pre:has-text("px_")');
    await expect(pixelSnippet).toBeVisible();

    // Verify the snippet contains the tracking ID
    await expect(pixelSnippet).toContainText(trackingId);
    await expect(pixelSnippet).toContainText('/trace/');
  });

  test('shows stats cards when service has hits', async ({ page, server }) => {
    const { id: serviceId, trackingId } = await createServiceWithTrackingId(page, server.baseURL, 'Stats Service', {
      origins: '*',
    });

    // Send tracking requests to generate data
    await sendPixelTrack(server.baseURL, trackingId, {
      origin: 'http://example.com',
      userAgent: 'Mozilla/5.0 TestBrowser/1.0',
    });

    // Small delay to allow processing
    await page.waitForTimeout(500);

    // Reload the page
    await page.goto(`${server.baseURL}/service/${serviceId}`);

    // Should now show stats instead of setup instructions
    await expect(page.locator('.stat-card')).toHaveCount(6);

    // Check specific stat cards
    await expect(page.locator('text=Sessions').first()).toBeVisible();
    await expect(page.locator('text=Hits').first()).toBeVisible();
    await expect(page.locator('text=Load Time')).toBeVisible();
    await expect(page.locator('text=Bounce Rate')).toBeVisible();
  });

  test('displays date range picker', async ({ page, server }) => {
    const { id: serviceId, trackingId } = await createServiceWithTrackingId(page, server.baseURL, 'Date Range Service', {
      origins: '*',
    });

    // Send a tracking request to show the stats view
    await sendPixelTrack(server.baseURL, trackingId, { origin: 'http://example.com' });
    await page.waitForTimeout(500);
    await page.goto(`${server.baseURL}/service/${serviceId}`);

    // Check for date inputs
    await expect(page.locator('input#startDate')).toBeVisible();
    await expect(page.locator('input#endDate')).toBeVisible();
  });

  test('manage link navigates to settings', async ({ page, server }) => {
    const serviceId = await createService(page, server.baseURL, 'Manage Link Test');

    await page.click('a:has-text("Manage")');

    await expect(page).toHaveURL(`${server.baseURL}/service/${serviceId}/manage`);
    await expect(page.locator('h1')).toContainText('Manage Service');
  });

  test('displays service link when set', async ({ page, server }) => {
    await createService(page, server.baseURL, 'Linked Service', {
      link: 'https://example.com',
    });

    // Check for the link
    const link = page.locator('a[href="https://example.com"]');
    await expect(link).toBeVisible();
    await expect(link).toContainText('example.com');
  });
});
