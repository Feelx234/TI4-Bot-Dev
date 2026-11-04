# core-integration-20: merge record and shadow round 6 (2026-11-04)

Branch `core-integration-20`, cut from `core-integration-19` @ `fc910308d`. One merge (the silent-gap fixes), one new string bug (S5) found by the merge.
Not pushed; the live branch `nightly-fixes-2026-10-26` and lane D (`core-d28d-wrapper`) were not touched.
`RULES_VERSION` stays 35; serde, fixtures, wire TS unchanged. Quick game: 1,165 steps, fingerprint `74d5675ac754ac81` (unchanged: the silent fixes do not move it).

Read first: `docs/INTEGRATION19.md`, `docs/I19_RESUME.md`, `docs/SILENT_GAPS_18.md`, `docs/SILENT_FIX_RESUME.md`, `docs/STRING_ENGINE_BUGS.md` (S5 is new).

## 1. Merge

| # | Branch | Tip | What it brings | Conflicts |
|---|---|---|---|---|
| 1 | `core-silent-fix-18` (contains `core-silent-gaps-18` `a8e420eab`) | `0cac7be7c` | X-89 (`holds_x89` reads the deck printing; `bomb_window` sets `Bombardment::x89`), B-X01 (scoring unlocks after every score), Sh'val skip (`general.skip_to_commit`, `SardakkSkip`, literals in BOTH `render/leaders.rs`), Nomad / Mentak seam sustain, `silent_census` tests, `SILENT_GAPS_18.md`, bail-map tsv and scripts | **none** (textual). Both `render/leaders.rs` copies (`ti4-engine-legacy` at `crates/ti4-engine`, and the thin `ti4-engine` at `crates/ti4-engine-next`) merged with the same Sh'val literal `sardakk_hero_skip`: both kept. |

`cargo check --workspace --all-targets` (no torch crates) is green after the merge and at the end.

## 2. What the merge exposed: string bug S5 (B-X01)

After the merge three bridge tests failed (`a_wormhole_token_the_game_places_joins_the_map_of_the_core` yin 4p s4 step 892, `the_naalu_mech_is_..._known_s1` cabal 6p s4 step 729, `the_nomad_mech_sustains_...` nomad 3p s5 step 585).
All three: the core now unlocks a hero / commander at the first score of a secret objective (B-X01), the string does not.

* Cause: the silent note assumed the string's `award_tail` always runs. It does not for this path: `objectives::ScoringWindow::select` sends a secret in hand to `secrets::award`, which runs no unlock check; only `objectives::award` (public / revealed) calls `award_tail`. The string unlocks at its next check (next turn start / status bookkeeping, where it announces `LEADER_UNLOCKED`).
* Who is right: the core (51.7: a leader unlocks the moment its condition is met; `award_tail`'s own comment says a hero "must not wait for a status phase the game may never reach").
* Fix of the string tried and **not kept**: one `check_unlocks` line after `secrets::award` made the three games pass, but it moves recorded legacy games (`golden::tests::the_smoke_subset_reproduces` failed: g020 step 795, state differs from the recording) and the string is frozen for replay. Reverted.
* Recorded as **S5** in `docs/STRING_ENGINE_BUGS.md`. Harness: `KNOWN_S5`, `s5_unlock_differs` (two forms: the string announces `LEADER_UNLOCKED:..` where the core does not; the leader's state is `unlocked` in the core and `locked` in the string), applied at the three places that build an events difference (`owed_events_difference` for the two owed-event sites) and at the state difference. `bin/native_play` message says S1 to S5.
* Tests: legacy `status_live_tests::a_secret_scored_for_the_first_time_leaves_the_hero_locked_in_the_string_s5` pins the difference (string `locked`, core `unlocked`; fails if the string is changed, and before the merge the core side would be `locked`); bridge `s5_stops_a_game_the_guards_of_three_older_tests_ran_through` (the three games stop at S5, recogniser accepts the two forms and rejects the reverse direction, two different unlocks, other paths). The core side is guarded by the silent branch's own `status_phase::..._secret_scored_for_the_first_time`.
* **Cost, to be decided by the user (question Q2):** S5 is a stop of the shadow, which cannot play on. 43 of the 768 gate games (5.6%) now stop at S5 (they were clean in int. 19), so the shadow's reach in those games ends early. In three older bridge tests the guarded step lies past the S5 stop (yin 4p s4 step 943, cabal 6p s4 step 943, nomad 3p s5 step 832); their assertion for those entries is now "no difference, or the known S5" (the other entries stay strict). The guarded fixes keep their unit tests (thin `live::kernel::tests::the_live_kernel_lets_a_nomad_mech_...`, legacy combat texts, the core `tactical`/`wormhole` tests), only the whole-game reach is lost. Options: (a) fix the string behind a `RULES_VERSION` (one line, tested above) and remove S5, restoring full reach; (b) keep as is. Default: (b).

## 3. Matrices (final binary, `gate` profile)

### 3.1 Plain (the core alone)

| run | games | finished, zero gaps | native steps | notes |
|---|---|---|---|---|
| `--quick` | 1 | 1 | 1,165 | fingerprint `74d5675ac754ac81` |
| 96-set | 96 | **96** | 102,992 | = int. 19 |
| 288-set | 288 | **288** | 298,270 | = int. 19 |

### 3.2 Z-gate (`--factions all --players 3,4,5,6 --seeds 8 --shadow`, four processes by `--players`, two at a time): **all four exit 0**

| set | games | finished, zero gaps | whole game, no difference | difference of the core | known string bugs |
|---|---|---|---|---|---|
| seed 1 | 96 | 96 | 78 | **0** | 18 (12 S1, 1 S2, 5 S5) |
| seeds 2 to 4 | 288 | 288 | 230 | **0** | 58 (26 S1, 14 S2, 18 S5) |
| seeds 5 to 8 | 384 | 384 | 308 | **0** | 76 (35 S1, 19 S2, 1 S3, 1 S4, 20 S5) |
| **seeds 1 to 8** | **768** | **768** | **616** | **0** | **152** (73 S1, 34 S2, 1 S3, 1 S4, **43 S5**) |

S1 to S4 are identical to int. 19 (73 / 34 / 1 / 1); the 43 games that went from clean (659) to stopped (616) are exactly the S5 games. Per `--players`: 3p 20, 4p 39, 5p 34, 6p 59 known stops. No gap, no unknown difference.

## 4. Cheap checks (private target `/root/deploytmp/core-i20`, profile `lockstep`, `CARGO_BUILD_JOBS=2`)

| Check | Result |
|---|---|
| `cargo check --workspace --all-targets` (no torch crates) | ok (after merge, after the harness edit, at the end) |
| core-tests binaries: `silent_census` 4, `seam_only_providers` 3, `invasion_composer` 18 (x89), `action_dispatch` 75, `status_phase` 33, `tactical_composer` 22 (Sh'val), `hero_windows` 67, `kernel_hooks` 5, `seams` 15, `query_tech` 31, `action_card_effects` 26, `action_cards` 11, `aftermath_chain` 24, `combat_machine` 27, `resolver_basics` 7 | all passed |
| `ti4-engine-core --lib`, `-core-factions --lib` | 476, 153 passed |
| thin `ti4-engine --lib` + `wire_parity`, `ties_parity`, `surface_parity` | 364 (+1 ignored); 11, 6, 3 passed |
| `ti4-engine-legacy --lib` | 5300 passed (int. 19: 5299; +1 the S5 pin), 7 ignored |
| bridge `--lib` and `--test host_create_golden` (in the form int. 19 left: 302 TE rows refused, golden file untouched) | 141 passed (int. 19: 140; +1 the S5 test); `host_create_golden` ok, golden smoke subset (`golden::tests::the_smoke_subset_reproduces`) ok |

Not run (user rule): heavy gates (golden / wide dual, benches, soak, census, ratchet, workspace clippy), mutation batches, `TI4_CORE_STRICT=1`, torch crates.
