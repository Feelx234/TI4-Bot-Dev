# D2 lane M6d (`core-m6d-tactical`): closing the tactical island's last abilities, resume note (2026-10-31)

Branch `core-m6d-tactical`, cut from `core-m6c-tactical` (6e21b0b4) and merged with `core-g-silent` (afae39dd: it carries `core-m7-native`, `core-m2-production`, lane G's registry audit and loud gaps). Never pushed;
`nightly-fixes-2026-10-26` untouched. Read `docs/M6C_RESUME.md` first (its "Remaining gaps" list was this lane's task list) and `docs/G_SILENT_RESUME.md` (the all-timing audit this lane extends).
Pure refactor: `RULES_VERSION`, serde and fixtures unchanged.

## The merge (what a later merger needs to know)

* `core-g-silent` was merged INTO this branch (15 files conflicted, all resolved by keeping both sides; `Unknown::ReflectiveShielding` went away as in G's base). The audit `ti4-engine/src/ability_audit.rs`
  (every ability of the string registry, any event, any relation) is lane G's: this lane did not duplicate it, it only turned rows into rules. The `Lane("core-m6c-tactical")` entries are gone (this branch has M6c).
* Rows of `silent::gaps::GAPS` / `ability_gaps::GAP_IDS` deleted here because the ability is a rule now (the rows are positional, the slot is `41_000 + index`: every later row moved up; `Status::Gap(n)` of the audit and the
  `SUFFI_AN*` consts were renumbered by a script, `tools`-less, see the commit diffs): awaken, titans_flagship, arboreccommander, arborec_flagship, yssarilcommander, yssarilcommander_clear, l1z1xagent (Ssruu copy),
  nomadagentmercer, muaathero, muaathero_moved, ghostcommander (SHIP_MOVED, MOVEMENT_FINISHED), empyreancommander, empyreanagent. A lane that also deletes rows will conflict on those three files; resolve by
  deleting the same ids on both sides and renumbering.
* `reactions_core.rs`: `NAMED` (additive lines), `table()` (`activation::register` beside lane G's `silent::register_abilities`), `PAIRS` (the Ssruu copy of I48S), `set_table_codes` (`ActivationCodes` gets six more texts).

## Native (core half + typed Ask/Opt/Subtype + byte-exact renderer + lockstep with the string)

New typed questions live in `choice/activation.rs` (`reaction::{Ask,Opt,Subtype}::Act`, renderer `render/activation.rs`); none of them carries the window's trigger (`Ask::carries_trigger`).

| ability | core file | notes / lockstep |
|---|---|---|
| Titans Awaken, Ouranos (also the Nekro flagship through `flagship_has_text`) | `cd_registry/titans/timing.rs` | every question before the first placement; Coalescence mark; units from the supply (`fx::box_has`, `place_counted`); `tactical_live::awaken_and_ouranos_ask_in_lockstep` (one/two tokens, Hecatoncheires, coalescence, Ouranos with one/two PDS) |
| Arborec commander Dirzuga Rophal, flagship Duha Menaimon | `ce_movement/arborec/timing.rs` + `driver/windows/production/live.rs` (`produce_in`, `can_produce_by_ability`, `ability_facts`) | a PRODUCTION window as a child of the ability: the `ProductionMachine` in `UseMode::Ability` stepped inside the effect (`Eff::with_pay_sx`), its questions put to the effect's tape, `UNITS_PRODUCED` announced; the kernel is the adapter's production kernel of the game's scope (`AdapterFacts::production_kernel`, `production_view::kernel_for_core`); the production questions are rendered with it (`live::render_asking`). `tactical_live::the_arborec_flagship_and_commander_produce_in_lockstep` |
| Yssaril commander So Ata and its end-of-action clear | `cd_registry/yssaril/timing.rs` | `CardsReveal` mark (scope `action`, source `yssarilcommander`), notes listed in the adapter's text order (`PlayFacts::note_order`); the clear is an `ACTION_COMPLETED` rule (the finish also clears every action-scope reveal). `so_ata_looks_at_the_activating_players_cards_in_lockstep`. NOT carried: the string tells the undo tracker that hidden information was seen (`rng.observe_hidden_information()`); the core holds no tracker, it is the host's |
| Ssruu copy of I48S (the string asks the BENEFICIARY which infantry) | `cd_registry/yssaril/ssruu.rs` (`L1Z1X`, registered ahead of the round agents like the string) | `the_ssruu_copy_of_i48s_asks_the_activating_player_in_lockstep`; a Naaz beneficiary stages the mech placement (`stage_naaz_mech_placed`) |
| Field Marshal Mercer | `ce_movement/nomad/timing.rs` | asked of the ACTIVE player; `TacticalGap::SsruuLends` is gone. FINDING (fixed, guarded by `mercer_hears_the_end_of_the_action_when_readied_and_the_tail_never_stops_for_him`): `tail::ssruu_lends` also counted an exhausted Mercer, the string only lends T'ro (there is NO Ssruu copy of Mercer in the string registry); the end of the action was announced when it must not be. `combat_live::mercer_*` (1-3 ground forces, one and two landing planets, exhausted card) |
| Muaat hero Adjudicator Ba'al: Nova Seed + the war sun record | `ce_movement/muaat/timing.rs`, `kernel/fx_map.rs`, `galaxy.rs` (`swap_systems`, `replace_system`, `edits_applied`) | tile replace on the core map, contents move (units, command tokens, frontier token, wormhole tokens, ion storm), planets purged, laws naming a planet repealed, ships/ground forces staged as destroyed (`breakthrough:nova_seed`, `NovaSeed`), hero purged. The core map REPLAYS recorded `MapEdit` marks (`fx_map::replay_map_edits`): the prelude no longer stops with `TurnGap::MapEdits` (it still does when an edit does not fit the map), which also lets the Ghosts hero's swap (Riftwalker Meian) through; the Map codec copies the string galaxy's `edits_applied`. `combat_live::nova_seed_replaces_the_tile_in_lockstep` (hero used, plain and rich contents, declined), core tests `map_edits.rs` (5). Counted gap kept: `TacticalGap::NovaSeedCargo` (another player's ground forces in the space area: the string announces them with the system id in the place of a planet, no typed event carries that) |
| Ghosts commander Sai Seravus | `cc_ground/ghost/timing.rs` | `combat_live::sai_seravus_places_fighters_for_the_ships_that_moved_through_a_wormhole_in_lockstep` |
| Empyrean commander Xuange, agent Acamar | `ce_movement/empyrean/timing.rs` | `the_empyrean_commander_returns_its_token_when_another_player_moves_in_lockstep`, `acamar_gives_the_mover_a_command_token_in_lockstep` |

Registry order: `the_native_registry_equals_the_string_one_for_the_ported_abilities` has pairs for every new rule (Arborec, Titans, Yssaril with L1Z1X, Nekro with Arborec, Ghost/Empyrean/Muaat/Nomad).

## The wider silent-gap class: triage of the string abilities without a core rule

The audit (`ability_audit.rs`, all events and relations, 24 lobby factions plus a Yssaril third seat) lists every string ability the core table lacks; the test FAILS for an unlisted one and for a listed one that gets a rule. Now:

* 96 are `Dropped` (Last Bastion, Crimson, Keleres, Deepwrought, Firmament, Obsidian, Ral Nel, Thunder's Edge breakthroughs, the Hydrothermal/Nanomachines/Radical/Subatomic/Proxima/Parasite technologies of dropped factions).
* Ported by someone: the 14 abilities above (this lane), lane G (Gift of Prescience, Mitosis, Infantry II / Letani / Crimson Legionnaire returns, Scavenge, Pillage), lane M6c (Aether Passage, Aetherstream, Crucible, Neuroglaive, Taivra, Icarus Drive, Mendosa, Edict, T'ro and their Ssruu copies). `technology:*:tcs:AGENT_EXHAUSTED` stays `Lane("core-m1-trade")`.
* `Carried`: `reclamation_tactical` and `sardakkagent_mark` (marks the windows read).
* LOUD GAPS left (43 rows of `GAPS`; a native game stops with `Unknown::AbilityGap(slot)` where a seat holds the card), TACTICAL-RELATED first, each not done for size/time of this lane:
  * movement: Naalu Foresight (relocation), Voidwatch (`vw`, the promissory note given on arrival), Stymie (`SHIP_MOVED`, a command token placed on the mover's system);
  * activation/ship windows: Naalu agent Z'eu (`naaluagent-te`, reacts to every `COMMAND_TOKEN_PLACED` the game emits or stages: the native token placements would have to announce all of them), Nekro flagship (`SPACE_COMBAT_STARTED`), Nekro C4Y;
  * combat: Mentak commander S'ula Mentarion (`SPACE_COMBAT_WON`: promissory note take with `may_receive` and Support for the Throne), Sol commander, Exotrireme II (`exo2`), Magen Defense hit (`md_hit`), Tekklar x3, Ambuscade, Strike Wing Alpha II (`swa2`), Argent commander, Ragh's Call (`UNITS_COMMITTED`), Saar mech (`PLANET_CONTROL_GAINED`);
  * action completed: Aetherstream clear (`asn_clear`), Winnu mech, Winnu Reclamation family and agent (+ the Ssruu copy);
  * not tactical (status/strategy/production/turn windows, lanes C/E/F/G): Sardakk agent/hero turn starts, Naaz agent/commander (+ copy), Naalu hero and mech, Nekro commander, Muaat commander x2, Titans commander, HTP, Yin Spinner Omega, Bioplasmosis, IFF, Spy Net, Scepter, Mentak agent (+ copy), Cabal agent, Propagation.
* "Unreachable" (none found): every listed gap fires for a seat that can hold the card in an agreed game.

## Findings left for other lanes

* `combat_live::a_space_fight_of_each_faction_...` fails for Argent and Nekro with `materializes: unknown text code` in the `Transient` slice (`presentation.phase`): a STATE text minted by `combat_texts` into the position's table while the harness' renderer table (continuing it, `Render::with_codes`, lane M7) is the one handed back. It is in the merge base, not in a rule of this lane; the other 22 factions pass. `native_play --shadow` has the same shape of risk wherever a state code is minted inside a step.
* The M6c sweeps (`tactical_live::a_tactical_action_of_each_faction...`, `combat_live` faction sweeps) bail loudly for the factions whose listed gaps still fire inside a tactical action (Naalu: Z'eu; Mentak: S'ula Mentarion; Sol: commander). That is the intended behaviour of the gap table; the tests need those entries ported, not filtered.
* `tactical_live` harness: carries a staged destruction's cause into the position's texts (`adopt_staged_texts`), ignores the string's turn-flow fields when a script runs the action out, and prints the prompts when the question counts differ.

## Checks

See the final report of the lane (numbers of the last run). Private `CARGO_TARGET_DIR=/root/deploytmp/core-m6d`, `CARGO_BUILD_JOBS=2`, no debug info, incremental removed after every build.
Not run (standing rule): golden/wide dual sweeps, benches, soak, the mutation ratchet, census, workspace clippy, `native_play.sh --shadow` (disk; no zero-bail playthrough exists: Z'eu and the other gaps stop every Naalu/Mentak/Sol game).

## Mutation checks

Hand-picked mutants, single file each, runner agent on the committed snapshot d8dea9a3 in a scratch worktree (list: this section is filled in by the lane's final report).
