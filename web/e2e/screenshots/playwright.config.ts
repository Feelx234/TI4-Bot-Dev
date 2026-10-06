import { defineConfig, devices } from "@playwright/test";

// Screenshot captures render the dev gallery or a mocked game over websocket routes, so only the
// vite dev server is needed. Playwright starts it on its own free port and stops it afterwards.
const port = process.env.TI4_SHOT_PORT ?? String(20_000 + Math.floor(Math.random() * 30_000));

export default defineConfig({
  testDir: ".",
  testMatch: /capture-.*\.ts$/,
  outputDir: "/tmp/ti4-screenshot-runs",
  timeout: 60_000,
  workers: 1,
  fullyParallel: false,
  reporter: "list",
  use: {
    baseURL: `http://127.0.0.1:${port}`,
    viewport: { width: 1440, height: 900 },
    colorScheme: "dark",
    actionTimeout: 10_000,
    navigationTimeout: 15_000,
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"], viewport: { width: 1440, height: 900 } } }],
  webServer: {
    name: "frontend",
    command: `exec npx vite --host 127.0.0.1 --port ${port} --strictPort`,
    cwd: "../..",
    wait: { stdout: new RegExp(`Local:\\s+http://127\\.0\\.0\\.1:${port}/`) },
    timeout: 30_000,
    reuseExistingServer: false,
  },
});
