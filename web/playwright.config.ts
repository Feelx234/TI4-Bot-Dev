import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './e2e',
  timeout: 30_000,
  globalTimeout: 120_000,
  expect: {
    timeout: 5_000,
  },
  fullyParallel: false,
  workers: 1,
  reporter: 'list',
  use: {
    baseURL: 'http://127.0.0.1:3000',
    trace: 'on-first-retry',
    actionTimeout: 5_000,
    navigationTimeout: 5_000,
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
  webServer: [
    {
      name: 'backend',
      command: 'exec ../target/debug/server',
      // This environment accepts unopened loopback connections without replying, so an HTTP
      // readiness probe never returns. The startup banner prints only after a successful bind.
      wait: { stdout: /Listening on:\s+http:\/\/127\.0\.0\.1:8080/ },
      timeout: 5_000,
      env: {
        PORT: '8080',
        HOST: '127.0.0.1',
        // Never recover developer sessions while starting the bounded E2E server.
        TI4_DATA_DIR: '/tmp/ti4-playwright-games',
        TI4_SEAT_LEASE_MS: '30000',
      },
    },
    {
      name: 'frontend',
      command: 'exec npx vite --host 127.0.0.1 --port 3000',
      wait: { stdout: /Local:\s+http:\/\/127\.0\.0\.1:3000\// },
      timeout: 5_000,
    },
  ],
});
