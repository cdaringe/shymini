/**
 * Dummy Frontend Server for testing shymini tracking
 *
 * Usage:
 *   deno run --allow-net --allow-env server.ts
 *
 * Access at:
 *   http://fe.localhost:3333
 *
 * Environment variables:
 *   FE_PORT         - Port to listen on (default: 3333)
 *   TRACKER_URL     - shymini tracker base URL (default: http://shymini.localhost:3000)
 *   SERVICE_ID      - shymini service UUID (required for tracking to work)
 */

const FE_PORT = parseInt(Deno.env.get("FE_PORT") || "3333");
const TRACKER_URL = Deno.env.get("TRACKER_URL") || "http://localhost:3000";
const SERVICE_ID = Deno.env.get("SERVICE_ID") || "app_fw5irz8t";

// Note: When accessed via proxy at fe.localhost:3000, the tracker at shymini.localhost:3000
// will be on the same port, enabling same-origin-like testing scenarios.

function log(message: string) {
  const timestamp = new Date().toISOString();
  console.log(`[${timestamp}] ${message}`);
}

// Generate the tracking script tag
function trackingScript(): string {
  if (SERVICE_ID === "YOUR_SERVICE_ID_HERE") {
    return `
    <!-- shymini tracking NOT configured. Set SERVICE_ID env var -->
    <script>console.warn('shymini: Set SERVICE_ID environment variable to enable tracking');</script>`;
  }
  return `
    <!-- shymini Analytics -->
    <script defer src="${TRACKER_URL}/trace/${SERVICE_ID}.js"></script>`;
}

// Common page layout
function layout(title: string, content: string): string {
  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>${title} - Test Site</title>
  <style>
    * { box-sizing: border-box; }
    body {
      font-family: system-ui, -apple-system, sans-serif;
      line-height: 1.6;
      margin: 0;
      padding: 0;
      background: #f8fafc;
      color: #1e293b;
    }
    nav {
      background: #4f46e5;
      padding: 1rem 2rem;
      display: flex;
      gap: 1.5rem;
      align-items: center;
    }
    nav a {
      color: white;
      text-decoration: none;
      font-weight: 500;
    }
    nav a:hover { text-decoration: underline; }
    nav .brand {
      font-size: 1.25rem;
      font-weight: bold;
      margin-right: auto;
    }
    main {
      max-width: 800px;
      margin: 2rem auto;
      padding: 0 1rem;
    }
    .hero {
      background: white;
      border-radius: 12px;
      padding: 3rem;
      text-align: center;
      box-shadow: 0 1px 3px rgba(0,0,0,0.1);
      margin-bottom: 2rem;
    }
    .hero h1 { margin-top: 0; color: #4f46e5; }
    .card {
      background: white;
      border-radius: 8px;
      padding: 1.5rem;
      margin-bottom: 1rem;
      box-shadow: 0 1px 3px rgba(0,0,0,0.1);
    }
    .card h2 { margin-top: 0; }
    .grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
      gap: 1rem;
    }
    .btn {
      display: inline-block;
      background: #4f46e5;
      color: white;
      padding: 0.75rem 1.5rem;
      border-radius: 6px;
      text-decoration: none;
      font-weight: 500;
    }
    .btn:hover { background: #4338ca; }
    footer {
      text-align: center;
      padding: 2rem;
      color: #64748b;
      font-size: 0.875rem;
    }
    .debug {
      background: #fef3c7;
      border: 1px solid #f59e0b;
      border-radius: 6px;
      padding: 1rem;
      margin: 1rem 0;
      font-size: 0.875rem;
    }
    .debug code {
      background: #fef9c3;
      padding: 0.125rem 0.25rem;
      border-radius: 3px;
    }
  </style>
  ${trackingScript()}
</head>
<body>
  <nav>
    <a href="/" class="brand">TestSite</a>
    <a href="/">Home</a>
    <a href="/about">About</a>
    <a href="/products">Products</a>
    <a href="/blog">Blog</a>
    <a href="/contact">Contact</a>
  </nav>
  <main>
    ${content}
  </main>
  <footer>
    <p>Test site for shymini tracking development</p>
    <p>Tracker: <code>${TRACKER_URL}</code> | Service: <code>${SERVICE_ID}</code></p>
  </footer>
</body>
</html>`;
}

// Page content generators
const pages: Record<string, () => string> = {
  "/": () => layout("Home", `
    <div class="hero">
      <h1>Welcome to TestSite</h1>
      <p>This is a dummy frontend for testing shymini analytics tracking.</p>
      <a href="/products" class="btn">View Products</a>
    </div>
    ${SERVICE_ID === "YOUR_SERVICE_ID_HERE" ? `
    <div class="debug">
      <strong>Tracking not configured!</strong><br>
      Set the <code>SERVICE_ID</code> environment variable to your shymini service UUID.<br>
      Example: <code>SERVICE_ID=abc-123-def deno task fe</code>
    </div>
    ` : ""}
    <div class="grid">
      <div class="card">
        <h2>Latest News</h2>
        <p>Check out our latest blog posts and updates.</p>
        <a href="/blog">Read More →</a>
      </div>
      <div class="card">
        <h2>Our Products</h2>
        <p>Explore our amazing product lineup.</p>
        <a href="/products">Browse →</a>
      </div>
      <div class="card">
        <h2>Get in Touch</h2>
        <p>Have questions? We'd love to hear from you.</p>
        <a href="/contact">Contact Us →</a>
      </div>
    </div>
  `),

  "/about": () => layout("About", `
    <div class="card">
      <h1>About Us</h1>
      <p>We are a fictional company created for testing web analytics.</p>
      <p>This page exists to test that shymini correctly tracks page views across different URLs.</p>
      <h2>Our Mission</h2>
      <p>To provide realistic test pages for analytics development and debugging.</p>
      <h2>Our Team</h2>
      <p>A group of dedicated test pages, working tirelessly to be visited.</p>
    </div>
  `),

  "/products": () => layout("Products", `
    <h1>Our Products</h1>
    <div class="grid">
      <div class="card">
        <h2>Widget Pro</h2>
        <p>The ultimate widget for all your widget needs.</p>
        <p><strong>$99.99</strong></p>
        <a href="/products/widget-pro" class="btn">Learn More</a>
      </div>
      <div class="card">
        <h2>Gadget Plus</h2>
        <p>A gadget that's plus-sized in features, not dimensions.</p>
        <p><strong>$149.99</strong></p>
        <a href="/products/gadget-plus" class="btn">Learn More</a>
      </div>
      <div class="card">
        <h2>Thing Enterprise</h2>
        <p>For when you need enterprise-grade things.</p>
        <p><strong>$299.99</strong></p>
        <a href="/products/thing-enterprise" class="btn">Learn More</a>
      </div>
    </div>
  `),

  "/products/widget-pro": () => layout("Widget Pro", `
    <div class="card">
      <h1>Widget Pro</h1>
      <p>The Widget Pro is our flagship widget, designed for professionals who demand the best.</p>
      <h2>Features</h2>
      <ul>
        <li>Advanced widgeting capabilities</li>
        <li>Premium widget materials</li>
        <li>24/7 widget support</li>
      </ul>
      <p><strong>Price: $99.99</strong></p>
      <a href="/products" class="btn">← Back to Products</a>
    </div>
  `),

  "/products/gadget-plus": () => layout("Gadget Plus", `
    <div class="card">
      <h1>Gadget Plus</h1>
      <p>The Gadget Plus takes gadgeting to the next level with its plus-sized feature set.</p>
      <h2>Features</h2>
      <ul>
        <li>Extra gadget functionality</li>
        <li>Plus-sized battery life</li>
        <li>Compact design</li>
      </ul>
      <p><strong>Price: $149.99</strong></p>
      <a href="/products" class="btn">← Back to Products</a>
    </div>
  `),

  "/products/thing-enterprise": () => layout("Thing Enterprise", `
    <div class="card">
      <h1>Thing Enterprise</h1>
      <p>When your business needs serious things, Thing Enterprise delivers.</p>
      <h2>Features</h2>
      <ul>
        <li>Enterprise-grade thing architecture</li>
        <li>Thing clustering support</li>
        <li>Advanced thing analytics</li>
      </ul>
      <p><strong>Price: $299.99</strong></p>
      <a href="/products" class="btn">← Back to Products</a>
    </div>
  `),

  "/blog": () => layout("Blog", `
    <h1>Blog</h1>
    <div class="card">
      <h2>Getting Started with Widgets</h2>
      <p><em>January 15, 2024</em></p>
      <p>Learn how to make the most of your new Widget Pro with these helpful tips.</p>
      <a href="/blog/getting-started-with-widgets">Read More →</a>
    </div>
    <div class="card">
      <h2>The Future of Gadgets</h2>
      <p><em>January 10, 2024</em></p>
      <p>Our predictions for where the gadget industry is heading in 2024.</p>
      <a href="/blog/future-of-gadgets">Read More →</a>
    </div>
    <div class="card">
      <h2>Why Things Matter</h2>
      <p><em>January 5, 2024</em></p>
      <p>A deep dive into the importance of things in modern business.</p>
      <a href="/blog/why-things-matter">Read More →</a>
    </div>
  `),

  "/blog/getting-started-with-widgets": () => layout("Getting Started with Widgets", `
    <div class="card">
      <h1>Getting Started with Widgets</h1>
      <p><em>January 15, 2024</em></p>
      <p>Congratulations on your new Widget Pro! Here's how to get the most out of it.</p>
      <h2>Step 1: Unbox Your Widget</h2>
      <p>Carefully remove the Widget Pro from its premium packaging.</p>
      <h2>Step 2: Initialize the Widget</h2>
      <p>Press and hold the widget button for 3 seconds until it widgets.</p>
      <h2>Step 3: Enjoy Your Widget</h2>
      <p>You're now ready to experience premium widgeting!</p>
      <a href="/blog" class="btn">← Back to Blog</a>
    </div>
  `),

  "/contact": () => layout("Contact", `
    <div class="card">
      <h1>Contact Us</h1>
      <p>We'd love to hear from you! Fill out the form below or reach us directly.</p>
      <form onsubmit="alert('Form submitted! (This is a test page)'); return false;">
        <p>
          <label><strong>Name</strong></label><br>
          <input type="text" style="width: 100%; padding: 0.5rem; border: 1px solid #ccc; border-radius: 4px;">
        </p>
        <p>
          <label><strong>Email</strong></label><br>
          <input type="email" style="width: 100%; padding: 0.5rem; border: 1px solid #ccc; border-radius: 4px;">
        </p>
        <p>
          <label><strong>Message</strong></label><br>
          <textarea rows="4" style="width: 100%; padding: 0.5rem; border: 1px solid #ccc; border-radius: 4px;"></textarea>
        </p>
        <button type="submit" class="btn">Send Message</button>
      </form>
    </div>
  `),
};

// 404 page
function notFound(): string {
  return layout("404 Not Found", `
    <div class="hero">
      <h1>404 - Page Not Found</h1>
      <p>The page you're looking for doesn't exist.</p>
      <a href="/" class="btn">Go Home</a>
    </div>
  `);
}

function handler(request: Request): Response {
  const url = new URL(request.url);
  const path = url.pathname;

  log(`${request.method} ${path}`);

  // Check if we have a page for this path
  const pageGenerator = pages[path];

  if (pageGenerator) {
    return new Response(pageGenerator(), {
      headers: { "Content-Type": "text/html; charset=utf-8" },
    });
  }

  // 404 for unknown paths
  return new Response(notFound(), {
    status: 404,
    headers: { "Content-Type": "text/html; charset=utf-8" },
  });
}

// Start server
log("Starting frontend test server...");
log(`  URL: http://fe.localhost:${FE_PORT}`);
log(`  Tracker URL: ${TRACKER_URL}`);
log(`  Service ID: ${SERVICE_ID}`);
if (SERVICE_ID === "YOUR_SERVICE_ID_HERE") {
  log("");
  log("  ⚠️  SERVICE_ID not set! Tracking will not work.");
  log("  Set it with: SERVICE_ID=your-uuid deno task fe");
}
log("");

Deno.serve({ port: FE_PORT, hostname: "0.0.0.0" }, handler);
