import { execSync } from 'child_process';
import path from 'path';
import fs from 'fs';

const PROJECT_ROOT = path.resolve(__dirname, '..');
const BINARY_PATH = path.resolve(PROJECT_ROOT, 'target/release/shymini');

export default async function globalSetup() {
  console.log('Building shymini for E2E tests...');

  // Check if we need to build
  const needsBuild = !fs.existsSync(BINARY_PATH) ||
    process.env.SHYMINI_E2E_FORCE_BUILD === 'true';

  if (needsBuild) {
    try {
      execSync('cargo build --release', {
        cwd: PROJECT_ROOT,
        stdio: 'inherit',
        timeout: 300000, // 5 minute timeout for build
      });
      console.log('Build completed successfully.');
    } catch (error) {
      console.error('Failed to build shymini:', error);
      throw new Error('Build failed. Run `cargo build --release` in shymini directory.');
    }
  } else {
    console.log('Using existing binary (set SHYMINI_E2E_FORCE_BUILD=true to rebuild).');
  }

  // Verify binary exists
  if (!fs.existsSync(BINARY_PATH)) {
    throw new Error(`Binary not found at ${BINARY_PATH}. Build may have failed.`);
  }

  console.log('Global setup complete.');
}
