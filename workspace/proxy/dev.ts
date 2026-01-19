#!/usr/bin/env -S deno run --allow-run --allow-net --allow-env --allow-read

/**
 * Development runner - starts both shymini backend and the reverse proxy
 *
 * Usage:
 *   deno run --allow-run --allow-net --allow-env --allow-read proxy/dev.ts
 *
 * Or make executable:
 *   chmod +x proxy/dev.ts
 *   ./proxy/dev.ts
 */

import { resolve, dirname, fromFileUrl } from "https://deno.land/std@0.208.0/path/mod.ts";

const PROXY_PORT = parseInt(Deno.env.get("PROXY_PORT") || "3000");
const BACKEND_PORT = parseInt(Deno.env.get("BACKEND_PORT") || "8080");
const VERBOSE = Deno.env.get("VERBOSE") === "true";

const scriptDir = dirname(fromFileUrl(import.meta.url));
const projectRoot = resolve(scriptDir, "../..");

function log(message: string) {
  console.log(`[dev] ${message}`);
}

async function waitForServer(url: string, timeoutMs = 30000): Promise<boolean> {
  const startTime = Date.now();
  while (Date.now() - startTime < timeoutMs) {
    try {
      const response = await fetch(url);
      if (response.ok || response.status < 500) {
        return true;
      }
    } catch {
      // Server not ready yet
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  return false;
}

async function main() {
  log("Starting development environment...");
  log(`  Project root: ${projectRoot}`);
  log(`  Backend port: ${BACKEND_PORT}`);
  log(`  Proxy port:   ${PROXY_PORT}`);
  log("");

  // Check if backend is already running
  try {
    const response = await fetch(`http://127.0.0.1:${BACKEND_PORT}/`);
    if (response.ok || response.status < 500) {
      log(`Backend already running on port ${BACKEND_PORT}`);
    }
  } catch {
    log("Starting shymini backend...");

    // Start the backend server
    const backendCmd = new Deno.Command("cargo", {
      args: ["run"],
      cwd: projectRoot,
      env: {
        ...Deno.env.toObject(),
        SHYMINI__PORT: BACKEND_PORT.toString(),
        SHYMINI__HOST: "127.0.0.1",
        RUST_LOG: "info",
      },
      stdout: "inherit",
      stderr: "inherit",
    });

    const backendProcess = backendCmd.spawn();

    // Wait for backend to be ready
    log("Waiting for backend to start...");
    const backendReady = await waitForServer(`http://127.0.0.1:${BACKEND_PORT}/`);
    if (!backendReady) {
      log("ERROR: Backend failed to start within timeout");
      Deno.exit(1);
    }
    log("Backend is ready!");

    // Handle cleanup on exit
    Deno.addSignalListener("SIGINT", () => {
      log("Shutting down...");
      backendProcess.kill("SIGTERM");
      Deno.exit(0);
    });

    Deno.addSignalListener("SIGTERM", () => {
      log("Shutting down...");
      backendProcess.kill("SIGTERM");
      Deno.exit(0);
    });
  }

  log("");
  log("Starting reverse proxy...");
  log("");

  // Start the proxy (inline, not as subprocess)
  const proxyModule = await import("./server.ts");
  // The server starts automatically when imported
}

main().catch((err) => {
  console.error("Fatal error:", err);
  Deno.exit(1);
});
