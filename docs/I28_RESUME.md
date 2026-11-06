# I28 resume

Branch `core-integration-28` = `core-integration-27` (`163270ad8`) + `core-nestq-1`, `core-jolnar-mech-1`, `core-fork-tests-1`, `core-chance-1`
(no textual conflicts) + own commits (memo census test, g040 bless, the 77 bench entries, a lint fix, docs). Rules version 36 (Rearmament changed
in place, user-accepted). Not pushed, not deployed. Local tag `heavy-base-int28` = `1c8e67dd1`. Details: `docs/INTEGRATION28.md`.

**Stopped on the wasm size gate**: 6,636,894 B raw (budget 6,630,000) / gzip 2.12 MB (2.11); the +39,832 B are all lane nestq-1 (int-27 6,597,047;
nestq alone 6,636,879; mech alone 6,597,062). Needs a user decision: trim nestq or ratchet the budget.

Done and green: g040 proven (only `placements.rs` reverted -> 4/4 reproduce) and re-blessed, golden check 200/200 (default, core, string);
77 bench entries re-recorded, identical to the mech lane's table, all victories; nestq debug check over whole games, 0 mismatches, memo hit
rate 99.7-100 % (thin quick game 513/514); fingerprint `538dc68f23dddcc9` and suite hash `775f4136e60a99d5` unchanged; layers, production graph,
clippy 41, web tsc/vitest.

Open: the heavy run (`/root/end-of-d1-20261106-r4`; stopped in `tests`; lint fixed in `cd8df2911`), gate tests of core/bridge/wasm/server/chance.
To resume: decide on the wasm, then rerun the full heavy command of `docs/HEAVY_RUN_PLAN.md` 12.3 with `heavy-base-int28` (moved to the new tip if
code changes, after checking that `web/` and `state.rs` are unchanged) and a fresh out dir. Disk is short (about 16 GB free at the stop).
Scripts and logs: `/root/deploytmp/i28-logs/`.
