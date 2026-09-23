import { defineConfig, devices } from '@playwright/test';

const backendPort = process.env.TI4_E2E_BACKEND_PORT ?? '8080';
const frontendPort = process.env.TI4_E2E_FRONTEND_PORT ?? '3000';

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
    baseURL: `http://127.0.0.1:${frontendPort}`,
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
      wait: { stdout: new RegExp(`Listening on:\\s+http://127\\.0\\.0\\.1:${backendPort}`) },
      timeout: 5_000,
      env: {
        PORT: backendPort,
        HOST: '127.0.0.1',
        // Never recover developer sessions while starting the bounded E2E server.
        TI4_DATA_DIR: '/tmp/ti4-playwright-games',
      },
    },
    {
      name: 'frontend',
      command: `exec npx vite --host 127.0.0.1 --port ${frontendPort} --strictPort`,
      wait: { stdout: new RegExp(`Local:\\s+http://127\\.0\\.0\\.1:${frontendPort}/`) },
      timeout: 5_000,
    },
  ],
});
