/**
 * Deno Reverse Proxy for shymini local development
 *
 * Routes based on hostname:
 *   - fe.localhost:3000      → Frontend server (default: http://127.0.0.1:3333)
 *   - shymini.localhost:3000  → shymini backend (default: http://127.0.0.1:8080)
 *   - localhost:3000         → shymini backend (default)
 *
 * Usage:
 *   deno run --allow-net --allow-env --allow-read server.ts
 *
 * Environment variables:
 *   PROXY_PORT     - Port to listen on (default: 3000)
 *   BACKEND_URL    - shymini backend URL (default: http://127.0.0.1:8080)
 *   FRONTEND_URL   - Frontend server URL (default: http://127.0.0.1:3333)
 *   VERBOSE        - Enable verbose logging (default: false)
 */

import { dirname, fromFileUrl, join } from "https://deno.land/std@0.208.0/path/mod.ts";

const PROXY_PORT = parseInt(Deno.env.get("PROXY_PORT") || "3000");
const BACKEND_URL = Deno.env.get("BACKEND_URL") || "http://127.0.0.1:8080";
const FRONTEND_URL = Deno.env.get("FRONTEND_URL") || "http://127.0.0.1:3333";
const VERBOSE = Deno.env.get("VERBOSE") === "true";

// Get the directory where this script is located
const SCRIPT_DIR = dirname(fromFileUrl(import.meta.url));

// Headers to skip when proxying (hop-by-hop headers)
const HOP_BY_HOP_HEADERS = new Set([
  "connection",
  "keep-alive",
  "proxy-authenticate",
  "proxy-authorization",
  "te",
  "trailers",
  "transfer-encoding",
  "upgrade",
]);

function log(message: string, ...args: unknown[]) {
  const timestamp = new Date().toISOString();
  console.log(`[${timestamp}] ${message}`, ...args);
}

function verboseLog(message: string, ...args: unknown[]) {
  if (VERBOSE) {
    log(message, ...args);
  }
}

// Determine which upstream to use based on the Host header
function getUpstreamUrl(request: Request): string {
  const host = request.headers.get("host") || "";
  const hostname = host.split(":")[0].toLowerCase();

  if (hostname === "fe.localhost") {
    return FRONTEND_URL;
  }

  // Default to backend for shymini.localhost, analytics.localhost, localhost, etc.
  return BACKEND_URL;
}

async function proxyRequest(request: Request, upstreamUrl: string): Promise<Response> {
  const url = new URL(request.url);
  const targetUrl = new URL(url.pathname + url.search, upstreamUrl);

  verboseLog(`${request.method} ${url.host}${url.pathname} -> ${targetUrl}`);

  // Build headers for upstream request
  const headers = new Headers();
  for (const [key, value] of request.headers.entries()) {
    if (!HOP_BY_HOP_HEADERS.has(key.toLowerCase())) {
      headers.set(key, value);
    }
  }

  // Set X-Forwarded headers for proper client IP detection
  const clientIP = request.headers.get("x-forwarded-for") || "127.0.0.1";
  headers.set("X-Forwarded-For", clientIP);
  headers.set("X-Forwarded-Host", url.host);
  headers.set("X-Forwarded-Proto", url.protocol.replace(":", ""));
  headers.set("X-Real-IP", clientIP);

  // Preserve the original host for CORS validation
  const originalHost = request.headers.get("host");
  if (originalHost) {
    headers.set("X-Original-Host", originalHost);
  }

  try {
    // Forward the request to the upstream
    const upstreamResponse = await fetch(targetUrl.toString(), {
      method: request.method,
      headers,
      body: request.body,
      redirect: "manual", // Don't follow redirects, pass them through
    });

    // Build response headers
    const responseHeaders = new Headers();
    for (const [key, value] of upstreamResponse.headers.entries()) {
      if (!HOP_BY_HOP_HEADERS.has(key.toLowerCase())) {
        responseHeaders.set(key, value);
      }
    }

    // Add proxy identification header
    responseHeaders.set("X-Proxy", "shymini-dev-proxy");
    responseHeaders.set("X-Upstream", upstreamUrl);

    verboseLog(
      `  <- ${upstreamResponse.status} ${upstreamResponse.statusText}`
    );

    return new Response(upstreamResponse.body, {
      status: upstreamResponse.status,
      statusText: upstreamResponse.statusText,
      headers: responseHeaders,
    });
  } catch (error) {
    log(`Proxy error to ${upstreamUrl}: ${error}`);

    if (error instanceof TypeError && error.message.includes("connect")) {
      const serverName = upstreamUrl === FRONTEND_URL ? "Frontend" : "Backend";
      return new Response(
        `${serverName} server not available at ${upstreamUrl}\n\n` +
        (upstreamUrl === FRONTEND_URL
          ? `Start the frontend:\n  cd workspace/frontend && deno task fe\n`
          : `Start shymini:\n  cargo run\n`),
        {
          status: 502,
          headers: { "Content-Type": "text/plain" },
        }
      );
    }

    return new Response(`Proxy error: ${error}`, {
      status: 500,
      headers: { "Content-Type": "text/plain" },
    });
  }
}

async function handler(request: Request): Promise<Response> {
  const url = new URL(request.url);
  const host = request.headers.get("host") || "";
  const hostname = host.split(":")[0].toLowerCase();

  // Health check endpoint
  if (url.pathname === "/__proxy/health") {
    return new Response(JSON.stringify({
      status: "ok",
      backend: BACKEND_URL,
      frontend: FRONTEND_URL,
    }), {
      headers: { "Content-Type": "application/json" },
    });
  }

  // Test page for tracking verification (only on shymini host)
  if (url.pathname === "/__proxy/test" && hostname !== "fe.localhost") {
    try {
      const testPagePath = join(SCRIPT_DIR, "test-page.html");
      const html = await Deno.readTextFile(testPagePath);
      return new Response(html, {
        headers: { "Content-Type": "text/html" },
      });
    } catch (error) {
      return new Response(`Error loading test page: ${error}`, {
        status: 500,
        headers: { "Content-Type": "text/plain" },
      });
    }
  }

  // Proxy status page (only on shymini host)
  if (url.pathname === "/__proxy/status" && hostname !== "fe.localhost") {
    const html = `<!DOCTYPE html>
<html>
<head>
  <title>shymini Dev Proxy</title>
  <style>
    body { font-family: system-ui, sans-serif; max-width: 700px; margin: 50px auto; padding: 20px; }
    h1 { color: #4f46e5; }
    .info { background: #f3f4f6; padding: 15px; border-radius: 8px; margin: 20px 0; }
    code { background: #e5e7eb; padding: 2px 6px; border-radius: 4px; }
    a { color: #4f46e5; }
    table { width: 100%; border-collapse: collapse; margin: 1rem 0; }
    th, td { text-align: left; padding: 8px; border-bottom: 1px solid #e5e7eb; }
    th { background: #f9fafb; }
  </style>
</head>
<body>
  <h1>shymini Dev Proxy</h1>
  <div class="info">
    <p><strong>Proxy Port:</strong> ${PROXY_PORT}</p>
    <p><strong>Backend:</strong> ${BACKEND_URL}</p>
    <p><strong>Frontend:</strong> ${FRONTEND_URL}</p>
  </div>

  <h2>Routing</h2>
  <table>
    <tr><th>URL</th><th>Upstream</th></tr>
    <tr><td><a href="http://shymini.localhost:${PROXY_PORT}">http://shymini.localhost:${PROXY_PORT}</a></td><td>shymini Backend</td></tr>
    <tr><td><a href="http://fe.localhost:${PROXY_PORT}">http://fe.localhost:${PROXY_PORT}</a></td><td>Test Frontend</td></tr>
    <tr><td><a href="http://localhost:${PROXY_PORT}">http://localhost:${PROXY_PORT}</a></td><td>shymini Backend</td></tr>
  </table>

  <h2>Quick Links</h2>
  <ul>
    <li><a href="http://shymini.localhost:${PROXY_PORT}">shymini Dashboard</a></li>
    <li><a href="http://fe.localhost:${PROXY_PORT}">Test Frontend</a></li>
    <li><a href="/__proxy/test">Interactive Tracking Test</a></li>
  </ul>

  <h2>Tracking Script</h2>
  <p>Use this in your HTML (replace SERVICE_ID):</p>
  <pre><code>&lt;script defer src="http://shymini.localhost:${PROXY_PORT}/boot/{SERVICE_ID}.js"&gt;&lt;/script&gt;</code></pre>
</body>
</html>`;
    return new Response(html, {
      headers: { "Content-Type": "text/html" },
    });
  }

  // Route to appropriate upstream
  const upstreamUrl = getUpstreamUrl(request);
  return proxyRequest(request, upstreamUrl);
}

// Start the server
log(`Starting reverse proxy...`);
log(`  Listening on: http://localhost:${PROXY_PORT}`);
log(``);
log(`  Routing:`);
log(`    http://shymini.localhost:${PROXY_PORT}  →  ${BACKEND_URL} (shymini)`);
log(`    http://fe.localhost:${PROXY_PORT}      →  ${FRONTEND_URL} (Frontend)`);
log(`    http://localhost:${PROXY_PORT}         →  ${BACKEND_URL} (default)`);
log(``);
log(`  Status: http://localhost:${PROXY_PORT}/__proxy/status`);
log(``);
log(`Tip: *.localhost domains work in Chrome/Safari without any setup.`);
log(`     For Firefox, add 'shymini.localhost,fe.localhost' to network.dns.localDomains`);
log(``);

Deno.serve({ port: PROXY_PORT, hostname: "0.0.0.0" }, handler);
