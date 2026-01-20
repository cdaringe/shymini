import { test as base, expect as baseExpect, Page } from '@playwright/test';
import { startServer, stopServer, getAvailablePort, ServerInstance } from './server.js';

// Extend the base test with server management
export const test = base.extend<{}, { server: ServerInstance }>({
  // Worker-scoped fixture: one server per test file
  server: [async ({}, use) => {
    const port = await getAvailablePort();
    const server = await startServer(port);

    await use(server);

    await stopServer(server);
  }, { scope: 'worker' }],
});

// Service info returned from createService
export interface ServiceInfo {
  id: string;
  trackingId: string;
}

// Helper to create a service and return its ID and tracking ID
export async function createService(page: Page, baseURL: string, name: string, options?: {
  link?: string;
  origins?: string;
  respectDnt?: boolean;
  ignoreRobots?: boolean;
  collectIps?: boolean;
}): Promise<string> {
  const info = await createServiceWithTrackingId(page, baseURL, name, options);
  return info.id;
}

// Helper to create a service and return both service ID and tracking ID
export async function createServiceWithTrackingId(page: Page, baseURL: string, name: string, options?: {
  link?: string;
  origins?: string;
  respectDnt?: boolean;
  ignoreRobots?: boolean;
  collectIps?: boolean;
}): Promise<ServiceInfo> {
  await page.goto(`${baseURL}/service/new`);

  await page.fill('input[name="name"]', name);

  if (options?.link) {
    await page.fill('input[name="link"]', options.link);
  }

  if (options?.origins) {
    await page.fill('input[name="origins"]', options.origins);
  }

  // Handle checkboxes
  const respectDntCheckbox = page.locator('input[name="respect_dnt"]');
  if (options?.respectDnt === false) {
    await respectDntCheckbox.uncheck();
  } else if (options?.respectDnt === true) {
    await respectDntCheckbox.check();
  }

  const ignoreRobotsCheckbox = page.locator('input[name="ignore_robots"]');
  if (options?.ignoreRobots === true) {
    await ignoreRobotsCheckbox.check();
  } else if (options?.ignoreRobots === false) {
    await ignoreRobotsCheckbox.uncheck();
  }

  const collectIpsCheckbox = page.locator('input[name="collect_ips"]');
  if (options?.collectIps === false) {
    await collectIpsCheckbox.uncheck();
  } else if (options?.collectIps === true) {
    await collectIpsCheckbox.check();
  }

  await page.click('button[type="submit"]');

  // Wait for navigation to service detail page
  await page.waitForURL(/\/service\/[0-9a-f-]+/);

  // Extract service ID from URL (may have query params after the ID)
  const url = page.url();
  const idMatch = url.match(/\/service\/([0-9a-f-]+)/);
  if (!idMatch) {
    throw new Error(`Could not extract service ID from URL: ${url}`);
  }

  // Extract tracking_id from the page content (shown in tracker snippet)
  const pageContent = await page.content();
  const trackingIdMatch = pageContent.match(/app_([a-z0-9]+)\.js/);
  if (!trackingIdMatch) {
    throw new Error('Could not extract tracking ID from page content');
  }

  return {
    id: idMatch[1],
    trackingId: trackingIdMatch[1],
  };
}

// Helper to send a pixel tracking request
// trackingId is the 8-character alphanumeric ID (not the service UUID)
export async function sendPixelTrack(baseURL: string, trackingId: string, options?: {
  origin?: string;
  userAgent?: string;
  referer?: string;
  dnt?: boolean;
}): Promise<Response> {
  const headers: Record<string, string> = {};

  if (options?.origin) {
    headers['Origin'] = options.origin;
  }
  if (options?.userAgent) {
    headers['User-Agent'] = options.userAgent;
  }
  if (options?.referer) {
    headers['Referer'] = options.referer;
  }
  if (options?.dnt) {
    headers['DNT'] = '1';
  }

  return fetch(`${baseURL}/trace/px_${trackingId}.gif`, { headers });
}

// Helper to send a script POST tracking request
// trackingId is the 8-character alphanumeric ID (not the service UUID)
export async function sendScriptTrack(baseURL: string, trackingId: string, payload: {
  idempotency?: string;
  location?: string;
  referrer?: string;
  loadTime?: number;
}, options?: {
  origin?: string;
  userAgent?: string;
  dnt?: boolean;
}): Promise<Response> {
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
  };

  if (options?.origin) {
    headers['Origin'] = options.origin;
  }
  if (options?.userAgent) {
    headers['User-Agent'] = options.userAgent;
  }
  if (options?.dnt) {
    headers['DNT'] = '1';
  }

  return fetch(`${baseURL}/trace/app_${trackingId}.js`, {
    method: 'POST',
    headers,
    body: JSON.stringify(payload),
  });
}

// Re-export expect
export const expect = baseExpect;
