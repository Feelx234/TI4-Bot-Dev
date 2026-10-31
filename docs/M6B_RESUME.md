# D2 lane M6b (`core-m6b-tactical`): finishing the tactical island, resume note (2026-10-31)

Branch `core-m6b-tactical`, cut from `core-m5-aftermath` (b15c8773), then merged with `core-x-explore` (the native planet exploration window; Van Hauge moved to slot 7005 beside
Indoctrination 7003 and Greyfire 7004) and `core-d2-c2-turn` (lane B, the turn flow: `driver::turn`, `Asking.at`, the `End` / `Finish` jobs). Never pushed; the live branch
`nightly-fixes-2026-10-26` is untouched. Read `docs/M6_RESUME.md` (the tactical composer), `docs/M5_RESUME.md` (the aftermath) and `docs/D2_LF_RESUME.md` (the recipe) first.

## What is native now

| item | where | how |
|---|---|---|
| Activation hooks that ask | `ti4-engine-core/src/driver/windows/tactical/hooks.rs` | ONE step in string order (marks, Mahact commander, Spatial Conduit, Scanlink, E-Res, Nullification Field, Starlancer, Minister of Peace, Ceasefire, Dominus Orb); every question goes to the TAPE (`Sx::ask`, the way lane B's jobs ask): the driver rolls the step back at a question and replays it with the answers (`Pending::nested`). New typed questions `choice::tactical::{Ask::Hook, Ask::Scanlink, Opt::Use, Opt::Explore}` and their renderer arms (`render/tactical.rs`) |
| A hook that ends the turn | same | `end_turn`: active system and pending tag cleared, the plain `TURN_ENDED_BY_*`, then lane B's `TurnJob::End` is opened (`driver::turn::open_job`) and the action leaves the stack. The Mahact commander returns both tokens (`TStage::Hooks(returns)`, `Machine::held_before`); `activate::holds_leave` lets the Activating question list a system holding the seat's own token |
| Spatial Conduit links | `kernel/fx_move.rs` `set_conduit_links` / `clear_conduit_links` | written into the map's `extra_links` for the activation, cleared by the tail |
| Scanlink | `hooks.rs` | planet question, then `Exploring` (lane X) run inline; plain `SCANLINK_EXPLORED` |
| Minister of Peace | `hooks.rs`, `live/turn.rs` | `OfferDiscard` child; the offer facts (law text, the activated system and seat) in `live/turn.rs::complete` |
| Routes | `rules/tactical/routes.rs` (`ship_routes`), `tactical_live.rs` (`details.routes`), `live/mod.rs::pick_of` (`|rN`), `TacticalAction::accepts_unlisted` | the string's selection (default, the safe boosted way, one per profile, fill-up) over `MovementRules::all_routes_from_ship`; the same function lists them and resolves an answer; `MovementRules::is_anomalous` is new |
| Space cannon offense hits that ask | `tactical/mod.rs` (`Heading`) | the questions after the enforcement go to the tape; the head is re-derived from the position at every replay (the enforcement is over by then). `TacticalGap::CannonHits` is gone |
| Tail explorations | `tactical/tail.rs` | Crown of Emphidia (`CrownExplore`, deck = the planet's printed type) and Dark Energy Tap (`Exploring::frontier`, plain `FRONTIER_EXPLORED`) inline; `open_finish` after `TACTICAL_ACTION_COMPLETE` (lane B's job) |
| Initiative order | `driver/windows/live_common.rs::driver_of` (used by tactical and invasion), `driver/live.rs::sync_clock` | `Resolver::set_initiative_order(initiative_order(core))` |
| Silent-gap audit | `reactions_core.rs` test `every_system_activated_ability_the_string_registers_is_ported_or_accounted_for` | diffs the string's `SYSTEM_ACTIVATED` abilities (24 factions) against the core table; each of the 27 is `Carried`, `Dropped` or `Gap` |
| `NativeSubject` | `ti4-engine-core-bridge/src/native_subject.rs::open_arm` | `Arm::Tactical` opens `TacticalAction` from the observed `Window::Tactical` payload (not exercised by a test: see Not done) |

## Findings fixed on the way (each guarded by a test that fails on the old code)

* **The copied ground round was numbered twice** (lane M5's finding, "conquest 2 vs 1"): the Sol agent exhausts at the very start of a round, so the roll tail asked `copied_round` again, found it false and bumped `combat_round_seq` once more. The native fight now decides it once, at the start of the round (`state::Invasion::round_copied`, a LIVE-only payload field). Guard: `invasion_live::a_conquest_runs_natively_...` now compares `combat_round_seq` with the string (it failed 2 against 1 for the Sol invader before). (The "five-round fight 6 vs 5" of the note was the same single extra bump per fight, not one per round.)
* **The initiative order** was never set on the live drivers (`pending.resolver.initiative` empty at the first window of a game): `live_common::driver_of` (tactical and invasion now use it; they had their own copies, without the play facts of the position either) and `driver::live::sync_clock` set it from the position.
* **The merge slip of integration 9 (B1) came back** with the lane-B merge: the neutral bail in `dual_window::settle_run`. Removed again (both neutral combat lockstep tests failed with "0 compared, 60 bailed").
* **`SeatReach` and the conduit**: the Spatial Conduit's links live in the map's `extra_links` for the activation (`fx_move::set_conduit_links`), so the route lists see them; the tail and the turn-ending hooks clear them.

## Checks run (cheap) and results

All with `CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0`, a private `CARGO_TARGET_DIR=/root/deploytmp/core-m6b` (deleted at the end).

* `cargo test -p ti4-engine --lib`: 5006 passed, 2 failed (the two neutral combat lockstep tests: the slip above, fixed afterwards); `TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1 cargo test -p ti4-engine --lib` on the last commit: 5008 passed, 0 failed, 4 ignored (634 s).
* `tools/engine_core/playthrough_quick.sh --maxsteps 1500` (sol, 4 players, seed 1, gate build): the island table is UNCHANGED (M6 tactical 631 counted bails, M5 482, M3 114, M4 71, B 865): `Game::step` does not call `live_step` until the wrapper (D2-8) exists, so the bin counts the dual's windows, not the native arms. The proof of this lane is the lockstep and the sweeps above, not that count.
* Engine lockstep, all green: `tactical_live` (Spatial Conduit use / decline / route 1, Nullification Field, Minister of Peace, Dominus Orb, Mahact commander and Starlancer, Scanlink, a ship exiting a gravity rift for 16 seeds with the rng positions compared, a clone taken while a hook waits, the 24-faction activation sweep with `mahact` and `titans` and no filter), `invasion_live`, `combat_live` (the 24-faction fight sweep, Mahact entries named residual below), `reactions_core` (the audit).
* `cargo test -p ti4-engine-core --lib` (469), `-p ti4-engine-core-tests --test tactical_composer --test aftermath_chain --test invasion_composer --test exploring` (all green).
* `check_layers.py core`: 0 violations (one found and fixed: `rules::tactical` imported `rules::leaders`; it uses `query::leaders` now); `lint_core.sh`: 0 hits.

## Not done (precise)

1. **Agents that listen to the end of the action** (`TACTICAL_ACTION_ENDED`): T'ro (Sardakk, `leader:*:sardakkagent:ACTION_COMPLETED:after`), Field Marshal Mercer (Nomad) and the Ssruu copies are string-only abilities. The tail does not skip them silently: it stops with the counted gap `TacticalGap::SsruuLends` when one would be offered (a readied T'ro with infantry in the actor's reinforcements and a planet the actor controls in the active system; Mercer with a planet and ground forces). Porting T'ro = a `Spec` in `cd_registry/sardakk` whose effect runs `PlaceUnits(actor, infantry, 2, Some(system), ..)` through the tape; Mercer is the Nomad's `mercer_plan` (two questions per take).
2. **Thunder's Edge Warfare's follower window and a hero's strategy primary** after the free tactical action (`Then::Secondary`, `Then::LeaderStrategy`): both need the after-action redistribution (`redistribute_around_te_warfare(.., false)`, a split question, string-only) and lane E's follower composer; counted gaps `WarfareSecondary` / `LeaderStrategy`.
3. **Nova Seed** (a `MapEdit` mark written during `MOVEMENT_FINISHED`): the core galaxy has no replace / swap of a tile and the contents move (`movement::apply_map_edit`); counted gap `NovaSeed`.
4. **The 13 in-scope `SYSTEM_ACTIVATED` abilities the core table lacks** (`Status::Gap` rows of the audit test): Empyrean Aether Passage and Aetherstream, Titans Awaken and the flagship Ouranos, the Ghosts agent (Emissary Taivra) and mech, the Saar agent (and its Ssruu copy), the Arborec commander and flagship, the Yssaril commander So Ata, the Cabal promissory Crucible, Promise of Protection, Naalu Neuroglaive. An eligible activation misses their offer SILENTLY in the native path (the string-side lockstep shows it as a missing question, which is how the audit's list is guarded); the test only keeps the list from growing. Each needs a `Spec` of its faction crate (most are 10-40 lines in the pattern of `cd_registry/yin/timing.rs`).
5. **Mahact at the table, two open differences** (named residuals in `combat_live`'s sweep, no bail): the string asks 8 more questions than the native run answers after the fight (`mahact v letnev`), and the fleet limit shown by the removal question of a seat whose token sits in the Mahact pool is 4 in the string and 3 in the core (`letnev v mahact`; `fleet::limit` is the same code on both sides, so the position the core loads differs: the pool marks or the Armada reading).
6. **Starlancer's gain** asks the pool question as a raw `gain_pool_choice` (no `COMMAND_TOKEN_GAINED` plain: the string's `gain_tokens` writes none either, matching).
7. **`NativeSubject::open_arm(Arm::Tactical)`** is wired but not exercised by a test (the 3 subject tests run the tokens arm); `window_of(Arm::Tactical)` is left `None` because the action hands its entry to the aftermath and stays on the stack.
8. **Mutation checks, `playthrough_quick.sh`, benches, the golden sweeps**: not run (see the final report).

The lane-B jobs `Finish` / `End` are opened by the tail and by the turn-ending hooks; nothing in this lane runs them (`driver::turn`).
