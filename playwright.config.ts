import { defineConfig, devices } from '@playwright/test';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const PORT = Number(process.env.RECONDO_E2E_PORT ?? 3199);
const BASE_URL = `http://127.0.0.1:${PORT}`;

// The web server process runs from this directory, so its redb database and
// `assets/` media copies land here — never in the repository tree. Run the
// suite from the repo root: `pnpm exec playwright test`.
const SERVER_DIR = join(tmpdir(), 'recondo-pw-server');
const CARGO_MANIFEST = join(process.cwd(), 'Cargo.toml');

export default defineConfig({
  testDir: './tests',
  /* Tests are independent: every upload uses unique file content. */
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  reporter: [['list'], ['html', { open: 'never' }]],
  use: {
    baseURL: BASE_URL,
    trace: 'on-first-retry',
  },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
  webServer: {
    /* Builds (incrementally) and starts the Rust server in an isolated dir. */
    command: `bash -c 'rm -rf "${SERVER_DIR}" && mkdir -p "${SERVER_DIR}" && cd "${SERVER_DIR}" && exec cargo run --quiet --manifest-path "${CARGO_MANIFEST}" -p recondo-web'`,
    url: `${BASE_URL}/upload`,
    reuseExistingServer: false,
    timeout: 120_000,
    env: {
      RECONDO_DB_PATH: join(SERVER_DIR, 'graph.redb'),
      RECONDO_ADDR: `127.0.0.1:${PORT}`,
    },
    stdout: 'pipe',
    stderr: 'pipe',
  },
});
