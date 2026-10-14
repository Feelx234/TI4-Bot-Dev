import { registerOfflineLauncher } from "./registry.ts";
import { createMockLauncher, type MockOptions } from "./mockLauncher.ts";

/**
 * Dev builds only (App imports this behind import.meta.env.DEV): registers the in-memory
 * launcher when the page sets `window.__TI4_OFFLINE_MOCK__` (the screenshot captures do) or the
 * URL has `?offline-mock`. A real launcher registered later replaces it.
 */
declare global {
  interface Window {
    __TI4_OFFLINE_MOCK__?: MockOptions;
  }
}
const options = window.__TI4_OFFLINE_MOCK__;
if (options || new URLSearchParams(window.location.search).has("offline-mock"))
  registerOfflineLauncher(createMockLauncher(options ?? {}));
