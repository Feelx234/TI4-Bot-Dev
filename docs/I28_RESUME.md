# I28 resume

Branch `core-integration-28` = `core-integration-27` (`163270ad8`) + `core-nestq-1`, `core-jolnar-mech-1`, `core-fork-tests-1`, `core-chance-1`
(no textual conflicts) + `core-stepmachine-1` (one conflict in `answer_nested`, both sides kept) + own commits (memo census test, g040 bless,
the 77 bench entries, a lint fix, the wasm trim, docs). Rules version 36 (Rearmament changed in place, user-accepted). Not pushed, not deployed.
Local tag `heavy-base-int28` = the tip (moved there in round 2; `web/`, `state.rs`, `RULES_VERSION.md` equal `heavy-base-int27`). Details:
`docs/INTEGRATION28.md`.

**Wasm: within budget after the trim** (`0286f095f`: `Window::PARKS`, `PARK_CHECK_BUILT`): real build 6,624,713 raw / 2,106,859 gzip / 1,516,834
brotli against 6,630,000 / 2,110,000 / 1,520,000 (before the trim, with the step machine: about 6.666 / 2.119 / 1.524 MB). Hashes unchanged
(`538dc68f23dddcc9`, `775f4136e60a99d5` with parking on, off and checked).

Done and green: g040 proven and re-blessed, golden check 200/200 (default, core, string); 77 bench entries re-recorded (identical to the mech lane's
table, all victories); nestq debug check over whole games, 0 mismatches, memo hit rate 99.7-100 % (thin quick game 513/514); stepmachine tests;
layers, production graph, clippy 41, lint, web tsc/vitest.

**Open: the heavy run (round 2) was NOT started**: 12-15 GB free on `/` (the rule: at least 25 GB; the run needs about 30 GB). Space is held by
caches I may not delete (`/root/deploytmp/eod` 13 GB, `eod26` 12 GB, `hf1` 23 GB) and by lane `sm1*` (24 GB). To resume once space is freed:
`docs/HEAVY_RUN_PLAN.md` 12.3 with `--skip ratchet`, fresh out dir `/root/end-of-d1-20261106-r5`, lint base `heavy-base-int28` (move it if commits
touch code, after checking `web/` and `state.rs`). Also not run since the last merge: gate tests of core / bridge / wasm / server / chance.
Scripts and logs: `/root/deploytmp/i28-logs/` (`wasm-named.sh`, `wasm_syms.py`, `sizes.mjs`, `wasm-real.sh`).
