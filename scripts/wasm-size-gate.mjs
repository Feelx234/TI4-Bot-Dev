// The size gate of the browser build. Prints the size of the wasm file (raw, gzip, brotli) and
// fails when one is over its budget in crates/ti4-wasm/size-budget.json.
//
//   cargo build -p ti4-wasm --target wasm32-unknown-unknown --profile wasm-release
//   node scripts/wasm-size-gate.mjs [file.wasm]
//
// Raise a budget on purpose, in the commit that makes the file larger, and say why in the
// message; a file that grows by itself is how a one-megabyte download becomes five.
// RATCHET (docs/ENGINE_CORE_STATE.md 6.3): gzip 2.4 MB / brotli 1.8 MB (was 1.8 / 1.3, already exceeded at
// 8f2d03e3) while the string and the core engine coexist in the binary. Lower both in the commit that deletes
// a string-engine area, down to the real size plus ~3%; the end state is below the old 1.8 / 1.3 MB.
// 2026-11-04 (heavy-prep, int-23 line, binaryen 123 wasm-opt): shipped file 6 432 131 / 2 041 438 / 1 469 073
// (raw/gzip/brotli); budget set to real +3% rounded up: 6.63 / 2.11 / 1.52 MB (was 7.0 / 2.4 / 1.8).
// 2026-11-06 (core-multiask-1, user decision: raise instead of trim): the multi-ask conversion (one multi-pick question per payment, purchase, discard, removal;
// the rule bot's multi_answer, the host batch mapping, the Choice helpers, render set_text/set_of) grew the shipped file from 6 624 713 / 2 106 859 / 1 516 834
// (int-28) to 6 655 307 / 2 120 897 / 1 527 345 (raw/gzip/brotli); budget raised from 6.63 / 2.11 / 1.52 to 6 660 000 / 2 126 000 / 1 532 000 (about 5 KB margin).
// 2026-11-06 (int-29): 6 630 000 / 2 110 000 / 1 520 000 -> 6 661 206 / 2 126 000 / 1 532 000. The user approved
// 6 660 000 / 2 126 000 / 1 532 000 for the multi-ask conversion (core-multiask-1, about +30 KB); chance-2 (randomness as
// a question, +5.9 KB raw alone) on top left the merged real build at 6 661 206 / 2 120 780 / 1 524 274, so raw is
// raised by exactly the 1 206 B left over after the cheap trims were assessed (docs/INTEGRATION29.md section 3).
import { readFileSync, appendFileSync } from "node:fs";
import { brotliCompressSync, constants, gzipSync } from "node:zlib";

const file = process.argv[2] ?? "target/wasm32-unknown-unknown/wasm-release/ti4_wasm.wasm";
const budgetFile = new URL("../crates/ti4-wasm/size-budget.json", import.meta.url);
const budget = JSON.parse(readFileSync(budgetFile, "utf8"));
const bytes = readFileSync(file);

const size = {
  raw: bytes.length,
  gzip: gzipSync(bytes, { level: 9 }).length,
  brotli: brotliCompressSync(bytes, {
    params: {
      [constants.BROTLI_PARAM_QUALITY]: 11,
      [constants.BROTLI_PARAM_SIZE_HINT]: bytes.length,
    },
  }).length,
};

const mb = (n) => (n / 1e6).toFixed(2).padStart(6);
const rows = ["raw", "gzip", "brotli"].map((kind) => {
  const over = size[kind] > budget[kind];
  return { kind, over, line: `${kind.padEnd(7)} ${mb(size[kind])} MB   budget ${mb(budget[kind])} MB   ${over ? "OVER" : "ok"}` };
});
console.log(`${file}\n${rows.map((r) => r.line).join("\n")}`);

if (process.env.GITHUB_STEP_SUMMARY) {
  appendFileSync(
    process.env.GITHUB_STEP_SUMMARY,
    `### ti4-wasm size\n\n| | size | budget |\n|---|---|---|\n${["raw", "gzip", "brotli"]
      .map((kind) => `| ${kind} | ${mb(size[kind])} MB | ${mb(budget[kind])} MB |`)
      .join("\n")}\n`,
  );
}
if (rows.some((r) => r.over)) {
  console.error("The wasm file is over its size budget (crates/ti4-wasm/size-budget.json).");
  process.exit(1);
}
