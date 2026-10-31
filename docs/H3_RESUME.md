# D2 lane H3 (`core-h3-leftovers`): the remaining leftovers after lane H2, resume note (2026-11-01)

Branch `core-h3-leftovers`, cut from `core-h2-leftovers` (`5d2b13ce`) and merged with `core-m6d-tactical` (`44fa75a0`, which carries M6c and M6b). Never pushed; the live branch is untouched.
Integration 11 (`core-integration-11`) is NOT finished (no `docs/INTEGRATION11.md`), so this branch is not based on it. Read `docs/H2_RESUME.md`, `docs/M6D_RESUME.md` first.

## The merge of `core-m6d-tactical` (what was reconciled)

* The three gap files (`silent/gaps.rs`, `ability_gaps.rs`, `ability_audit.rs`): took H2's side and deleted M6d's rows by id with `tools/engine_core/delete_gap_rows.py`
  (9 rows: awaken, arboreccommander, muaathero (2), nomadagentmercer, yssarilcommander (2), arborec_flagship, titans_flagship; the Ssruu copy of the Sol agent too, found by the audit test),
  the other rows M6d deleted were already gone in H2 (empyrean, ghost). The `Lane("core-m6c-tactical")` lines of the audit went (this branch has M6c). 29 rows left; `cargo test -p ti4-engine --lib ability_audit` passes.
* The neutral-side bail in `dual_window::settle_run` is absent (only `tables_of_sides`, the barrage gate and table builders mention `is_neutral`).
* `native_subject.rs`: H's generic `open_observed` arms kept (a superset), both openers' tests kept. `tail.rs`: H2's free-action tail (`open_after`) kept, M6c's `resume_strategy` seam deleted.
  `reactions_core.rs`: the PAIRS union, the lobby-factions list union (H's tables plus M6c's), the NAMED line of Mercer deleted (ported). `tactical_live.rs`: M6c's `without_seen` filter kept, H's cards kept (`planet_hub` of H renamed `one_planet_hub`).
  `live/turn.rs`: `discard_law(.., facts, ..)` of M6d, `pub(super)` of H. `combat_fight.rs` had a doubled `show_neutral` field.

## Task 1: the unknown text code (`presentation.phase`), DONE

Root cause: `combat_live::combat_texts` minted the STATE texts (presentation phases, unit names, roll groups) into the position's table (`core.texts.codes`) and the SHOWN texts into the render's table
(`Render::code`), two tables that continue the same lineage and then disagree about every code minted after the split. The product loop (`native_play` step commit: `core.texts.codes = render.codes_snapshot()`)
and the harness hand back the renderer's table: a state code (the phase `pre_roll`) then named a shown text or none (`unknown text code` in the Transient slice when the position is materialised: the
shadow, the dual and the faction sweeps), and a unit-name code could name the wrong unit.
Fix: `combat_texts` syncs the two tables first, mints both kinds through the render, and the position takes the render's table back (`Render::sync_codes`, additive; the same body as lane F2c's `8f53669d`, so the merge is
identical text); `live::live_step_inner` syncs after every `advance` call (F2c's three lines, identical) so a window that mints straight into the state (the exploration's words) is not lost either;
`native_play` commits with `sync_codes` instead of overwriting the position's table. `native_subject` (shadow): the shadow's render now continues the position's table (it was created empty before the position loaded: codes of the observed windows collided with the position's
table). Regression: `combat_live::tests::the_texts_of_a_combat_step_are_one_table_for_the_render_and_the_position` (fails on the old `combat_texts`: the render's table names the phase `pre_roll` by "minted before the step").
The Argent/Nekro rows of the faction sweep pass with it (the sweep fails only on lane G's Naalu Z'eu gap, 41004).

## Task 2: `the_turn_flow_arms_agree_with_the_legacy_game_for_every_lobby_faction`, a branch-composition artefact

Replayed with the staged mark printed (`H3_STAGED` debug print, removed): the staged event is `{"payload":{"amount":3,"player":"c","source":"trade_primary"},"type":"TRADE_GOODS_GAINED"}` at the turn prelude
of the jolnar/l1z1x/arborec/mentak table, round 1: the string's ONE legacy step holds the prelude AND the action menu (answers `strategic|pok5trade`, `done`) and so the Trade primary of the seat that chose it; the core in this tree
has no route from the menu to the strategic action (lane F2b's dispatch), so the core shadow does not stage the goods. Verified on `core-integration-11` (`127d9ec5`): the test passes there (F2b's `432b90ae` chains the prelude into the menu step
and `69da711f` stages the Trade primary's goods). So: not a defect of this tree's rules and not a shadow artefact of the oracle itself, it passes once F2b's dispatch is merged (integration 11 does). Nothing loosened; the test stays red on this branch alone.
