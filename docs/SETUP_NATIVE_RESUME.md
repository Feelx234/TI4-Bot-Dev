# Native setup: resume (branch `core-setup-native`, cut from `core-integration-25` 3e7a099ab)

Design: `docs/SETUP_NATIVE_DESIGN.md`. Rules version stays 36, no wire/TS change, `tests/host_create.golden` unchanged.

## What is done

A table is now set up **straight into the typed core**. `ti4_engine_core_setup::start_game(&TableSpec, Galaxy) -> CoreStart` builds all 17
slices of the `CoreState` (owner table, seats, players with fleet/technology/leaders, board, decks, promissory notes, objectives, strategy,
diplomacy ledger, frontier tokens, map slice, the starting-technology picks and their settle, the private "seen" baselines, the text tables).
No string `GameState` is built by the setup and `Codec::load` does not run on it.

### Where it sits

New package `ti4-engine-core-setup` (`crates/ti4-engine-core-setup`), layer L7: above the factions crate, below the thin crate. It names no
`ti4_engine` / legacy / host crate (`python3 -I tools/engine_core/check_layers.py setup`: 0 violations; `check_production_graph.sh`: 0 violations;
`check_layers.py core` and `thin`: 0). `.cargo/config.toml`: the package is in `c-web`, `l-web`, `t-web` and the dev sets.

### What moved into it (the thin crate re-exports it; one source of truth)

`deck` (shuffles on the core's `Rng` with the `SHA-256(seed_le || domain)` seeding), the seeded speaker (`rng::seeded_speaker_index`), mecatol
constants, `MapCensus`/`census`/`unsatisfiable_*` (`feasibility`), the leaders of a faction (`leaders`), the faction constants and the Tribuni
check (`factions`), the strategy-card sets (`strategy`), the live kernel (`kernel`), the unit roster and scope mask (`roster`), the lane table.
Copied (with an equality test against the thin original): the typed galaxy (`galaxy::typed_map`), the Tribuni check (messages), `seeded_speaker`.
`IdStr` (typed id <-> corpus id) is the same generated file the legacy and thin crates include by `#[path]`.

### What switched to the new path (no string setup, no load)

* `ti4_host::setup::create_start` (plain tables; `Ok(None)` for scenarios and start presets, which are string edits) and
  `ti4_host::map::build_galaxy` (the state-free board of a table).
* Thin crate: `ti4_engine::core_setup::{GameStart, TableSpec, start_game, ...}`, `Game::from_start[_with_table]`, `GameStart::{opening_state, raw_galaxy,
  into_parts}`, `Ctx::of_core`.
* Server lobby start (`registry.rs` -> `SessionConfig::start` -> `worker.rs` `Game::from_start_with_table`); the stored opening state is
  `GameStart::opening_state()` (the `create` state, field by field). A restart, a recovery and a replay still run from the stored string state.
* Wasm host setup (two sites, one helper `started`).
* Selfplay `GameSpec::build` / `GameSpec::start` (`build_string` is the old pipeline, kept for Thunder's Edge and pinned older rules versions), and
  through it `golden`, `playthrough`, `bot_shadow`, `perf_ab`, the bench corpora; bridge `NativeGame::from_spec` -> `NativeGame::from_start`.
* `perf_kernel` is not in this branch's tree (it lives on `core-perf-kernel`); it builds its game through `GameSpec::build`, so it is on the new path as
  soon as it is merged.

### What stays on the string path, and why

Start presets (17, with suffixes) and the scenarios: hand edits of a finished string position (dev/test tables; 901 of the 1 405 golden rows).
`Codec::load` stays for stored games (recovery, replay, resume) and the host's fork-and-edit paths. Thunder's Edge: refused at creation as before
(`create_start` and `start_game` refuse; the 252 TE rows of the golden still read `err`). Pinned older rules versions: refused (as `Game` refuses them).
The old string setup (`start_game_seeded_with_card_set`, `seating::deploy`, `map_feasibility::apply`, `starting_tech::open`) is the oracle of the tests.

## Proof

See the final report of the lane for the numbers; the files are:

* `crates/ti4-engine-core-bridge/tests/setup_native_parity.rs`: whole-`CoreState` equality (`==` and the Debug text of every slice, texts and owner table
  included) of `start_game` against `create` + `Game::with_seeded_random` + `Codec::load`, for the 252 plain rows of the golden matrix, every map
  template, every faction at the first and last seat of 3/4/6-seat tables (with and without starting technology), card sets, speakers, pinned versions
  (refused), the Keleres/Winnu/Argent picks (open and settled); `Game::from_start` against the string game for 250 steps (steps, decisions, state, streams);
  the opening state against `create`'s state (JSON and Debug); the speaker, the typed galaxy, the Tribuni text, the strategy sets, the stream names
  against the originals; the refusals.
* `crates/ti4-engine-core-bridge/tests/host_create_golden.rs`: unchanged golden; a second test makes the 252 plain rows by `create_start` and compares
  the digests with the golden lines (legacy-recorded at 04b519e25), and counts 252 native / 252 refused / 901 string-only.
* Mutation check of the new files (`tools/engine_core/mutants.sh`, `CRATE=ti4-engine-core-setup TEST_CRATE=ti4-engine-core-bridge`).

## What remains

* Presets and scenarios as typed builders (a typed `preset::apply`): only worth it when those tables become product features.
* The galaxy is still the string `Galaxy` (a content object built by the host's template loader / seeded filler); a typed template loader would remove
  `galaxy_to_core`'s input conversion. The typed map slice is built from it by `galaxy::typed_map` (copied, test-equal to the thin one; the thin
  `ties::core_adapter::galaxy_from_legacy` can delegate once its strict counters move).
* `GameStart` still builds a string mirror (`Game::state`, a `pub` field read by the whole host) over a seat-only shell; removing the mirror is the
  wrapper's flip, not the setup's.
* The server keeps `initial_state` (the stored opening) as a `GameState`; the init record format is unchanged on purpose (old saves load).
* `core_sync::guard::{ids_supported, factions_known}` are still called by the host (lane A's glue).
