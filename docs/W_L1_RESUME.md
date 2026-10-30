# W lane 1 resume note, branch `core-w-l1` (from `core-w` @ `4808a0ab`)

Done: W5 (guard narrowing, production by an ability), W8 (5a window, 5b composition), W2 (Letnev opening, produced hits, Thundarian, Raid Formation, Dunlain Reaper, Assault Cannon and Duranium in the machine). Open: see "Next".
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

Ported since the first note: the hits the modules produce (Ambush, Impulse Core, Devotion, Dimensional Splicer: window and composed machine), the
Thundarian in the space roll, Raid Formation as a child of the barrage (window and machine), the Dunlain Reaper, production by an ability, Assault
Cannon and Duranium Armor in the machine. The remaining tail is W2-bail-02 in `docs/ENGINE_KNOWN_ISSUES.md` (each a counted gate with a test).
