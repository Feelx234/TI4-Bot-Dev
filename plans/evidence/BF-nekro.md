# BF-nekro (part A)

Part B (flagship The Alastor, mech Mordred, agent, hero, commander unlock, Thunder's Edge technologies
`nekroc4y` / `nekroc4r`) is `factions/nekro_units.rs`. This file: abilities, Valefar Assimilators X/Y,
the technology-rights predicate, the Z token helpers and Antivirus.

## Items

| Kind | Id | Status | Tests (`factions::nekro::tests`) |
|---|---|---|---|
| ability | propagation | done | `the_technology_card_gives_the_nekro_tokens_instead_of_a_technology` (real route: `strategy_cards::primary` on the Technology card), `the_game_announces_a_replaced_research_at_its_next_step` (Game step) |
| ability | technological_singularity | done | `a_destroyed_opposing_unit_lets_the_nekro_gain_a_technology_once_per_combat`, `a_faceup_antivirus_bars_the_singularity_and_returns_on_activation` |
| ability | galactic_threat | done (incl. "You cannot vote on agendas") | `a_correct_prediction_gains_a_technology_of_a_voter_who_voted_that_way`, `the_nekro_cannot_vote` |
| promissory | antivirus | done | `a_faceup_antivirus_bars_the_singularity_and_returns_on_activation` |
| technology | vax, vay | mechanism done; claim by coordinator (`nekro_units::TECHNOLOGIES`), see "Not offered" and Rules question 1 | `a_token_on_a_faction_technology_lends_its_text_without_owning_it`, `a_unit_upgrade_is_gained_but_never_assimilated` |
| breakthrough | nekrobt | **not claimed**: token and helpers exist, no flagship text can be lent (see below) | `the_z_token_is_not_offered_while_no_flagship_text_can_be_lent` |
| neutrality | no Nekro | done | `a_game_without_a_nekro_is_untouched` |

## How it works

* **Propagation.** Every research route (Technology primary and secondary, action cards, agents, the
  waiver routes) ends in `technology::complete_research`. For a Nekro it records one pending
  replacement (`nekro:propagation:<player>` count) and returns without gaining the technology or firing
  anything that triggers on research. `Game::announce_gains` (game.rs, 5 lines) emits one
  `PROPAGATION_RESEARCH` per pending replacement at the next step; the mandatory Nekro ability places
  3 tokens through `strategy_cards::gain_tokens` (the owner picks each pool). Any resource payment a
  route took before the research stays paid (Rules question 3).
* **Technological Singularity.** `*_COMBAT_STARTED` writes `nekro:ts:<system>[|<planet>]` =
  `<opponent>|open`, `*_COMBAT_ENDED` removes it, the first destruction of the opponent's unit in that
  combat (`SHIP_DESTROYED` with `during_space_combat`, `GROUND_FORCE_DESTROYED` with
  `cause = ground_combat`) offers the gain and flips the mark to `used`. Optional. A faceup
  Antivirus held by the opponent bars it.
* **Galactic Threat.** After `AGENDA_REVEALED`, once per round (`nekro:threat_phase:<player>`), the
  Nekro may predict (`action_cards::predicted_outcome`, auto when there is one outcome), stored as
  `nekro:threat:<player>` = `<agenda_seq>|<outcome>`. After `AGENDA_RESOLVED`, a matching outcome (and
  no Deadly Plot discard) offers a gain from the players in `agenda_votes` who voted that outcome.
  Mandatory once predicted. `VoteWindow::new` drops the Nekro from the vote order (Elder Qanoj's
  exception for a seat with it is kept).
* **Gaining another player's technology** (`nekro::take_from`, shared by Singularity and Threat): one
  choice listing `gain|<source>|<tech>` (a technology the Nekro lacks; another faction's faction
  technology is excluded, 90.11), `x|`/`y|<source>|<faction tech>` (Valefar card owned, token free,
  technology not already carrying a token), and `z|` (never generated, see below).
* **Valefar X/Y.** The token is `Player::assimilated_technologies["vax"|"vay"]` (existing model field,
  "not compared"). `nekro::assimilated_card` is live only while the Nekro owns the card and another
  player still owns the technology. `technology::has_technology_text(state, player, tech)` = owns it, or
  assimilated. Assimilated text never enters `Player::technologies`, so it counts for no prerequisite,
  colour, upgrade or objective. Exhaustion goes to the Valefar card
  (`technology_text_ready`, `exhaust_technology_text`). Helpers for part B:
  `nekro::assimilators_with_tokens(state, player)` (the commander's "counts only if its X or Y token is
  on a technology"), `nekro::assimilated_card`.
* **Z token.** `nekro::z_assimilated_faction`, `nekro::z_assimilated_flagship(state, nekro) ->
  Option<unit id>`. Both exist; the second returns `Some` only for flagships in
  `nekro::LENDABLE_FLAGSHIPS`, which is empty (every other flagship's text is delivered by hooks that
  compare the unit type with their own id; no shared hook lets the Nekro flagship stand in). So the Z
  option is never offered and `nekrobt` is not claimed. To claim it later: make each lendable
  flagship's hooks also accept the Nekro flagship when `z_assimilated_flagship` names theirs, then add
  its unit id to `LENDABLE_FLAGSHIPS`.
* **Antivirus.** `promissory::take` no longer puts it faceup on receipt (its `playArea` flag would have
  done so); the holder may place it at `SPACE_COMBAT_STARTED` / `GROUND_COMBAT_STARTED` (any combat,
  per the printed text). It returns through `promissory::spend_support_on_activation`, which every
  activation path already calls (Blood Pact / Dark Pact list, `"antivirus"` added).

## Technology-rights sweep

`technology::has_technology_text` now backs the effect gate of these faction technologies (the local
`has_technology` / `owns_technology` / `technology_ready` helper of each file, or the inline check):
arborec `bio`; argent `ah`; empyrean `as`, `vw` (dropped redundant `is_empyrean`); ghost `ds`, `wg`;
keleres `iihq`, `asn` (dropped `is_keleres`); mahact `gr` (holder = any seat with the text ready, via
`vote.rs`); mentak `mc`, `so`; muaat `mr`; naalu `ng`; naaz `pfa`, `sc`; nomad `tcs`; saar `cm`
(also `technology.rs`); sardakk `vpw`; winnu `lgf`, `htp`; yin `yso`, `ic`; yssaril `tp`, `mi`; cabal
`vtx`; hacan `qdn`, jol-nar `scc`, `ers`, xxcha `nf` (`faction_techs.rs`).

Unit-upgrade faction technologies (`ac2`, `so2`, `lw2`, `swa2`, `pws2`, `sdn2`, `cl2`, `hcf2`, `m2`, `exo2`,
`se2`, `ht2`, `dt2`, `ffac2`): a unit upgrade's text is the stat block of a unit the Valefar card is not,
and the Nekro has no such units, so assimilating one does nothing. The rules-faithful choice taken:
**not offered** as an X/Y target (Rules question 2); gaining one is impossible anyway (another
faction's faction technology). An upgrade the Nekro owns through `apply_unit_upgrades` is unaffected.

### Not offered (checks in files outside my scope; coordinator)

`nekro::assimilation_options` currently offers X/Y on any non-upgrade faction technology of the source.
These gates still read `technologies.contains`, so a token on them would lend nothing:

| Tech | Gate | File |
|---|---|---|
| `l4` (Letnev) | `invasion.rs:4052` | not mine |
| `nes` | `combat.rs:2108` | not mine |
| `pm` | `faction_abilities.rs` (`biomes`, ~338/410) | not mine |
| `it` | `reactions.rs:1036/1085` | not mine |
| `executiveorder` | Keleres-only by design (the vote has the Nekro as speaker, who cannot vote) | keleres.rs |

Either swap those checks to `technology::has_technology_text` / `technology_text_ready` /
`exhaust_technology_text`, or add the ids to an exclusion list in `assimilation_options` (done below
under "Exclusion list").

## Hook / change requests (files not mine)

1. `seating.rs`: the Nekro's `startingTech` is only `dxa`, Nekro cannot research and nobody owns
   `vax`/`vay` to be gained, so **no route gives the Nekro the Valefar cards**. Either deploy with `vax`
   and `vay` or document another route (Rules question 1). Tests grant them directly.
2. `invasion.rs:4052` (`l4`), `combat.rs:2108` (`nes`), `faction_abilities.rs` (`pm`),
   `reactions.rs` (`it`): the sweep above.
3. `tests/decision_delivery_inventory.rs`: register the sites below.
4. `nekro_units.rs` TECHNOLOGIES: `vax`, `vay` (part A reports them complete subject to request 1).
5. Part B: a Nekro commander unlock should call `factions::nekro::assimilators_with_tokens`.

## Decision sites to register

* `nekro.rs`: `take_from` (1 Choice: "gain a technology, or assimilate instead"; decision source
  `FactionAbility("technological_singularity" | "galactic_threat")`, subtype `assimilate`).
* `action_cards::predicted_outcome` (existing, now also used by Galactic Threat).
* Propagation pool choice: existing `strategy_cards::gain_tokens` site.

## Rules questions

1. How does the Nekro get Valefar Assimilator X and Y? Starting technology is Dacxive Animators only,
   the Nekro cannot research, and a gain needs another owner.
2. Unit-upgrade faction technologies: no text to lend; excluded as targets (an alternative would be to
   offer them and lend nothing, which only wastes a token).
3. Propagation after a priced research: the Technology secondary charges 4/6 resources before the
   research; "gain 3 command tokens instead" replaces the research, not the price. The player can
   decline the optional research, so no one is forced to overpay.
4. A token whose technology is no longer owned by anyone else lends nothing; the card may then take
   a new token.
5. Antivirus is offered at the start of any combat, to any holder, as printed (no participation test).
6. Galactic Threat after a Deadly Plot discard pays nothing (same reading as Construction Rider's
   payout).

## Commands (all `-j1`, `CARGO_PROFILE_TEST_DEBUG=0 ..._INCREMENTAL=false ..._CODEGEN_UNITS=16`)

* `cargo test -p ti4-engine --lib -j1 factions::nekro::` : 10 nekro.rs tests pass (plus part B's
  `nekro_units` tests, part B's two failures while it was mid-edit are theirs).
* `cargo test -p ti4-engine -j1 --no-fail-fast -- --test-threads=12`: lib 2508 passed, 1 failed, 1 ignored;
  `content_ids_resolve` 1/1; `decision_delivery_inventory` 3 passed, 1 failed; doctests 5/5.
  * `faction_abilities::tests::the_gap_is_reported_rather_than_implied` fails: `unimplemented(POK)` is now
    empty (Nekro's three abilities were the last unclaimed ones). Not my file; the assertion
    `!missing.is_empty()` should become "every unanswered ability is genuinely unclaimed" or be dropped.
  * `decision_delivery_inventory::every_producer_and_delivery_site_matches_the_reviewed_registry`: unreviewed
    `nekro.rs take_from` (Choice 1, AskObserved 1) = mine, expected; `nekro_units.rs ask` (part B) and
    `production.rs produce_unit_by_ability` (AskObserved 1, not from this package).
* A first run exposed a trap: a `#[cfg(test)] use ...` line near the top of a source file makes the
  decision-registry scanner treat the rest of the file as test code (it hid arborec, ghost, mentak, naalu,
  naaz, winnu and yssaril sites). Removed; the test imports live inside each `mod tests`.
* `cargo clippy -p ti4-engine --all-targets -j1`: nothing in `nekro.rs`, `faction_techs.rs` or my hunks
  (the `promissory.rs:385` and `vote.rs` too-many-lines warnings predate this change).
* rustfmt (edition 2024) applied only to files that were rustfmt-clean before: nekro, technology,
  promissory, arborec, ghost, mentak, naalu, naaz, winnu, yssaril. `game.rs` untouched by rustfmt.

## Exclusion list

`nekro::TEXT_NOT_LENDABLE` = `l4`, `nes`, `pm`, `it`, `executiveorder`: never offered an X/Y token (see
"Not offered"). Remove an id when its gate is swept.

## Ledger

`print_faction_ledger` was not rerun after the final edits (nekro's technologies/units/leaders are part B's
claims and the coordinator adds `vax`, `vay`).

## Integration

Coordinator-integration pass (single agent).

* `nekro.rs` MODULE.hooks now sets `commander_unlocked`, `leader_action`, `use_leader`, `component_actions`, `perform_component` from `nekro_units` (part A set none, nothing to chain). Real-route tests in `nekro_units`: `nekro_agent_is_offered_and_resolves_through_the_leader_route`, `nekro_hero_is_offered_and_resolves_through_the_leader_route` (both via `leaders::component_actions` / `leaders::use_leader`), `nekro_commander_unlocks_through_check_unlocks`, `nekro_error_error_is_offered_and_resolves_through_the_component_route` (`faction_abilities::component_actions` / `perform_component`).
* Valefar X/Y: coordinator ruling, the Nekro begins with both cards. `seating::deploy` inserts `vax`, `vay` for `nekro` only (test `the_nekro_begins_with_both_valefar_assimilators`); `nekro_units::TECHNOLOGIES` = `vax`, `vay`, `nekroc4y`, `nekroc4r`. Hook request 1 and 4 above are done.
* Gates swept to `technology::has_technology_text` / `technology_text_ready` / `exhaust_technology_text`: `l4` (`invasion::space_cannon_defense`), `nes` (`combat::non_euclidean_shielding`), `pm` (`faction_abilities` production biomes), `it` (`reactions` instinct training). `TEXT_NOT_LENDABLE` is now only `executiveorder` (the Nekro cannot vote, so its text is unusable by design). Tests: `invasion::an_assimilated_l4_disruptors_silence_space_cannon_defense_for_the_nekro`, `nekro::an_assimilated_nes_and_the_lifted_exclusions_work_for_the_nekro`.
* `faction_abilities::the_gap_is_reported_rather_than_implied` now checks, per catalogue ability, that it is reported exactly when neither registered nor blocked (may be empty).
* `decision_delivery_inventory.rs`: producers `nekro.rs take_from`, `nekro_units.rs ask` (ObservedHere, via `ask_seeing`); observed asks for those and `production.rs produce_unit_by_ability`.
* **`nekrobt` stays unclaimed**: its flagship lending needs every faction's flagship hooks to accept the Nekro flagship; `LENDABLE_FLAGSHIPS` is empty and the Z option is never offered.

Results: `cargo test -p ti4-engine -j1 --no-fail-fast -- --test-threads=12`: lib 2516 passed, 1 ignored; content_ids_resolve 1; decision_delivery_inventory 4; doctests 5; all green. `cargo test -p ti4-policy -p ti4-sim`: 273 + 51 passed, 0 failed. Ledger: `nekro 13/14 implemented`, missing only `nekrobt`. Soak `base_faction_soak -- nekro 0 25 10`: 25 games, 0 failures.
