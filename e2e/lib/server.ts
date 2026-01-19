import { ChildProcess, spawn } from 'child_process';
import { createServer } from 'net';
import path from 'path';

const BINARY_PATH = path.resolve(__dirname, '../../target/release/shymini');
const SERVER_STARTUP_TIMEOUT = 15000;
const HEALTH_CHECK_INTERVAL = 100;

export interface ServerInstance {
  process: ChildProcess;
  port: number;
  baseURL: string;
}

/**
 * Find an available port by binding to port 0 and releasing it
 */
export async function getAvailablePort(): Promise<number> {
  return new Promise((resolve, reject) => {
    const server = createServer();
    server.listen(0, '127.0.0.1', () => {
      const address = server.address();
      if (address && typeof address !== 'string') {
        const port = address.port;
        server.close(() => resolve(port));
      } else {
        server.close(() => reject(new Error('Could not get port')));
      }
    });
    server.on('error', reject);
  });
}

/**
 * Wait for server to be ready by polling the health endpoint
 */
async function waitForServer(port: number, timeout: number): Promise<boolean> {
  const startTime = Date.now();

  while (Date.now() - startTime < timeout) {
    try {
      const response = await fetch(`http://127.0.0.1:${port}/`);
      if (response.ok || response.status === 200) {
        return true;
      }
    } catch {
      // Server not ready yet
    }
    await new Promise(resolve => setTimeout(resolve, HEALTH_CHECK_INTERVAL));
  }

  return false;
}

/**
 * Start a shymini server instance with in-memory SQLite database
 */
export async function startServer(port: number): Promise<ServerInstance> {
  const env = {
    ...process.env,
    SHYMINI__DATABASE_URL: 'sqlite::memory:',
    SHYMINI__PORT: port.toString(),
    SHYMINI__HOST: '127.0.0.1',
    RUST_LOG: 'warn',
  };

  const proc = spawn(BINARY_PATH, [], {
    env,
    stdio: ['ignore', 'pipe', 'pipe'],
    cwd: path.resolve(__dirname, '../..'),
  });

  // Capture output for debugging
  let stdout = '';
  let stderr = '';

  proc.stdout?.on('data', (data) => {
    stdout += data.toString();
  });

  proc.stderr?.on('data', (data) => {
    stderr += data.toString();
  });

  // Handle early exit
  const exitPromise = new Promise<never>((_, reject) => {
    proc.on('exit', (code) => {
      reject(new Error(`Server exited with code ${code}\nstdout: ${stdout}\nstderr: ${stderr}`));
    });
  });

  // Wait for server to be ready
  const ready = await Promise.race([
    waitForServer(port, SERVER_STARTUP_TIMEOUT),
    exitPromise,
  ]);

  if (!ready) {
    proc.kill('SIGKILL');
    throw new Error(`Server failed to start within ${SERVER_STARTUP_TIMEOUT}ms\nstdout: ${stdout}\nstderr: ${stderr}`);
  }

  return {
    process: proc,
    port,
    baseURL: `http://127.0.0.1:${port}`,
  };
}

/**
 * Stop a server instance
 */
export async function stopServer(instance: ServerInstance): Promise<void> {
  return new Promise((resolve) => {
    if (instance.process.killed) {
      resolve();
      return;
    }

    instance.process.on('exit', () => {
      resolve();
    });

    // Try graceful shutdown first
    instance.process.kill('SIGTERM');

    // Force kill after timeout
    setTimeout(() => {
      if (!instance.process.killed) {
        instance.process.kill('SIGKILL');
      }
    }, 5000);
  });
}

/**
 * Check if the binary exists
 */
export function binaryExists(): boolean {
  try {
    const fs = require('fs');
    return fs.existsSync(BINARY_PATH);
  } catch {
    return false;
  }
}
