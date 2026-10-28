# The core driver and the window contract (package D1, `core-d1-driver`)

What exists after D1's first slice: a generic `Window` contract in the kernel, a `Driver` that runs one window to
its next question and resumes it, a lockstep dual for stateful windows, and one real window ported end to end
(the Rear Admiral Farran offer of the combat window). B1 and B3 copy the pattern; the note for them is at the end.

## 1. The contract (`ti4-engine-core/src/kernel/stage.rs`, layer L3)

```rust
pub trait Window: Clone {
    type Stage: Copy + Debug + PartialEq;                        // one variant per suspension point
    fn stage(&self) -> Self::Stage;
    fn step(&mut self, sx: &mut Sx<'_>) -> Next;                 // string `settle`: run to a question or the end
    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice>;   // string `pending_choice`: pure
    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal>;  // string `resolve`
}
```

* `step` returns `Next::Continue(ask)` at a question (the driver checks `ask == ask_of(..).ask`), `Next::Done` at the
  end, `Next::Open(window)` to hand a window of another area to the driver, `Next::Emit(rec)` as a shorthand for
  `sx.emit`. `answer` is the only place an answer changes the state; the driver calls `step` again afterwards.
* `Sx` is `Cx` plus what a window needs: `sx.state`, `sx.kernel`, `sx.q()` (the `Q` of the seams), `sx.cx()` (a
  plain `Cx` for `Offer::apply` and observers), `sx.observe(&Obs, Phase)` (P5), `sx.emit(rec)` /
  `sx.emit_with(rec, resolve)` (the string `ctx.emit`: WHEN, resolution, AFTER through the A5 `Resolver`, trace ids
  from the driver), `sx.ask(&choice)` (a question that is not a stage of this window).
* Offer points (P6) are a stage, as in `ENGINE_DISENTANGLE.md` 2.6: `Offering { point, idx, then }`; `step` calls
  `k.hooks.seam.offers.next_ask(sx.q(), point, idx, &site)` and returns `Continue` with that ask; `answer` calls
  `offers.apply(&mut sx.cx(), point, idx, &site, opt)` and handles the returned `Next`. Observers: `sx.observe`.
* What a window may read: `(window, state, kernel)` and the answer. No clock, no global, no hash iteration; random
  numbers are the state's `rng` streams (`state.rng`), so a re-run draws the same.

### Two kinds of question

1. **The window's own** (`Next::Continue`): a stage. The state is exactly `(window, CoreState)`; a checkpoint is a
   clone.
2. **Inside an emission** (a reaction window asks while `sx.emit` runs): cannot be a stage of the window. The
   resolver's `Host::decide` reads the `Tape`. If the tape is dry, the driver rolls back `(state, window,
   resolver, emitted, next_id)` to the start of the step, reports the question, and re-runs the same step with the
   answer appended. Effects therefore run once per committed run; a step must be deterministic over `(snapshot,
   tape)`. This is the string engine's replay trick, at step granularity instead of whole-game.

`AbilityTable` is the `Host` behind the resolver (`condition`, `effect`, `forbids` by `AbilityId`). It is empty until
the abilities are ported; an unknown ability is eligible and does nothing. Effects get `&mut CoreState` and stage
their events (`fx::stage`), they do not emit.

## 2. The driver (`ti4-engine-core/src/driver/mod.rs`, layer L5)

```rust
let mut d = Driver::new(window, Resolver::new(&initiative, active), AbilityTable::default(), next_event_id);
match d.run(&mut state, &kernel)? {            // Status::Asking(choice) | Status::Done
    Status::Asking(choice) => d.resume(&mut state, &kernel, pick)?,   // answer, then step on
    Status::Done => {}
}
// d.emitted: the events the window emitted itself; d.opened: the windows it asked to open
```

Illegal picks are refused with the state untouched; `resume` after `Done` is `NotAsking`. The driver owns the
`Resolver`, the `AbilityTable` and the trace-id counter; D2 moves them into the state.

## 3. The dual for stateful windows

`dual_window!(Area::Driver, "label", state, |s| string side -> Run<R>, |s, script| core side -> Option<CoreWindow<R>>)`
(`dual.rs`, `run_dual_window`; adapter `dual_window.rs`). On `driver=dual` (`TI4_AREAS=driver=dual`):

1. The string closure runs for real. A thread-local recorder collects the window's own questions (`Table::ask*` at
   emission depth 0) with the option id they were answered with (`None`: the decider had no answer and the window
   unwound), and the events emitted at the outermost level (`Resolver::emit*`). Nested reaction questions/events
   are left to the A5 shim.
2. The core closure gets a copy of the state and the recording; it lends the slices of its `Footprint`
   (`with_core`), runs `dual_window::drive`, which renders each core question (`render::Typed`) and answers it with
   the string side's answer id (`render::parse_pick`), so both sides stay on one path. It stores the written slices
   back and applies the staged events as the string `private:#staged:event:*` marks (`apply_staged`).
3. Compared: the question count and each question (`compare_choice`: ids, kinds, labels, payload, context,
   decision hashes V1/V2), the returned value, the resulting `GameState` (leaves and bytes), the events (ids
   zeroed), the rng positions when set. `core` returns `None` when the position holds something the core cannot
   express (the string result stands, nothing is compared).
4. `Backend::Core` runs the string closure until D2 gives the driver the table.

The mutation test (change the gain to 2 in the core window) fails the unit test
`letnevcommander_2_the_sustain_offer_runs_as_a_core_window_in_lockstep`.

### Dice in a window (B1c)

`Sx::die(domain, sides)` draws from the stream of the state's `rng` slice (`Rng::die`, the same ChaCha8 words
as `GameRng::die`). The streams are seeded by `Sx::seeds`, which the adapter gives the driver with
`Driver::with_seeds(Rc<dyn SeedSource>)` (default `NoSeeds`: all-zero seeds, for windows that never draw). The
rng is part of the state, so a step rolled back for a nested question restores the streams with everything
else and the re-run draws the same dice (unit test `dice_drawn_in_a_step_that_is_rolled_back_are_drawn_once`).
Recording the roll in `ledger.dice` is the rule area's (`rules::combat::record_rolls`), not the kernel's.

## 4. The worked example (`driver/windows/sustain_commander.rs`)

String `combat::ask_sustain_commander`: if the holder has an unlocked Letnev commander, ask "gain 1 trade good for the
sustained damage"; on `gain`, +1 trade good and `TRADE_GOODS_GAINED` staged. Core: `SustainCommander { player,
source, stage }` with `Stage::{Check, Asking, Done}`; `ask_of` builds the typed `Choice` (`Ask::Leaders(SustainCommander)`,
options `[GainTradeGood, Decline]`, origin `Content(Leader(RearAdmiralFarran))`); `answer` calls
`fx::gain_trade_goods_staged`. Choice types: `choice/leaders.rs`; renderer: `ti4-engine/src/render/leaders.rs` (the
`Leaders` entry of the registry is now live). Call site: `combat.rs` `ask_sustain_commander_windowed` is the `dual_window!`
wrapper around the unchanged string body `ask_sustain_commander` (the delivery-inventory registry keeps its name).

Known hole, in the dual as `None` (skipped, not compared): the commander ability held other than by an unlocked
leader (granted mark, Mahact Imperia, an "An" promissory note). That is `promissory::has_commander_ability`, B7's.

The example lives under `driver/` only because `rules/` is created by B1 and B3; B1 moves it to `rules/combat/`.

## 5. Ready to paste: porting a window onto the driver (B1, B3)

> You port a window onto D1's driver (branch `core-d1-driver`; merge it first; read `docs/ENGINE_CORE_DRIVER.md` and
> `driver/windows/sustain_commander.rs`, which is the template). For each window (`CombatWindow`, `BeforeCombat`,
> `ProductionWindow`, ...): (1) write `enum Stage` with one variant per string `Stage` **and one per inline `table.ask`**
> (lists in `ENGINE_DISENTANGLE.md` 6.3), plus the window struct (`Clone`, small, `SmallList`s, no heap) and
> `impl Window`: `step` is the string `settle` (a `loop`/`match` that moves between stages and returns
> `Next::Continue(ask)` at a question, `Next::Done` at the end), `ask_of` is `pending_choice` (pure, from the window and
> the state), `answer` is `resolve` (the only place an answer mutates). (2) Every `ctx.emit(state, "NAME", payload)`
> becomes `sx.emit(fx::event(EventKind::.., payload))?` at the same program point; the `?` error is `TimingError`.
> Every `table.ask` that is *the window's* becomes a stage; one asked by a callee you do not own stays `sx.ask` /
> an emission and is answered through the tape (do not port it as a stage). Seam calls: `sx.q()`; offer points:
> `Stage::Offering` as in 2.6; observers: `sx.observe(&Obs::.., Phase::Before|After)` at the string hook's program
> point. (3) Choice types: add `Ask/Opt/Subtype` variants in your `choice/<area>.rs` and the `render/<area>.rs`
> `Family` (prompt, subtype, option, parse) byte for byte; options in today's order. (4) The dual: wrap the string
> call site in `dual_window!(Area::Driver, "<fn>", state, string_closure, core_closure)` exactly as `combat.rs`
> `ask_sustain_commander` does: keep the old body under its name (tests/decision_delivery_inventory.rs lists it), add a `<fn>_windowed` wrapper, build the `Driver` in an adapter function in
> `dual_window.rs` (or your own file) with a `Footprint` of the slices the window reads/writes, call
> `dual_window::drive`, store back, `apply_staged`. Return `None` for positions the core cannot express yet; list
> them in your report. (5) Run `TI4_AREAS=driver=dual TI4_DUAL_FAIL_FAST=1` over the engine suite, then the 20 golden
> games (`dual_run tools/engine_profile/profile-20.json 20 driver=dual`); mutate the core half once to see the dual fail.
> Pure refactor: no CPU A/B, no wasm, `RULES_VERSION`, serde, fixtures unchanged. Stateful `ctx.emit` sites that
> are reached from several windows (e.g. `emit_sustain_used`) are shared `fx`-style helpers taking `&mut Sx`.

## 7. B2 invasion windows (state after the hits-step slice)

Core windows of `Area::Invasion` (each a `dual_window!` with documented bails; counts are the 20 golden games,
`dual_run profile-20.json 20 invasion=dual,driver=dual`, 0 differences):

| window (string site) | core | compared / bailed |
|---|---|---|
| `GroundHits` (`ground_hits_window`, `remove_ground`, `take_bombard_hits`) | `rules/invasion/hits.rs` + `kernel/hits.rs` | 413 / 8, 2487 / 0, nested in `bombard_step` |
| `Bombardment` (`bombard_settle`, `answer_bombardment_target`, `complete_bombard_plan`) | `rules/invasion/bombard.rs` | step 1371 / 6, answer 0 / 0 (no coexisting bombardment in the golden games; the unit test covers it) |
| `LandOne` over the string commit pool (`commit_one`) | `rules/invasion/commit.rs` | 2007 / 23 |
| `Custodians`, `FightRound`/`NextCombat`/`BombardTarget` questions | `rules/invasion/stages.rs` | unchanged |

* `kernel::hits` is the shared "before a hit would be assigned" step: `open_window` (the event, the cancellations a
  card played in it granted: `combat_timers.cancel_hits_round`), `spend`, `grant`, `flush_staged`. B1's
  `HITS_TO_ASSIGN` can use it as is.
* `GroundHits { batch, hits, do_open, do_apply }`: `open` (the `GROUND_HITS_TO_ASSIGN` window) and `apply`
  (sustain first, Non-Euclidean Shielding, cheapest ground force; stages `GROUND_FORCE_SUSTAINED/DESTROYED`
  in `pending.staged`; the caller announces them where the string flushes its `log`). Both are plain functions of
  `Sx` (`hits::open`, `hits::apply`) for the bigger windows.
* `Bombardment` carries the plan as data (the string window's `bombard_plan` entries, with the `InvasionStep`
  template built by the `invasion_step` codec): the window mirrors `last_step`, the feat and the scoring occurrence
  it pushes; the string reads them back as `BombardOut`.
* Bails, all counted per reason: nested (a card played in the hits window, a reaction to `UNITS_COMMITTED`), a
  neutral victim, Keleres seat (toll), diplomacy promises, Crimson deploy offered, a ship-form unit flipping on
  landing (not temporary), seats not in id order (the bombard target answer).
* The candidates of the commit pool (Sardakk commander origins, Naalu Matriarch fighters) are still computed by
  the string modules; the core window receives the pool and lands from it (`PoolIn`), so the landing, its events and
  the question are checked, the candidate rules are not.

Stage 3 and 4 slices (counts are the 20 golden games, `dual_run profile-20.json 20 invasion=dual,driver=dual`, 0 differences;
each window was mutated once in its core half and the dual failed):

| window (string site) | core | compared / bailed |
|---|---|---|
| `SpaceCannonDefense` (`announce_and_defend`: `GROUND_COMMITMENT_FINISHED`, then per committed planet the guns' dice, the hits step, `InvasionStep` `space_cannon_defense`) | `rules/invasion/defense.rs` (`defense_plan`, dice from `Sx::die`, `ledger.dice` `SpaceCannon`) | 1074 / 6 |
| `GroundRoll` (`roll_ground_pair`: both sides' pools from `ground_pools`, dice, `reroll_staging` kind `ground`, `last_reroll_player`) | `rules/invasion/roll.rs` | 367 / 0 |
| `GroundRoundHits` (`ground_round_hits`: both hits windows, both removals, the `ground_round` `InvasionStep`, the removals' events) | `rules/invasion/round.rs` | 279 / 83 |

* Bails of the new windows, each counted by reason: defense: entropic scar, L4 Disruptors, attachment cannon (Geoform),
  irreproducible dice (test faces, stream restore), neutral invader, nested (a reaction to the announcement); roll:
  irreproducible dice, neutral side, X-89 Bacterial Weapon (doubled hits), a pool the window cannot name; round hits:
  neutral side, L1Z1X (Harrow), Alarum (Ral Nel mech), hits a faction adds to a roll, nested (a card played in a hits window).
  Golden coverage: nested (6 / 82) and L1Z1X (1) occur in the 20 games; the others have unit-level cover only
  (the bail is a pure pre-check of the string side).
* `List::Planets` is a new payload list (`GROUND_COMMITMENT_FINISHED.planets`); `event_of` renders it.
* The ground `may_sustain` oracle (`core_invasion::register_ground_slots`) answers from the string position the window
  started from. A window with several batches (the round: one side's casualties first) now reads the planet's
  units from the core state when they moved on (`patched_planet`); found by the dual (Mentak's Moll Terminus).
* Study: `finish_committing` keeps Research Team / Viper offers, forced combats and `advance_fighting` in the string
  (they ask questions or change `stage`); only the announcement and the defense run in the core window.

Stage 5 slice (counts as above, 0 differences; the begin and tail windows were mutated once in their core half and the dual failed):

| window (string site) | core | compared / bailed |
|---|---|---|
| `BeginGroundRound` (`begin_ground_round`: `active_invasion.ground_round`, `combat_round_seq` of a numbered round, `GROUND_COMBAT_ROUND_STARTED` of a copied round agent) | `rules/invasion/begin.rs` | 248 / 69 |
| `GroundRollTail` (`roll_ground_tail`: round number, `GROUND_ROLLS_MADE` for both sides, hits read back from the staged dice, the dice as the board shows them, staging cleared) | `rules/invasion/tail.rs` | 307 / 13 (+ 47 pre-check bails) |
| `Advance` (`next_fight`, the selection of `advance_fighting`: planet index, defender, coexisters; starting the combat stays string) | `rules/invasion/advance.rs` | 1253 / 0 |

* Bails: begin: neutral side, Naaz seat (Supercharge, 35 in the golden games), Proxima Targeting VI, a Letnev side with
  infantry on the planet (Dunlain Reaper, 10), nested (69: the copied-round event always reaches the Sol agent's
  offer). Tail: neutral side, X-89, a readied The Thundarian (`GROUND_COMBAT_ROLL_STEP_ENDED` and the replays, 47),
  a roll-replay mark, nested (13). Advance: seats not in id order, nested. X-89, the neutral side, the replay mark
  and the unit-level begin bails have unit cover only (`core_ground_round_prechecks`).
* `GROUND_COMBAT_ROLL_STEP_ENDED` and the replay loop stay in the string (a readied Thundarian is the only listener).
* Nested census of `ground_round_hits` (the 82 bails; `TI4_NESTED_LOG=1 dual_run ...` prints `Recording.nested_kinds`
  of each): 24 with no event at all (an ability effect ran), 29 asks of reaction windows `after GROUND_FORCE_DESTROYED`
  (alone or twice), 14 `TRADE_GOODS_GAINED` (a reaction's effect), 8 `when GROUND_HITS_TO_ASSIGN` asks,
  4 Crimson Legionnaire asks, 2 technology/assimilate asks, 1 `GROUND_FORCE_SUSTAINED` ask. All of them are reaction
  windows or ability effects: handling them natively is the reaction/ability port (the driver's `AbilityTable`),
  not a B2 window, so the bail stays.
* Sensible next step for the hits window: nothing cheaper than the reaction port; the `may_sustain` oracle and
  `temporary_ground_force` are only consulted with the position of the window start, which the single-batch windows keep.

Stage 6 slice (golden: 0 differences; `commit_one` 2007 / 23, `ground_round_hits` 280 / 82 now with the one L1Z1X game compared):

* Crimson deploy: `IOpt::Deploy { planet }` (`deploy_commit|<planet>`) is an option of the commit question after the
  troops' landings; `LandOne::with_deploy` lands a Revenant from reinforcements. `commit_deploy` runs through the same
  `dual_window::land_one` as `commit_one` (the `Crimson deploy offered` bail is gone); `note_deployed` is applied by the adapter.
* Keleres toll: no blanket Keleres bail any more. When the answered planet owes the toll, the adapter runs B3's `PayStep`
  (influence, `TOLL_COST`) between the answer and the landing and sets the toll mark (`mark_toll_paid`). Bails: a landing the
  toll bars (the string question leaves it out), a toll at a table `production_view::unported_window` declines (a Crimson
  invader, Keleres as invader...). Unit tests with several payable planets (the mutation of the spend kind is caught).
* Harrow (`rules/invasion/harrow.rs`): its own window run by `GroundRoundHits` after both removals: `bombardable`, the
  invader's space-area bombarders in order, Plasma Scoring on the first roller, dice from `Sx::die` (ledger `Bombardment`),
  hits window and removal with cause `harrow`, then the report holds `harrow_hits`. Compared: dice (hit number, faces) and the
  stream positions. Bails: irreproducible dice, Proxima Targeting VI. Unit cover: a warsun fight; mutation of the dice count caught
  (a mutation of `harrow_hits` in the report is not caught by that game, no harrow hit lands in it).
* Advance: unit tests for the nested bail and a lockstep fight (mutation of the planet filter caught; the
  controller choice needs three seats and is golden-only). "Seats not in id order" is unreachable (the owner table is a sorted set).

Not done, and why: `FinalizingControl` / `finish_control_gain` (a chain of faction, technology, legendary, fracture and
exploration hooks plus the `PLANET_CONTROL_GAINED` reactions: belongs to B5/technology and the ability port), the rest of
`finish_committing` (Research Team / Viper asks, forced combats), the setup/end hooks and the scoring pauses (the driver
pause for occurrences), and `ground_combat_round_ended` modules (no module implements the hook; only Harrow does).

## 6. B1 windows after the first slice: the roll step and the stage transitions

* `rules/combat/roll.rs` (`RollStep`): the "Roll Dice" step of a round (string `CombatWindow::roll_round`, now a
  `dual_window!` around `roll_round_string`): presentation, both sides' dice from `Sx::die` recorded in
  `ledger.dice` (Munitions Reserves' pre-roll reroll included), free/forced hits (tag 2 = the hulls 0.0.1 binds,
  tag 1 = Hylarim's two extra hits on 9-10), `combat_round_hits`, `combat_round_dice` and the queue.
  Reroll questions are stages of the same window (string `reroll_window` of the fleet kind, `heart_ixth`,
  `wrath_of_kenara`, `war_funding_step`): per side, after its dice are staged (a `RerollSet`, mirrored to
  `transient.combat.reroll_staging` while a question is open so the adapter can fill `Choice.details`), Munitions
  Reserves after the roll (offer, then "choose which dice"), the Crown of Thalnos relic and law (one question per
  die, `Ask::RerollDie`, the picks drawn again at the end of the walk, +1 for the relic, the casualties of entries
  whose rerolled dice missed), the Heart of Ixth (one question per holder), Wrath of Kenara (purchase); after both
  sides, War Funding per holder. Compared: queue, damaged-before-round, dice (hit number, faces, rerolls
  included), stream positions, every question (ids, labels, payload, context, details), events, resulting state.
  Bails (`CombatWindow::roll_unported`, the string result stands, windows inside run bare): a readied The
  Thundarian / Yssaril agent (roll-step reaction: the ability table is empty in core), the Keleres agent granting
  commodities as trade goods to a side that may pay Munitions or Wrath, neutral sides, test faces or
  `rng.seed() != state.rng_seed`, a roll that ends the fight (a reroll casualty emptied a fleet), a hit a module
  produces after the roll, reaction activity inside a casualty's announcement. Golden coverage: the unit tests
  `the_crowns_of_thalnos_reroll_questions_run_in_the_core_roll_window`, `the_heart_of_ixth_runs_in_the_core_roll_window`,
  `war_funding_runs_in_the_core_roll_window`, `wrath_of_kenara_runs_in_the_core_roll_window`,
  `munitions_after_the_roll_runs_in_the_core_roll_window` (16 seeds each, whole fights on `driver=dual`, seeded
  random and first-option deciders), mutation-checked.
  Known string defect copied on purpose: `heart_ixth` splits the option id `hfix|e:d:sign` on `|` and wants four
  parts, so every pick fails to parse: no die is shifted and the card is never exhausted. The window asks the
  same questions and ignores the answers; fixing it is a RULES_VERSION change.
* `rules/combat/flow.rs` (`Flow`): one transition of `settle_stages` (`announcing_step`, `retreating_step`,
  `hit_queue_head`, wrapped by `CombatWindow::flow_dual`). The string answers the rule facts (over, can retreat,
  destinations, ships left, sustain candidates); the core owns the control flow, the events
  (RETREAT_STEP_STARTED, SPACE_COMBAT_ROUND_ENDED, HITS_TO_ASSIGN) and the window fields it changes
  (hit phase, resume flag, retreats announced, once-per-round announcement). Bails: cancellations granted, a
  single casualty candidate, neutral sides, Duranium Armor repairs, a lone retreat destination (the move is
  `retreat_to`, string), Devotion, staged destructions, reaction windows in an announcement.
* `rules/combat/retreat.rs` (`Retreat`): the move of a retreat (string `combat::retreat_to`, now a `dual_window!`
  around `retreat_to_string`; the value compared is the number of units stranded). Alastor participants forgotten,
  Bastion's retreat note, movers (ships with a move value), carried fighters and ground forces within capacity, the
  stranded lost (`remove_ship`, a staged `retreat_capacity` destruction with staging), the command token and its staged
  `COMMAND_TOKEN_PLACED`. No new `Obs` variants: `ParticipantsCleared` and `ShipLostTaken` are the direct hooks
  `nekro_units_alastor_clear` and `bastion_take_ship_lost` (as `assign.rs` already calls them) and the `ShipRetreated`
  observer has no ship list, so the window calls `bastion_note_retreat` directly. Bails (`dual_window::retreat_unported`,
  one test each): a unit that flips form or fights from its planet (Eidolon in the fleet or where it lands, Voltron), a
  Floating Factory, a Last Bastion seat (`bastion_note_retreat` is not ported in core-factions: its list is sorted by the
  unit type TEXT), neutral players. Tests: `a_retreat_runs_in_the_core_window` (sustained and galvanized pieces,
  stranding, with and without staging), mutation-checked.
* `rules/combat/machine.rs` (`Machine`, package B1g): the composition. `CombatWindow::settle` (string
  `settle_stages` + `announce_end`) is now compared as ONE core Window on `driver=dual`
  (`CombatWindow::settle_machine_dual`, label `combat::settle`, adapter `dual_window::settle_machine`). The machine owns
  the stage loop: round opening (`BeginRound`), round-1 barrage (`BarrageStep`), announce/retreat/queue transitions
  (`Flow`, facts now read from the core state: fight over, retreat destinations, ships left, sustain candidates),
  the roll step with its reroll questions (`RollStep`, its questions are the machine's questions), the move of a
  retreat (`Retreat`), the end (`EndAnnounce`). It stops at a pending decision (announce a retreat, choose a
  destination, sustain, assign), at the barrage's scoring pause and at the end. The string hands it the stage and the
  window fields (`Fields`) and gates that cannot change inside one settle (`Env`: interned texts, the neighbours a
  retreat may enter, `may_retreat`, Duranium/Devotion flags); the compared value is the stage, outcome, fields,
  the dice of the settle and the stream positions, besides events, state and questions. Windows nested in the string
  loop run bare under the outer window (`run_dual_window` nests). Where the machine is known not to carry the
  position (`settle_args` gates) the string loop runs with its inner windows compared one by one as before, so no
  coverage is lost; unit tests of the inner windows switch the composed comparison off (`dual_window::with_composed`,
  default off under `cfg(test)`, on in the binaries).
  Bails (string stands; test `the_composed_combat_stays_string_where_the_core_does_not_carry_the_position`, plus the
  lockstep test `the_composed_combat_machine_runs_whole_fights_in_lockstep`): Naaz / Letnev / diplomacy round
  openings; start-of-combat hits (Assault Cannon, Mentak Ambush, Dimensional Splicer, Impulse Core) and the factions
  of the module hooks; barrage with reactions (core ability table empty) and the barrage cases of `barrage_unported`;
  The Thundarian / Keleres agent / test dice (roll gates); a roll that ends the fight; neutral sides; hit
  cancellations; Duranium Armor repairs; Devotion's hits after a round; destructions
  waiting for their announcement (also after a retreat); a retreat of a flipping unit (Eidolon), Voltron, a Floating
  Factory, a Last Bastion seat, Dark Energy Tap, a Ralnel flagship arriving; the announcement of the end when a
  survivor flips back, a blockaded dock dies or faction marks change; reaction windows inside any emission.
* The pending decisions (package B1g, second slice): `Machine::awaiting(env, fields)` is the machine built for the answer
  to the decision a settle stopped at (`Stop::Pending`). It puts the question the string `pending_choice` puts and applies
  the answer the string `resolve` applies, nothing more (the settle that follows is a machine of its own).
  `CombatWindow::resolve` is `resolve_answer` (dual, label `combat::answer`, adapter `dual_window::answer_machine`) +
  `settle`; the string body is `resolve_answer_string`. The recording is made by hand (`note_ask(choice, answer)`), the
  way `invasion::answer_custodians` does, so the harness compares the pending question and then the commander's.
  New core vocabulary (`choice/combat.rs`): `Ask::AnnounceRetreat {system, forced, ships}` (Rout: no "stay"),
  `Ask::RetreatTo {system, fleet}`, `Opt::Stay/Announce/RetreatTo {system, there}`, `Subtype::AnnounceRetreat/RetreatTo`,
  and `Ask::CancelHit.window: Option<u16>` for the sustain question OF THE WINDOW (space sustainers named by their index
  `sustain|3` instead of `sustain|space:3`, previews). The assign question reuses `Ask::AssignHit`, its options carry the
  index into the owner's whole ship list (`casualty_candidates`), not into the filtered list. The renderer
  (`render/combat.rs`) now attaches the `ShipsInSystem` previews (`Wording.preview` for the generic decline "take the hit");
  the `sustainers` detail is the adapter's `complete` closure (as for the offer). Previews are `serde(skip)`, so the whole
  `Choice` comparison cannot see them: `settle_run` compares them question by question in the answering mode
  (`combat::answer.previews`). The sustain answer is `apply_sustain` + the commander's offer (`SustainCommander` as a
  sub-window, its question is the machine's) + `sustain_offer::emit_used` + `cancel_per_sustain`; the assign answer is
  `remove_ship` + `announce_loss`; the retreat answer is `retreat_fleet`; the announce answer emits RETREAT_DECLARED and
  moves on (defender silences attacker, pending retreats).
  The single casualty candidate is no longer a bail: `Machine::lone_casualty` loses the ship without a question.
  Bails of the answer (string stands, counted under `combat::answer`): a hit the producer assigns (Devotion), a retreat the
  core does not move (`retreat_unported`), a sustain below `RULES_FACTION_BUGFIXES`, a commander held another way than as
  an unlocked leader, neutral sides, reaction activity (`Recording.nested`). Tests:
  `the_pending_decisions_of_a_combat_run_in_the_core_machine` (hub map, 24 seeded fights, Rout on a third of them),
  `a_sustain_in_a_game_below_the_faction_bugfixes_is_answered_in_the_string`,
  `a_retreat_of_a_fleet_with_a_floating_factory_is_answered_in_the_string`,
  `a_hit_the_producer_assigns_is_answered_in_the_string`; mutation-checked (pending retreats, hits owed, Rout, the used
  event, the defender silencing the attacker, every preview).
  Still string: the settle gates (`settle_args`: round openings, barrage, roll, retreat moves), see ENGINE_KNOWN_ISSUES.md.
