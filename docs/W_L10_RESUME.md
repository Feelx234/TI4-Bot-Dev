# W lane 10 resume note, branch `core-w-l10` (from `core-integration-8`; `core-integration-8` re-merged at `0271ca64`, `core-eod1-a` is part of it)

Task: close the remaining counted gates that are not heroes (lane 9 owns Empyrean/Yin/Mahact/Warfare-Imperial). Pure refactor: `RULES_VERSION`, serde and fixtures are unchanged.
Lines for `docs/ENGINE_KNOWN_ISSUES.md` are in the section "W lane 10" at its end (rows W10-design-01 to W10-design-11).

## Ported (core half + Ask/Opt/Subtype + byte-exact renderer + `dual_window!`/`dual::write` + lockstep test, each failing on the pre-W10 code)

| gate | where |
|---|---|
| hits cancelled by a card earlier in the round (machine and flow) | `rules/combat/flow.rs`, `machine.rs`, `dual_window::FLOW_STEP` (writes Players), `combat::flow_dual` |
| a roll that ends the fight inside the roll step | `rules/combat/roll.rs` (`ended`), `machine.rs` (`finish_roll`), `combat::roll_round` |
| destructions staged by a retreat (stranded ships) announced by the machine | `machine.rs` (`announce_staged`, `Env::retreat_event`) |
| Jol-Nar commander reroll in the anti-fighter barrage | `rules/combat/barrage.rs` (new stages, `RerollSource::Commander`), `Env::{jolnar, shown_codes, barrage_extra_shown}`, `SettleArgs::jolnar` |
| Waylay played in the barrage | `barrage.rs` (`Absorb` child, `Absorb::in_space_combat`), `dual_window::waylay_check` |
| Munitions Reserves paid at the opening below rules version 8 | `rules/combat/begin_round.rs` (`Ask::MunitionsStart { have }`, `Facts::munitions_start`), render `details` |
| galvanized ground force that falls | `kernel/fx_ground.rs`, `rules/agenda/table_effects.rs`, `rules/hero_windows.rs` (the `\|g` field = `Word::Galvanized`, codec test already in `core_sync::keys::tests`) |
| command tokens in Mahact fleet pools (Warfare, Leadership, TE Warfare, Fabrication, Leadership Rider) | `kernel/fx.rs` (`tokens_held_by_mahact`), footprints with the Marks slice |
| Hacan breakthrough / Prophecy of Ixth / swapped planet value after a production by an ability | `leaders_core::announce_uses` |
| X-89 Bacterial Weapon (ground roll, its tail, the bombardment's planet exhaust) | `rules/invasion/{roll,tail,bombard}.rs`, `dual_window` |
| Nekro Malleon used by a seat not holding it readied (Ssruu copy): an empty window | `leaders_core::nekro_agent` |
| Riftwalker Meian on a map that lags the recorded edits | `leaders_core::ghost_hero` (the guard is gone) |

## Added after the coordinator's "no bail" decision (2026-10-31)

* The machine answers a SUSTAIN DAMAGE below rules version 17 (no Farran offer, one cancelled hit even with Non-Euclidean Shielding): `Machine::cancels`, `Sustaining::commander: Option<_>`; the gate "a sustain answer in a game below the faction bug fixes" is gone (test `a_sustain_in_a_game_below_the_faction_bugfixes_is_answered_in_the_core`).
* Production Biomes, Chaos Mapping and the technology exhaust of a Nekro seat that carries the text on a Valefar Assimilator: `fx_strategy::{text_carrier, exhaust_technology_text}` (the carrier is the card exhausted); tests `production_biomes_carried_by_a_valefar_assimilator_runs_in_lockstep_with_the_core`, `a_nekro_seat_that_does_not_own_the_card_may_carry_its_text_..._and_the_core_exhausts_the_carrier`, `the_text_of_a_technology_is_carried_by_the_card_or_by_the_assimilator_with_its_token`.
* Bail-as-failure in the lockstep fights: `combat::tests::no_bails` (every bail or gate label of the whole fights of `roll_step_fights` fails the test) for the Jol-Nar commander, Munitions at the opening, the roll that ends the fight and the cancelled-hit pool; the Waylay fight and the X-89 invasion fights assert it over their whole report.
* Found while trying: the string's C4R (`nekroc4r`) PD and repair and the Cabal Vortex require the card itself (`technology_ready` / `seat.technologies.contains`), so a Valefar-carried C4R or Vortex is **unreachable** in the product (the adapters' guard `carried by something other than the player's own card` can never fire); not ported, not needed.

## Remaining gates, with the precise reason (not closed by this lane)

Dropped by decision (stay dropped, not counted as open): Last Bastion, Crimson Rebellion, Keleres, Deepwrought, Firmament, Obsidian, Ral Nel, deck reserve / turn redo, structured diplomacy (`gs.diplomacy.enabled`: the "diplomacy opening" gate of `round_opening_why` and the diplomacy-promise events of a leader's use; `unsupported::Disabled::Diplomacy` panics in the core by design). Thunder's Edge (Heart of Ixth, Saar Deorbit Barrage, Arborec Psychospore, the Yin breakthrough event, TE Construction production nested in the card).

Open, product-reachable (a game can reach them), each with the missing core piece:

1. **Neutral sides** (`crate::neutral_units::is_neutral` gates in about 30 windows: barrage, roll, flow, round opening, retreat, tail, end announcement, cannon, Assault Cannon against a neutral side): the neutral force is no seated player (no seat, no hand, no decider); the core would need a seat-less owner for units and the string's automatic answers (`neutral_answer`: sustain, casualty, no retreat, rules 5 to 7). Reached after Pirate Fleet / Pirate Contract / Mercenary Contract. Largest open item of the combat area.
2. **Action cards played inside another area's window** (`reactions_core`, "unported ability ... (card X)"): `Fx::nested` is false for Ghost Ship, Mercenary Contract, Pirate Contract/Fleet, Jam, Upgrade, Refit Troops, Decoy, Ghost Squad, Harness Energy, Brilliance, Impersonation, Plagiarize, Focused Research, Divert Funding, Lie in Wait, Bribery, Infiltrate, Crash Landing, Exploration Probe, Strategize/Overrule, Fire Team's and Scramble Frequency's rerolls (`Reroll`, `Scramble`), Courageous To The End and Experimental Battlestation, and `plays_in` is false for Reflective Shielding. Their facts (forms, roster, map, traders, hit texts, reroll texts) come from the adapter's own play window, not from `PlayFacts`; each host window would need to hand them in and, for the dice-rolling ones, be lent the streams (`with_seeds`). Scramble Frequency also needs the staged dice mirrored at `UNIT_ABILITY_ROLLED` and re-read after it in every host (barrage: staged now, not re-read; space cannon; bombardment); Reflective Shielding needs the sustain step to drain the hits it stages.
3. **Temporal Command Suite and Mercer (Nomad)**: no core site emits `AGENT_EXHAUSTED` (`Unknown::NomadAgentExhaustion` in the Tellurian, L1Z1X, Yin, Argent, Ssruu abilities and the Keleres agent), and TCS runs a whole trade window as a child.
4. **Umbat copied through Ssruu**: the copy is atomic in the string (a failed decision restores the state and the log); the core `Umbat` window, driven for the copier, ended at once with no question in the first try (the copier is not the Muaat seat the window assumes); needs the window to take the carrier and a failure path.
5. **The Inferno on a Nekro flagship that carries Muaat's text (Z token)** and **Fires of the Gashlai whose war sun technology is carried by a Valefar Assimilator**: the core has no query for "this seat's flagship has that faction's text" outside `FlagshipText::{Letnev, Hacan}`, nor the carrier of a technology the Fires window grants.
6. **Titans' Hecatoncheires as an alternative to a PDS** in Construction (`StructureFacts` holds one unit per structure).
7. **Legacy rules versions**: the standalone sustain offer below `RULES_FACTION_EFFECTS` (`dual_window::sustain_offer_unported`), `strategy_cards_core` below `RULES_TECH_PAYMENT_CHOICE`. Old saves only.
8. **Items of the coordinator's list this lane did not touch** (no owner assigned inside it): the invasion tail (5b) Research Team / Viper EX-23 offers and the combats a faction rule forces (Titans' Coalescence, Slumberstate Computing: `commit_tail_unported`), Integrated Economy / Sling Relay at a table with a Keleres or Deepwrought seat (dropped factions, so these two are only the dropped part), the first claim of a Fracture planet (TE), the exploration reward composition (D1).
9. Duranium Armor and Devotion are not gates any more (ported by earlier lanes); the "staged destructions pending at a transition" guard of the machine stays for a position handed over from a nested card play (the card plays of item 2 stage them).

## Checks (cheap only; private target dir `/root/deploytmp/core-w-l10`, `CARGO_BUILD_JOBS=2`)

See the report; run chains are `cargo test -p ti4-engine --lib` (normal and with `TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1`), the touched `ti4-engine-core-tests` files, `tools/engine_core/check_layers.py core`, `tools/engine_core/lint_core.sh`. Mutation checks: scratch worktree `core-w-l10-mut`, `/root/deploytmp/core-w-l10/mut/{runmut.py,spec1.py}` (`MUT_ROOT=... MUT_TARGET=... python3 runmut.py spec1.py`).
Not run: golden / wide dual sweeps, benches, soak, ratchet, workspace clippy, census (the end-of-D1 heavy run is in progress elsewhere).
