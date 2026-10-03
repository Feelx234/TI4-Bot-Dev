# BF-naalu: The Naalu Collective

Implementer: Sonnet subagent. Branch `wp/base-factions`, nothing committed or staged. Only
`crates/ti4-engine/src/factions/naalu.rs` and this file were written. Texts: `crates/ti4-content/content/*.json`
at `DEFAULT`. Historical Python reference: not used.

Printings resolved at `DEFAULT` (tested): mech `ti4_content::units::faction_unit(.., "naalu", "mech", DEFAULT)` =
`naalu_mech_te` (Thunder's Edge); agent `leaders::for_faction` = `naaluagent-te` (ledger test
`leaders_follow_the_sheet_and_leave_the_six_unchanged` pins `[naalucommander, naaluhero, naaluagent-te]`).

## Items

| Kind | Id | Status | Tests (`factions::naalu::tests::`) |
|---|---|---|---|
| ability | `telepathic` | done: `StrategyHooks::strategy_phase_ended` sets the 0 override for every Naalu seat unless Gift was played this round | `telepathic_makes_naalu_first_in_initiative_order`, `gift_is_optional_and_not_offered_to_its_owner_or_a_non_holder` |
| ability | `foresight` | done in the module; **ineffective in most live moves until Request 1** | `foresight_moves_the_ships_and_spends_a_strategy_token_into_the_new_system`, `foresight_is_refused_without_a_token_a_legal_system_or_for_the_mover_itself`, `foresight_excludes_systems_that_already_hold_naalus_token` |
| technology | `ng` Neuroglaive | done (mandatory, no question) | `neuroglaive_costs_the_activator_a_fleet_token_where_naalu_has_ships`, `neuroglaive_needs_the_technology_ships_there_and_another_player` |
| technology | `hcf2` | **partial, not claimed**: "moves without transport" is data (moveValue 2, tested); the half-ship charge is not (Request 2) | `the_hybrid_crystal_fighter_is_the_printed_unit` |
| unit | `naalu_fighter` | done (data: hits on 8, cost 0.5, fighter, no own move) | `the_hybrid_crystal_fighter_is_the_printed_unit` |
| unit | `naalu_fighter2` | not claimed (same as `hcf2`) | same |
| unit | `naalu_flagship` | **blocked**, Request 3 | |
| unit | `naalu_mech_te` | **partial, not claimed**: DEPLOY implemented for Fracture's relic event and a new `RELIC_GAINED` name; other relic gains emit nothing (Request 4) | `the_mech_is_the_thunders_edge_printing_and_deploys_after_a_fracture_relic` |
| promissory | `gift` | done: played at `STRATEGY_PHASE_ENDED`, returned at `STATUS_PHASE_ENDED` | `gift_gives_its_holder_the_token_and_switches_telepathic_off_until_status_ends`, `gift_is_optional_and_not_offered_to_its_owner_or_a_non_holder` |
| leader | `naaluagent-te` | **partial, not claimed**: only the activation token (typed `SYSTEM_ACTIVATED`) is covered (Request 5) | `the_agent_returns_an_activation_token_and_exhausts`, `the_agent_needs_a_token_there_and_may_be_declined` |
| leader | `naalucommander` | **blocked**: unlock is shared code; the effect needs a live view (Request 6) | |
| leader | `naaluhero` | done | `the_hero_takes_a_note_from_each_other_player_and_is_purged`, `the_hero_needs_to_be_unlocked_may_be_declined_and_keeps_the_card_when_declined` |
| breakthrough | `naalubt` Mindsieve | done: one `follow|waived` option per note in hand | `mindsieve_offers_one_waiver_per_note_in_hand_and_hands_the_note_over`, `mindsieve_is_not_offered_for_ones_own_card_or_without_the_breakthrough` |
| regression | no Naalu seat | done | `a_game_without_naalu_is_offered_nothing_and_keeps_its_initiative_order` (hook dispatch leaves overrides empty and the order unchanged; six events with no scripted answers leave the whole `GameState` equal) |

Claimed in `MODULE`: abilities `telepathic`, `foresight`; technologies `ng`; units `naalu_fighter`;
promissory `gift`; leaders `naaluhero`; breakthroughs `naalubt`. Ledger line: `naalu 7/13 implemented`.

## Design notes

* **Gift of Prescience.** Played by its holder (any seat of another faction holding `gift:naalu`) in a
  `STRATEGY_PHASE_ENDED` window: override 0 for the holder, note put faceup, `faction_marks["naalu:gift_round"] =
  <round>`. Telepathic's hook reads that mark (`state.round`) and does nothing that round. A second ability returns the
  note (`promissory::give_back`) and clears the mark at `STATUS_PHASE_ENDED`. `promissory::take` already marks a
  play-area note faceup the moment it changes hands, so Gift reads as faceup in a holder's hand from receipt; the
  module therefore does not use the faceup flag to decide whether it is "played", only its own mark.
* **Foresight.** Destinations are adjacent (`PlayerAdjacency`, so wormhole links count) systems with no other
  player's ships and no Naalu command token already there (a system holds one token per player), and a
  `relocate_ships` dry run on a cloned state must succeed (supernova/asteroid bars, adjacency). The ships move through
  `transit::relocate_ships`; Naalu's non-ship units in the space area (carried ground forces) move with them, since a
  ship cannot leave its cargo behind. Then 1 strategy-pool token is spent and a command token placed in the new
  system. `announce_relocation` (`SHIPS_RELOCATED`) is **not** called: a stateful ability has no `Resolving`.
* **Neuroglaive** returns the token by `spend_token(Fleet)` (reinforcements are derived from sheet and board).
* **Mindsieve** puts the chosen note in the waiver id (`ms|<note>`), since the waiver hooks have no table.
  Only notes in hand (not faceup) are offered.
* **Oracle.** Offered only when some other player holds a note in hand; each such player picks the note (one note is
  given without a question). Support for the Throne is not in `promissory_notes`, so it is never taken (see questions).

## Hook requests

1. **`game.rs` `begin_one_move` (~line 2623), blocking Foresight in the common case.** A ship that sails with no cargo
   emits only the log string `self.emit("SHIP_MOVED")`; the typed `SHIP_MOVED` (with `player`, `system`, `origin`, `unit`)
   comes from `note_arrival`, which is called only in the `Loading` stage (line 2445). Call
   `self.note_arrival(&window.player, origin, &ship, &outcome)` after `self.sail(...)` in the no-cargo branch too.
   (Also affects the other two cards that read "after a player moves ships into".)
2. **`fleet.rs`, `hcf2`.** `fighters_charged_to_fleet_pool` charges 1 per excess upgraded fighter (any fighter with a
   `requiredTechId`). Naalu's text is 1/2 of a ship per excess fighter. Needs half-ship arithmetic in
   `Standing::fleet_charged` (for example count in half-units, or `ceil(excess / 2)` after deciding the rounding rule) when
   the fighter's id is `naalu_fighter2`. The fleet-supply hook cannot do it (the limit is per player, the charge per
   system).
3. **`hooks_ground.rs` / `invasion.rs`, `naalu_flagship`.** New hook, for example
   `GroundHooks::space_commit_types(&GameState, &ContentStore, SourceSet, invader: &PlayerId, system: &SystemId) -> Vec<Unit>`:
   units in the active system's space area that may be committed to planets as ground forces (Naalu: its fighters, only
   while a Matriarch is in that system). The engine would commit them like infantry (they roll their own combat die),
   and **return survivors to the space area** after combat. `commit_candidates` cannot do it (it names planet-resident
   units).
4. **Typed relic-gain event.** Emit `RELIC_GAINED` (`player`, `relic`) from `agenda_effects.rs:1068` and
   `exploration.rs:630` (Fracture already emits `FRACTURE_RELIC_GAINED`; do not emit both for the same gain, since the
   module listens on both names). Then `naalu_mech_te` can be claimed.
5. **Typed command-token placement event.** `COMMAND_TOKEN_PLACED` (`player`, `system`) wherever a token goes on the
   board (activation already carries `SYSTEM_ACTIVATED`; Foresight, leader and card placements do not). The agent then
   needs a second ability on that event. Not tested in a running `Game`: returning the activation token mid-tactical
   action may interact with `game.rs` token bookkeeping.
6. **Commander `naalucommander`.** "At any time" look at neighbours' promissory hands and the top and bottom of the
   agenda deck. Needs a live (computed, not stored) view: for example `CardHooks::sees_hand(&GameState, &ContentStore,
   viewer, owner, RevealKind) -> bool` read by `SeatObservation::revealed_*`, plus an agenda-deck peek accessor. Stored
   `Standing` reveals go stale when a hand changes.

## Decision sites to register (`tests/decision_delivery_inventory.rs`)

| Module | Function | Choice | AskObserved |
|---|---|---|---|
| `naalu.rs` | `foresight` | 1 | 1 |
| `naalu.rs` | `hero` | 1 | 1 |

(Confirmed from the failing registry diff: exactly these four entries are new from this file. The mech asks through the
shared `place_units_choosing`.)

## Rules questions

* Foresight: do carried ground forces and fighters in the space area move with the ships? Implemented yes. Fighters are
  ships and move in any case.
* Foresight with the Naalu token already in the destination: implemented as not allowed (a system holds one token per
  player); the text offers no alternative.
* Foresight and fleet supply/capacity at the destination: not refused (37.3 settles it at the end of the turn).
* Oracle: "if you do" is read as "if anyone has a note to give". Support for the Throne counts as a promissory note
  in hand; this engine keeps it outside `promissory_notes` (`support_holders`), so it is never given. Faceup play-area
  notes are not "in hand".
* Mindsieve: same reading of "a promissory note" (notes in hand only, no Support for the Throne).
* Mech DEPLOY and Foresight are optional here ("may" / DEPLOY is a choice).

## Commands run and exact results (`LIBTORCH=D:/Projects/ti4-engine-rs/out/libtorch-2.9.1-cpu`)

| Command | Result |
|---|---|
| `cargo test -p ti4-engine --lib -- factions::naalu` | 18 passed, 0 failed |
| `cargo test -p ti4-engine --lib -- factions::` | 185 passed, 0 failed, 1 ignored (ledger printer) |
| `cargo clippy -p ti4-engine --all-targets` | no warning in `factions/naalu.rs` (one `needless_update` was fixed) |
| `rustfmt --edition 2024 crates/ti4-engine/src/factions/naalu.rs` | applied |
| `cargo test -p ti4-engine -q --no-fail-fast` | lib 1694 passed, 0 failed, 1 ignored; `decision_delivery_inventory` 3 passed, **1 failed** (the four unregistered `naalu.rs` sites above; the coordinator registers them); other binaries ok |

Ledger (`cargo test -p ti4-engine --lib print_faction_ledger -- --ignored --nocapture`):

```
naalu      7/13 implemented
    Technology hcf2
    Unit naalu_fighter2
    Unit naalu_flagship
    Unit naalu_mech_te
    Leader naaluagent-te
    Leader naalucommander
```

Not run: `ti4-sim`, any live `Game` with a Naalu seat (every test drives the typed events through
`fixtures::armed_resolver`).

## Review fixes

* S3 The Oracle: every victim's note is chosen first, then all are taken and the hero purged; an illegal answer
  changes nothing.
* S4 `timing_abilities` registers Foresight, Z'eu and Iconoclast only under `cfg!(test)`; real games get none of them
  until a running-`Game` test proves them. Coordinator removed `foresight` from `MODULE.abilities`.

## Round 3 (new shared routes: BF-F2, BF-F5, MOVEMENT_FINISHED)

* Foresight now resolves once per movement step on typed `MOVEMENT_FINISHED` (player, system), live, claimed. Proven in
  a running `Game` (`foresight_resolves_once_after_a_real_movement_step`: activate, move in, Foresight, Naalu ships and
  the strategy token end in the neighbour, no combat in the active system).
* `hcf2` and `naalu_fighter2` claimed: `CombatHooks::fighter_fleet_weight_halves` set; test through `fleet::standing`
  (4 excess fighter IIs charge 2 ships; 4 without the hook).
* Z'eu moved to typed `COMMAND_TOKEN_PLACED` (activation emits it; other sites do not yet). Partial, still gated to test
  builds and unclaimed. Iconoclast: no new route (needs `RELIC_GAINED`); flagship: none; commander: none.
* Decision sites unchanged (`foresight` 1+1, `hero` 1+1).
* Commands: `factions::naalu` 20 passed; `factions::` 289 passed; clippy clean for naalu.rs; full suite lib 1857 passed,
  2 failed (`transit::` rides-free tests, not naalu), `decision_delivery_inventory` 2 failed (other agents' sites).
* Foresight requires `ships_moved > 0` on MOVEMENT_FINISHED (review); test `foresight_needs_the_mover_to_have_moved_ships_in`.

## Round 4 (Matriarch invasion seam)

* Matriarch is claimed. `GroundHooks::temporary_space_commit_candidates` supplies Naalu fighters
  only while its flagship is in the active system. `InvasionWindow` records those landings as
  temporary and returns surviving copies to the space area after their ground combat (or before
  control when no combat happens), so fighters can roll but cannot establish control alone.

## Round 4

* `naalu_mech_te` claimed: DEPLOY on typed `RELIC_GAINED` (announced at the start of the next step by `Game::announce_gains`,
  so the window is step-delayed, not mid-effect; a free placement, nothing observable differs). The `FRACTURE_RELIC_GAINED`
  listener was dropped (the diff-based announcement already covers Fracture; both would double-fire). Running-`Game`
  test `a_relic_gained_in_a_running_game_offers_the_deploy_once` (needs `.with_sources(DEFAULT)`, else the game deals the
  PoK `naalu_mech`).
* Z'eu: unchanged, partial (card says any placement; only activation announced). Stays test-build only, unclaimed.
* Flagship: `GroundHooks::commit_candidates` returns `CommitCandidate {system, planet, unit}` and the engine drops any
  that is not a ground force standing on that planet, so fighters in the space area cannot be offered. Missing hook:
  commit units from the active system's space area as ground forces (they keep their type and roll its die), and
  return survivors to the space area after the invasion.
* Commander: `RevealKind` has no agenda variant and stored reveals go stale; needs a live computed view (a
  `CardHooks::sees_hand`-style read by `SeatObservation`) and an agenda-deck peek. Not done.
* Decision sites unchanged.
