# BF-ORIGINAL-SIX: ledger closure for sol, hacan, letnev, xxcha, jolnar, l1z1x

Branch `wp/base-factions` (uncommitted). Source: `crates/ti4-content/content/*.json` card text.
Historical Python not used.

## Mechanism

`factions::LEGACY_CLAIMS` (`crates/ti4-engine/src/factions/mod.rs`) is a (faction, kind, id) table
that `factions::implemented` / `missing` / the ledger consult next to `MODULES`. The six are not in
`MODULES` on purpose: `supply::staging_enabled` and other paths switch on "a module faction is
seated", so a module would change every original-six game (all training checkpoints use them).
Test `legacy_claims_name_real_assets_of_their_own_original_faction` checks every claim names a real
asset of its own in-scope faction and that none of the six is in `MODULES`.
`quash` is also in `faction_abilities::registered()` and removed from `blocked()`.

## Ledger result

`cargo test -p ti4-engine --lib -j1 print_faction_ledger -- --ignored --nocapture`:
sol 15/15, hacan 12/12, letnev 11/11, xxcha 11/11, jolnar 12/12, l1z1x 13/13; all 15 module
factions unchanged n/n (21 of 21 factions complete).

## Rows claimed on the audit verdict (26, no engine change)

| faction | ids | verdict |
|---|---|---|
| sol | ac2, so2, sol_carrier, sol_carrier2, sol_flagship, sol_infantry, sol_infantry2 | complete; direct board-swap tests added (below) |
| hacan | pm, qdn, hacan_flagship | complete (`pm`: `faction_abilities` Production Biomes test) |
| letnev | l4, letnev_flagship, war_funding | complete |
| xxcha | it, nf, xxcha_flagship, favor | complete (`favor`: `game::tests::political_favor_*`) |
| jolnar | ers, scc, jolnar_flagship | complete |
| l1z1x | is, sdn2, l1z1x_dreadnought, l1z1x_dreadnought2, l1z1x_flagship, ce | complete; sdn2 swap + Direct Hit immunity tests added |

The audit verdict is the evidence for these rows; this package added only the direct tests in the
last section.

## Fixes (gameplay changes for the original six)

### 1. xxcha `quash`
Card: "When an agenda is revealed: You may spend 1 token from your strategy pool to discard that
agenda and reveal 1 agenda from the top of the deck. Players vote on this agenda instead."
Code: `game.rs` `offer_quash`, called in `reveal_agenda` before Political Favor and only while no
replacement is pending. Reuses `agenda_veto_replacement`, so the loop discards, reveals the next and
offers Quash again on it. Offered to each Xxcha seat with a strategy token and a non-empty deck;
asked via `ask_to_use_note` (DecisionSource `Content("quash")`); token spent and staged
(`note_strategy_token_spent`, reason `quash`); event `QUASH_USED`. Atomic: asks before any change.
Tests (real `reveal_agenda`): `quash_discards_the_agenda_and_costs_xxcha_a_strategy_token`,
`quash_declined_leaves_the_agenda_and_the_token`, `quash_without_a_strategy_token_is_never_offered`.

### 2. hacan `convoys` (Trade Convoys)
Card: "ACTION: Place this card faceup in your play area. While ... you may negotiate transactions
with players who are not your neighbor. If you activate a system that contains 1 or more of the
Hacan player's units, return this card to the Hacan player."
Rule decisions: (a) `promissory::take` no longer puts convoys faceup on receipt (the text opens
ACTION); it stays in hand until the holder spends a component action
`faction|trade_convoys` ("Trade Convoys: place it faceup") offered by
`faction_abilities::component_actions` (never to Hacan for its own copy), performed by
`promissory::play_convoys`. Other "ACTION: place faceup" notes (pop, blood_pact, dark_pact, ...)
still go faceup on receipt: out of scope, owned by other modules. (b) Return clause:
`promissory::spend_support_on_activation` now also returns faceup convoys when the activated
system holds Hacan units (ships or ground), via new `convoys_returned_by_activation`
(shared helper with Alliance). Covers every activation path that already calls it.
`reaches_anyone` still needs faceup, so trading with non-neighbours now needs the ACTION first.
Tests: `trade_convoys_reach_the_whole_table_only_when_faceup`,
`trade_convoys_return_when_the_holder_activates_a_system_with_hacan_units`,
`trade_convoys_is_an_action_that_places_the_card_faceup`; updated
`receipt_puts_play_area_notes_faceup_and_the_rest_in_hand`, the `available_notes` convoys case and
`invasion::tests::ground_combat_uses_note_holdings_from_tactical_action_start`.

### 3. sol `ms` (Military Support)
Card: "At the start of the Sol player's turn: Remove 1 token from the Sol player's strategy pool,
if able, and return it to their reinforcements. Then, you may place 2 infantry from your
reinforcements on any planet you control. Then, return this card to the Sol player."
Code: `promissory::turn_started(context, player)` now runs through a `TimingContext`
(`game.rs` `resolve_military_support`, called where the old call was) and uses
`action_cards::place_units_choosing` (optional, `ControlledPlanet`, `PlacementLimits::Respect`):
the holder picks the planet or declines; infantry id from `placed_unit_id` (holder's upgrade);
count capped by `supply::allowed`. Rule decisions: declining plays nothing and the holder keeps the
card; nothing is asked if no planet or no infantry in the box; the Sol token is removed (if able)
only once infantry are placed, then the card goes home. Decision site reuses
`place_units_choosing` (already registered).
Tests: `military_support_plants_two_of_the_holders_infantry_and_the_note_goes_home`,
`declined_military_support_changes_nothing`,
`military_support_lets_the_holder_pick_the_planet_and_uses_their_upgrade`,
`military_support_is_not_asked_without_a_planet_and_the_note_stays`.

### 4. letnev `nes` (Non-Euclidean Shielding)
Card: "When 1 of your units uses SUSTAIN DAMAGE, cancel 2 hits instead of 1."
Code (`invasion.rs`): helper `sustain_extra` (reuses `combat::non_euclidean_shielding`, now
`pub(crate)`); after a ground sustain the remaining hits shrink by one more, per use, in
`absorb_ground_with_origin` (ground combat, Harrow), `take_bombard_hits`, `remove_ground`
(space cannon defence, call site now uses it), `assign_ground_hits_with_origin_in_timing`.
`assign_selected_ground_hit_in_timing` now returns how many hits the assignment used (0 / 1 / 2);
Saar Deorbit Barrage consumes the extra. New `ground_hit_outcome_logged` returns the outcome;
`ground_hit_logged` kept for tests (`cfg_attr(not(test), allow(dead_code))`).
Tests: `non_euclidean_shielding_cancels_two_ground_hits_per_sustain`,
`..._applies_per_use_not_per_hit`, `..._cancels_two_bombardment_hits_per_sustain`,
`..._cancels_two_removed_ground_hits_per_sustain`. No direct test for the Saar and timing-path
loops (same pattern).

### 5. jolnar `ra` (Research Agreement)
Card: "After the Jol-Nar player researches a technology that is not a faction technology: Gain that
technology. Then, return this card to the Jol-Nar player."
Code: the automatic grant is removed from `technology::complete_research`; new
`technology::offer_research_agreement` (and `_in` for timing contexts) asks the holder (use /
decline, `Content("ra")`); no ask for faction technologies or a technology the holder already owns;
accept grants it, applies unit upgrades and returns the note; decline changes nothing. Called by
every table-having research path: `strategy_cards::resolve_research` (both branches),
`specialist_compounds`, `action_cards` (Reveal Prototype-style paid research x2, Focused Research),
Sardakk research reaction. Table-less `technology::research` never takes it silently (same rule as
optional waivers). Decision site registered in `tests/decision_delivery_inventory.rs`
(`offer_research_agreement`: Choice + AskObserved).
Tests: `a_research_agreement_hands_the_holder_the_same_technology_and_goes_home`,
`a_declined_research_agreement_changes_nothing`,
`a_research_agreement_is_not_offered_for_a_faction_technology_or_a_known_one`.

### 6. Direct tests
- `technology::tests::faction_unit_upgrades_replace_the_units_already_on_the_board`:
  ac2 sol_carrier to sol_carrier2, so2 sol_infantry to sol_infantry2 (on a planet), sdn2
  l1z1x_dreadnought to l1z1x_dreadnought2, all on the board.
- `action_cards::tests::direct_hit_cannot_destroy_a_dreadnought_ii_or_a_super_dreadnought_ii`
  (dreadnought and l1z1x_dreadnought destroyed; both II forms immune).

## Checks

- `cargo test -p ti4-engine -j1 -- --test-threads=12` (default features): lib 2348 passed, 0 failed,
  1 ignored; content_ids_resolve 1; decision_delivery_inventory 4; doc 5 passed. Exit 0.
  Build env: CARGO_PROFILE_TEST_DEBUG=0, INCREMENTAL=false, CODEGEN_UNITS=16, `-j1`.
- rustfmt --edition 2024 applied to invasion, promissory, technology, faction_abilities,
  strategy_cards, saar, sardakk (clean at base). Not formatted: game.rs (another session's hunks),
  combat.rs / action_cards.rs (already non-rustfmt at base; my edits hand-formatted),
  factions/mod.rs (module tree not checkable standalone; hand-formatted).
- game.rs: only my hunks were added (call sites in `step`, `reveal_agenda`, new methods, tests);
  the other session's `round_income` hunks are untouched.

## Known gaps

- Other "ACTION: place faceup" promissory notes still go faceup on receipt.
- Quash decision uses the shared `ask_to_use_note` option labels ("use"/decline).

## Operator correction, 2026-10-06 (coordinator)

The operator ruled that a promissory note resolves when its window comes; only clauses that say "may" are optional.
- **Research Agreement (`ra`)**: the audit's PARTIAL ("holder is never asked") was wrong — the card has no "may". Reverted to the original automatic grant inside `technology::complete_research` (every research route, table or not); `offer_research_agreement*`, its 7 call sites, its decline tests and its registry entries are removed; the original test `a_research_agreement_hands_the_holder_the_same_technology_and_goes_home` is restored. `ra` stays claimed (it was complete before).
- **Military Support (`ms`)**: only "you may place 2 infantry" is optional. The Sol token removal ("if able") and the return now always happen, even when the holder declines the infantry or has no planet. Tests `declining_the_infantry_still_takes_the_token_and_sends_the_card_home` and `without_a_planet_nothing_is_asked_but_the_card_still_resolves` replace the two tests that encoded the wrong reading.

## Review (Sonnet, post-correction) — no blockers

Known gaps recorded, not fixed: (S1) if Veto has already set `agenda_veto_replacement` in the reveal window, Quash (like Political Favor) is not offered for the original agenda — needs an ordering ruling; (S2) with two Quash holders the offer follows seat order, not speaker order (one Xxcha per game in practice). Nits left: `ground_hit_logged` is test-only; NES subtracts even when the sustain lookup returns None (pre-existing); no end-to-end test that placing Trade Convoys spends the turn.
