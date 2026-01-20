import { test, expect, createServiceWithTrackingId, sendPixelTrack, sendScriptTrack } from '../lib/fixtures.js';

test.describe('Tracking Ingestion', () => {
  test('pixel tracker creates session', async ({ page, server }) => {
    const { id: serviceId, trackingId } = await createServiceWithTrackingId(page, server.baseURL, 'Pixel Track Service', {
      origins: '*',
    });

    // Send pixel tracking request
    const response = await sendPixelTrack(server.baseURL, trackingId, {
      origin: 'http://example.com',
      userAgent: 'Mozilla/5.0 PixelTest/1.0',
    });

    expect(response.status).toBe(200);
    expect(response.headers.get('content-type')).toBe('image/gif');

    await page.waitForTimeout(500);

    // Verify session was created
    await page.goto(`${server.baseURL}/service/${serviceId}/sessions`);
    await expect(page.locator('table tbody tr')).toHaveCount(1);
  });

  test('script POST creates hit with data', async ({ page, server }) => {
    const { id: serviceId, trackingId } = await createServiceWithTrackingId(page, server.baseURL, 'Script Track Service', {
      origins: '*',
    });

    // Send script tracking request
    const response = await sendScriptTrack(server.baseURL, trackingId, {
      idempotency: 'test-unique-key',
      location: '/test-page',
      referrer: 'https://google.com',
      loadTime: 250,
    }, {
      origin: 'http://example.com',
      userAgent: 'Mozilla/5.0 ScriptTest/1.0',
    });

    expect(response.status).toBe(200);
    const json = await response.json();
    expect(json.status).toBe('OK');

    await page.waitForTimeout(500);

    // Verify session and hit were created
    await page.goto(`${server.baseURL}/service/${serviceId}/sessions`);
    await page.click('table tbody tr:first-child a');

    // Check hit details
    await expect(page.locator('text=/test-page')).toBeVisible();
  });

  test('heartbeat with same idempotency key updates existing hit', async ({ page, server }) => {
    const { id: serviceId, trackingId } = await createServiceWithTrackingId(page, server.baseURL, 'Heartbeat Service', {
      origins: '*',
    });

    const idempotencyKey = 'heartbeat-test-key';
    const heartbeatCount = 5;

    // Send initial hit
    await sendScriptTrack(server.baseURL, trackingId, {
      idempotency: idempotencyKey,
      location: '/heartbeat-page',
      loadTime: 100,
    }, {
      origin: 'http://example.com',
      userAgent: 'Mozilla/5.0 HeartbeatTest/1.0',
    });

    await page.waitForTimeout(200);

    // Send multiple heartbeats with same idempotency key
    for (let i = 0; i < heartbeatCount; i++) {
      await sendScriptTrack(server.baseURL, trackingId, {
        idempotency: idempotencyKey,
        location: '/heartbeat-page',
        loadTime: 100,
      }, {
        origin: 'http://example.com',
        userAgent: 'Mozilla/5.0 HeartbeatTest/1.0',
      });
      await page.waitForTimeout(100);
    }

    await page.waitForTimeout(300);

    // Should still be just one session
    await page.goto(`${server.baseURL}/service/${serviceId}/sessions`);
    await expect(page.locator('table tbody tr')).toHaveCount(1);

    // Get session ID from the page
    const sessionLink = page.locator('table tbody tr:first-child a');
    const sessionHref = await sessionLink.getAttribute('href');
    const sessionId = sessionHref?.split('/').pop();

    // Verify via API that there's only 1 hit and heartbeats increased
    const hitsResponse = await fetch(`${server.baseURL}/api/sessions/${sessionId}/hits`);
    const hitsJson = await hitsResponse.json();

    expect(hitsJson.success).toBe(true);
    expect(hitsJson.data).toHaveLength(1); // Only 1 hit, not multiple
    expect(hitsJson.data[0].heartbeats).toBeGreaterThanOrEqual(heartbeatCount); // Heartbeats incremented
    expect(hitsJson.data[0].location).toBe('/heartbeat-page');
  });

  test('same IP and UA creates one session', async ({ page, server }) => {
    const { id: serviceId, trackingId } = await createServiceWithTrackingId(page, server.baseURL, 'Session Dedup Service', {
      origins: '*',
    });

    // Send multiple requests with same IP/UA
    await sendPixelTrack(server.baseURL, trackingId, {
      origin: 'http://example.com',
      userAgent: 'Mozilla/5.0 Dedup/1.0',
    });

    await page.waitForTimeout(200);

    await sendPixelTrack(server.baseURL, trackingId, {
      origin: 'http://example.com',
      userAgent: 'Mozilla/5.0 Dedup/1.0',
    });

    await page.waitForTimeout(500);

    // Should be only one session
    await page.goto(`${server.baseURL}/service/${serviceId}/sessions`);
    await expect(page.locator('table tbody tr')).toHaveCount(1);
  });

  test('DNT header with respect_dnt prevents tracking', async ({ page, server }) => {
    const { id: serviceId, trackingId } = await createServiceWithTrackingId(page, server.baseURL, 'DNT Respect Service', {
      origins: '*',
      respectDnt: true,
    });

    // Send request with DNT header
    const response = await sendPixelTrack(server.baseURL, trackingId, {
      origin: 'http://example.com',
      userAgent: 'Mozilla/5.0 DNTTest/1.0',
      dnt: true,
    });

    expect(response.status).toBe(200);

    await page.waitForTimeout(500);

    // Should not create any sessions
    await page.goto(`${server.baseURL}/service/${serviceId}/sessions`);
    await expect(page.locator('text=No sessions found')).toBeVisible();
  });

  test('DNT header ignored when respect_dnt is false', async ({ page, server }) => {
    const { id: serviceId, trackingId } = await createServiceWithTrackingId(page, server.baseURL, 'DNT Ignore Service', {
      origins: '*',
      respectDnt: false,
    });

    // Send request with DNT header
    await sendPixelTrack(server.baseURL, trackingId, {
      origin: 'http://example.com',
      userAgent: 'Mozilla/5.0 DNTIgnoreTest/1.0',
      dnt: true,
    });

    await page.waitForTimeout(500);

    // Should still create session
    await page.goto(`${server.baseURL}/service/${serviceId}/sessions`);
    await expect(page.locator('table tbody tr')).toHaveCount(1);
  });

  test('non-matching origin is rejected', async ({ page, server }) => {
    // Create service with specific origins
    const { trackingId } = await createServiceWithTrackingId(page, server.baseURL, 'CORS Test Service', {
      origins: 'https://allowed.com',
    });

    // Send request from non-allowed origin
    const response = await sendPixelTrack(server.baseURL, trackingId, {
      origin: 'https://notallowed.com',
    });

    // Should be rejected
    expect(response.status).toBe(403);
  });

  test('wildcard origin allows any origin', async ({ page, server }) => {
    const { id: serviceId, trackingId } = await createServiceWithTrackingId(page, server.baseURL, 'Wildcard Origin Service', {
      origins: '*',
    });

    // Send from any origin
    const response = await sendPixelTrack(server.baseURL, trackingId, {
      origin: 'https://any-domain.com',
      userAgent: 'Mozilla/5.0 WildcardTest/1.0',
    });

    expect(response.status).toBe(200);

    await page.waitForTimeout(500);

    // Should create session
    await page.goto(`${server.baseURL}/service/${serviceId}/sessions`);
    await expect(page.locator('table tbody tr')).toHaveCount(1);
  });
});
