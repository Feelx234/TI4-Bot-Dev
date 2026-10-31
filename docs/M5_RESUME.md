# D2 lane M5 (`core-m5-aftermath`): the aftermath chain island, resume note (2026-10-31)

Branch `core-m5-aftermath`, cut from `core-m6-tactical` (acc9035e) and merged with `core-m3-invasion` (conflicts in `driver/live.rs`, `driver/step.rs`,
`kernel/ability.rs`, `dual_window.rs` kept both sides). Never pushed; the live branch is untouched. Read `docs/D2_LF_RESUME.md` (the live loop and the recipe),
`docs/M6_RESUME.md` (the tactical hand-off) and `docs/M3_RESUME.md` (the invasion) first.

## What is native

| piece | where | what |
|---|---|---|
| **EventScoring window** | `ti4-engine-core/src/driver/windows/scoring.rs` | `EventScoring` (`Held`, `Arm::EventScoring` registered): the `ScoringBook` walk over core-computed offers (`CoreOffers`: the hand's secrets whose feat is recorded for the occurrence, the leaked secrets from rules version 24; no publics, no plots), one `ScoreOne` per seat, the plain `OBJECTIVE_SCORED:{objective}` and `GAME_FINISHED` records, the window closing when the game ends. `open_event_scoring(core, occurrence, one_per_player)`. `feat_of(SecretObjective)` is the table `secrets::feat_for` (by card). Yin hero (lane 14) can open the same window |
| **Space combat as a window** | `driver/windows/combat/{mod,env}.rs`, `rules/combat/machine.rs` (live mode) | `CombatLive` (`Held`, `KIND = Combat`) runs the composed `Machine` in its LIVE mode: the machine is rebuilt from `(CombatFields, position)` at every call; its stage questions are the players' decisions (announce a retreat, a retreat destination, sustain, casualty incl. the PRODUCER-assigned casualty of Devotion); a question of a child window (reroll, Raid Formation, Ambush, Assault Cannon, the Letnev commander's payment) goes through the TAPE (`Machine::ask_tape`) and is a nested question of the step (the driver replays the step with the answer), so no child has to be kept between two calls. `env_of` builds the machine's `Env` from the position + the adapter's `CombatTexts` (retreat neighbours through `SeatReach`, Dark Energy Tap, Duranium, Assault Cannon, the Jol-Nar commander, Munitions, wormhole, `staging_enabled`); the string gates (`round_opening_why`, ...) name dropped content and are off. `open_combat` writes `Window::Combat` (or finds no fight: `Opened::NoFight`); `advance_combat` parks on the barrage's scoring pause (scoring window above) and fails with `AftermathGap::{CombatUnported, RollEndsFight}` |
| **Staged-destruction drains** (lane 14 leftover) | `rules/combat/{assign,flow,end_announce,machine}.rs` | `assign::drain_staged`; the four string drain points in the live machine: after a retreat (was there), after the module-produced hits (`finish_produce`), after `SPACE_COMBAT_ROUND_ENDED` (`Flow::with_drain`), in the end announcement (`EndAnnounce::with_drain`). The `transition` guard that bailed on a pending destruction is off in live mode (the string keeps them pending across windows). The dual is untouched (builders, `live` flag) |
| **Van Hauge** | `ti4-engine-core-factions/src/cd_registry/yin/timing.rs` (`VAN_HAUGE`, slot 7003), `reactions_core.rs` NAMED + `Codes.van_hauge` | the Yin flagship as a rule of the core table (every ship of the system removed and staged with the cause `unit_ability:yin_flagship`, announced at the drain points) |
| **The chain** | `driver/windows/chain.rs` | `AftermathChain` (`Held`, `KIND = Aftermath`, `Arm::Aftermath` registered): `Fighting` (`AFight::{Opening, Fighting, Closed, Announced, Over}`) -> feats (`feats::feats/record`, the `OnePerPlayer` pause) -> `FightOver` (win announcement + `EnforceAll`, resumed from the position) -> `Invading` (`open_invasion` / `open_at_commit` for the Sardakk skip) -> `Producing`. `advance_aftermath` is the arm: it hands the answer to the sub-window on top (combat, invasion), runs the chain, stops at a stage question, a nested question or a scoring window opened above (`Live::Done` with the entry still on the stack), and writes the plain `SPACE_COMBAT_RESOLVED` / `INVASION_RESOLVED` / `PRODUCTION_RESOLVED` AFTER everything the step emitted (`Aftermath.log`, flushed at the end of the call). `driver/windows/live_common.rs`: `advance_parkable` (the generic advance with a close/park/fail hook), `table_at` (play facts), `driver_of` |
| **Typed state** | `state/aftermath.rs` | `Aftermath` gained `fight`, `occurrence`, `outcome`, `holds`, `invaded`, `log` (all LIVE, the string observed copy sets `fight`); `Aftermath::new`; `AFight`. `d1_action::aftermath_of`/`aftermath_back` map them (compare-only; the key is unchanged) |
| **Engine half** | `ti4-engine/src/combat_live.rs`, `live/mod.rs` (`Cx.texts`), `live/details.rs` | `combat_texts` (interns the state and shown texts of the unit types in the fight and every roll group, per call; the core holds no text), `complete` (the `sustainers` detail, the casualty subtype), `gap_text`; the lockstep tests |

## Findings fixed on the way (each guarded by a lockstep that fails on the old code)

* the live resolver carried an EMPTY initiative order (first window of a game): an emission offered its windows to the first seat only (found by the Letnev agent
  `COMBAT_ROUND_STARTED`). `chain::sync_initiative` writes the position's order into `pending.resolver` before every aftermath/scoring call. The same hole is in
  `driver::live::sync_clock`, `invasion/live.rs::driver_of` and `tactical/live.rs::driver_of` (they set the clock but not the order): lane A2/M3/M6 should set it there.
* `EnforceAll::resumed` did not start the asking pair's window (`NotAsking` on the answer of a `remove a unit` after the fight).
* `supply::staging_enabled` is "some seated faction has a module" (NOT Sol, Hacan, Jol-Nar, L1Z1X, Letnev, Xxcha): M6 hard-coded `true` for the cannon facts; now `combat::staging_enabled`.
* an empty fight (fewer than two sides) presents `complete` once (string `settle_stages`).
* the producer-assigned casualty (Devotion) was not asked by the machine (`pending_choice` returned `None`: the dual bailed it); it is asked of the producer now (the dual's
  `a_hit_the_producer_assigns_is_answered_in_the_string` still passes).

## Finding for lane M3 (not fixed here)

The native invasion bumps `combat_round_seq` once more than the string over a ground fight (conquest: native 2, string 1; a five-round fight 6 vs 5): `driver::windows::invasion::fight`
(the numbered-round bump) AND `rules::invasion::tail::GroundRollTail` (`f.bump`) both add one. `M3`'s lockstep does not compare it; the combat_live harness filters it
(`combat_round_seq`) with a comment. Round-scoped marks compare to the counter, so it is harmless until a mark of one round is read in the next.

## Contract with the neighbours

* Tactical (M6) hands over by writing `Window::Aftermath` and keeps its entry; this arm closes ITS entry, the tactical tail runs next (`TStage::Aftermath` -> `Tail`).
* Invasion (M3): `advance_aftermath` calls `advance_invasion` while the invasion is the top sub-window; the scoring window the invasion opens is closed by `Arm::EventScoring`.
* Production (M2, not started): the `Producing` stage is a counted gap `AftermathGap::ProductionStep` when the seat has a PRODUCTION capacity in the system (otherwise the step ends at once and
  `PRODUCTION_RESOLVED` is announced). When M2 lands: open `Window::Production` there (`chain::producing`) and let `advance_aftermath` hand its steps over like the invasion's.
* Contact pause: `AftermathGap::Negotiating` (unreachable with diplomacy off).

## Counted gaps of this island (`Unknown::UnportedAftermath(AftermathGap)`)

`NoProgress`, `ProductionStep`, `Negotiating`, `NoCombatEnv` (unused), `CombatUnported` (Duranium Armor repair queues, a retreat the core does not move: Last Bastion, ...), `RollEndsFight`,
`NeutralSide` (unused: a neutral side makes `may_retreat` false and the flow bail through the machine), `EventScoring` (unused).

## Not done (precise)

1. **Production step** (see above; M2's island).
2. **Spinner (`yso`, Yin)**: a placement question after `UNITS_PRODUCED` over planets AND ship spaces. `rules::promissory_windows::PlaceUnits` offers planets only (`Opt::Promissory(Spot(System, Planet))`). Needs a space spot option
   (`Spot` without a planet or a `Space(System)` option + renderer id), a `PlaceUnits` scope `PlanetOrShipSpace` (spots: controlled planets, then systems with the seat's ships; the capacity/fleet limits `PlacementLimits::Respect`)
   and the `Spec` on `UNITS_PRODUCED` after (technology `yso`, optional, the seat owns it, a spot and infantry in the box). It fires only after a production, which the aftermath does not run natively yet.
3. **Child windows that need a persisted state across a stage question**: none are kept (all go through the tape). The one transient the machine keeps is `round_end_produced` (Devotion's hits are produced once per round): it is lost between two calls
   when the round-end hits queue asks a stage question; the flow then returns to the next round, not to the same retreat step, so it is harmless in the cases tested (Yin sweep); a Devotion + retreat of the same round is not tested.
4. **Display facts**: the `RerollDie` label uses the group's STATE code as the shown code (the core cannot mint the renderer's table); the reroll questions (Jol-Nar commander, Munitions Reserves, Hacan flagship, Crowns) are not in a lockstep.
   `details::complete` has the combat family only for `CancelHit` (sustainers) and `AssignHit` (subtype); the string's `about_battle` context of Ambush/Sacrifice/Splicer/Raid is given by the nested rendering (`NestedPos::of_core`): Ambush is compared, the others are not.
5. **EventScoring**: `award_tail` (the leaders a PUBLIC score unlocks, Yin's staged event) is not run for a leaked secret scored as a public objective; no plots (Firmament is dropped).
6. **Residuals of other lanes seen by the faction sweep** (filtered in `combat_live::tests::a_space_fight_of_each_faction_...`): the Saar agent (`leader:saar:saaragent:SYSTEM_ACTIVATED:after`, a string-registered ability the core registry lacks: M6 item 2),
   Titans/Mahact activation hooks (M6 items 1), the Empyrean commander's unlock (`commander_unlocked` hook slot: A2/F2).
7. Thunder's Edge Warfare's follower window after the free tactical action, Crown/DET explorations: the tactical tail's (M6).

## Tests (cheap)

* `ti4-engine` `combat_live::tests` (9; the Letnev commander/agent case is the ninth): lockstep of a real tactical action through `Arm::Tactical` / `Arm::Aftermath` / `Arm::EventScoring` against the string `Game` (questions: player, prompt, options, details, context; typed + plain events of the aftermath's arms in order; the whole position):
  a multi-round fight with sustains and casualties, a retreat, an undefended system -> invasion, a won fight -> invasion, a destroyed capital ship -> scoring window -> `OBJECTIVE_SCORED`, Mentak Ambush (a child question through the tape), a space fight of every faction on both sides (24 factions x 2), Van Hauge x 24 (12 seeds, either side).
  `M5_DEBUG=1` prints both question lists and the events; `M5_ONLY=<faction>` narrows the sweep.
* `ti4-engine-core-tests` `aftermath_chain` (15; the last seven are the event scoring window on its own: order, decline, score, the one-per-player cap, leaked secrets by rules version, game end, points shown): a whole aftermath in the core, a clone mid-window continues identically, payload identity after every question, an undefended system to the invasion, the Sardakk skip to the commit step, the production gap, the contact gap, a feat -> scoring window -> `OBJECTIVE_SCORED` (the aftermath waits under the window).

## Mutation checks (single file, `tools/engine_core/mutants.sh` on a committed snapshot in a scratch worktree, 2 jobs)

Two test sets per file: the core tests (`--test aftermath_chain`) and the engine lockstep (`--lib combat_live`). Core tests alone, before the last scoring tests: scoring.rs 54 mutants 13 killed, chain.rs 95/55, combat/mod.rs 21/12, live_common.rs 7/3.
Engine lockstep alone: scoring.rs 16/54, chain.rs 57/95, combat/mod.rs 16/21, live_common.rs 5/7. With the seven added scoring tests the core tests kill 27/54 of scoring.rs. Survivors that are equivalent or unobservable by design:
`publics`/`plots` bodies (both empty), the capacity 8 of the offer lists (a hand holds 3), `EventScoring::stage`, `one_public_before` (leaked secret scored as public), the idle guard of `chain::step` (3 laps), `is_open`/`merge`/`flush_open` guards the lockstep reaches only through
the order of plain events. NOT all survivors were classified: chain.rs still has about 38 survivors against the core tests alone (the engine lockstep kills 20 more of them); a full union run and the classification is the first thing to redo.
A first run of the mutation batch used the filter word `aftermath_chain` (a test NAME substring, matching nothing): invalid, discarded.

## Slip

The cargo builds before the last third of the lane ran without `CARGO_TARGET_DIR`/`CARGO_BUILD_JOBS` exported in their shell (each Bash call is a new shell): the worktree's `target/` grew to 8.8 GB and used the default job count. It was moved to `/root/deploytmp/core-m5/target`, `incremental/` deleted (3.7 GB); every later call exports `CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/root/deploytmp/core-m5/target CARGO_INCREMENTAL=0`.

## Checks run (cheap) and results

See the final report of the lane (numbers below were taken on the last commit): `cargo test -p ti4-engine --lib` (4933+ passed), `TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1 cargo test -p ti4-engine --lib` (one failure on the first run, the native-registry equality of `reactions_core`: Van Hauge is registered for every seat in the string; fixed), `cargo test -p ti4-engine-core --lib` (466), `-p ti4-engine-core-tests`, `-p ti4-engine-core-factions`,
`tools/engine_core/check_layers.py core` (0 violations), `tools/engine_core/lint_core.sh` (0 hits). Private target `/root/deploytmp/core-m5/target` (`CARGO_INCREMENTAL=0`, `CARGO_BUILD_JOBS=2`).

Not run: any heavy gate (golden/wide dual, benches, soak, mutation ratchet, census, workspace clippy), `playthrough` (the production step keeps a whole game from being native).
