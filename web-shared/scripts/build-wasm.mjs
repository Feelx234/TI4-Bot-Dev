#!/usr/bin/env node
// Builds the engine (crates/ti4-wasm) for the browser and copies it to the file that an app
// imports. web (`web/scripts/build-wasm.mjs`) and web2 (`web2/scripts/build-wasm.sh`) call it with
// their file; both run the same cargo command, so they share one build.
//
//   node web-shared/scripts/build-wasm.mjs <out.wasm>         cargo build --profile wasm-release, then copy
//   node web-shared/scripts/build-wasm.mjs <out.wasm> --dev   --profile wasm-dev: not optimized, incremental
//   TI4_WASM_FILE=<file> node ...                             copy a prebuilt file (CI, the e2e build)
//   TI4_SKIP_WASM=1                                           do nothing: the build has no engine
//   TI4_WASM_OPT=0                                            do not run wasm-opt over the release build (below)
//   TI4_WASM_OPT=<path to wasm-opt>                           use that binary
//
// wasm-opt (binaryen, the `binaryen` devDependency of web/, or any `wasm-opt` on the PATH) is run over
// the release build, never over a prebuilt TI4_WASM_FILE and never over --dev. The flags are -Oz
// without inlining: raw -19%, gzip -6%, brotli -3%, and the same game (see "wasm-opt" in
// docs/BUILD_SIZES_AND_TIMES.md). Without the tool the cargo build is copied as it is, so a checkout
// that never ran `npm ci` still builds.
//
// Without cargo or the wasm32 target the script warns, removes <out.wasm> and exits 0, so a plain
// `npm run build` of web still works (the result simply has no offline mode). TI4_REQUIRE_WASM=1
// makes that an error (exit 1, <out.wasm> kept).
// Needs: rustup target add wasm32-unknown-unknown
import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, rmSync, statSync } from "node:fs";
import { delimiter, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const usage = "usage: node web-shared/scripts/build-wasm.mjs <out.wasm> [--dev]";

const args = process.argv.slice(2);
const dev = args.includes("--dev");
const files = args.filter((arg) => arg !== "--dev");
if (files.length !== 1 || files[0].startsWith("-")) {
  console.error(usage);
  process.exit(2);
}
const out = resolve(files[0]);
const profile = dev ? "wasm-dev" : "wasm-release";
const required = process.env.TI4_REQUIRE_WASM === "1";

function giveUp(message) {
  console.warn(`build-wasm: ${message}`);
  if (required) {
    process.exit(1);
  }
  rmSync(out, { force: true });
  console.warn("build-wasm: continuing without the offline engine");
  process.exit(0);
}

if (process.env.TI4_SKIP_WASM === "1") {
  giveUp("TI4_SKIP_WASM=1");
}

// The features of the module are those of rustc's wasm32-unknown-unknown (its `target_features`
// section): enabling exactly these keeps binaryen from using any other.
const WASM_OPT_FEATURES = [
  "bulk-memory",
  "bulk-memory-opt",
  "call-indirect-overlong",
  "multivalue",
  "mutable-globals",
  "nontrapping-float-to-int",
  "reference-types",
  "sign-ext",
].map((feature) => `--enable-${feature}`);
// `-Oz` inlines small functions; without that the file compresses better (inlining makes the code
// less regular: gzip and brotli get bigger even though the raw size is smaller).
const WASM_OPT_ARGS = [
  ...WASM_OPT_FEATURES,
  "-Oz",
  "--always-inline-max-function-size=0",
  "--flexible-inline-max-function-size=0",
  "--one-caller-inline-max-function-size=0",
  "--partial-inlining-ifs=0",
];

/** The wasm-opt binary to use, or null. TI4_WASM_OPT=0 (or off) turns it off. */
function findWasmOpt() {
  const wanted = process.env.TI4_WASM_OPT;
  if (wanted === "0" || wanted === "off") return null;
  if (wanted) return wanted;
  const candidates = [
    ...(process.env.PATH ?? "").split(delimiter).map((dir) => join(dir, "wasm-opt")),
    join(repo, "web/node_modules/.bin/wasm-opt"),
    join(repo, "web2/node_modules/.bin/wasm-opt"),
  ];
  return candidates.find((file) => file && existsSync(file)) ?? null;
}

let built = process.env.TI4_WASM_FILE;
let optimize = false;
if (!built) {
  const targetDir = process.env.CARGO_TARGET_DIR ?? join(repo, "target");
  const run = spawnSync(
    "cargo",
    ["build", "-p", "ti4-wasm", "--target", "wasm32-unknown-unknown", "--profile", profile],
    { cwd: repo, stdio: "inherit", env: process.env },
  );
  if (run.error || run.status !== 0) {
    giveUp("cargo build of ti4-wasm failed (is the wasm32-unknown-unknown target installed?)");
  }
  built = resolve(repo, targetDir, `wasm32-unknown-unknown/${profile}/ti4_wasm.wasm`);
  optimize = !dev;
}
if (!existsSync(built)) {
  giveUp(`${built} does not exist`);
}
mkdirSync(dirname(out), { recursive: true });
const wasmOpt = optimize ? findWasmOpt() : null;
if (wasmOpt) {
  const before = statSync(built).size;
  const opt = spawnSync(wasmOpt, [...WASM_OPT_ARGS, built, "-o", out], { stdio: "inherit" });
  if (opt.error || opt.status !== 0) {
    console.warn(`build-wasm: ${wasmOpt} failed; copying the cargo build as it is`);
    copyFileSync(built, out);
  } else {
    console.log(`build-wasm: wasm-opt ${(before / 1e6).toFixed(2)} MB -> ${(statSync(out).size / 1e6).toFixed(2)} MB`);
  }
} else {
  copyFileSync(built, out);
}
console.log(`build-wasm: ${built} -> ${out} (${(statSync(out).size / 1e6).toFixed(2)} MB)`);
