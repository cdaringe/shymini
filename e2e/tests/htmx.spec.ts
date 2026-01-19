import { test, expect, createServiceWithTrackingId, sendPixelTrack, sendScriptTrack } from '../lib/fixtures.js';

test.describe('HTMX Interactions', () => {
  test('date range change updates stats via HTMX', async ({ page, server }) => {
    const { id: serviceId, trackingId } = await createServiceWithTrackingId(page, server.baseURL, 'HTMX Date Range Service', {
      origins: '*',
    });

    // Create some tracking data
    await sendPixelTrack(server.baseURL, trackingId, {
      origin: 'http://example.com',
      userAgent: 'Mozilla/5.0 HTMXTest/1.0',
    });

    await page.waitForTimeout(500);

    // Go to service detail
    await page.goto(`${server.baseURL}/service/${serviceId}`);

    // Wait for stats to load
    await expect(page.locator('.stat-card')).toHaveCount(6);

    // Change the start date (using datetime-local format)
    const startDateInput = page.locator('input#startDate');

    // Set up response listener before triggering change
    const responsePromise = page.waitForResponse(resp => resp.url().includes('/stats'));

    // Set value and dispatch change event to trigger HTMX
    await startDateInput.evaluate((input: HTMLInputElement) => {
      input.value = '2024-01-01T00:00';
      input.dispatchEvent(new Event('change', { bubbles: true }));
    });

    // Wait for HTMX request to complete
    await responsePromise;

    // Stats should still be visible (HTMX updated the content)
    await expect(page.locator('.stat-card').first()).toBeVisible();
  });

  test('datetime-local inputs allow time selection', async ({ page, server }) => {
    const { id: serviceId, trackingId } = await createServiceWithTrackingId(page, server.baseURL, 'Datetime Selection Service', {
      origins: '*',
    });

    // Create some tracking data
    await sendPixelTrack(server.baseURL, trackingId, {
      origin: 'http://example.com',
      userAgent: 'Mozilla/5.0 DatetimeTest/1.0',
    });

    await page.waitForTimeout(500);

    // Go to service detail
    await page.goto(`${server.baseURL}/service/${serviceId}`);

    // Verify datetime-local inputs exist
    const startDateInput = page.locator('input#startDate');
    const endDateInput = page.locator('input#endDate');

    await expect(startDateInput).toBeVisible();
    await expect(endDateInput).toBeVisible();

    // Verify they are datetime-local type
    await expect(startDateInput).toHaveAttribute('type', 'datetime-local');
    await expect(endDateInput).toHaveAttribute('type', 'datetime-local');

    // Verify values contain time component (format: YYYY-MM-DDTHH:MM)
    const startValue = await startDateInput.inputValue();
    const endValue = await endDateInput.inputValue();

    // Values should match datetime-local format (contains 'T' separator and time)
    expect(startValue).toMatch(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}$/);
    expect(endValue).toMatch(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}$/);
  });

  test('datetime validation shows error for invalid range', async ({ page, server }) => {
    const { id: serviceId, trackingId } = await createServiceWithTrackingId(page, server.baseURL, 'Datetime Validation Service', {
      origins: '*',
    });

    // Create some tracking data
    await sendPixelTrack(server.baseURL, trackingId, {
      origin: 'http://example.com',
      userAgent: 'Mozilla/5.0 DatetimeValidationTest/1.0',
    });

    await page.waitForTimeout(500);

    // Go to service detail
    await page.goto(`${server.baseURL}/service/${serviceId}`);

    // Get the date inputs and error span
    const startDateInput = page.locator('input#startDate');
    const endDateInput = page.locator('input#endDate');
    const dateError = page.locator('#dateError');

    // Initially error should be hidden
    await expect(dateError).toBeHidden();

    // Set start date AFTER end date (invalid range)
    const endValue = await endDateInput.inputValue();
    // Parse and create a date that's definitely after the end date
    await startDateInput.fill('2099-12-31T23:59');

    // Trigger validation by changing focus
    await startDateInput.dispatchEvent('change');

    // Wait for validation to run
    await page.waitForTimeout(100);

    // Error should now be visible
    await expect(dateError).toBeVisible();

    // Start input should have error styling
    await expect(startDateInput).toHaveClass(/border-red-500/);
  });

  test('stats cards render with data', async ({ page, server }) => {
    const { id: serviceId, trackingId } = await createServiceWithTrackingId(page, server.baseURL, 'Stats Cards Service', {
      origins: '*',
    });

    // Create tracking data
    for (let i = 0; i < 5; i++) {
      await sendScriptTrack(server.baseURL, trackingId, {
        idempotency: `stats-card-${i}`,
        location: `/page-${i}`,
        loadTime: 100 + i * 50,
      }, {
        origin: 'http://example.com',
        userAgent: `Mozilla/5.0 StatsTest/1.${i}`,
      });
    }

    await page.waitForTimeout(1000);

    // Go to service detail
    await page.goto(`${server.baseURL}/service/${serviceId}`);

    // Verify all stat cards are present
    await expect(page.locator('text=Sessions').first()).toBeVisible();
    await expect(page.locator('text=Hits').first()).toBeVisible();
    await expect(page.locator('text=Load Time')).toBeVisible();
    await expect(page.locator('text=Bounce Rate')).toBeVisible();
    await expect(page.locator('text=Duration')).toBeVisible();
    await expect(page.locator('text=Hits/Session')).toBeVisible();
  });

  test('locations list shows top pages', async ({ page, server }) => {
    const { id: serviceId, trackingId } = await createServiceWithTrackingId(page, server.baseURL, 'Locations Service', {
      origins: '*',
    });

    // Create tracking data with various locations
    const locations = ['/home', '/about', '/contact', '/products', '/blog'];
    for (const loc of locations) {
      await sendScriptTrack(server.baseURL, trackingId, {
        idempotency: `loc-${loc}`,
        location: loc,
        loadTime: 150,
      }, {
        origin: 'http://example.com',
        userAgent: 'Mozilla/5.0 LocationTest/1.0',
      });
    }

    await page.waitForTimeout(1000);

    // Go to service detail
    await page.goto(`${server.baseURL}/service/${serviceId}`);

    // Check top pages section
    await expect(page.locator('h3:has-text("Top Pages")')).toBeVisible();

    // Should show at least some locations
    const topPagesSection = page.locator('.bg-white:has(h3:has-text("Top Pages"))');
    const rowCount = await topPagesSection.locator('table tbody tr').count();
    expect(rowCount).toBeGreaterThanOrEqual(1);
  });

  test('referrers section displays sources', async ({ page, server }) => {
    const { id: serviceId, trackingId } = await createServiceWithTrackingId(page, server.baseURL, 'Referrers Service', {
      origins: '*',
    });

    // Create tracking data with referrers
    await sendScriptTrack(server.baseURL, trackingId, {
      idempotency: 'ref-1',
      location: '/landing',
      referrer: 'https://google.com',
      loadTime: 150,
    }, {
      origin: 'http://example.com',
      userAgent: 'Mozilla/5.0 RefTest/1.0',
    });

    await sendScriptTrack(server.baseURL, trackingId, {
      idempotency: 'ref-2',
      location: '/landing',
      referrer: 'https://twitter.com',
      loadTime: 150,
    }, {
      origin: 'http://example.com',
      userAgent: 'Mozilla/5.0 RefTest/2.0',
    });

    await page.waitForTimeout(500);

    // Go to service detail
    await page.goto(`${server.baseURL}/service/${serviceId}`);

    // Check referrers section
    await expect(page.locator('h3:has-text("Referrers")')).toBeVisible();
  });

  test('browsers and OS sections display data', async ({ page, server }) => {
    const { id: serviceId, trackingId } = await createServiceWithTrackingId(page, server.baseURL, 'UA Stats Service', {
      origins: '*',
    });

    // Create tracking data with various user agents
    const userAgents = [
      'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/120.0.0.0',
      'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) Safari/605.1.15',
      'Mozilla/5.0 (X11; Linux x86_64; rv:120.0) Firefox/120.0',
    ];

    for (let i = 0; i < userAgents.length; i++) {
      await sendPixelTrack(server.baseURL, trackingId, {
        origin: 'http://example.com',
        userAgent: userAgents[i],
      });
    }

    await page.waitForTimeout(1000);

    // Go to service detail
    await page.goto(`${server.baseURL}/service/${serviceId}`);

    // Check browser section
    await expect(page.locator('h3:has-text("Browsers")')).toBeVisible();

    // Check OS section
    await expect(page.locator('h3:has-text("Operating Systems")')).toBeVisible();

    // Check device types section
    await expect(page.locator('h3:has-text("Device Types")')).toBeVisible();
  });

  test('recent sessions table in service detail', async ({ page, server }) => {
    const { id: serviceId, trackingId } = await createServiceWithTrackingId(page, server.baseURL, 'Recent Sessions Service', {
      origins: '*',
    });

    // Create tracking data
    for (let i = 0; i < 3; i++) {
      await sendPixelTrack(server.baseURL, trackingId, {
        origin: 'http://example.com',
        userAgent: `Mozilla/5.0 RecentTest/${i}`,
      });
    }

    await page.waitForTimeout(500);

    // Go to service detail
    await page.goto(`${server.baseURL}/service/${serviceId}`);

    // Check recent sessions section
    await expect(page.locator('h3:has-text("Recent Sessions")')).toBeVisible();

    // Should have session rows
    const sessionsTable = page.locator('.bg-white:has(h3:has-text("Recent Sessions"))');
    const sessionRowCount = await sessionsTable.locator('table tbody tr').count();
    expect(sessionRowCount).toBeGreaterThanOrEqual(1);
  });
});
