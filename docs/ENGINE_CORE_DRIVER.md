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
  cancellations; a single casualty candidate; Duranium Armor repairs; Devotion's hits after a round; destructions
  waiting for their announcement (also after a retreat); a retreat of a flipping unit (Eidolon), Voltron, a Floating
  Factory, a Last Bastion seat, Dark Energy Tap, a Ralnel flagship arriving; the announcement of the end when a
  survivor flips back, a blockaded dock dies or faction marks change; reaction windows inside any emission.
* Not done: the PENDING questions themselves (announce a retreat, retreat to which system, sustain, assign a hit)
  and their answers are still the string's `pending_choice` / `resolve`; the machine only stops where they are asked
  (`Stop::Pending`). Core `Ask` variants and renderers for them (with the previews and the `sustainers` detail the string
  question carries) are the next slice.
