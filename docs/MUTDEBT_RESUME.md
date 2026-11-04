# MUTDEBT (core-mutdebt-20): mutation debt of lanes K and L, tests only

Branch `core-mutdebt-20`, cut from `core-integration-20` (b8e43ef8d). No production behaviour change: tests added, `// mutants:skip` with a reason
on equivalent lines, one line split in two (`card_name_key`, formatting only).

Method: `tools/engine_core/mutants.sh <file> <filter>` with `D4_COVERS_FILE=/dev/null`, the test run being the legacy engine lib
(`ti4-engine-legacy --lib`, which holds the lockstep tests; the engine files have no core-crate counterpart), JOBS=2, private target dir.
A full-lib run is ~7 min per surviving mutant under load (baseline 420 s), so the filters name the module's own tests (`plays`,
`reactions_core`, `leader render`, `invasion_live`); a survivor therefore means "the module's own tests do not see it".

| file | mutants | killed before | killed after | skipped (reason) |
|---|---|---|---|---|
| `crates/ti4-engine/src/live/plays.rs` | 32 (+1 not compilable) | 27 | 32 | none |
| `crates/ti4-engine/src/reactions_core.rs` | 150 -> 142 (+2 not compilable) | 96 | 141 | 8 points on 4 lines: `card_name_key` unwrap defaults (unreachable), `ran` of `unported` (every rule ports its effect), `next_id` of `resolver_of` (replaced by the caller's `into_state`), `reveal_payable = false` of `Follow::arm` (reached only where string and core hooks disagree), `p.commodities > 0` of `seat_more` (`agent_could_grant` reads the commodities itself); tested but the line is not counted. 1 left: see below |
| `crates/ti4-engine/src/render/leaders.rs` | 41 -> 38 (+1 not compilable) | 6 | 38 | `*f < 4` of `round_target_of` (3 points: a flag of 4+ fails the canonical-id check) |
| `crates/ti4-engine/src/invasion_live.rs` | 1 | 1 | 1 | none (the `announce: true` survivors of L_RESUME are in `driver/windows/invasion/fight.rs`, core crate, already recorded as killed by this lockstep file) |

Left alone, deliberately: `reactions_core.rs:360:109` (`s.slot == p.0` -> `!=` in the Ssruu style-C branch of `slot_of`): equivalent as long as the
first spec of the table has the same event and relation as the Saar agent's copy; not skipped because the line holds three killed mutants.

New tests: `live::plays::tests::reveal_prototype_is_payable_only_with_the_card_four_resources_and_an_open_upgrade`; in `reactions_core::tests`
`a_reaction_slot_is_one_of_the_hundred_after_the_base`, `the_printed_name_groups_and_effect_flags_...`, `an_ability_id_with_the_owner_left_out_...`,
`the_id_of_a_ssruu_copy_names_its_slot_only_with_...`, `an_eligible_ability_whose_effect_the_core_does_not_port_...`,
`a_nested_window_is_followed_with_...`, `a_nested_window_without_a_resolver_or_with_a_diplomacy_promise_...`,
`the_resolver_of_a_window_offers_the_seats_in_the_order_of_its_phase`, `the_armed_follow_...`, `arming_a_follow_...`,
`the_play_facts_of_the_string_carry_each_seat_...`; a new `render::leaders::tests` module (5 tests).

Not done (remaining from the lane docs): K_RESUME section 5 survivors in `driver/menu_cards.rs` lines 188-222 (13, "killed by the bridge" tests),
S_RESUME `native_play.rs` 498-506 (8 pre-existing chain terms) and `galaxy.rs` 266-295, L_RESUME fight.rs `announce: true` x2 (killed by the lockstep, not re-run
per mutant), I19/I20 batches ("mutation batches for lanes N..S, A, B2, C, E": heavy gate, not run, user rule). The `end_of_d1` mutation ratchet was not run.

Checks: `ti4-engine-legacy --lib` (full) passes (5318 passed, 7 ignored); no heavy gate; RULES_VERSION, serde, fixtures, fingerprints untouched (tests and comments only).
