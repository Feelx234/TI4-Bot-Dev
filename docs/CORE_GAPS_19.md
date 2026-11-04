# Core gaps 19 (branch `core-gaps-19`, cut from `core-integration-19` fc910308d; not pushed)

Three gaps found by the wrapper lane (`core-d28d-wrapper`, `docs/D28D_RESUME.md` rounds 3/4). Reproduced on a detached worktree of `a726b4277` (not merged here).

## 1. Dacxive Animators (done)

`driver/windows/invasion/fight.rs::round_unported` bailed `Unported(Dacxive)` when a side owned the technology. The composed invasion fight now plays it natively, as the string `faction_techs::dacxive_animators` and the committed `rules::invasion::fight::CommittedFight` already did: in `finish_ground_round`, before the temporary commitments return, a winner that still has forces on the planet and holds the technology places the infantry form it builds (`fx_strategy::placed_unit`, the roster of the table facts) when the reinforcements hold one. A missing roster is the existing counted bail `AssimilateRoster`. `Unported::Dacxive` and its literal in the legacy `invasion_live.rs` are removed (no remaining producer).

Test: `ti4-engine-core-tests/tests/invasion_composer.rs::dacxive_animators_places_an_infantry_for_the_winner_of_the_composed_fight` (12 seeds, invader and defender holders, same dice with and without the technology: +1 infantry for the winner, nothing for a loser). Fails on the old code with `Err(Unknown(Unported(Dacxive)))`.

## 2. `aftermath: MissingFact` (found: Fire Team; plus two other stops of the same campaign, all fixed)

On `a726b4277` the whole campaign of `scored_games_stay...` (3 rotations x 17 seeds) was played with a backtrace on every producer of `Unknown::MissingFact` (`CondCx::unknown`, `Eff::unknown`, `Sx::unknown`): none fired. `faction_mech` is not the producer (the live Env lends the roster; in the policy game every faction has one mech printing). The stops found were:
* seed 7780 rotation 0: `Unported(Dacxive)` (gap 1, fixed);
* seed 7793 rotation 0: `UnportedAftermath(CombatUnported)`. Cause (a `CQ` trace on every `?` of `Machine::run`, then on the three `return None` sites of `open_round`): `every_roller_is_named` false because `combat_texts_in` named 12 unit types and `MAX_TYPES` is 10, which names none. The fight is Sol-like (Carrier, Cruiser, Destroyer, Dreadnought II, Super-Dreadnought, L1Z1X flagship) and the carriers hold infantry in the SPACE area: the infantry (and the second form of each kind, a twin) were named although they roll no die in a space combat. Fix (both copies, `ti4-engine-next/src/ties/combat_live.rs` and the legacy `ti4-engine/src/combat_live.rs`): units of the space area are named only when `fights_as_a_ship` (the planet units already were).
Test: `combat_live::tests::the_texts_of_a_fight_do_not_name_the_ground_forces_a_ship_carries` (legacy crate; fails on the old filter). The test `the_texts_of_a_fight_name_ten_kinds...` listed infantry/mech/pds/dock among the ten kinds; it now reaches the cap with ships and unit upgrades (the cap itself is asserted exactly as before).
* **MissingFact itself**: seed 7850 rotation 1. Producer (a `UC::set` backtrace on the tape cell): `ti4_engine_core_factions::cards::fire_team`, `host.table.facts.reroll` is `None` (no adapter ever lends `AdapterFacts::reroll`, so every Fire Team played in a ground round stopped the step). `faction_mech` was NOT the producer. Fix: `fire_team` reads the invasion's own texts (`table.codes.invasion.ground` and `names`) when no adapter facts are lent. Test: `ti4-engine-core-factions` `cards::tests::fire_team_asks_about_the_dice_of_the_ground_round_with_the_invasions_own_texts`. Note: the product Env never fills `table.codes.invasion` (only the legacy lockstep tests do, `invasion_live::fill_codes`), so in the product all those texts are `Code(0)`; ground dice questions are therefore labelled by code 0. Filling them in `set_table_codes` is a wrapper/product task (it changes the minted text table, so a fingerprint question).
* After the three fixes the whole `scored_games_stay...` campaign (3 rotations x 17 seeds = 51 games) plays without a core stop on the `a726b4277` tree (checked with a `dbg` test calling `scored_game` for every job; the 10 minute test itself was not run as such).
Remaining risk: a fight with more than ten ship kinds (with upgrades) still names none (`MAX_TYPES`); see section 5.

## 3. Single-way payment note (core side done)

`AbilityTable::auto_paid: Option<Arc<Mutex<Vec<AutoPaid>>>>` (kernel/ability.rs, next to where the wrapper branch adds `skipped`; the two additions are adjacent fields, expect a trivial textual join). When a log is installed, `PayStep::step` (the `[only]` branch) and `pay_in` (the lone option) push `AutoPaid { choice, taken, at }`: the question as it would have been asked, the option taken, and the position it would have been asked in. Nothing is built without a log, no behaviour or fingerprint changes. The log of a step the driver replays holds the same entry again; drain it from the committed step only (or dedupe on `(choice, at)`).

Wrapper side (integration lane), in `game/asker.rs` or after the step: install `table.auto_paid = Some(arc.clone())` where the wrapper installs `skipped`; after a committed step drain the vec and for each entry render `entry.choice` at `entry.at` (as for a question) and call `Game::table.auto_resolve(&rendered, "it was the only way left to pay")`; do not journal it. Replaces the `rendered.options.len() == 1 && subtype == "pay_resources"` check of the wrapper's asker (the core no longer asks that question, so it never reaches the asker).

Tests (`rules/production/pay.rs`): `a_single_way_payment_is_logged_with_its_question_and_the_position`, `a_payment_with_a_choice_is_asked_and_not_logged`.

## 4. Extra gap from lane D: `TurnGap(StrategyAbility)` in `smart_bots` (done)

`ti4-server` `smart_bots::slow_smart_bots_clearly_outplay_random_bots` stopped at the Imperial primary: `imperial_facts` was `None` because a secret objective a leak had moved into the public area (`Secret(EstablishHegemony)`) is among the scoreable revealed objectives and `ImperialFacts` only held public ones (`Began::Unsupported`). Now `ImperialFacts.scoreable`, `Report::Scored`, `Finished.scored`, `ImperialPrimary` (and `HeroWindows`' `scored`) carry `ObjectiveRef`; a new option `strategy::Opt::LeakedSecret(SecretObjective)` (rendered with the secret's id and label, parsed back from the id; the string's answer is the objective alias too); the secret scores through `fx::score_public(ObjectiveRef::Secret)` with no price. Legacy lockstep adapters (`strategy_cards_core.rs`, `hero_dual.rs`) follow. Test: `strategy_primaries::imperial_offers_and_scores_a_secret_a_leak_moved_into_the_public_area`. With gaps 1 to 4 all 5 `smart_bots` tests pass on `a726b4277` (381 s, dev profile, two threads). `Opt` is a core type with no serde: wire TS unchanged.

## 5. Remaining

* More than `MAX_TYPES` ship kinds in one fight names none (a counted `CombatUnported`); a real fix names only the subsets that can share a combat value.
* Second `MissingFact` producers (`pay_in` without a `pay_kernel`, `produce_in` without production facts, the action-card `unsupported()` site) are not exercised by any of the 51 policy games; no change.
