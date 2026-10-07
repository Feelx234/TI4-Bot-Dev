# BF Nomad part B: agents, Quantum Manipulator, The Cavalry

Branch `wp/base-factions`, uncommitted. Files: `factions/nomad_agents.rs` (all tests inline),
`combat.rs`, `invasion.rs`, `leaders.rs`, `factions/hooks_combat.rs`, `factions/hooks_ground.rs`
(docs only). Part A (`nomad.rs`) is a separate record.

## Items

| Kind | Id | Status | Route | Tests |
|---|---|---|---|---|
| leader | `nomadagentartuno` | done | `TRADE_GOODS_GAINED` (After); pile = public mark `nomad:artuno:<player>`; paid by `leaders::ready_all` / `leaders::ready` | `artuno_copies_a_gain_from_the_supply_onto_its_card`, `artuno_declined_gain_changes_nothing`, `artuno_ignores_what_is_not_a_gain_from_the_supply`, `artuno_pays_its_pile_out_when_it_readies`, `artuno_through_the_gain_helper_the_engine_uses` |
| leader | `nomadagentmercer` | done as an ability; **not reachable in a real game until `game.rs` emits `TACTICAL_ACTION_ENDED` for it (hook request 1)** | `TACTICAL_ACTION_ENDED` (After) | `mercer_moves_two_ground_forces_to_a_planet_in_the_active_system`, `mercer_takes_from_the_space_area_and_never_moves_structures`, `mercer_asks_where_each_unit_goes_when_there_are_several_planets`, `mercer_declined_or_unanswered_changes_nothing`, `mercer_is_not_offered_without_a_planet_a_ground_force_or_a_ready_card`, `mercer_refuses_an_unoffered_answer_before_changing_anything` |
| leader | `nomadagentthundarian` | done (space and ground) | new `SPACE_COMBAT_ROLL_STEP_ENDED` (`combat::CombatWindow::roll_round`) and `GROUND_COMBAT_ROLL_STEP_ENDED` (`invasion::resolve_ground_round`); replay through `combat::request_roll_replay` | `the_thundarian_discards_the_roll_and_plays_the_step_again`, `the_thundarian_is_used_once_and_its_replay_cannot_be_replayed`, `the_thundarian_is_offered_in_a_combat_the_nomad_is_not_part_of`, `the_thundarian_needs_a_ready_card` (space, through `CombatWindow` with the armed resolver); `invasion::tests::the_thundarian_replays_the_ground_roll_without_assigning_its_hits` (ground, through `InvasionWindow`) |
| unit | `nomad_mech` | done | `CombatHooks::grants_sustain` (new) read by `combat::sustains_in_space`, so both the combat window's sustain stage and `offer_sustain` see it | `the_mech_cancels_a_hit_against_the_nomads_ships_with_its_sustain_damage`, `without_the_mech_the_same_hit_destroys_the_cruiser`, `the_mech_can_decline_and_a_damaged_mech_cancels_nothing`, `a_mech_on_a_planet_or_a_mech_that_is_not_the_nomads_kind_cancels_no_hit`, `the_mech_is_not_a_combat_ship_and_space_cannon_hits_are_not_part_of_a_combat` |
| promissory | `cavalry` | done | `SPACE_COMBAT_STARTED` (play), `SPACE_COMBAT_ENDED` (return); `CombatHooks::grants_sustain` + `CombatHooks::borrowed_stats` (new) read by `fleet_groups`, `roll_barrage_side`, `sustains_in_space` | `the_cavalry_lends_a_ship_the_flagships_combat_value_and_barrage_then_returns`, `the_cavalry_follows_the_nomads_flagship_upgrade`, `the_cavalry_ship_can_sustain_damage_once`, `the_cavalry_asks_which_ship_when_there_is_a_choice`, `the_cavalry_is_played_only_against_a_player_other_than_the_nomad`, `the_cavalry_returns_at_the_end_of_the_combat_and_only_then` |
| neutrality | all | done | | `games_without_the_nomad_see_nothing_of_the_agents` (no ask, state byte-equal) |

Claims added: `LEADERS = [nomadagentartuno, nomadagentmercer, nomadagentthundarian]`,
`UNITS = [nomad_mech]`, `PROMISSORY = [cavalry]`. `nomad.rs` reports its own claims separately.

## Card text (corpus, latest printing)

* Artuno: "When you gain trade goods from the supply: You may exhaust this card to place an equal
  number of trade goods on this card. When this card readies, gain the trade goods on this card."
* Mercer: "At the end of a player's turn: You may exhaust this card to allow that player to remove
  up to 2 of their ground forces from the game board and place them on planets they control in the
  active system." (corpus note: the window should be the end of the tactical action.)
* The Thundarian: "After the \"Roll Dice\" step of combat: You may exhaust this card. If you do,
  hits are not assigned to either player's units. Return to the start of this combat round's
  \"Roll Dice\" step."
* Quantum Manipulator: "While this unit is in a space area during combat, you may use its SUSTAIN
  DAMAGE ability to cancel a hit that is produced against your ships in this system."
* The Cavalry: "At the start of a space combat against a player other than the Nomad: During this
  combat, treat 1 of your non-fighter ships as if it has the SUSTAIN DAMAGE ability, combat value,
  and ANTI-FIGHTER BARRAGE value of the Nomad's flagship. Return this card to the Nomad player at
  the end of the combat."

## Rule decisions

1. **Artuno copies, it does not redirect.** The gain still happens; the card receives an equal
   number. The window is read After the gain (the engine announces gains after they are made).
   Sources that are not "from the supply" are ignored: `transaction`, `pillage`, `extreme_duress`,
   `trade_agreement` (goods another player gave or took) and `artuno` (its own payout). The pile is
   the public mark `nomad:artuno:<player>`; it is paid out (as a staged `TRADE_GOODS_GAINED`,
   source `artuno`) by every route that readies the card (`ready_all` in the status phase, `ready`
   for single-card effects). **Coverage limit:** only gain sites that stage or emit
   `TRADE_GOODS_GAINED` reach it (`supply::gain_trade_goods_*`, `note_trade_goods_gained`); sites
   that do `seat.trade_goods += n` silently are invisible to it (existing gap from BF-00b-economy).
2. **Mercer** is hung on `TACTICAL_ACTION_ENDED`, as the Sardakk agent and the corpus note do. The
   Nomad decides to use it; the active player then answers up to two "which ground force" questions
   (decline stops) and, per unit, "which planet" when more than one is eligible (one is not a
   decision). All questions are asked before the first mutation. The card is exhausted only if at
   least one unit moves, so a player who removes nothing does not cost the Nomad the card. Moved
   units are placed fresh (undamaged: removed from the board, then placed). Ground forces may be
   taken from any planet or space area on the board. **Not moved:** structures (the Titans' PDS is a
   ground force and a structure); recorded as a deliberate exclusion.
3. **The Thundarian** is offered to its owner in any combat, including one the Nomad is not part
   of: the text names no player. The replay discards the dice without producing, assigning or
   announcing a hit and rolls the step again from the game's own dice stream (`Dice`/`GameRng`),
   under the same round number. Costs already paid in the discarded roll (a reroll card played in
   it, Munitions Reserves, War Funding returned) stay paid; the step's rerolls are available again.
   One replay per exhaust; the request is a private mark (`private:#combat:replay_roll`) cleared
   before each emit so it can never leak between rounds.
4. **Quantum Manipulator**: the mech is offered as a sustain choice for any hit produced against
   the owner's ships in a space combat (dice, "start of combat"/"after a round" module hits, Waylay
   barrage). It is never a casualty (it is not a ship). SPACE CANNON OFFENSE hits are not "during
   combat", so the mech does not apply to them (`CombatUnit.context == "space_cannon"`). A mech on
   a planet is not "in a space area". A damaged mech cannot sustain again.
5. **The Cavalry**: "the Nomad's flagship" is the Nomad's own current flagship (Memoria II once the
   Nomad owns the upgrade). Only the combat **value** is lent (the ship keeps its own dice); the
   barrage is lent whole, value and dice (x3), since a ship with no barrage has no dice of its own
   to fire it with. The note stays faceup with the holder during the combat and returns at
   `SPACE_COMBAT_ENDED`. Ships carry no identity, so the borrower is "the ship of the chosen type
   that has not yet used the borrowed SUSTAIN DAMAGE" (a type that cannot sustain by itself shows
   its user as the damaged one); for several ships of one type the owner's casualty choice is
   therefore the player-optimal one. A neutral opponent is not "a player", so it is not allowed.
6. `sustains_in_space` no longer requires `is_ship()` first: ships keep exactly the old test;
   `grants_sustain` is the only other way in, and every module's `may_sustain` still applies.

## Hook requests (files not owned here)

1. **`game.rs` (`tro_window`, ~line 3703):** `TACTICAL_ACTION_ENDED` is emitted only when some seat
   holds a readied `sardakkagent`. Extend the condition with a readied `nomadagentmercer`, or Mercer
   is never offered in a real game. (Tested here by emitting the event through the armed resolver.)
2. Ssruu / copy-an-agent routes (`borrowed_commanders*`, `hooks_cards::borrowable_agents`) do not
   know the three agents; not attempted.
3. `reactions::EMITTED_EVENTS`: the two new events are for faction timing abilities only (not card
   windows), like `GROUND_COMMITMENT_FINISHED`; no entry added.

## Decision sites to register (`tests/decision_delivery_inventory.rs`)

`nomad_agents.rs`, function `ask`: 1 x `Choice::new` and 1 x `ask_seeing` (`AskObserved`), delivered
here (`ObservedHere`). Subtypes asked through it: `mercer_take`, `mercer_place`, `cavalry_ship`.
No other `Choice` is built in the files touched (`combat.rs`, `invasion.rs`, `leaders.rs` add none).
The resolver's own optional-ability question is the existing generic one
(ability ids `leader:<faction>:nomadagentartuno:TRADE_GOODS_GAINED:after`,
`leader:<faction>:nomadagentmercer:TACTICAL_ACTION_ENDED:after`,
`leader:<faction>:nomadagentthundarian:{SPACE,GROUND}_COMBAT_ROLL_STEP_ENDED:after`,
`promissory:<faction>:cavalry:SPACE_COMBAT_STARTED:after`).

## Shared-code changes

* `hooks_combat.rs`: `CombatHooks::{grants_sustain, borrowed_stats}`, `BorrowedStats`, dispatchers.
* `combat.rs`: `sustains_in_space` (+`in_combat`), `effective_from` (`effective_hits_on` split),
  `fleet_groups` and `roll_barrage_side` (borrowed stats), `roll_round` (new
  `SPACE_COMBAT_ROLL_STEP_ENDED` + replay), `request_roll_replay` / `take_roll_replay` /
  `ROLL_REPLAY_MARK`.
* `invasion.rs`: `resolve_ground_round` wraps its "Roll Dice" step in a loop and emits
  `GROUND_COMBAT_ROLL_STEP_ENDED`; the round number is restored on each pass so a replay is rolled
  exactly like the first pass. The synchronous test-only entry points emit nothing, as before.
* `leaders.rs`: `ready_all` / `ready` call `nomad_agents::agent_readied`.
* `hooks_ground.rs`, `hooks_combat.rs`: documentation of the new events and hooks.

## Commands run

Env: `CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_TEST_INCREMENTAL=false CARGO_PROFILE_TEST_CODEGEN_UNITS=16`, `-j1`.

* `cargo test -p ti4-engine --lib -j1 nomad`: 51 passed, 0 failed (includes part A).
* `cargo test -p ti4-engine --lib -j1 factions::nomad_agents`: 27 passed.
* `leaders::tests::exhausting_an_agent_stages_agent_exhausted_only_for_a_watching_nomad`: passed
  (`leaders::exhaust` now stages `AGENT_EXHAUSTED` for a Nomad holding `tcs`; part A's request).
* `cargo test -p ti4-engine -j1 --no-fail-fast -- --test-threads=16` (before the last two small
  edits): lib 2240 passed / 0 failed / 1 ignored; `content_ids_resolve` 1 passed; doc-tests 5 passed;
  `decision_delivery_inventory` 3 passed / 1 failed, the expected registry failure naming only
  `nomad_agents.rs` `ask` (Choice 1, AskObserved 1) and part A's `nomad.rs` sites.
* Ledger: `nomad 9/14 implemented` (gaps are part A's: tcs, flagships, hero, commander).
* clippy (`--all-targets`): the one warning in `nomad_agents.rs` fixed; none in the touched shared hunks.
* rustfmt `--edition 2024`: `nomad_agents.rs`, `hooks_combat.rs`, `invasion.rs` formatted; `combat.rs`
  has pre-existing unformatted code, my hunks hand-formatted.


## Review fixes

1. **Thundarian replay bound.** The replay is still unlimited per text (each needs an exhaust), but
   a depth guard `combat::MAX_ROLL_REPLAYS = 8` per combat round now ends a pathological
   ready/exhaust loop (space: `CombatWindow::roll_replays`; ground: a local counter): past it the
   dice stand as rolled. Rationale: a chain needs one agent exhaust per replay and one re-ready
   (TCS, Ssruu) per further replay, so legal chains are at most a few; 8 is far above that and
   bounds the recursion in the space round.
2. **Swallowed errors.** `SPACE_COMBAT_ROLL_STEP_ENDED` is propagated with `?` (`CombatError::Timing`);
   rollback is the driver's snapshot taken before the first die, as for every other `?` in
   `roll_round`. `GROUND_COMBAT_ROLL_STEP_ENDED` goes through `strict_timing_error` (as
   `GROUND_COMBAT_ROUND_STARTED` does), which `drive`/the isolated helper turn into an error.
   Test: `an_illegal_thundarian_answer_is_refused_without_changing_the_board` (space). No ground
   error-path test: the ground path shares the mechanism with Round Started, covered elsewhere.
3. **Mercer and structures.** Ground forces that are also structures (Titans' PDS) are now movable;
   structures that are not ground forces never were. Test `mercer_moves_a_structure_that_is_also_a_ground_force`.
   The earlier "Not moved: structures" decision (Rule decisions 2) is superseded.
4. **Event-id drift.** The two roll-step events are emitted only if `nomad_agents::watches_roll_step`:
   some seat holds a readied The Thundarian, or a seat holding a readied Ssruu could borrow a seated
   one. Test `the_roll_step_event_is_emitted_only_when_a_thundarian_could_hear_it`.
5. **Cavalry barrage.** The card lends the ANTI-FIGHTER BARRAGE *value*; the borrower keeps its own
   number of dice (a destroyer fires 2). A borrower with no barrage of its own fires the lent value
   with the flagship's dice (documented choice: it has none of its own). Tests:
   `the_cavalry_lends_..._then_returns` (destroyer keeps 2), `a_borrower_with_no_barrage_fires_the_lent_value_with_the_flagships_dice`.
   Rule decision 5's "lent whole" is superseded.

### Scope ledger

| Deferred | Why |
|---|---|
| Mercer window: only the end of a tactical action is offered; the card says "at the end of a player's turn". | The corpus note supports a tactical-only window and the engine emits no end-of-turn event for it; kept, deliberately incomplete (a pure-component/strategic/pass turn never offers Mercer). |

### Review-fix results

Env as above, `-j1`. `cargo test -p ti4-engine --lib -j1 nomad`: 58 passed, 0 failed. Full
`cargo test -p ti4-engine -j1 --no-fail-fast -- --test-threads=12`: lib 2294 passed / 0 failed / 1
ignored; `content_ids_resolve` 1; doc-tests 5; `decision_delivery_inventory` 2 passed / 2 failed
(the unregistered-site registry checks: Nomad `ask` sites and the in-progress Cabal sites, not
caused by these fixes; no new `Choice`/ask site added). rustfmt applied to the files clean at base;
`combat.rs` hunks hand-formatted.
