# BF-nomad, part A (abilities, flagship, technologies, hero, commander, breakthrough)

Part B (three agents, mech, The Cavalry) is `factions/nomad_agents.rs`, written in parallel; its
evidence is the other agent's. Files for part A: `factions/nomad.rs` (module + 20 inline tests),
`factions/hooks_movement.rs`, `movement.rs`, `transit.rs` (two new movement hooks).

## Items

| Kind | Id | Status | Route | Tests |
|---|---|---|---|---|
| ability | `the_company` | done (shared deal already correct) | `leaders::for_faction` / `deploy`: sheet lists 3 agents, all dealt Readied; hero and commander Locked | `the_company_deals_three_readied_agents_beside_a_locked_commander_and_hero` |
| ability | `future_sight` | done | `AGENDA_RESOLVED` After window (typed, `game.rs close_vote`); reads `agenda_votes` / `agenda_predictions`, skips a Deadly-Plot discard | `future_sight_pays_one_trade_good_for_a_voted_or_predicted_outcome_only`, `future_sight_pays_through_a_real_vote_only_when_the_backed_outcome_wins` (driven `Game`, both outcomes) |
| technology | `m2` (Memoria II) | done | data-driven upgrade (`technology::apply_unit_upgrades`, stats from `units.json`) + the Memoria adjacency below | `memoria_ii_is_the_data_driven_upgrade_with_the_printed_statistics` |
| unit | `nomad_flagship`, `nomad_flagship2` | done, **coordinator to claim in `nomad_agents::UNITS`** | new hook `MovementHooks::unit_adjacent_systems` -> `MovementRules::path_from_ship`, reached by `tactical::movable_into` and `Game::begin_one_move` | `memoria_is_adjacent_to_systems_with_your_mechs_for_the_flagship_only` (both ids), `memoria_is_offered_through_the_tactical_movement_list`, `memoria_changes_nothing_in_a_game_without_the_nomad` |
| leader | `nomadhero` | done, **coordinator to claim in `nomad_agents::LEADERS`** | `leader_action`/`use_leader` hooks; effect is a round-stamped mark read by new hook `MovementHooks::ignores_command_tokens` (`path_from_ship` departure + `transit::CargoWindow::for_ship` 95.5) | `the_hero_frees_the_flagship_and_its_cargo_from_command_tokens_for_the_round`, `the_hero_is_not_offered_without_a_flagship_or_to_anyone_else` |
| leader | `nomadcommander` | done, **coordinator to claim in `nomad_agents::LEADERS`** | `commander_unlocked` hook (`secrets::scored_count >= 1`); free flagship already in `production.rs` | `the_commander_unlocks_on_one_scored_secret_objective_only`, `the_unlocked_commander_makes_the_flagship_free_in_production` |
| breakthrough | `nomadbt` | done | `TURN_BEGAN` After window | `thunders_paradox_*` (3 tests) |
| technology | `tcs` | **blocked (not claimed)**: no typed event for an exhausted agent | ability written and tested on `AGENT_EXHAUSTED`; see hook request | `tcs_*` (4), `a_staged_agent_exhaustion_reaches_tcs_through_the_staged_event_flush` |

Neutrality: `no_nomad_window_opens_in_a_game_without_the_faction`,
`memoria_changes_nothing_in_a_game_without_the_nomad`.

## Card text (latest printing)

* Future Sight: "During the Agenda phase, after an outcome that you voted for or predicted is
  resolved: Gain 1 trade good."
* Memoria I/II and technology m2: "You may treat this unit as if it were adjacent to systems that
  contain 1 or more of your mechs." (II: cost 8, combat 5 (x2), move 2, capacity 6, SUSTAIN DAMAGE,
  ANTI-FIGHTER BARRAGE 5 (x3).)
* Temporal Command Suite: "After any player's agent becomes exhausted, you may exhaust this card to
  ready that agent; if you ready another player's agent, you may perform a transaction with that
  player."
* Ahk-Syl Siven: "ACTION: Place this card near the game board. Your flagship and units it transports
  can move out of systems that contain your command tokens during this game round. At the end of that
  game round, purge this card." Unlock: 3 scored objectives (shared `leaders::check_unlocks`).
* Navarch Feng: "When you produce: You can produce your flagship without spending resources."
  Unlock: "Have 1 scored secret objective."
* Thunder's Paradox: "At the start of any player's turn, you may exhaust 1 of your agents to ready any
  other agent."

## New shared hooks (this part)

* `MovementHooks::unit_adjacent_systems(state, content, sources, player, ship_type) -> Vec<String>`:
  systems one ship type is treated as adjacent to from wherever it stands. `MovementRules` computes
  it per ship type the mover owns; `path_from_ship` adds them to each step's neighbours (only when
  called with that ship type, which `movable_into`, `begin_one_move` and `preview_moves` all do).
* `MovementHooks::ignores_command_tokens(state, content, sources, player, ship_type) -> bool`:
  that ship type may leave a system with its owner's command token (`may_depart_ship`), and
  `CargoWindow::for_ship` lifts 95.5 for its cargo (`transit::loadable_by`).

## Rule decisions

* **Memoria "adjacent"** is applied to the flagship's own movement (route search), at every step it
  takes, not to `PlayerAdjacency` (transactions, space cannon range, retreat, agents). A mech on a
  planet or in the space area counts. Only the owner's mechs count. "May": adding neighbours never
  removes the ordinary route.
* **Hero card status**: shared `leaders::use_leader` purges a hero as soon as it resolves. The effect
  lives in `faction_marks["nomad:hero:<player>"] = <round>` and ends with that round, so only the
  card's displayed status (Purged from use, not from round end) differs from "purge at the end of that
  game round". No `leaders.rs` change was made. The hero is offered only while the player has a
  flagship on the board.
* **Hero scope**: only the flagship may leave; units it transports may be picked up from token
  systems it starts in or moves through (95.5 lifted for that ship). Other ships stay pinned.
* **Future Sight timing**: fires in the AFTER window of `AGENDA_RESOLVED`, which `game.rs` opens
  before the agenda's own effect is applied (`resolve_agenda_outcome`). The trade good does not
  depend on the effect. A Deadly Plot discard (flag set in the When window) pays nothing.
* **Thunder's Paradox**: "any other agent" = any exhausted agent of any player other than the one
  just exhausted; offered only when such a target exists. Several pairs: one follow-up choice.
* **TCS transaction**: counts against the once-per-player-per-turn limit (not offered if already
  spent); reach to the partner is granted through `CardHooks::transaction_reach` for the duration of
  the negotiation (mark removed even on error). Needs a map (`context.galaxy`).
* **Nomadic**: not a Nomad ability in the corpus (Saar's); nothing reused.

## Hook requests (files not owned by part A)

1. **`leaders.rs` `exhaust`** (blocks `tcs`): after a successful exhaust of an agent, announce
   `AGENT_EXHAUSTED` when a Nomad could hear it:
   ```rust
   if crate::factions::nomad::watches_agent_exhaustion(state)
       && kind_of(ContentStore::embedded(), leader).as_deref() == Some(AGENT)
   {
       crate::supply::stage_event(
           state,
           crate::factions::nomad::AGENT_EXHAUSTED,
           &std::collections::BTreeMap::from([
               ("player".to_owned(), player.to_string().into()),
               ("leader".to_owned(), leader.as_str().into()),
           ]),
       );
   }
   ```
   (`watches_agent_exhaustion` is true only with a seated Nomad holding `tcs`, so other games stay
   byte-identical.) The existing staged-event flush then delivers it; the test
   `a_staged_agent_exhaustion_reaches_tcs_through_the_staged_event_flush` proves that half. Then add
   `"tcs"` to `nomad.rs` `technologies` and a `leaders::exhaust`-driven test.
2. **Claims** to add in `nomad_agents.rs`: `UNITS += nomad_flagship, nomad_flagship2`;
   `LEADERS += nomadhero, nomadcommander`.

## Decision sites to register (`tests/decision_delivery_inventory.rs`)

From the failing registry test (`unreviewed decision sites`), part A:

* `nomad.rs` `ask`: `Choice` 1, `AskObserved` 1 (Thunder's Paradox pair choice; TCS "transact?").
* `nomad.rs` `temporal_command_suite`: `AskObserved` 1 (the deal inside the transaction loop).

(`nomad_agents.rs` `ask` is part B's.)

## Commands

* `cargo test -p ti4-engine --lib -j1 factions::nomad::tests`: 20 passed, 0 failed (part A tests; the
  package filter `factions::nomad` also matches part B's `nomad_agents` tests, which were mid-edit).
* `cargo test -p ti4-engine -j1 --no-fail-fast -- --test-threads=16`: lib 2233 passed, 0 failed,
  1 ignored; `content_ids_resolve` 1 passed; `decision_delivery_inventory`: 3 passed, 1 failed (the
  expected unreviewed-sites failure, naming only the sites above and part B's `ask`);
  doc-tests 5 passed.
* Ledger (`print_faction_ledger`): `nomad 9/14 implemented`; gaps `tcs`, `nomad_flagship`,
  `nomad_flagship2`, `nomadcommander`, `nomadhero` (the last four are done and await the claim).
* `rustfmt --edition 2024` clean on `nomad.rs`; hunks in `movement.rs`, `transit.rs`,
  `hooks_movement.rs` checked with `--check --config skip_children=true`.

## Review fixes

* **TCS error path** (`nomad.rs`, `temporal_command_suite`): the use now snapshots `GameState` after
  the guard and restores it if the readying/transaction body returns `Err`, so the tech is not left
  exhausted and the agent not left readied. Test:
  `tcs_error_in_the_transaction_leaves_the_agent_and_the_card_untouched` (an unoffered deal answer).
* **Agents exhausted outside `leaders::exhaust`**: Emissary Taivra (`ghost.rs`), Brother Milor
  (`yin.rs` `exhaust_agent`) and Ssruu's copy (`borrowed_round_agents.rs`) now go through
  `leaders::exhaust`, so a watching Nomad's TCS hears them. Tests:
  `emissary_taivra_exhausting_is_heard_by_a_watching_nomad`,
  `brother_milor_exhausting_is_heard_by_a_watching_nomad`, and an added assertion in the Ssruu
  copy test (`resolver_selects_one_unit_and_invalid_selection_leaves_ssruu_ready`).
* **Ambiguity noted:** the card says "you may perform a transaction with that player"; whether a TCS
  transaction counts against the once-per-turn transaction limit is not settled by the text. The
  implementation reads it as counting (see Rule decisions); the opposite reading (a free extra
  transaction) is equally defensible and is not tested either way.
