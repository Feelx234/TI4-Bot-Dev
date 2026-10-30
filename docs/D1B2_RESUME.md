# D1b2 (the remainder of D1b): state

Branch `core-d1b2`, cut from `core-integration-5` @ `670963aa` (contains D1b @ `604aa2ed`, P1, A2b, B6d and the gated B12). Not pushed, not merged.
Rows `D1b2-*` of `docs/ENGINE_KNOWN_ISSUES.md` and section 11 of `docs/ENGINE_CORE_DRIVER.md` describe the pieces; this file says where the work stands.

## Done (committed, unit and lockstep tests green on `driver=dual`)

1. A2b-10b the space cannon offense: `rules/combat/space_cannon_offense.rs`, adapter `ti4-engine/src/cannon_dual.rs`, string site `AftermathWindow::cannon_prelude`.
2. The aftermath head as a core window (`driver/windows/aftermath.rs`), `WindowKind::{Combat, Invasion, Production}`, `Window::Combat`, the `BeforeCombat::capitals()` accessor.
3. `choice::turn` + `render/turn.rs` + `driver::menu::{action_menu, closing_choice}` compared by `menu_dual.rs`; `driver/dispatch.rs`; `driver/step.rs` (`Session::step`).
4. `close_tactical`: `driver/tactical_tail.rs`.
5. `fleet::enforce_everywhere` (`driver/windows/enforce_all.rs`), the turn prelude pieces, the Strategy arm of `advance_phase`, the pass writes; the lent hooks counted by name.

## Not done / bails

See `D1b2-bail-01..04` in the known issues: the chain after the head, the invasion and production machine states, the hooks (Minister of War, `technology::end_turn`, `legendary::end_turn`),
the status/agenda halves of `step_phase`, the bodies of the non-native menu sources.

## Tools

* Runner script: `/root/deploytmp/core-d1b2-run.sh TAG COMMIT cargo ...` (stable scratch worktree `/root/deploytmp/core-d1b2-wt`, target dir `/root/deploytmp/core-d1b2`, logs in `/root/deploytmp/core-d1b2-logs`).
* Tests: `cargo test --profile lockstep -p ti4-engine --lib -- cannon_prelude_tests d1_pending turn_write menu_dual_tests`, `-p ti4-engine-core --lib -- driver::`.

## Gate

See the final report of the implementation agent for what was run and what was not.
