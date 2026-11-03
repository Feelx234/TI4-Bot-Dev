# core-integration-18: merge record (2026-11-03)

Branch `core-integration-18`, cut from `core-integration-17` @ `d61ef6b08`. Four merges, one at a time, in the ordered list: lane R (`core-r-shadow3`, the whole-game shadow differences, round 3) and the D2-8 lanes A (`core-d28a-types`), B + B2
(`core-d28b2-surface`, contains `core-d28b-surface`) and C (`core-d28c-setup`). Not pushed. The live branch `nightly-fixes-2026-10-26` was not touched. Pure refactor except for what is named in sections 2 and 3: `RULES_VERSION`, serde, fixtures,
wire TS and the fingerprints of the string engine are unchanged (no file of those was edited). The `tests/host_create.golden` of lane C (1,405 digests, recorded at the legacy setup) was NOT re-recorded and passes.

Read first: `docs/INTEGRATION17.md`, `docs/INTEGRATION16.md`, `docs/R_RESUME.md`, `docs/D28A_RESUME.md`, `docs/D28B_RESUME.md`, `docs/D28B2_RESUME.md`, `docs/D28C_RESUME.md`, `docs/todo/D2-8-wrapper.md` (6.2: file ownership).

## 1. Merges

| # | Branch | Tip merged | Merge commit | What it brings | Files in conflict |
|---|---|---|---|---|---|
| 1 | `core-r-shadow3` (R, cut from P's tip `f800ceb2f`) | `92e78d186` | `4764c3c00` | shadow harness fixes, the activation marks as registered abilities (`hooks::marks` deleted), Harness Energy x Trade Agreement, the extra die is a combat die, the hit flush, the Technology follower gate, S1 outside the action phase / S2 recognition, step-start glue; `docs/R_RESUME.md`, S1/S2 in `docs/STRING_ENGINE_BUGS.md` | 2 (`driver/kernel_env.rs`, `native_play.rs`) |
| 2 | `core-d28a-types` (A, cut from `core-d27-split`) | `f2ef551e3` | `edbac70d7` | the wire types, the glue copies (`core_sync`, `kernel_sync`, `live`, `render`, `product`, `seen_load`), `ties/`, `Observed` (part), `tests/{wire_parity,ties_parity,observed_parity}`, the bridge's `native_play` drives the thin glue | 0 |
| 3 | `core-d28b2-surface` (B + B2) | `905335b70` | `666db6458` | `kinds`, `queries/*`, shell modules, all 26 `Observed` accessors native, `tools/engine_core/surface_paths.{py,baseline.json}`, `tests/{surface_parity,surface_objectives,surface_state}` | 0 |
| 4 | `core-d28c-setup` (C) | `003391c55` | `e6b58ef33` | native `setup`, `seating`, `deck`, `opening`, `starting_tech`, `map_feasibility`, `fixtures`, `builders::*`; host call sites switched; bridge `setup_parity`, `host_create_golden` | 3 (`ti4-engine-next/{Cargo.toml, src/lib.rs}`, `Cargo.lock`: the known join points) |

After EVERY merge: `cargo check --workspace --all-targets` (without ti4-review, ti4-replayer, ti4-mlp, ti4-tensor, ti4-advisor) green and `awk '/^fn settle_run/,/^}/' crates/ti4-engine/src/dual_window.rs | grep -ci neutral` = **0**
(after R, A, B2, C and at the end). No `Out { .. }` literal or registration was dropped by a textual merge: the legacy lib (registry-equality tests, the audit test, `no_two_rules_of_the_core_table_share_a_slot`) is green (section 6), and
the silent duplicates that the textual merges DID leave are section 3.

## 2. The user ruling of 2026-11-03 (status-phase windows without an active player)

Asked by the coordinator during the integration. A window with NO active player in the status phase (a card played at `STRATEGY_CARDS_WOULD_RETURN`, mahact 6p seed 1) is offered **clockwise from the seat after the speaker, the speaker last**
(the speaker gets the biggest advantage). It replaces the initiative-order reading (LRR 1.19) that lane R had left as an open question. Strategy and agenda phases (from the speaker) and the action phase (active player first, then initiative order) are unchanged.

* `Resolver::player_order` (`kernel/resolver.rs`): `phase == Status && active_player == None && speaker == Some(..)` -> the seating order (the initiative order when there is none) starting after the speaker, the speaker last (a speaker who is not
  seated goes last, the sequence as it is). Test (fails on the old code: the initiative list `3, 0, 2, 1`): `resolver_basics::a_status_phase_window_without_an_active_player_goes_clockwise_after_the_speaker_and_the_speaker_last`.
* The string: its list for such a window is the seating order from the FIRST seat of `Game::new` whoever the speaker is; the two agree only when the speaker is the last seat clockwise. So it **stays a known string bug (S1 extension)**; entry updated in
  `docs/STRING_ENGINE_BUGS.md` (the open question is closed). The string engine was not changed.
* The shadow recognition follows: `s1_orders_differ` compares the string's seating list with the ruling's order in the status phase without an active seat (test `s1_is_recognised_only_for_a_reaction_and_only_when_the_two_orders_differ`, extended: speaker second seat differs; speaker last seat does
  not, whatever the strategy cards say). The dual adapters (`reactions_core::resolver_of`, `kernel_sync::shim::replay`) hand the core the string's list as the seating order with its last seat as speaker for that case, so the lockstep still compares the rest of
  the window (the same device lane P used for the action phase).
* **Found because of it (a core bug, fixed)**: `driver/turn/phase.rs` emitted `StrategyPhaseBegan` of the round flip (and `AgendaPhaseBegan`) with the resolver's clock still on the phase BEFORE (status). With the ruling that put the windows of the flip in "speaker last" order
  (mentak 4p seed 4 step 512, saar 4p seed 4 step 581: first differences that no S1/S2 class covered; before the ruling the stale status phase happened to offer the initiative order). The strategy and agenda phases start at the speaker, so both arms now call `sync_clock`
  first (as `run_announce` of the first round and the status arm already did). Test `native_play::the_windows_of_the_flip_into_a_strategy_phase_are_ordered_from_the_speaker_not_by_the_phase_before` (fails without the fix: unknown difference at step 512 for mentak; with it the string's own stale order shows as the known S2 at step 386).

## 3. Bugs found at the joins, and how each is guarded (every one fails on the pre-fix code)

| # | Where | What | Fix | Guard |
|---|---|---|---|---|
| B1 | **Q x R** (`production::payable` x R's `technology_secondary_affordable`) | two lanes' gates for the same string check `payment::affordable`: Q's `payable` was `available - Triad - War Machine` (still counts the LARGER face of a planet: Freelancers, Archon's Gift, Xxekir Grom's combination), R's body summed ONE face (`planet_value`) and trade goods, which is what `payment::plans` does (`planet_value_now`) | ONE gate: `payable` is now the sum of one face per spendable planet plus the goods; `technology_secondary_affordable` and the research window / Inheritance Systems share it | `production_mod2::the_research_gate_reads_one_face_of_a_planet_not_the_larger_one` (new; seen failing with the old `payable` body: `left: 6, right: 4`) |
| B2 | **R x A** (`ties/reactions_core.rs`) | the thin crate's copy of the string-side `NAMED` table was cut before R: the two mark rules (`tro_marks::MARK` slot 42019, `winnu::RECLAMATION_TACTICAL` 43008) had no name, `id_text` panics `every spec is named` in the first window of a game with a Sardakk agent or a Winnu seat (the native loop drives the thin glue) | the two rows added | `ties::reactions_core::tests::every_rule_of_the_core_table_has_a_name_and_the_two_marks_are_found_by_their_ids` (new; fails with the rows removed: `no name for slot Slot(42019)`) |
| B3 | **P/Q x A** (`ties/combat_live.rs`) | the copy of the combat texts was cut before P and Q: no space cannon gunners (`CombatTexts::guns`: the reroll question named `space_cannon`, the string `pds`), no Brother Milor fighter forms (`CombatUnported`) | P's gunner block and Q's `milor_in_play` / `forms_of` ported into the copy | bridge `native_play::tests::{a_reroll_question_of_the_space_cannon_offense_names_the_unit_type_of_the_gun, a_string_step_that_runs_through_the_removals_and_the_warfare_question_is_given_the_answers_ahead}` (existing, failed on the merged tree before the port: `reroll die 1 of space_cannon` vs `of pds`) and the new `a_fight_with_brother_milor_in_play_names_the_fighters_he_lands_in_the_thin_texts` (yin 6p seed 3 past step 568; fails with `milor_in_play` disabled: `CombatUnported` at step 568) |
| B4 | **A/B2 x C** (duplicates) | lane C's `builders::*`, `seating`, `deck` and lane A/B2's `ties`, `queries`, `kinds` carried the same items twice (listed in section 4) | ONE definition each | `setup_parity::the_native_modules_name_only_native_modules_and_the_listed_joins` (a first attempt that made `builders` import from `kinds` failed it: builders must not reach into the legacy glob, so `kinds` re-exports the builders' constants) + `ties_parity`, `surface_*` |
| B5 | **B x C** (`seating.rs::saar_deploys_its_printed_production_unit`) | the test asked `production::capacity`, native since B; on the bare `start_game` + `deploy` position the typed core refuses to load (`Unmapped { slice: Cards, what: "faction", text: "generic" }`) it answers the neutral 0 (documented in D28B2) | the test sums the printed production of the units at home (same value 5) | the test itself (failed `left: 0, right: 5`) |
| B6 | **ruling x core** (section 2) | stale resolver clock at the round flip | `sync_clock` | section 2 |

## 4. The duplicates of the join (ONE definition per item, lane C's kept where it owns the item)

| Item | Was defined in | Now |
|---|---|---|
| `note_id`, `alias_of`, `owner_of`, `faction_name`, `held_by`, `may_receive` | `queries::promissory` and `builders::promissory` | `builders::promissory` (C); `queries::promissory` re-exports (`holder_of` stays B2's) |
| `support` | `ties::promissory`, `builders::promissory` | `builders::promissory`; `ties` re-exports |
| `capacity_of`, `is_fracture_system` | `queries::{transit,fracture}`, `builders::support` | `builders::support`; `queries` re-exports |
| `points_for` | `queries::content`, `builders::objectives` | `builders::objectives`; `queries::content` re-exports |
| `EXPLORATION_TRAITS` | `queries::content`, `deck` | `deck` |
| `LOBBY_FACTIONS`, `MECATOL`, `MECATOL_TE` | `ties::seating` / `kinds`, `seating` | `seating` (the `ties` and `kinds` paths re-export) |
| `MODULE_ALIASES`, `is_keleres_faction`, `tag_belongs_to`, `VARIANTS` | `ties::{factions,breakthroughs}`, `queries::factions` (private copy), `builders::factions` | `builders::factions` |
| `leaders_for_faction` (private, `queries::factions`) | text copy of `builders::leaders::for_faction` | deleted; `assets` calls `builders::leaders::for_faction` |
| `SUPPORT_PREFIX`, `VICTORY_TARGET` | `kinds`, `builders::{promissory,objectives}` | `builders` (a guard of C forbids `builders` -> `kinds`); `kinds` re-exports |
| `ObjView::terms` (B2, a copy of the core's private `Seen::terms`) | -- | kept: it is not a duplicate of C (the request to the core owner of D28B2 stands) |

`Cargo.toml` of the thin crate: lane A's dependency block already contains lane C's (`ti4-model`, `ti4-content`, `thiserror`, `rand`, `serde_json`); union = A's block + B's dev-dependency `ti4-host`. `lib.rs`: A and B sections kept, lane C's module section added
(its `pub mod` / `pub use` shadow the facade glob). `Cargo.lock`: A's side, then cargo resolved. `ti4-server` `session/worker.rs` line 1015 still calls `builders::exploration::mark_legacy_device_ids`.

## 5. Edits of assertions, fixtures and harnesses (reasons)

| Where | Edit | Reason |
|---|---|---|
| `native_play` `a_native_game_plays_a_whole_game_with_zero_gaps` | none (R's pin = 1,165 steps, fingerprint `74d5675ac754ac81`, the same as int. 17) | -- |
| `native_play` `the_shadow_run_agrees_with_the_legacy_game_up_to_the_known_split_of_the_commitment_step` | replaced by R's `..._for_the_whole_quick_game` (808 legacy steps, no difference) | R's cause 6 fixed the follower window the old pin stood on; both pins were re-measured by R, not loosened |
| `native_play` `s1_is_recognised_only_...` | the status-phase half rewritten to the ruling of 2026-11-03 | section 2 |
| `seating.rs` `saar_deploys_its_printed_production_unit` | sums the printed production at home instead of asking `production::capacity` (value 5 unchanged) | B5 |
| `resolver_basics` | none (a test added) | -- |

No `native_play` pin moved; no assertion was loosened.

## 6. Checks (cheap), `CARGO_BUILD_JOBS=2`, private target `/root/deploytmp/int18` (profile `lockstep`, debug assertions + overflow checks for the three core crates, `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_INCREMENTAL=0`; deleted at the end)

| Check | Result |
|---|---|
| `cargo check --workspace --all-targets` (without the torch crates) after each merge | ok (R, A, B2, C, final); `settle_run` neutral-side grep **0** each time |
| `cargo test -p ti4-engine-legacy --lib` plain | **5295 passed**, 0 failed, 7 ignored (int. 17: 5295) |
| same with `TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1` (WHOLE lib) | 5295 passed, 0 failed, 7 ignored |
| thin `ti4-engine`: `--lib`, `wire_parity`, `ties_parity`, `observed_parity`, `surface_parity`, `surface_objectives`, `surface_state` | 361 (+1 ignored), 3, 6, 1, 11, 7, 21 passed |
| bridge `--lib`, `--test setup_parity`, `--test host_create_golden`, `kernel_env`, `play_facts`, `menu_sources` | 130, 28, 1 (golden unchanged), 3, 2, 3 passed |
| `ti4-engine-core --lib`, `-core-factions --lib`, `ti4-rule-bot-typed --lib` | 477, 151, 1 passed |
| `ti4-host`, `ti4-rule-bot`, `ti4-policy`, `ti4-server`, `ti4-wasm`, `ti4-selfplay` `--lib` | 299, 190 (+1 ignored), 287, 58, 9, 7 passed |
| the D4 crate (`ti4-engine-core-tests`, lockstep profile, `--no-fail-fast`) | 154 result lines, **1972 passed, 0 failed** (int. 17: 1962; +R's 8, +B1, +the ruling's test) |
| `check_layers.py core`, `lint_core.sh`, `surface_paths.py --check` | 574 files 0 violations; 576 files 0 hits; ok (189 paths, 43 legacy = baseline) |
| `tools/engine_core/clippy_core.sh` | ok (engine 41 = baseline 41; the thin crate and the core crates 0 warnings) |
| `cargo check -p ti4-wasm --target wasm32-unknown-unknown`; `--features ts` for thin / host / wasm; `-p ti4-engine-core-tests --features wrapper --all-targets` | ok |
| usage | five-hour window 0 % at the start, 8 % at the last reading |

Not run (user rule): `end_of_d1.sh` and its steps (golden / wide dual, `board_equivalence`, `core_sync_gate`, `alpha_roundtrip`, benches incl. the bench-hash check, soak, census, ratchet, workspace clippy beyond `clippy_core.sh`), mutation batches, `TI4_CORE_STRICT=1`,
Playwright, `cargo wire-web`, the torch crates (cannot build here).

## 7. The native playthrough and the shadow (`tools/engine_core/native_play.sh`, gate profile, `te` card set), final tree

### 7.1 The whole game, plain

| run | games | finished with zero gaps | native steps | notes |
|---|---|---|---|---|
| `--quick` (sol, 4 players, seed 1) | 1 | **1** | 1,165, 9 rounds, fingerprint `74d5675ac754ac81` | `--twice`: the same fingerprint; unchanged from int. 17 (A, B, C do not change engine behaviour; the ruling and the clock fix do not touch this game) |
| `--factions all --players 3,4,5,6 --seeds 1` | 96 | **96** | 103,015 (int. 17: 103,150) | R's changes |
| `--factions all --players 3,4,5,6 --first-seed 2 --seeds 3` | 288 | **288** | 298,315 (int. 17: 298,499) | R's changes, the clock fix moves a few steps |

### 7.2 The shadow (`--shadow`)

`--factions all --players 3,4,5,6 --seeds 1 --shadow` (96 games): **0 gaps, 78 play the whole game with no difference, 3 show a first difference of the core, 15 stop at a known string bug (14 S1, 1 S2)**. Lane R's tip: 79 / 3 / 14. The 3 differences are R's three (sol 5p step 867
(`82a` missing from Signal Jamming's list, the Mallice adjacency), jolnar 5p step 592 (`combat_presentation.phase`, cosmetic), sardakk 4p step 770 (the Scramble-Frequency-like reaction)); the one game that moved from clean to "known" is a status-phase window recognised under the
ruling of 2026-11-03 (S1 extension, section 2: the string offers the seating list from the first seat). Exit code 1 as before (differences are reported, not hidden).

`--first-seed 2 --seeds 3 --shadow` (288 games): **0 gaps, 234 play the whole game with no difference, 15 first differences of the core, 39 known string bugs (26 S1, 13 S2)**. Lane R's tip: 242 / 13 / 32 (and one gap, lane N's, since fixed). Why the clean count fell by 8: the ruling of 2026-11-03 (status-phase
windows) and the clock fix (section 2, the flip into a strategy phase is ordered from the speaker, which makes the string's stale order of the flip visible as S2) show more of the STRING's orders as known bugs, and a game cannot go on past a known bug, so the games stop there; the 15 differences are
R's 14 listed classes (xxcha 4p s2, arborec 4p s2, letnev 5p s4, yin 4p s4, sardakk 4p s2: Scramble-Frequency-like `["stay","retreat"]` question offered by the core only; winnu 4p s4, yin 6p s3, empyrean 6p s2: Ambuscade / `[cruiser, decline]`; ghost 4p s2: Mallice `82a`; xxcha 5p s4: card-play window after an
event-scoring pause; yssaril 6p s2, argent 4p s2, cabal 6p s2, cabal 6p s4: the single games) plus arborec 5p s4 (step 877: the Naalu mech's label `mech` / `naalu_mech`, the cabal 6p s4 class, which used to be hidden behind an earlier stop). No new class, none caused by A, B or C.

Per class of the 96-game set: SP / OT / RW / RD / TG / EV: none left but the three above (R's work). The work list is `docs/R_RESUME.md` "What is left, by class".

## 8. Remaining gates

1. **The shadow classes of section 7.2** are the work list of the next shadow lane (`docs/R_RESUME.md` "What is left", with the ruling's new known-S1 cases).
2. D2-8: lane D (the `Game` wrapper and the swap commit: the `pub use` of the pending list printed by `surface_paths.py`), lane F (bridge/gates: `native_play` / `oracle` still start games through the legacy setup), the remaining `ties` copies that lane D replaces;
   `ObjView::terms` waits for the core to expose `Seen::pos_for`. The thin copies of legacy glue are cut at `core-d27-split`: **any lane that changes `combat_live`, `reactions_core`, `core_sync`, `live`, `render`, ... in the legacy crate must make the same edit in the thin copy** (B2, B3 above
   were found only because the bridge runs the thin glue: the legacy lib tests do not see them).
3. Everything of INTEGRATION17 section 8 that is not touched here (Thunder's Edge corners, the Ssruu / borrowed-agent variant of the waiver, staged-event announcers, `CombatUnported` of other forms).
4. Heavy gates still to run: golden and wide dual sweeps, `board_equivalence`, `core_sync_gate --strict`, `alpha_roundtrip --strict`, the benches (bench-hash), `census`, `soak`, the mutation ratchet and the mutation batches of every resume note since integration 9 (plus N, P, Q, R, A, B2, C), workspace clippy, `TI4_CORE_STRICT=1`, `cargo wire-web`.

## 9. Tip

`git log -1 core-integration-18` (the last commit is this document's). The private target dir and the logs under `/root/deploytmp/int18*` were deleted after the last check.
