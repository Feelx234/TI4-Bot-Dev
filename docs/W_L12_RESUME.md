# W lane 12 resume note, branch `core-w-l12` (from `core-w-l10` @ `959b95d1`)

Task: items 3 to 8 of the open list of `docs/W_L10_RESUME.md`. Pure refactor: `RULES_VERSION`, serde and fixtures are unchanged. Lines for
`docs/ENGINE_KNOWN_ISSUES.md` are in the section "W lane 12" at its end (rows W12-design-01 to W12-design-06, and 07 once the forced-combat window is in).

## Ported (core half + Ask/Opt/Subtype + byte-exact renderer + `dual_window!` + lockstep test)

| item | what | where |
|---|---|---|
| 7a | stand-alone sustain offer below `RULES_FACTION_EFFECTS`: the holder's Farran trade good is paid unasked | `rules/combat/sustain_offer.rs` (meta version), `sustain_commander.rs` (`settled`); adapter bail removed in `dual_window::sustain_offer`, `cannon_dual::unported`, `action_cards_view::hit_facts` |
| 7b | `Research` below `RULES_TECH_PAYMENT_CHOICE`: the first `payment::plans` plan is spent unasked | `rules/production/strategy_cards.rs` (`fx_pay::pay_first_plan`); bails removed in `strategy_cards_core::research`, `agenda_close_dual`, `action_cards_view::carried_of`, `reactions_core` |
| 4 | Umbat copied through Ssruu, atomic | `rules/leader_production.rs` (`Umbat::carried`, `copied`), `leaders_core::muaat_agent` (restores the position when the decisions stop before the window is over) |
| 5 | The Inferno on a Nekro Z-token flagship; Fires of the Gashlai with the war sun text carried by a Valefar Assimilator | `query/reach.rs` (`flagship_text`), `rules/muaat_component.rs`; adapter bails removed in `factions/muaat.rs`, `faction_dual.rs` |
| 6 | Titans' Hecatoncheires instead of a PDS | `kernel/children.rs` (`StructureFacts::alt`), `rules/production/strategy_cards.rs` (`CStage::Alternative`), `choice/strategy.rs` (`Ask::PdsAlternative`, `Opt::{PlacePds, Alternative}`), `render/strategy.rs`, `strategy_cards_core::structure_facts` |
| 8 (part) | the combats Coalescence forces and Slumberstate Computing's coexist question | `rules/invasion/forced.rs` (`ForcedCombats`), `choice/invasion.rs` (`Ask::CoexistInstead`), `render/invasion.rs` (offer card), `dual_window::forced_combats`, `invasion.rs::add_forced_combats_windowed` (both call sites) |
| 3 (sites) | agents exhausted at a table with a Nomad holding Temporal Command Suite are announced by the core: a staged `AGENT_EXHAUSTED` | `kernel/fx_strategy.rs` (`exhaust_agent`); Tellurian, L1Z1X agent, Brother Milor, Trilossa, the Sol/Letnev round agents, Ssruu copies, The Thundarian |

Tests (each fails on the pre-W12 code, see the mutation checks in the report): `farran_pays_automatically_below_version_16_in_the_core_window_in_lockstep`,
`a_game_below_the_faction_effects_fires_the_guns_in_the_core_window_and_pays_the_commander_unasked`, `technology_declines_what_the_core_window_does_not_carry`,
`a_reveal_prototype_below_the_technology_payment_choice_is_played_by_the_core_in_a_fight`, `a_copy_of_umbat_through_ssruu_runs_in_lockstep_with_the_core_and_is_atomic`,
`the_inferno_and_fires_of_a_nekro_seat_run_in_lockstep_with_the_core`, `construction_asks_hecatoncheires_instead_of_a_pds_in_lockstep`,
`an_agent_exhausted_at_a_table_with_temporal_command_suite_is_announced_by_the_core_in_lockstep`, `the_sol_agent_with_a_listening_nomad_is_announced_by_the_core_in_lockstep`,
`coalescence_and_slumberstate_run_as_the_core_forced_combats_window_in_lockstep`; core-tests: `forced_combats.rs`, `construction.rs` (Hecatoncheires), `combat_small.rs` (legacy commander), `leader_production.rs` (Ssruu copy).

## Finding: the alpha saves are all rules version 0

`/var/lib/ti4-public` holds 14 games with an `init.json` (11 more directories have a lobby only): none carries `rules_version` in `initial_state` (the field is absent
while zero, `storage.rs`), so every saved game of the alpha runs at rules version 0, below every version gate. Both legacy branches of item 7 are therefore reached by
every old save, and both are ported (nothing is refused at load). Other version gates the core windows already follow natively: Munitions at the opening (< 8), the
sustain in the machine (< 17), the ground round numbering (< 16), Gravleash (< 16), retreat hazards (< 11), Plagiarize / Infiltrate / official action deck / no-empty-convert.

## What is left, with the precise reason


* **Temporal Command Suite itself (the ability and its trade window as a child) and Mercer (Nomad)**. The string TCS answers `AGENT_EXHAUSTED` when the staged events are flushed
  (`Game::announce_staged_ground_events` / `supply::try_flush_staged_events`, at the start of a step and after component and leader actions): that flush is no dual window and
  the exhaustion never reaches an ability inside the windows that exhaust the agent (it is staged, as the core sites now stage it). A core TCS (a `Spec` on `AgentExhausted`/`After`,
  exhaust the card, ready the agent, an optional `TradeWindow` child through `Inline::with_sx`) could only be run by a live loop that flushes staged events, which is the
  `core-d2-f-live` lane's contract, so it has no lockstep partner here. Mercer hangs on `TACTICAL_ACTION_ENDED`, which no core window emits (the tactical tail is the driver's).
  The family stays named by `bail_every_family_that_stays_in_the_string_is_named`.
* **Keleres agent** exhaustion (`Unknown::NomadAgentExhaustion` in `production/offers.rs`): dropped faction.
* **Invasion tail (item 8)**: Research Team / Viper EX-23 offers are Deepwrought / Firmament (dropped; `commit_tail_unported` names only them now). The forced combats are ported (above).
  The exploration reward composition after a first claim (`invasion::finish_control_gain`, the control-gain chain: Assimilate, PLANET_CONTROL_GAINED and its windows, technology AFTER
  windows, legendary and Fracture, exploration) was not touched: it is the D1 composition (the gate `invasion::finish_control_gain`), a chain of windows of several areas on one
  planet gain that needs the exploration windows of `rules::explore_windows` hosted inside the invasion's control step, not a single new window. Integrated Economy / Sling Relay at a
  Keleres or Deepwrought table are dropped; the first claim of a Fracture planet is Thunder's Edge.

## Checks (cheap only; private target dir `/root/deploytmp/core-w-l12`, `CARGO_BUILD_JOBS=2`, deleted at the end)

* `cargo test -p ti4-engine --lib`: 4884 passed, 0 failed; the same with `TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1`: 4884 passed, 0 failed.
* core + factions + bridge `--lib`: 624 passed; `ti4-engine-core-tests`: 1198 passed (new: `forced_combats.rs`, Hecatoncheires in `construction.rs`, the legacy commander in `combat_small.rs`, the Ssruu copy in `leader_production.rs`).
* `check_layers.py core`: 438 files, 0 violations; `lint_core.sh`: 440 files, 0 hits.
* Mutation checks (every mutant of the new core halves fails its tests; 20 of 20 killed): `sustain_offer.rs` (the unasked payment, the version read), `leader_production.rs` (the Ssruu carrier, the lender's status), `invasion/forced.rs`
  (the Slumberstate question, the coexistence write, the committed-twice guard, the breakthrough check), `production/strategy_cards.rs` (the Hecatoncheires offer, the mech fit, the infantry, the legacy first plan),
  `leaders_core.rs` (the atomic copy), `muaat_component.rs` (the Z-token flagship, the carried war sun), `kernel/fx_strategy.rs` (the staged `AGENT_EXHAUSTED`). Run with `runmut.py` in the lane's worktree itself (not a scratch worktree: a second
  target directory would not fit the 6 GB budget); one run committed a mutated file by accident (`git add -A` while the batch ran) and the next commit restores it.
* Not run: golden / wide dual sweeps, benches, soak, the mutation ratchet, workspace clippy, the census (the rule of the lane: only when a full zero-bail playthrough is possible).
