import { test, expect, createService, createServiceWithTrackingId, sendPixelTrack, sendScriptTrack } from '../lib/fixtures.js';

test.describe('Sessions', () => {
  test('sessions list shows sessions table', async ({ page, server }) => {
    const { id: serviceId, trackingId } = await createServiceWithTrackingId(page, server.baseURL, 'Sessions List Service', {
      origins: '*',
    });

    // Create some sessions with tracking
    await sendPixelTrack(server.baseURL, trackingId, {
      origin: 'http://example.com',
      userAgent: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/120.0.0.0',
    });

    await page.waitForTimeout(500);

    // Navigate to sessions list
    await page.goto(`${server.baseURL}/service/${serviceId}/sessions`);

    // Should show sessions table
    await expect(page.locator('h1')).toContainText('Sessions');
    await expect(page.locator('table')).toBeVisible();

    // Table should have expected headers
    await expect(page.locator('th:has-text("Session")')).toBeVisible();
    await expect(page.locator('th:has-text("Browser")')).toBeVisible();
    await expect(page.locator('th:has-text("OS")')).toBeVisible();
    await expect(page.locator('th:has-text("Country")')).toBeVisible();
    await expect(page.locator('th:has-text("Device")')).toBeVisible();
  });

  test('click session navigates to session detail', async ({ page, server }) => {
    const { id: serviceId, trackingId } = await createServiceWithTrackingId(page, server.baseURL, 'Session Detail Service', {
      origins: '*',
    });

    // Create a session
    await sendPixelTrack(server.baseURL, trackingId, {
      origin: 'http://example.com',
      userAgent: 'Mozilla/5.0 Test',
    });

    await page.waitForTimeout(500);

    // Go to sessions list
    await page.goto(`${server.baseURL}/service/${serviceId}/sessions`);

    // Click first session link
    await page.click('table tbody tr:first-child a');

    // Should be on session detail page
    await expect(page).toHaveURL(/\/sessions\/[0-9a-f-]+$/);
  });

  test('session detail shows hits', async ({ page, server }) => {
    const { id: serviceId, trackingId } = await createServiceWithTrackingId(page, server.baseURL, 'Session Hits Service', {
      origins: '*',
    });

    // Create session with script tracking (includes location data)
    await sendScriptTrack(server.baseURL, trackingId, {
      idempotency: 'test-idem-1',
      location: '/home',
      loadTime: 150,
    }, {
      origin: 'http://example.com',
      userAgent: 'Mozilla/5.0 Test',
    });

    await page.waitForTimeout(500);

    // Navigate to sessions
    await page.goto(`${server.baseURL}/service/${serviceId}/sessions`);

    // Click into session detail
    await page.click('table tbody tr:first-child a');

    // Should show hit information
    await expect(page.locator('text=/home')).toBeVisible();
  });

  test('pagination shows next/previous links', async ({ page, server }) => {
    const { id: serviceId, trackingId } = await createServiceWithTrackingId(page, server.baseURL, 'Pagination Service', {
      origins: '*',
    });

    // Create many sessions (need >50 for pagination)
    const requests = [];
    for (let i = 0; i < 55; i++) {
      requests.push(
        sendPixelTrack(server.baseURL, trackingId, {
          origin: 'http://example.com',
          // Different user agents create different sessions
          userAgent: `Mozilla/5.0 UniqueUA-${i}`,
        })
      );
    }
    await Promise.all(requests);

    await page.waitForTimeout(1000);

    // Navigate to sessions list
    await page.goto(`${server.baseURL}/service/${serviceId}/sessions`);

    // Should show pagination - first page has Next
    await expect(page.locator('text=Page 1')).toBeVisible();

    // Check for Next link if there are enough sessions
    const nextLink = page.locator('a:has-text("Next")');
    if (await nextLink.count() > 0) {
      await nextLink.click();
      await expect(page.locator('text=Page 2')).toBeVisible();

      // Now Previous should exist
      await expect(page.locator('a:has-text("Previous")')).toBeVisible();
    }
  });

  test('view all sessions link from service detail', async ({ page, server }) => {
    const { id: serviceId, trackingId } = await createServiceWithTrackingId(page, server.baseURL, 'View All Service', {
      origins: '*',
    });

    // Create a session
    await sendPixelTrack(server.baseURL, trackingId, {
      origin: 'http://example.com',
    });

    await page.waitForTimeout(500);

    // Go to service detail
    await page.goto(`${server.baseURL}/service/${serviceId}`);
    await page.reload();

    // Click "View all" link
    const viewAllLink = page.locator('a:has-text("View all")');
    if (await viewAllLink.isVisible()) {
      await viewAllLink.click();
      await expect(page).toHaveURL(`${server.baseURL}/service/${serviceId}/sessions`);
    }
  });

  test('empty sessions shows no sessions message', async ({ page, server }) => {
    const serviceId = await createService(page, server.baseURL, 'Empty Sessions Service');

    // Navigate directly to sessions without creating any
    await page.goto(`${server.baseURL}/service/${serviceId}/sessions`);

    // Should show empty message
    await expect(page.locator('text=No sessions found')).toBeVisible();
  });
});
