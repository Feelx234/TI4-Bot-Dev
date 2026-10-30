# W lane 1 resume note, branch `core-w-l1` (from `core-w` @ `4808a0ab`)

Done: W5 (guard narrowing), W8 (5a window, 5b composition), W2 Letnev round opening (+ the stale `have` of the Munitions question). Open: see "Next" (all of it pinned as counted gates with tests).
Lines for `docs/ENGINE_KNOWN_ISSUES.md` are in the section "W lane 1" at its end.

## Gate deferred to the end of D1

By the user's directive no heavy work runs before the end of D1: no `verify_package.sh`, golden or wide dual runs, board_equivalence, bench,
soak, clippy of the workspace, wasm size, release builds. The census numbers of `docs/LENT_CENSUS.md` are NOT refreshed by this lane
(the post-fix census is run once at the end of D1; the commands are in `docs/W_RESUME.md`). Gate deferred to end of D1.

## Cheap checks that were run

`cargo test -p ti4-engine --lib` with targeted filters (and `invasion::`, `legendary::`, `technology::`, `faction_effects` after W8), `cargo check -p ti4-engine-core`,
single-file mutation checks (`tools/engine_core/mutants.sh`, Haiku runner, scratch worktree `core-w-l1-mut`) of `control.rs`, the W8 hunks of
`dual_window.rs` and `production_view.rs`. Private target dir `/root/deploytmp/core-w-l1` (delete when done).

## Next (not done)

Mentak Ambush, Assault Cannon / Dimensional Splicer / Impulse Core (`opening_why`), the Thundarian in the SPACE roll (`roll_why`),
Argent barrage excess, Jol-Nar commander, Waylay (`barrage_why`), Dunlain Reaper (`begin_ground_unported`), composed-machine staged destructions,
production by an ability (`ProductionWindow::to_core` reason `ability`: the core window has no ability/fixed-limit mode).
