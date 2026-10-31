# D2 lane G2 (`core-g2-abilities`): the loud gap rows of lane G turned into core rules, part 1, resume note (2026-10-31)

Branch `core-g2-abilities`, cut from `core-g-silent` (`1ef52a35`). Never pushed; the live branch is untouched. Read `docs/G_SILENT_RESUME.md` first (the rows, the audit, the recipe). Lane G3 (`core-g3-abilities`) does the rest of the rows,
lane M6d the tactical remainder. Slots `42_000..` are this lane's (G3 should take another range).

## Native now (19 of the 61 rows deleted: 61 -> 42 rows in `silent::gaps::GAPS`)

| ability (string id) | core rule | slot | question | lockstep with the string | core test |
|---|---|---|---|---|---|
| Winnu `reclamation_mark`, `reclamation_clear`, `reclamation` | `silent/winnu.rs` | 42_001..42_003 | none (optional window) | `silent_live_tests::{reclamation_*, the_turn_marks_of_the_winnu_*}` (end of an action, turn prelude), `combat_live::the_winnu_note_the_planet_they_take_*` (an invasion writes the note) | `g2_abilities` (marks, clear, the pair of structures, room, box) |
| Winnu mech Reclaimer `winnu_mech` | same | 42_004 | `Ask::G(Reclaimer)` (`reclaimer`) | `silent_live_tests::the_reclaimer_*` | yes |
| Winnu `htp` (Hegemonic Trade Policy) | same | 42_005 | `Ask::G(TradePolicy)` (`htp_planet`) | `combat_live::the_winnu_abilities_of_the_production_window_*` (a tactical action to the production step) | yes |
| Winnu agent `winnuagent` and Ssruu's copy | same | 42_006, 42_007 (pair) | none | same combat_live test (agent, htp and the copy) | yes |
| Muaat commander Magmus and its unlock | `silent/muaat.rs` | 42_020, 42_021 | none | none: no native emitter of `STRATEGY_TOKEN_SPENT` / a war sun in a reachable flow of the harness | yes (`g2_abilities`) |
| Ghost commander Sai Seravus (`ghostcommander` x2) | `silent/ghost.rs` | 42_030, 42_031 | `Ask::G(CommanderFighters)` | `combat_live::sai_seravus_*` (a wormhole route, the whole action) | yes |
| Empyrean agent Acamar, Voidwatch, commander Xuange | `silent/empyrean.rs` | 42_040..42_042 | the pool question (the strategy area's own), `Ask::G(Voidwatch)` | `combat_live::the_empyrean_abilities_*` (a tactical action each) | yes (Voidwatch with the note order of the plays facts) |
| Cabal agent The Stillness of Stars | `silent/cabal.rs` | 42_050 | `Ask::G(Stillness)` | none: the replenish is staged by the Trade strategic action (lane E), no harness arm | yes |
| Sol commander Claire Gibson | `silent/sol.rs` | 42_060 | none | `combat_live::claire_gibson_*` | yes |
| Naaz agent Garv and Gunn, Ssruu's copy, commander Dart and Tai | `silent/naaz.rs` | 42_070..42_072 | `Ask::G(NaazAgent / NaazCopy)`; the exploration is the explore area's own window `Exploring::planet(.., EDeck::Choose, announce = false)` stepped on the tape (`Child` + `sx.ask`) | `silent_live_tests::{garv_and_gunn_*, ssruus_copy_of_garv_*}` (end of a turn), `combat_live::dart_and_tai_*` (an invasion) | eligibility tests (decks, trait, who is offered) |

The questions are typed in `choice/silent.rs` (`Ask::{Reclaimer, TradePolicy, CommanderFighters, Voidwatch, Stillness, NaazAgent, NaazCopy}`, `Opt::{Reclaimer, Swap, Fighters, Note, Support, Capture, Explore}`) and rendered byte for byte in `render/silent.rs`
(Naaz's copy names the planet by its name, the agent by its id, as the strings do). `kernel::ability::G2Codes` carries the interned texts (`winnu_mech`, `muaat`, `cabal_agent`).

## Findings

* The string asks these questions WITHOUT the trigger in the context (the questions of Reclaimer, Hegemonic Trade Policy, Sai Seravus, Garv and Gunn): `Ask::carries_trigger` is false for them. `turn_live_tests::assert_same` compares only actor, source and subtype of the
  context, so the stricter `assert_same_contexts` of `silent_live_tests` compares the whole context (it found the Reclaimer and Naaz triggers).
* The live tactical and invasion drivers did not carry the initiative order: a window offered abilities of the ACTIVE seat only (a Voidwatch or a Xuange of another seat was never offered). The same hunks as lane M6b's commit `69eab360`
  (`live_common::driver_of` sets the initiative order; `tactical/live.rs` and `invasion/live.rs` call it) are in this branch, so the merge is clean.
* Registration order inside a window is the string's module order (alphabetical, then the technologies, then the acquired commanders): Naaz's Dart and Tai before Saar's Scavenge, Xuange after Sai Seravus, Claire Gibson before Xuange. `silent::register_abilities`
  documents the order; `reactions_core`'s registry test has 12 more seat pairs (winnu, muaat, naaz + yssaril, ghost).
* Muaat commander and Magmus's unlock, Naaz's agent: the effect repeats no guard of its condition any more (a guard in both places makes the mutants of either equivalent).
* The `downstream` rows (`reclamation`, `reclamation_clear`) are gone with their upstream.

## Remaining rows of the share (5), and why

* `leader:*:argentcommander`, `promissory:*:ambuscade`, `technology:*:swa2` (Strike Wing): all three read the STAGED roll of a unit ability (`transient.combat.reroll_staging`: one entry per rolling unit with `unit_types`, the planet and the faces; the extra die is appended to
  an entry, Alpha II counts the 9s of the `argent_destroyer2` entries). The core mirrors write that set without the unit: `rules/combat/barrage.rs::mirror` and `space_cannon_offense.rs::mirror` leave `unit_types` empty and the space cannon entries carry no unit at all
  (`Entry { hits_on, faces }`; the entry's `unit` is the roll kind). The port needs those two mirrors (and the bombardment's) to stage the unit per entry first: lanes M4/W15's files, so left loud (a seat holding the commander, the technology or the note stops the
  step with `AbilityGap`). Not started beyond reading.
* `leader:*:muaathero`, `muaathero_moved` (Adjudicator Ba'al, Nova Seed): the brief gives Nova Seed to lane M6d (the tile replace and the map edit). Left to M6d.

## For the merge

* `silent/gaps.rs`, `ability_gaps.rs` and `ability_audit.rs` change in the same lines as G3's and M6d's deletions: delete the rows by id and renumber (`tools/engine_core/delete_gap_rows.py` does it by id: it rewrites `GAPS`, `GAP_IDS`, the `Gap(i)` of `ACCOUNTED` and the `SUFFI_AN*` constants from the ids).
  After a merge run `cargo test -p ti4-engine --lib ability_audit`: it names what is out of step.
* `reactions_core.rs`: 12 `NAMED` lines and 2 `PAIRS` lines (additive), `set_table_codes` one line (`g2`).
* New: `silent/{winnu,muaat,ghost,empyrean,cabal,sol,naaz}.rs`, `ti4-engine-core-tests/tests/g2_abilities.rs`; tests appended at the end of `silent_live_tests.rs` and `combat_live.rs`.

## Checks (cheap), private target `/root/deploytmp/core-g2` (no incremental; deleted at the end), `CARGO_BUILD_JOBS=2`

* `cargo test -p ti4-engine --lib` (default areas): 5039 passed, 3 failed, 4 ignored. `TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1`: the same 5039 passed, the same 3 failed. The 3 are the bail-as-failure scenarios that meet OTHER lanes' loud rows
  (`combat_live::{an_undefended_system_with_a_space_dock_goes_on_to_the_production_step, a_space_fight_of_each_faction_runs_native_without_a_bail}`, `tactical_live::a_tactical_action_of_each_faction_runs_native_without_a_bail`:
  `AbilityGap` of `naaluagent-te` (G3), Awaken, the Arborec flagship and Mercer (M6d)); they fail before this branch too and pass when those rows are ported.
* `-p ti4-engine-core-tests --test g2_abilities`: 25 passed. `-p ti4-engine-core-bridge --lib` 95 and `--test kernel_env` 3: ok. `ability_audit` (3) and `reactions_core` (12, with 12 more seat pairs in the registry test): ok.
* `tools/engine_core/check_layers.py core`: 499 files, 0 violations; `lint_core.sh`: 501 files, 0 hits.
* Mutation (`tools/engine_core/mutants.sh` on the whole `silent/` directory minus lane G's files, `D4_COVERS_FILE=/dev/null`, union of two runs: the engine lockstep + registry tests (`--lib` with the names of the lockstep tests and
  `the_native_registry_equals`) and the core tests (`g2_abilities`)): 297 mutation points, 218 killed by the lockstep run alone and 257 by the core run alone; first union 9 survivors, all dealt with (a test for the ships-moved count, the decks present, the two
  pairs of Ssruu's copies registered for the seats that hold both cards; the rest named equivalent with `// mutants:skip (why)`: a guard the condition holds, an infantry/fighter box that has no limit, a capacity no ship prints, the dropped Last Bastion's dock,
  a bound). Re-run of the core half after the additions: 290 points, 0 survivors of the union. Earlier rounds found: the Voidwatch hand (Alliance and Support, the ordering), the Naaz offers (which seat, which deck), Cabal's exact list, Sai Seravus's capacity and the
  empty-box paths, the contexts of every question.
* Not run (standing rule): golden/wide dual sweeps, benches, soak, the mutation ratchet, census, workspace clippy, `playthrough`; the shadow (`native_play.sh --shadow`).
* Not covered by a lockstep (core tests only, no flow of the harness reaches the event): Magmus (`STRATEGY_TOKEN_SPENT`) and its unlock, The Stillness of Stars (the replenish is staged by the Trade strategic action: lane E), the Voidwatch note pick beyond the first
  option, Garv and Gunn's explorations beyond one planet.
