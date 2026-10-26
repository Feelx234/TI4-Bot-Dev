# Build sizes and build times

Measured on 2026-10-26 on the commit `48698d59` (`consolidated-2026-10-25`, the alpha) and on the
commits of branch `build-size-and-time` that follow it. Everything here can be regenerated:
see "How to reproduce" at the end (`tools/build_report.sh` and the helpers in
`tools/build_report/`).

**Conditions.** 8 cores, shared with the self-play sweeps of other agents: the load average was
22-35 during every measurement, so wall times are 2-4 times what an idle machine gives. The
comparable figure is **CPU seconds (user + sys)**, which moves by a few percent with the load, not
by multiples; wall time is given for orientation. Toolchain 1.94.1 (`rust-toolchain.toml`), the
default linker of that toolchain on x86-64 Linux is `rust-lld` (`-fuse-ld=lld`; checked with
`rustc --print link-args`), so "use a faster linker" is already done.

**Not built here.** The crates that need libtorch cannot be built on this machine and are left
out of everything below: `ti4-tensor` and `ti4-mlp` (both depend on `tch`), `ti4-advisor` (on
both), and the default features of `ti4-review` (`simulate`) and `ti4-replayer` (`host`) and the
`real-e2e` feature of `ti4-bot-agent`. `ti4-training`, `ti4-sim`, `ti4-cli`, `ti4-bridge`,
`ti4-legacy` and `xtask` do not need libtorch but are not part of the browser game, the server or
the self-play, and are only listed (see "Other binaries").

## A. Sizes

### A.1 What is built, and with which profile

| Output | Command | Profile (Cargo.toml) |
|---|---|---|
| `server` (deployed) | `cargo build --release -p ti4-server --bin server` (`deploy/public/deploy.sh`) | `release`: `lto = "thin"`, `codegen-units = 1`, `strip = true`, opt-level 3, panic = unwind |
| `selfplay` (rule-bot benches) | `cargo build --release -p ti4-selfplay` (`tools/rulebot/selfplay.sh`) | same |
| `ti4_wasm.wasm` (offline play) | `cargo build -p ti4-wasm --target wasm32-unknown-unknown --profile wasm-release` (`web-shared/scripts/build-wasm.mjs`) | `wasm-release`: inherits `release`, `opt-level = "z"`, `lto = "fat"`, `panic = "abort"` (still `strip`, `codegen-units = 1`) |
| web bundle | `cd web && npm run build` (`tsc && vite build`, `prebuild` builds the wasm) | vite defaults |
| tests (`cargo t-web`, `.cargo/config.toml`) | `cargo test -p <8 game crates> --lib --tests` | `dev`: `debug = "line-tables-only"`, `split-debuginfo = "packed"`, workspace crates at opt-level 0, **all non-member packages at opt-level 3** (`[profile.dev.package."*"]`, `debug = false`) |

### A.2 Native binaries

| File | bytes | `.text` | `.rodata` | unwind (`.eh_frame` + `.gcc_except_table` + `_hdr`) | `.rela.dyn` |
|---|---:|---:|---:|---:|---:|
| `target/release/server` (stripped) | 22 962 880 | 18 382 550 | 1 669 888 | 2 106 904 | 455 232 |
| `target/release/selfplay` (stripped) | 13 155 064 | 10 139 972 | 1 440 512 | 1 176 196 | 182 616 |
| `target/release/deps/libti4_wasm.so` | 15 209 816 | | | | |

(The `libti4_wasm.so` is the `cdylib` half of `crate-type = ["cdylib", "rlib"]` of `ti4-wasm`; it is
linked in every native build that has `ti4-wasm` in the graph, i.e. the self-play build and every
test build, and nothing uses it. In the test profile it is 83 MB + 34 MB of `.dwp`.)

Test-profile (debug) executables of `cargo t-web`, from a clean build (`ls -lS deps`):

| Executable | bytes | + `.dwp` |
|---|---:|---:|
| `it` of ti4-server (70 test modules in one binary) | 147 332 848 | 51 331 600 |
| `it` of ti4-wasm | 122 090 696 | 42 650 112 |
| `server` (debug binary the server tests start) | 105 742 248 | 33 719 760 |
| unit tests of ti4-engine | 104 139 672 | 26 704 160 |
| unit tests of ti4-server | 90 555 600 | 31 168 712 |
| unit tests of ti4-host | 73 950 944 | 23 651 120 |
| unit tests of ti4-wasm | 66 117 272 | 25.6 MB |
| `export_format1` (ti4-host integration test) | 61 506 320 | |
| unit tests of ti4-rule-bot / ti4-calc | 54 155 192 / 45 897 752 | |

**Why the release binaries are the size they are** (`cargo bloat`, symbols kept by building with
`CARGO_PROFILE_RELEASE_STRIP=false`; the figures are `.text` and add up to 17.5 MiB of `server`):

| crate | `server` .text | share |
|---|---:|---:|
| `ti4_engine` | 5.4 MiB | 30.7 % |
| `std` (= `core` + `alloc` generics instantiated for our types: `Vec`, iterators, `BTreeMap`, sort, fmt) | 5.0 MiB | 28.3 % |
| `serde_json` | 1.4 MiB | 8.2 % |
| `ti4_host` | 1.3 MiB | 7.4 % |
| `ti4_server` | 1.0 MiB | 5.9 % |
| `ti4_rule_bot` | 0.68 MiB | 3.9 % |
| `ti4_model` | 0.63 MiB | 3.6 % |
| `axum`, `tokio`, `hyper`, `http`, `tower*`, `tracing_subscriber` together | 1.0 MiB | 6 % |
| `ti4_content` | 187 KiB | 1.0 % |
| `regex_syntax` + `regex_automata` (`tracing-subscriber`'s `env-filter` via `matchers`) | 282 KiB | 1.6 % |

The function list is flat: the biggest function is the `Deserialize` of `GameState` (114 KiB),
then 68 KiB (`ClientMessage`), 50 KiB (`Player`), 50 KiB (`state_hash`), 47 KiB
(`agenda_effects::resolve_with`), and 16 467 smaller functions make up 15.9 MiB (90 %). The size
is the sum of a very large amount of ordinary game code, 271 000 lines in `ti4-engine`, with
serde derive code for every wire type and `std` generics for every collection of every type,
and `ti4_model`'s `GameState::clone` appearing four times at 30-38 KiB each.

Besides `.text`: 1.7 MB `.rodata` (the embedded corpus, 957 KB of JSON, plus strings), 2.1 MB of
unwinding tables (the server is `panic = "unwind"`: tokio and the session workers rely on it, so
`panic = "abort"` is not an option for the server), 0.46 MB of relocations (PIE).

### A.3 WebAssembly

`target/wasm32-unknown-unknown/wasm-release/ti4_wasm.wasm` of the alpha commit (it is byte-for-byte
the file in `/opt/ti4-public/current/dist/assets/`):

| | bytes | budget (`crates/ti4-wasm/size-budget.json`) | |
|---|---:|---:|---|
| raw | 7 532 609 (7.53 MB) | 7 000 000 | **OVER** |
| gzip -9 | 1 896 042 (1.90 MB) | 1 800 000 | **OVER** |
| brotli -11 | 1 316 568 (1.32 MB) | 1 300 000 | **OVER** |

The budget is checked by `scripts/wasm-size-gate.mjs` (not part of `deploy.sh`; it exists for a CI
job that this repository does not contain), in **decimal megabytes** (1 MB = 1 000 000 bytes), on
the file as cargo writes it, for all three of raw, gzip -9 and brotli -11. All three are over the
budget on the alpha; the `7.00` vs `7.50` of the task is the raw line.

What is in the 7.53 MB (twiggy on an unstripped build, 12.08 MB with 3.08 MB of function names
and 1.46 MB of debug sections that the shipped file does not have; code = 6.02 MB, data = 1.50 MB):

| group | bytes | share of the shipped 7.53 MB | note |
|---|---:|---:|---|
| `ti4_engine` | 1 603 163 | 21 % | 3 030 functions; `Game::step` retains 447 KB |
| `alloc` generics | 1 491 359 | 20 % | 9 319 functions, instantiated for our types |
| `core` generics | 1 235 726 | 16 % | 7 180 functions (iterators, sort, fmt, slices) |
| `.rodata` | 1 502 978 | 20 % | embedded JSON corpus 957 KB, **and `map_templates.json` a second time, 174 KB**, plus strings |
| `ti4_host` | 318 255 | 4 % | |
| `serde_core` + `serde_json` + `serde` | 781 184 | 10 % | derive-generated visitors are in the type's own crate; this is the generic part |
| `ti4_rule_bot` | 210 331 | 3 % | |
| `ti4_model` | 117 276 | 1.6 % | |
| `ti4_wasm`, `ti4_content`, `ti4_calc`, `std`, rest | ~190 000 | 2.5 % | |

Generic functions, summed over all their instantiations (`tools/build_report/group_symbols.py`,
the "monos" table, `tables/wasm-monos.txt`):

| generic | copies | bytes |
|---|---:|---:|
| `Vec<T>: SpecFromIterNested::from_iter` (`.collect()`) | 1 192 | 232 467 |
| `Map<I,F>::fold` | 734 | 176 521 |
| `Vec::extend_desugared` | 646 | 100 959 |
| `SerializeMap::serialize_entry` | 581 | 96 433 |
| `slice::sort::stable::{quicksort, stable_partition, drift, create_run, smallsort}` | 122-188 each | ~350 000 together |
| `serde::private::de::content::*` (buffering of internally tagged / `flatten` / untagged enums) | 50-72 each | ~160 000 together |
| `BTreeMap` node operations | 50-90 each | ~210 000 together |

So: the engine is a lot of code, and about 45 % of the code is `core`/`alloc` generics
monomorphised per element type (every `.collect()` of every type, every stable sort of every key
type, every `BTreeMap<K, V>`), plus serde.

`opt-level = "s"` instead of `"z"` makes it larger (8 011 572 bytes), `"z"` is already the
smallest setting. The profile is already `lto = "fat"`, `codegen-units = 1`, `panic = "abort"`,
`strip = true`; nothing in the shipped file is debug info or names (checked: no `name` and no
`.debug_*` sections). There was no `wasm-opt` step.

### A.4 Web bundle (`npm run build` in `web/`, vite 8)

| file (`dist/assets/`) | bytes | gzip (vite) |
|---|---:|---:|
| `ti4_wasm-<hash>.wasm` | 7 532 609 | 1 923 650 |
| `index-<hash>.js` (React + the whole app, one chunk) | 1 278 785 | 370 610 |
| `index-<hash>.css` | 244 383 | 38 900 |
| `session`, `jsx-runtime`, `launcher`, `worker`, `OfflineGameScreen`, `mapSource` | 15 466 + 8 774 + 5 777 + 4 045 + 1 301 + 950 | |

`dist/` is 8.8 MB; the wasm is 85 % of it. `deploy.sh` additionally writes `.gz` and `.br` of the
wasm next to it (1 896 042 / 1 316 568 bytes). There are no fonts and no fixtures in the bundle.
The JS chunk has no code splitting (vite warns about 500 kB), 371 kB gzip.

### A.5 Where the disk goes (target directories)

A clean `cargo t-web` build (`cargo test -p ti4-model -p ti4-content -p ti4-calc -p ti4-engine
-p ti4-host -p ti4-rule-bot -p ti4-server -p ti4-wasm --lib --tests --no-run`) on the alpha:

| | MB |
|---|---:|
| `debug/incremental/` | 3 927 |
| `debug/deps/` | 2 097 |
| rest (build scripts 10, loose files) | ~550 |
| **target total, first build** | **4 581** |

and after eight rebuilds with small edits 7 212 MB (incremental 4 849 MB). `deps/` by kind:
executables 926 MB (14 files), rlib 609 MB (166), `.dwp` 345 MB (15), rmeta 187 MB, shared
libraries 135 MB (the `libti4_wasm.so` and the proc-macro `.so`s). The incremental caches are
**more than 85 % of a fresh test target**, `ti4_engine` alone 1 963 MB of them, then `ti4_host`
(802), `ti4_server` (654), `it` (366), `ti4_wasm` (341). The release targets are small:
server 426-760 MB, self-play +340 MB, wasm-release 243-290 MB.

The "tens of GB" per worktree are accumulation, not one build. The main checkout's
`target/` is 70 GB (`tables/target-usage-main-checkout.txt`): `debug/deps` 45 GB, of which 21 GB of `.dwp` and 12.6 GB of
executables from builds with full debug info (before `debug = "line-tables-only"`), 181 `.dwp` and
180 executables; **28 hashes of `ti4_server`, 21 of `ti4_engine`, and 7 copies of each of the 70
separate integration-test binaries** (`ws_lifecycle`, `history`, ... 1.1 GB each, from before they
were merged into the one `it` binary), plus 12 GB of incremental caches. Cargo never garbage
collects `target/`: every change of flags, features or profile leaves the old artifacts.

Which crates dominate disk: the ones that are linked many times. Every unit-test executable and
every integration test links the whole stack below it (engine, host, rule-bot, model, content,
serde_json...) statically at opt-level 0 with line tables: 14 executables of 46-147 MB.
Proc-macro crates and build scripts are small on disk (`build/` 10-20 MB).

Duplicate dependency versions (`cargo tree -d` on the game crates): `hashbrown` 0.15 (ours,
unused by the sources) and 0.17 (via `indexmap`); `getrandom` 0.3 and 0.4 (`rand` and
`tempfile`, test only); `syn` 2 and 3 (`syn` 3 only via `futures-macro`/`displaydoc`, i.e. tokio
and the ICU stack); `tokio-tungstenite` 0.26 (our dev-dependency) and 0.29 (axum);
`tungstenite` likewise.

### A.6 Where the compile time goes (before any change)

| build | wall | CPU | peak RSS | units |
|---|---:|---:|---:|---:|
| `ti4-wasm`, `wasm-release`, clean | 91 s | 140 s | 1.5 GB | 87 |
| `ti4-server` `--release`, clean | 321 s | 559 s | 2.0 GB | 159 |
| `ti4-selfplay` `--release`, after the server build (shared dependencies) | 291 s | 471 s | 1.7 GB | 62 |
| `cargo test <t-web set> --lib --tests --no-run`, clean | 279 s | 587 s | 2.7 GB | 232 |
| ... again, nothing changed | 1 s | 0 s | | |
| ... after an edit in `ti4-engine` (new function at the end of lib.rs) | 30 s | 44 s | 1.7 GB | |
| ... after an edit in `ti4-server` | 14-30 s | 12-44 s | 0.8-1.7 GB | |

(`cargo --timings`, `timings/*.html` in the report directory; the longest units, with the machine
at load 25-33 so about 3x inflated: `ti4-engine` lib (test) 128 s and lib 51 s - it is compiled twice
by `--lib` - then syn 2 and syn 3 at 60 s each, `ti4-host` 48 + 42 s, `darling_core` 45 s,
`zerofrom-derive` 44 s, `serde_derive` 43 s, `ts-rs-macros` 42 s, `strum_macros` 41 s,
`yoke-derive` 38 s, `thiserror-impl` 34 s, `tracing-attributes` 34 s, `regex-automata` 35 s.
In the release builds: `ti4-engine` 153 s, `ti4-server` lib 144 s and its bin 94 s, `ti4-host`
121 s, `ti4-policy` 37 s; in the self-play build `ti4-wasm` 38 s, only to link a `.so` nobody uses.)

Findings from them (all verified, then changed in section B):

1. **The proc-macro crates were compiled with opt-level 3.** The comment in `Cargo.toml` said
   build scripts and proc macros are covered by `build-override` (opt-level 0), but
   `[profile.dev.package."*"]` wins over `build-override`: `cargo build -v` shows
   `-C opt-level=3` for `syn`, `serde_derive`, ... They run only inside rustc.
2. `ti4-server` has `ti4-policy` as a normal dependency, but only a test uses it.
3. Dependencies nothing uses: `globset` in `ti4-content` (pulls `regex-automata`,
   `regex-syntax`, `aho-corasick`, `bstr`: 35 s of every release/wasm build), `bincode`,
   `anyhow`, `tracing`, `itertools`, `hashbrown`, `smallvec`, `indexmap`, `bitflags`... in the
   model, content, engine and policy crates.
4. The server's test client `reqwest` pulls `url` -> `idna` -> `idna_adapter` 1.2, which is the
   ICU4X stack: 16 crates and four derive proc-macro crates, plus the second `syn`.
5. `ti4-content` is embedded as pretty-printed JSON, and `ti4-host` embeds `map_templates.json`
   a second time.

## B. Changes (branch `build-size-and-time`, one commit each, numbers in the messages)

### B.1 What was changed

| # | Commit | What | Effect (measured) |
|---|---|---|---|
| 1 | proc-macro crates at opt-level 0 | `[profile.dev.package.<name>] opt-level = 0` for the 63 proc-macro crates and the libraries only they use (generated from `cargo metadata`); `"*"` stays at 3 for everything else | clean t-web: **587 s -> 426 s CPU** (-27 %), wall 279 -> 201 s; edits after that: unchanged (44 s / 12 s) |
| 2 | unused dependencies | 22 `[dependencies]` entries of model, content, engine, policy that no source mentions (`globset`, `bincode`, `anyhow`, `tracing`, `itertools`, `hashbrown`, `smallvec`, `indexmap`, `bitflags`, `enumset`, ...) | wasm build 87 -> 63 units (no regex crates), self-play 62 -> 47 units, t-web 232 -> 214 units; CPU change inside the noise (426 -> 407 s); no binary changes by a byte |
| 3 | `ti4-policy` dev-only in ti4-server | it was only used by one test | deploy build (release server) loses the `ti4-policy` unit, 37 s of 559 s CPU; 159 -> 138 units |
| 4 | `idna_adapter = "=1.1.0"` (dev-dependency of ti4-server) | keeps the ICU4X stack out of the test build | t-web 407 -> 401 s CPU, wall 225 -> 194 s, 214 -> 196 units, lock file -16 packages |
| 5 | content embedded without whitespace, `map_templates.json` once | `ti4-content/build.rs` minifies a copy into `OUT_DIR`; `ti4-host` reads `ti4_content::MAP_TEMPLATES_JSON` | wasm 7 532 609 -> **7 085 994** raw (-446 615); server 22 962 880 -> **22 513 512**; self-play 13 155 064 -> 12 708 424; corpus digest unchanged (tests pass) |
| 6 | `wasm-opt` in `web-shared/scripts/build-wasm.mjs` | binaryen 123 (devDependency of `web/`), `-Oz` without inlining, features = rustc's `target_features` | wasm 7 085 994 -> **5 749 996** raw, gzip 1 891 194 -> **1 777 619**, brotli 1 312 625 -> **1 276 622**; costs about 50 s CPU per web build |

Cumulative (cargo only, same machine, load 25-35, CPU seconds user+sys / wall seconds):

| Build | Before | After | Note |
|---|---:|---:|---|
| `cargo test <t-web set> --no-run`, clean | 587 / 279 | **401 / 194** (best run; the last run, with `npm ci` and vitest running next to it, 448 / 350) | -32 % CPU |
| ... after an edit in `ti4-engine` | 44 / 30 | 42 / 27 | unchanged |
| ... after an edit in `ti4-server` | 12 / 14 | 12 / 12 | unchanged |
| ... nothing changed | 0 / 1 | 0 / 1 | |
| `cargo build --release -p ti4-server --bin server`, clean | 559 / 321 | 527 / 308 | -6 % CPU |
| `cargo build --release -p ti4-selfplay` (after the server) | 471 / 291 | 463 / 339 | inside the noise; 15 fewer units |
| wasm-release, clean | 140 / 91 | 126 / 90 | -10 % CPU, plus about 50 s CPU of wasm-opt in the web build |
| target dir after the clean t-web build | 4 581 MB | 4 506 MB | |

Verified after the changes: `cargo t-web` green (all test results ok), `tsc --noEmit` clean,
`npx vitest run` 2 727 passed (4 `DecisionGallery` tests timed out at 5 s with the machine at load
36, 13/13 pass alone with a longer timeout), the offline Playwright suite (`npm run
test:e2e:offline`, which builds with `build-wasm.mjs`, i.e. runs against the wasm-opt output),
`scripts/wasm-size-gate.mjs` on the shipped file (see C), `tools/rulebot/selfplay.sh bench` and
`bench-v1` to `bench-v5` identical (see C). `deploy/public/deploy.sh` was read and not run: it
runs `npm ci` (installs binaryen) and `npm run build` (prebuild = `build-wasm.mjs`), so it picks the
step up unchanged, and it builds the server with `cargo build --release -p ti4-server --bin
server`, which still builds.

### B.2 Measured, not landed

| Idea | Result | Decision |
|---|---|---|
| `CARGO_INCREMENTAL=0` (env, no repo change) | clean t-web **363 s CPU** (vs 401) and **2.0 GB** instead of 4.5 GB; but an edit in `ti4-engine` costs 213 s CPU instead of 43 s (5x) | Use it for throw-away agent worktrees that build once and are deleted; keep incremental for the inner loop. Not a repo default |
| `opt-level = "s"` for wasm | 8 011 572 bytes (larger than `"z"`: 7 085 994) | no |
| `wasm-opt -Oz` / `-Os` with inlining | raw 5.36 / 5.70 MB, but gzip +6 %, brotli +9 % over the cargo build | no: transferred size grows. Without inlining all three shrink (landed) |
| wasm-opt dedupe/merge passes only | raw 6 193 478, gzip 1 822 742, brotli 1 295 209 | worse than the landed variant |
| faster linker (mold) | not needed: the toolchain's default on x86-64 Linux is already `rust-lld` | no |
| lto / codegen-units of `release` | not touched; the benches pin outputs and the deploy build is the one that matters | no |

### B.3 Remaining ideas (not done), with estimated gains

| Idea | Est. gain | Risk |
|---|---|---|
| Stop building the native `cdylib` of `ti4-wasm` (`crate-type = ["cdylib", "rlib"]`): 83 + 34 MB per test target, 15 MB in release, a link step in every native build (38 s unit in the self-play build). Needs `cargo rustc --crate-type cdylib` or a separate wasm wrapper crate, so every `cargo build -p ti4-wasm --target wasm32-...` line in README, scripts, nightly-smoke and other branches changes | -120 MB per test target, ~10-30 s CPU per native build | medium (call sites in many branches) |
| Move `ti4-server` to `tokio-tungstenite` 0.29 in its dev-dependencies and `ti4-bot-agent` (one `tungstenite`) | ~10 units less in the test build | low, but the test code may need API changes |
| `reqwest` out of the server tests (a tiny blocking HTTP client in `tests/it/support.rs`) | -20 crates, `hyper-util`, `tower`... from the test build, ~30 s CPU | medium: rewrite of the HTTP helpers |
| Remove the unused dependencies of `ti4-bridge`, `ti4-legacy`, `ti4-training` (`parquet`, `arrow`!), `ti4-sim`, `ti4-cli` (every dependency of it is unused), `xtask` | big for `cargo build --workspace`, nothing for the game | low; not verified here (libtorch-free, but out of scope) |
| `hashbrown` 0.15 vs 0.17: no source uses `hashbrown` any more, so only `indexmap` pulls one version (0.17) | already done by commit 2 | |
| Split `ti4-engine` (271 000 lines, one crate, 50-150 s per compile, compiled twice by `--lib`) into crates by area | the biggest possible CPU and incremental win; every edit in the engine recompiles it all | high effort |
| Fewer generic instantiations in the wasm: one `collect` helper per element type, a sort helper per key type, `BTreeMap` -> a small map type; `serde(tag)`/`flatten` -> hand written visitors | ~0.3-0.8 MB raw | changes iteration orders / error texts if done carelessly; needs the benches each step |
| `-C target-feature` / `build-std` + `panic_immediate_abort`, `-Zfmt-debug=none`, `-Zlocation-detail=none` | ~10-15 % of the wasm | needs nightly |
| Compress the corpus in the binary (zstd/deflate + `include_bytes!`) | -0.4 MB raw wasm, nothing in brotli; adds a decoder | not worth it |
| `sccache` / a shared `CARGO_TARGET_DIR` for the worktrees | clean builds of a second worktree cost 0; but the artifacts' paths differ per worktree (path dependencies are keyed by path), so only registry crates are shared: ~40 % of the units | low; `sccache` is not installed |
| `cargo-sweep`-style cleaning: delete `target/debug/deps` entries not touched for N days; delete the target of a finished worktree with the worktree | the 70 GB main target is mostly stale (21 GB `.dwp`, old per-test binaries) | none |
| `debug = 0` for `dev` | saves the 345 MB of `.dwp` (8 %) but removes line numbers from panics' backtraces | the team chose line tables on purpose |
| `panic = "abort"` for the server | -2 MB unwind tables | breaks catch_unwind in the session workers |

## C. Final state

| | alpha | now | budget |
|---|---:|---:|---:|
| `ti4_wasm.wasm` as cargo writes it | 7 532 609 / 1 896 042 / 1 316 568 | 7 085 994 / 1 891 194 / 1 312 625 | 7 000 000 / 1 800 000 / 1 300 000 |
| the file `web/` ships (after wasm-opt) | the same | **5 749 996 / 1 777 619 / 1 276 622** (raw / gzip / brotli): `scripts/wasm-size-gate.mjs` says ok on all three | |
| `server` (release, stripped) | 22 962 880 | 22 513 512 | |
| `selfplay` (release, stripped) | 13 155 064 | 12 708 424 | |
| `web/dist` | 8.8 MB | 7.1 MB (the wasm is 5.75 of it) | |

The size gate is meant to run on the file the app ships (`node scripts/wasm-size-gate.mjs
web/src/offline/local/engine/ti4_wasm.wasm`); on the raw cargo output (what the task called "7.50
against 7.00") it still reports the content-only file as over, because wasm-opt is a later step.

Checks on the final head: `bench` and `bench-v1` ... `bench-v5` 200/200 identical to their
baselines (`SELFPLAY_RESULTS=off WORKERS=4`), `cargo t-web` green, `tsc --noEmit` clean, vitest
green (the four timeouts under load pass alone), the offline Playwright suite 12/12 on the
wasm-opt output, `js/run.mjs` and `js/host.mjs` identical on the cargo and optimized wasm.

Other binaries of the workspace that build without libtorch (release, stripped): `ti4` (ti4-cli)
339 056 bytes and `xtask` 339 024 bytes (both are stubs that link little), `ti4-bot-agent`
5 052 008 bytes (reqwest, tokio-tungstenite). `ti4-join` (eframe) was not built.

## D. How to reproduce

```sh
# sizes of everything that is built in a target dir (writes nothing, builds nothing unless asked)
tools/build_report.sh --target /path/to/target --out report.md
tools/build_report.sh --build-wasm --build-release --web --names      # build first, add twiggy / cargo-bloat tables

# the helpers it uses
python3 tools/build_report/target_usage.py <target-dir>               # disk by profile, kind, crate, incremental
python3 tools/build_report/group_symbols.py <twiggy.csv> 30            # wasm bytes by crate
python3 tools/build_report/timing_top.py <cargo-timing.html> 25       # longest units of a --timings run
node scripts/wasm-size-gate.mjs <file.wasm>                             # raw / gzip / brotli against the budget

# timings: cargo build --timings, and CPU seconds from `/usr/bin/time -f "%U %S"` (user + sys)
export CARGO_TARGET_DIR=/some/private/dir
/usr/bin/time -f "%e s wall, %U+%S s CPU, %M KB" \
  cargo test -p ti4-model -p ti4-content -p ti4-calc -p ti4-engine -p ti4-host -p ti4-rule-bot \
  -p ti4-server -p ti4-wasm --lib --tests --no-run --timings
cargo build --timings --release -p ti4-server --bin server
cargo build --timings -p ti4-wasm --target wasm32-unknown-unknown --profile wasm-release
# symbols: cargo install twiggy cargo-bloat; the wasm needs  --config profile.wasm-release.strip=false
# (this repo's README for ti4-wasm shows the command); native: CARGO_PROFILE_RELEASE_STRIP=false cargo bloat --release --bin server --crates
```

Compare CPU seconds, not wall time, when other jobs share the machine; repeat the key
measurements; delete the private target dir afterwards (a clean t-web target is 4.6 GB, 2 GB with
`CARGO_INCREMENTAL=0`).
