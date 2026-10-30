# W (lent bodies) resume note, branch `core-w`

State: W0 and W3 done, W1 items 1-2 done, W2 items 1-3 done (Naaz opening, Supercharge, Dark Energy Tap, dual-form retreat). See
`docs/LENT_CENSUS.md` (census tables, findings) and the W section of `docs/ENGINE_KNOWN_ISSUES.md`.

## Gate deferred to the end of D1

By the user's directive no heavy work runs before the end of D1: the post-fix dual census (golden `profile-20` and
`golden-lent.json`, `all=dual`) of the last commits was queued on `/root/.ti4-heavy.lock` and has been CANCELLED, not run.
The numbers in `docs/LENT_CENSUS.md` are those of commit `3de90cbe`; four findings of that census were fixed afterwards, each with a
regression test (ground Supercharge `invasion_seq`, Skilled Retreat destinations per round, ship-form roll texts, stale Supercharge +1)
and are NOT yet re-measured on the census. Gate deferred to end of D1. To run it later (`flock /root/.ti4-heavy.lock`):

```
CARGO_BUILD_JOBS=2 cargo build --profile gate -p ti4-engine-core-bridge --bins
TI4_NESTED_LOG=1 SHARDS=4 tools/engine_core/dual_sharded.sh $CARGO_TARGET_DIR/gate/dual_run tools/engine_profile/profile-20.json 20 all=dual OUT/golden-shards > OUT/golden.txt
TI4_NESTED_LOG=1 SHARDS=4 tools/engine_core/dual_sharded.sh $CARGO_TARGET_DIR/gate/dual_run tools/rulebot/golden-lent.json 20 all=dual OUT/lent-shards > OUT/lent.txt
tools/engine_core/lent_census.py OUT/golden.txt; tools/engine_core/lent_census.py OUT/lent.txt   # both must exit 0, 0 differences
```

Also not run: `verify_package.sh`, `board_equivalence`, benches, clippy, soak, wasm size, release builds, the G-L1 gate.

## Cheap checks that were run

`cargo test -p ti4-engine-core -p ti4-engine-core-factions` and `-p ti4-engine --lib` at `fd080f47` (core all ok, engine 4601 passed 0 failed),
`tools/engine_core/check_layers.py core` (0 violations), `tools/engine_core/lint_core.sh` (0 hits), single-file mutation checks per piece
(`/root/deploytmp/core-w-run/mutate_w.py`, scratch worktree `scratch-mut`).

## Next in the todo order

W1 items 3-6 (X-89 ground roll doubling, `ground_rolls_extra_hits`, defense/barrage guards, Hacan flagship with spendable goods, Naalu
custodians), W2 items 2 and 4 (Mentak Ambush, Assault Cannon / Dimensional Splicer / Impulse Core, machine bails), Letnev round opening,
Dunlain Reaper, W5, W8.
