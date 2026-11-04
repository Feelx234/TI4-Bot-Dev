# RV36 resume: RULES_VERSION 35 -> 36 (Ixthian Artifact and Parley as printed)

Branch `core-rv36`, cut from `core-botstops-1` (6b884bc84). Not pushed, not deployed. The user approved the bump for these two fixes only (only the newest
version is played: old saves are refused, nothing converts them). Cheap checks and targeted tests only; no Z-gate, ratchet or golden dual was run.

## What changed

### 1. Ixthian Artifact, For (alpha report fb_1793193205; string bug S6)

Printed card: roll 1 die; 6-10: each player may research 2 technologies; 1-5: destroy all units in Mecatol Rex's system and each player with units in the
systems adjacent to it destroys 3 of their own units in each of those systems (their choice). The string (`agenda_effects::effect_artifact`, untouched) defers
the 6-10 and destroys every ship without Sustain Damage in every system.

* `crates/ti4-engine-core/src/rules/agenda/effects/crater.rs` (new): the whole effect. 6-10: each player in speaker order gets a child `Research::agenda`
  window (two free researches, prerequisites apply, declinable, the agenda is the question's source). 1-5: everything in the Mecatol Rex system is destroyed
  (ground forces staged as destroyed); then system by system (system order) and player by player (speaker order) a player with more than 3 units there
  chooses the 3 to destroy (one question each); 3 or fewer go without a question; units that are all alike go without a question. The effect is always
  `Done::Resolved`. Mecatol is the Thunder's Edge tile when the galaxy has it, else the base tile; "adjacent" is `Galaxy::adjacent` (wormhole tokens included).
* New question: `choice::agenda::Ask::Crater { system, left }`, `Opt::Unit { planet, piece }`, `Subtype::CraterUnit` (wire subtype
  `ixthian_artifact_destroy_unit`, option ids `<planet|space>|<unit>[|damaged][|galvanized]`), rendered in both render copies
  (`ti4-engine-next/src/render/agenda.rs`, `ti4-engine/src/render/agenda.rs`: the legacy copy only had to compile against the new variants).
* `rules/production/strategy_cards.rs`: `Research::agenda(who, scope, agenda, times)` (+ `next_free`, a declinable free stage, the waiver path continues to the
  next free research); `kernel/children.rs`: `Spawn::AgendaResearch`; `driver/children.rs` starts it.
* Harness (legacy crate, behaviour untouched): `dual_window::agenda_effect` bails on Ixthian For ("known string bug S6"); the lockstep test
  `the_dice_effects_run_as_core_windows_in_lockstep` now expects 24 bails; `agenda_live_tests::sweep` skips `artifact` when the table backs For;
  `native_play::effect_bug` marks a shadow difference in a step that announced `AGENDA_EFFECT_{RESOLVED,DEFERRED}:artifact` as `known string bug S6`.

### 2. Parley (alpha report fb_1792786598; string bug S7)

The commit step lands one unit per decision; the string card returned the last unit only, so the invader landed again.

* `rules/invasion/commit.rs`: `return_to_space` (finds the piece in either of its forms, so a Naaz mech recorded in its ship form is found) and `parley`
  (returns the unit of the landing it answers and sets `transient.combat.parley_cancel`, a flag that never outlives the call).
* `state/invasion.rs`: `Invasion::step_commits` (every unit this commit step landed, by planet). `driver/windows/invasion/ops.rs`: `answer_commit` records each
  landing, and when the card flagged the landing `cancel_landings` returns everything landed on that planet in this step, drops the planet from the committed
  ones and ENDS the commit step (what landed on other planets stays and fights). A declined window leaves the next landing's window open as before.
* `ti4-engine-core-factions/src/cards/mod.rs`: the Parley card effect delegates to `commit::parley`.
* Harness: `native_play::effect_bug` marks a shadow difference in a step that answered `parley` as `known string bug S7`.

Both bugs are recorded in `docs/STRING_ENGINE_BUGS.md` (S6, S7); the version is described in `docs/RULES_VERSION.md` (Version 36). The core carries the printed
behaviour without a version branch (only the newest version exists); `RULES_CRATER_PARLEY = 36` documents it.

## Version touchpoints (grep RULES_VERSION and the refusal code)

Everything that reads the version does so through the constants (`ti4_model::state::RULES_VERSION` / `MIN_SUPPORTED_RULES_VERSION`): creation, load and recovery, replay,
the wasm resume, `native_play`, the wrapper. Changed by hand:

* `crates/ti4-model/src/state.rs`: `RULES_VERSION = 36`, new `RULES_CRATER_PARLEY = 36` (`MIN_SUPPORTED_RULES_VERSION` follows).
* `crates/ti4-engine-core-tests/src/scenario.rs`: `LATEST_RULES = 36` (the test kit plays the newest version). The `rules_version: 35` literals of the action-card
  tests are Facts for gates below 36 and were left.
* `fixtures/protocol/{actor,opponent,spectator}_snapshot.json`: `"rules_version": 36` (only that field; the generator `generate_protocol_fixtures` writes the same).
* Text that names the current version: `deploy/public/README.md`, `deploy/public/test-recovery-check.sh` (fixture refusal line), the `#[ignore]` reasons in
  `ti4-wasm/tests/it/host_*`, comments in `ti4-server/tests/it/{splice,stale_offer_state}.rs`, `docs/RULES_VERSION.md`.
* Web/wasm: nothing (the wasm host reports `RULES_VERSION`; `web/` has no constant).

## Pins that moved, and why (only because of the bump)

* `ti4-engine-core-bridge/tests/host_create.golden`: NOT re-recorded. Its digest hashes the serialized state, whose `rules_version` is now 36; the test puts the
  recorded version (35) back before the digest (`RECORDED_RULES_VERSION`, like the alias rename already there). It passes, which shows nothing else of a created
  game changed.
* Quick-game fingerprint `74d5675ac754ac81` -> `538dc68f23dddcc9` (`tests/wrapper_native.rs`, `native_play.rs`): same 1,165 steps and answers; computed with the state's
  `rules_version` put back to 35 the hash is the old pin (checked), so only the version field moved. Ixthian Artifact and Parley do not occur in that game.
* `tools/engine_core/d4/test_disposition.tsv`: the renamed core test and the new factions test.

## Tests (all red without the fix)

* `ti4-engine-core-tests/tests/invasion_parley.rs` (5): Parley after the 3rd landing returns all three and the invader is not asked again; after the 1st
  landing cancels the whole invasion; returns the units of its planet only (another planet's landing stays and fights); a declined window leaves the next
  landing open; no Parley = unchanged. (Checked red by disabling `cancel_landings`: 3 of 5 fail.)
* `ti4-engine-core-tests/tests/agenda_crater.rs` (6): the Mecatol Rex system is emptied (dreadnought, ground forces, PDS, both owners) and a system two steps away is
  not touched; the choice of 3 units (options, left count, alike units, 3-or-fewer lose all, planet units count); 6-10 researches in speaker order with declines;
  the other outcome rolls nothing.
* `ti4-engine-core-factions` `cards::tests::parley_returns_a_naaz_mech_recorded_in_its_ship_form` and the cancel flag assertion of the existing Parley test;
  `ti4-engine-core` lib test `ixthian_artifact_for_rolls_one_die_and_a_research_needs_the_roster` (replaces the obsolete string-copy test);
  `native_play::tests::s6_and_s7_are_recognised_by_the_step_that_ran_the_effect`.

Run (profile gate, `CARGO_INCREMENTAL=0`): the two core-tests files, `-p ti4-engine-core --lib rules::agenda`, `-p ti4-engine-core-factions --lib cards::`,
`-p ti4-engine-legacy --lib agenda_` (223 pass) and `dice_effects`/`irreproducible_dice`, `-p ti4-engine-core-bridge --test host_create_golden --test wrapper_native`, the bridge lib tests `a_native_game_plays_a_whole_game_with_zero_gaps` and
`s6_and_s7_...`, and the whole `ti4-engine-core`, `-factions`, `-model` lib suites and `ti4-engine-core-tests`. Not run: the `ti4-server`, `ti4-wasm`, `ti4-host` and web
test suites (the fixtures and ignore-reason edits there are text and the version field), `bot_shadow`, the Z-gate, the ratchet.

## What the integration and the heavy run must re-record / run

1. **Golden (red until done)**: `golden::tests::every_version_has_a_set` (T7) fails: no set recorded at 36 and no ladder game at 36. Run
   `golden bless-new --set v36-std --config tools/engine_profile/soak/std-100.json --tier slim ...` (the v35-std seed set freezes), `golden ladder` (records the
   version-36 game) and `golden index` (writes the frozen flag of v35-std in `sets.json`/`set.json`). Take `flock /root/.ti4-heavy.lock`. The older sets replay on the string engine
   at their own versions; that was not run here.
2. **Decision streams**: every game that reaches Ixthian Artifact (For) or a Parley changes (new questions `ixthian_artifact_destroy_unit`, research questions with
   the agenda as source, the end of the commit step after Parley). Z-gate/soak games that hit them differ from the string by design: the shadow reports them as
   `known string bug S6` / `S7`; the dual run bails on S6 (counted). Re-record whatever stored decision streams contain either card; the rule bot answers the new
   subtype with its random fallback (no bot version change; `BOT_VERSION` untouched).
3. `ENGINE_CORE_COVERAGE`/mutation tables (`tools/engine_core/d4`) are not regenerated; `crater.rs` and the new `ops.rs` branch are new rows.
4. Not done on purpose: the Parley reaction preview (`live/reaction_preview.rs`, both copies) still tallies the unit of the landing only; the web client shows the new
   question with its generic fallback (PendingChoiceModal list). The Ixthian research question reuses the technology question (`ResearchTechnology`, now with a
   decline option and the agenda as origin).
5. Disk: the target directory used for these checks was deleted at the end (`/root/TI4-Bot-Dev/.claude/worktrees/core-rv36/target`).
