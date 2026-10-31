# D2 lane M6c (`core-m6c-tactical`): closing the tactical island, resume note (WIP, 2026-10-31)

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
