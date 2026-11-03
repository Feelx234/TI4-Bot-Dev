import { defineConfig, mergeConfig } from "vite";
import base from "../../../vite.config";

// The app's vite config without the file watcher: captures never edit sources, and the watcher
// fails with ENOSPC on machines with a low inotify limit.
//
// The server transport has turn redo switched off (docs/DISABLE_XF123.md); the folders that show the
// redo bar run against a mocked server, so the capture build turns the capability back on
// (TI4_SHOT_TURN_REDO=0 keeps it off: the HJ folder shows that state).
export default mergeConfig(
  base,
  defineConfig({
    server: { watch: null },
    define: { "import.meta.env.VITE_TI4_SERVER_TURN_REDO": JSON.stringify(process.env.TI4_SHOT_TURN_REDO ?? "1") },
  }),
);
