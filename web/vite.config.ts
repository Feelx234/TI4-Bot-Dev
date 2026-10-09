import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

import compactLayout from "./postcss-compact-layout.mjs";

export default defineConfig({
  css: { postcss: { plugins: [compactLayout()] } },
  plugins: [react()],
  server: {
    host: "127.0.0.1",
    port: 3000,
    proxy: {
      "/api": `http://127.0.0.1:${process.env.TI4_E2E_BACKEND_PORT ?? "8080"}`,
      "/ws": {
        target: `ws://127.0.0.1:${process.env.TI4_E2E_BACKEND_PORT ?? "8080"}`,
        ws: true,
      },
      "/advisor": {
        target: `http://127.0.0.1:${process.env.TI4_ADVISOR_PORT ?? "8081"}`,
        rewrite: (path) => path.replace(/^\/advisor/, ""),
      },
    },
  },
  test: {
    globals: true,
    environment: "jsdom",
    pool: "vmThreads",
    setupFiles: "./src/test/setup.ts",
    include: ["src/**/*.test.{ts,tsx}"],
  },
});
