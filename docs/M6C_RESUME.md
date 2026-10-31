# D2 lane M6c (`core-m6c-tactical`): closing the tactical island, resume note (2026-10-31)

Branch `core-m6c-tactical` cut from `core-m6b-tactical` (7a3601bf). Never pushed; the live branch is untouched. Read `docs/M6B_RESUME.md` first (its "Not done" list is this lane's task list).

## Done so far

| item | where | evidence |
|---|---|---|
| Promise of Protection, Aether Passage, Aetherstream, Crucible, Neuroglaive, Emissary Taivra (Ghosts agent) and its Ssruu copy, Icarus Drive (Ghosts mech), Captain Mendosa and its Ssruu copy | `ti4-engine-core-factions/src/activation.rs` registers them in the string's module order; the rules are in `cc_ground/{mentak,ghost}/timing.rs`, `ce_movement/{empyrean,saar}/timing.rs`, `cb_economy/cabal/timing.rs`, `cd_registry/naalu/timing.rs`; new typed asks `Ask::{IcarusDrive, AgentShip, Edict}`, `Opt::{Token, Boost, Opponent}`, `Subtype::{IcarusDrive, AgentShip, Edict}` (`choice/reaction.rs`) with their renderer arms (`render/reaction.rs`) | lockstep tests in `tactical_live.rs` (string and native side by side: questions, options, details, context, events, position, rng); registry order test `the_native_registry_equals_the_string_one_for_the_ported_abilities` extended with the pairs |
| `query::reach::{neighbours_for, adjacent_to_anomaly}` | `ti4-engine-core/src/query/reach.rs` | the typed `PlayerAdjacency::neighbours` (extra wormholes, links, blocked borders); used by Aetherstream |
| Mahact residual 1 ("8 more questions"): NOT a Mahact difference | `combat_live.rs` sweep | the string asks the END-OF-TURN windows (Predictive Intelligence x7, Bio-Stims) after the action: the Mahact is the only sweep faction that starts with both. The sweep position drops the two technologies; the windows are lane B's `TurnEnd`. |
| Mahact residual 2 (fleet limit 4 vs 3): the core table lacked **Edict** | `cc_ground/mahact/timing.rs` (`EDICT_SPACE`, `EDICT_GROUND`), `driver/windows/aftermath.rs` (the enforcement limits are taken after the win is announced) | `a_space_fight_of_each_faction...` with NO Mahact filter (it failed 4 vs 3 and 24 vs 16 questions before); `edict_tests` |
| `open_arm(Arm::Tactical)` exercised | `ti4-engine-core-bridge/src/native_subject.rs` test `the_tactical_arm_opens_the_observed_tactical_window_and_nothing_else` | |
| The Saar sweep entries (`saar v ...`) no longer filtered | `combat_live.rs` | |

## Finding: the silent-gap class is wider than SYSTEM_ACTIVATED

`Edict` (an ability of `SPACE_COMBAT_WON` / `GROUND_COMBAT_ENDED`) was missing from the core table and nothing bailed. A probe over EVERY event (two-seat lobbies and one with a Yssaril third seat) lists 170 string abilities without a core slot (dropped content included). The in-scope remainder is below under "Remaining gaps" (the list is the work of the lanes that own those windows). The SYSTEM_ACTIVATED audit (`every_system_activated_ability_the_string_registers_is_ported_or_accounted_for`) now also seats a Yssaril third seat, which found two hidden gaps (the Ssruu copies of the Ghosts agent (now ported) and of the L1Z1X agent).

## Also done

| item | where | evidence |
|---|---|---|
| T'ro (Sardakk agent) and its Ssruu copy at the end of the action | `cd_registry/sardakk/timing.rs` (`TRO`, `TRO_COPY`; the id says `ACTION_COMPLETED` although the event is `TACTICAL_ACTION_ENDED`: `reactions_core::event_segment`); the tail no longer stops for T'ro | `combat_live::tro_places_two_infantry_at_the_end_of_the_action_in_lockstep` (agent, Ssruu copy, Sardakk as the active seat) |
| The tail's strategy continuation is ONE seam | `tactical/tail.rs::resume_strategy` (Warfare follower window, hero primary): still the counted gaps `WarfareSecondary` / `LeaderStrategy`; lane E replaces the body of that function only | the Naaz/Winnu hero primaries (W13/W14) run from their own windows and never come through here |
| `combat_live` compares the position without the string's `#seen:` rows | `combat_live.rs::leaves` | a map left with none of its own rows is no map |
| The audit seats a Yssaril third seat | `reactions_core.rs` | found the Ssruu copies of the Ghosts agent (ported) and of the L1Z1X agent (listed) |

## Remaining gaps, with reasons

In scope, NOT done (each is a `Status::Gap` row of the audit with its reason, so a native activation that would offer it is an honest listed gap, and a NEW gap fails the audit):

* Titans Awaken and the flagship Ouranos: the questions come first, the units from the supply (`supply::allowed`), Coalescence is an activation mark; Ouranos also serves the Nekro flagship (Z token).
* Arborec commander (Dirzuga Rophal) and flagship (Duha Menaimon): both run a PRODUCTION window as a child of the ability (`produce(..)`); the production composer is not runnable from inside a table rule yet.
* Yssaril commander So Ata: a `CardsReveal` mark plus the end-of-action clear (`yssarilcommander_clear`, an ACTION_COMPLETED ability: lane B's finish).
* The Ssruu copy of the L1Z1X agent (the string asks the BENEFICIARY which infantry to replace).
* Field Marshal Mercer (Nomad) and its Ssruu copy at the end of the action: `nomad_agents::mercer_plan` (two questions per take, ground forces from anywhere to a planet of the active system); the tail stops with `TacticalGap::SsruuLends` (now Mercer only).
* Nova Seed (`TacticalGap::NovaSeed`): the Muaat hero Adjudicator Ba'al is the string ability `leader:*:muaathero:MOVEMENT_FINISHED:after` (+ `muaathero_moved:SHIP_MOVED`), which is NOT in the core table (the gap only triggers when a map edit mark appears, which cannot happen natively). Needs: the core `Galaxy` tile replace (hex of the old tile to the new one), the contents move (units in space, command tokens, frontier token, Creuss tokens returned, ion storm purged, planets discarded), a `MapEdit` mark and the destruction of others' units. Not started.
* Warfare follower window / hero strategy primary after a free tactical action: the seam above; the follower composer is lane E's.

## The wider silent-gap class (for the integration agent)

The probe over every event (not only `SYSTEM_ACTIVATED`) listed 170 string-registered abilities with no core slot (two-seat lobbies plus a Yssaril third seat). Dropped content aside, the in-scope ones the native windows would miss silently are (`kind:*:card:EVENT:rel`):

`ability:*:{foresight:MOVEMENT_FINISHED, miniaturization:TACTICAL_ACTION_ENDED, mitosis:STATUS_PHASE_BEGAN, phoenixstandard:{GROUND,SPACE}_COMBAT_ENDED, pillage:{TRADE_GOODS_GAINED,TRANSACTION_RESOLVED}, propagation, reclamation*, scavenge:PLANET_CONTROL_GAINED, sundered:MOVEMENT_FINISHED, terragenesis:PLANET_EXPLORED, council_patronage, laws_order:TURN_BEGAN}`; `leader:*:{cabalagent:COMMODITIES_REPLENISHED, empyreanagent:MOVEMENT_FINISHED, ghostcommander:*, mentakagent:PILLAGE_USED, mentakcommander:SPACE_COMBAT_WON, muaatcommander*, muaathero*, naaluagent-te, naazagent:TURN_PASSED, naazcommander:PLANET_CONTROL_GAINED, nekrocommander:TECHNOLOGY_GAINED, nomadagentmercer, solcommander:GROUND_COMBAT_STARTED, titanscommander:PRODUCTION_USED, winnuagent:PRODUCTION_USED, empyreancommander:SHIP_MOVED}`; `promissory:*:{ambuscade:UNIT_ABILITY_ROLLED, blackops, gift*, iff:TURN_BEGAN, malevolency, ragh:UNITS_COMMITTED, raisethestandard, scepter, sever_return, share_knowledge_return, spynet:TURN_BEGAN, stymie:SHIP_MOVED, tekklar*}`; `technology:*:{asn_clear, bio:STATUS_PHASE_ENDED, exo2:SPACE_COMBAT_ROUND_ENDED, htp:PRODUCTION_USED, hydrothermal, inf2_return, md_hit:GROUND_COMBAT_STARTED, nanomachines, nekroc4y:SHIP_DESTROYED, proxima, radical, subatomic:SHIP_DESTROYED, swa2:UNIT_ABILITY_ROLLED, tcs:AGENT_EXHAUSTED, vw:MOVEMENT_FINISHED, yso:UNITS_PRODUCED}`; `unit:*:{arborec_infantry2_return, mahact_infantry2_return, nekro_flagship:SPACE_COMBAT_STARTED, saar_mech:PLANET_CONTROL_GAINED, winnu_mech:ACTION_COMPLETED}`. Several are ported elsewhere under other ids or fire in windows the core does not own yet (status/strategy phases are lanes C/E); this list is the CHEAP audit generalised, and the right next step is the same test over all events with every row `Carried/Dropped/Gap`.

## Mutation checks of `hooks.rs`, `routes.rs`, `tail.rs` (M6b skipped them)

Hand-picked mutants (single file each; 17 in the first round: negated conditions, a limit, a dropped clear), the union of the core tests (`ti4-engine-core-tests --test tactical_composer --test tactical --test movement --test fx_move`) and the engine lockstep tests (`tactical_live`, `combat_live`, `invasion_live`, aftermath). First round: 6 killed by the lockstep (h1 h2 h4 t3 t6) or the core tests (h6, h5, t5), 11 survived. New tests (all in `ti4-engine-core-tests/tests/tactical_composer.rs`): the T'ro mark for a readied or exhausted agent only, the conduit links cleared by a turn-ending hook and by the tail, Mercer's gap exactly with a planet and a ground force (and the end of the action announced to an exhausted T'ro only through a readied Ssruu), and `ship_routes` on a hub (default first, five routes, Gravity Drive / Ionian boosts as the safe way round a rift, Circlet of the Void rolls no rift). Second round on the survivors: all killed except `r2` (`path.len() > 2` against `>= 2` in `describe`: an EQUIVALENT mutant, `path[1..1]` is empty), and `r1` (`MAX_ROUTES` 5 to 4), killed after the test pinned the literal 5. The script is not committed (`/tmp` scratch); the mutants are in the text above.

## Coordinator decision applied

Only the latest RULES_VERSION is supported: Captain Mendosa's legacy list-position mark was dropped from the new rule (`saar/timing.rs` writes the by-kind mark only). The pre-existing `agent_move_bonus` reader in `saar/movement.rs` still has the old branch (not mine, left).

## Checks

Private `CARGO_TARGET_DIR=/root/deploytmp/core-m6c` (kept under 2.3 GB, deleted at the end), `CARGO_BUILD_JOBS=2`, no incremental.

* `cargo test -p ti4-engine --lib`: 5015 passed, 4 ignored (255 s). `TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1`: 5015 passed (581 s). (The first whole run found one failure of mine, a condition reading the galaxy of a position lent without its map: `live::plays::tests::a_card_played_inside_a_live_window_...`, fixed.)
* `-p ti4-engine-core --lib` 469, `-p ti4-engine-core-factions --lib` 129, `-p ti4-engine-core-bridge --lib` (73, incl. native_subject) and `--test kernel_env`, `-p ti4-engine-core-tests --test tactical_composer --test aftermath_chain --test invasion_composer --test exploring --test tactical`: all green. `islands` is a lib module of the bridge (inside the 73).
* `check_layers.py core`: 482 files, 0 violations; `lint_core.sh`: 484 files, 0 hits.
* Not run (standing rule: no zero-bail playthrough exists): golden/wide dual sweeps, benches, soak, the mutation ratchet, census, workspace clippy, `playthrough_quick.sh`, the full D4 crate (`ti4-engine-core-tests` as a whole: disk).

