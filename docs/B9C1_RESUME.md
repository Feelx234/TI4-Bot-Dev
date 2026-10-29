# B9c-1 agenda reveal window: resume note (branch core-b9-agenda, worktree .claude/worktrees/core-b9)

Base: core-b9-agenda was fast-forwarded to core-integration-2 (dc2f4255) first (it was a strict ancestor; the prereqs
doc assumes that base: B8 token spends, reactions, B6 cards). Work is on top of it, commits "B9c-1 WIP" and "B9c-1: lockstep tests ...".

## Done
* String split: `crates/ti4-engine/src/agenda_reveal.rs` (`Live`: reveal chain, offers, `ask_to_use_note`, `emit_typed`,
  `announce_staged_cards`, `push_agenda_record`); `Game::split`, `Game::reveal_agenda` wraps it in `dual_window!(AgendaLaws, "game::reveal_agenda")`.
  Delivery registry (`tests/decision_delivery_inventory.rs`) follows. `draw_covert_agenda` is now a plain free fn (covert is embedded in the reveal window; the old covert dual window, its adapter `dual_window::covert_draw` and its two tests are removed).
* Core: `kernel/fx_agenda.rs::log_discarded`, `choice::agenda::{Ask::UseOnAgenda, UseSource, Opt::UseCard, Subtype::Use*}`, `rules/agenda/reveal.rs` (`Reveal`), renderer in `render/agenda.rs`, adapter `agenda_reveal_dual.rs`, `dual_window::complete_nested` adds `details.agenda` for agenda triggers.
* Tests: `crates/ti4-engine/src/agenda_reveal_tests.rs` (18 lockstep tests, all green; each asserts compared==1, bailed==0 and no differences): Quash declined/used/no token/empty deck, Favor used+returned/unpayable/empty deck/Quash first, Secret used/declined/barred, redraw below/above v14, empty deck, covert v33/replaced/empty/below v33, Veto, two Vetos, 7 reaction cards followed, bails (deck reserve, generic fixture seats, unknown agenda).
* Mutation check (docs script: scratchpad mutate.py, 28 mutants): M01..M18 KILLED. NOT yet run: M19..M28 (in_play not set, outcomes not stored, AGENDA_REVEALED not emitted, favor spends holder's token, favor offer continues after use, fx_agenda enacted/round, empty-deck redraw guard, wrong note kind, result lost). Re-create each by hand (single-line edit in reveal.rs / fx_agenda.rs, run `cargo test -p ti4-engine --lib agenda_reveal_tests`, expect FAILED) and add a test for any survivor.
* Known-issues lines B9c1-* are in the core-integration worktree docs/ENGINE_KNOWN_ISSUES.md (committed there by this run).

## Next
1. Finish mutants M19..M28 (CARGO_BUILD_JOBS=2, CARGO_TARGET_DIR=/root/deploytmp/core-b9c1; ~4 min each).
2. Add a core-only unit test or two for `Reveal` if wanted (lockstep tests already cover every branch).
3. Heavy runs, ONLY now that the lockstep DoD is complete and after step 1: `cargo clippy` clean; `SKIP_AB=1 SKIP_WASM=1 tools/engine_core/verify_package.sh agenda <out> /bin/true 3` (--profile gate), golden dual with `agenda=dual` (0 differences, driver=dual), board_equivalence 0 mismatches, bench + bench-v1..v5 200/200 on the final commit. Behaviour-neutrality of the string split is NOT yet verified by the wide suite (only the targeted game::tests political/quash/veto/covert and the delivery inventory ran green earlier).
4. Failure regressions: none found so far by a gate (no gate has run). The one test fixed along the way: the removed covert-dual test (superseded by `covert_legislation_draws_its_face_down_agenda_and_loses_it_when_replaced`).
5. Delete /root/deploytmp/core-b9c1 when done.

## Update (resumed)
* Mutants M19-M28 run: M19-M22, M24, M25, M27, M28 killed; M26 (no-choice redraw guard dropped) kills by aborting the test process (the mutated loop never ends); M23 (Favor offer continues after use) survives and is an equivalent mutant: only one Political Favor note exists (one Xxcha), so the list has at most one entry and the position after it is the same.
* Wide suite `cargo test -p ti4-engine --lib --no-fail-fast`: 4212 passed, 0 failed (string split is neutral for it).
* Gate started: see the report; out dir /root/deploytmp/core-b9c1-out.
