# Native game setup in the core: design (branch `core-setup-native`)

Goal (user request): the game setup builds the typed core position (`CoreState`) directly. No string-shaped
`GameState` is built by the setup and `Codec::load` is not run on it. The old string setup stays as the test oracle
(and for the dev-only start presets and scenarios, which are string edits of a finished table; see 6).

Base: `core-integration-25` (3e7a099ab), rules version 36, quick fingerprint `538dc68f23dddcc9`.

## 1. Where it sits

New package `ti4-engine-core-setup` (directory `crates/ti4-engine-core-setup`), layer **L7**: above the factions crate
(L6), below the thin crate `ti4-engine`.

```
ti4-model, ti4-content            data (ids, content store, rules constants)
ti4-engine-core  (L0..L5)         typed state, kernel, driver
ti4-engine-core-factions (L6)     hook modules
ti4-engine-core-setup  (L7)       NEW: start_game(spec, galaxy) -> CoreStart
ti4-engine (thin)                 Game, host glue; depends on the setup crate
ti4-host -> ti4-server / ti4-wasm / ti4-rule-bot
```

Imports it may have: core (all layers), factions (the live kernel, needed by the starting-technology settle), content,
model, `sha2`, `rand`/`rand_chacha`, `thiserror`. It may NOT name `ti4_engine` (thin), `ti4_engine_legacy`, `ti4_host`.
Why not lower: the settle of the starting technology (`driver::turn::starting::settle_core`) needs the live kernel
(all faction modules), so it cannot sit next to the core. Why not in the thin crate: the thin crate owns the string
wire/`GameState` glue; the setup must be usable without it (a typed-only tool, a future wasm without the mirror).

Production graph: thin -> setup -> {core, factions, content, model}. The legacy crate is not reachable
(`tools/engine_core/check_production_graph.sh`: the new package is not a dev tool, nothing in it mentions `lend-string`).
`tools/engine_core/check_layers.py core` gets a third root (the setup crate, floor L7): it may import every core
layer, and a text check forbids the three crates above.

## 2. What is pure data/RNG and is MOVED into the setup crate (thin crate re-exports it, one source of truth)

| item | from | note |
|---|---|---|
| `deck` (`build_starting_decks*`, `Exclusions`, `StartingDecks`, the PoK supersede list) | thin `deck.rs` | shuffles run on the core's `state::Rng` with a literal-domain `SeedSource` (SHA-256(seed_le \|\| domain)), bit-identical to `GameRng` (the old parity tests against legacy prove it) |
| seeded speaker | thin `setup::seeded_speaker` | own `ChaCha8Rng` from the same stream seed; `random_range(0..n)` exactly as before |
| mecatol constants, `is_mecatol`, `mecatol_in_galaxy` | thin `seating.rs` | tiny, pure |
| `MapCensus`, `census`, `unsatisfiable_public/secret` | thin `map_feasibility.rs` | pure functions of `Galaxy` + content; the state-writing `apply` stays in thin (string oracle) |
| `leaders::{kind_of, for_faction, starting_states}` | thin `builders/leaders.rs` | content only |
| live kernel (`kernel_for`, `static_of`) and `roster_for` | thin `live/kernel.rs`, `ties/technology_core.rs` | the settle needs them |
| `IdStr` generated adapter | included by `#[path]` from the same file the legacy and thin crates include | typed id <-> corpus id text, no second table |

Reused as they are, NOT moved (they only produce inputs): the galaxy. Template loading, the seeded filler board, the
wormhole nexus, the Creuss/Crimson home swap and the lobby faction draw stay in `ti4_host::maps`, `ti4_host::map` and
`ti4_engine::seating`; they never touch the game state and are given to `start_game` as `Galaxy` and as the
faction of each seat. The galaxy is a content object (tiles on hexes), not a game position, so it is not a string state.
Its conversion to the typed map slice (`galaxy_from_legacy`, `typed_map`) is copied into the setup crate with an equality
test against the thin original (the thin one carries strict-mode counters).

## 3. Public API

```rust
pub struct TableSpec<'a> {
    pub content: &'a ContentStore,
    pub players: &'a [PlayerId],        // seat order, 1..=8 (the typed core holds 8)
    pub factions: &'a [FactionId],      // one per seat (lobby draw / preset / chosen)
    pub seed: u64,                      // the game seed (decks, dice, speaker)
    pub card_set: Option<&'a str>,      // "te" | "pok" | "base_game_codex1" | ...; None = the scope's set
    pub deck_sources: SourceSet,        // POK (FULL is refused: Thunder's Edge stays out)
    pub speaker: SpeakerChoice,         // Seat(usize) | Seeded
    pub starting_tech: bool,            // ask the picks (Winnu, Argent, ...) and settle those without a choice
    pub rules_version: Option<u32>,     // None = the newest
}
pub struct CoreStart { pub core: CoreState, pub galaxy: Galaxy }   // galaxy: the laws/lanes applied, as the core's map slice has it
pub fn start_game(spec: &TableSpec<'_>, galaxy: Galaxy) -> Result<CoreStart, SetupError>;
```

`CoreState` comes out fully loaded (all 17 slices, owners table = the sorted player ids, text tables, rng seed, no stream
positions: nothing has drawn from the game's own streams yet). Errors: unknown faction, no home system, the Tribuni
conflict, `Thunders Edge refused`, no players, more than 8, wrong lengths, unknown card set.

Thin crate (`ti4_engine`):

* `setup::core_start(spec, galaxy) -> GameStart`: runs `start_game`, builds the `Ctx` from the owner table, and the string
  mirror `GameState` by `materialize` over a shell (`GameState::new` of the players). The mirror is what `Game::state`
  has always been after the first step; it is also the opening state the server stores for replay and recovery.
* `Game::from_start(GameStart, content)`: a game whose core is already loaded (`native = Some`), table = seeded
  random as `with_seeded_random`, `with_sources(POK)`, galaxy attached. No `Codec::load` runs.
* Host: `setup::create_start(content, &TableSetup, players, &Drawn) -> (GameStart, ...)` for plain tables; `setup::create`
  (the golden-tested `(GameState, Galaxy)` API) becomes `create_start` + the materialized mirror for plain tables, and the
  old string pipeline for start presets and scenarios.
* Callers switched: server lobby start, wasm host setup (two sites), `GameSpec::build` (selfplay, bridge `perf_*`/golden
  corpora), the bridge `NativeGame` creation.

## 4. RNG streams stay identical

Every random draw of the setup, in today's order and from today's domain:

1. lobby faction draw / seat assignment: stays in thin `seating` (host calls it before `start_game`; unchanged);
2. the six decks: `deck:objectives` (stage I, stage II), `deck:exploration` (CULTURAL, HAZARDOUS, INDUSTRIAL, FRONTIER),
   `deck:relics`, `deck:agendas`, `deck:action_cards`, `deck:secrets`: one `Rng` per call, streams created on first use;
3. the map feasibility pass re-shuffles the same domains from a fresh generator of the same seed (the old `apply`
   rebuilds the decks with the seed in `GameState::rng_seed`); the new code does the same with the same function;
4. the speaker: `setup:speaker`, one `random_range(0..n)`.

The game's own `core.rng` starts with the seed and no stream (the setup draws from private generators). Parity is
checked on the shuffled decks themselves, the revealed objectives, the dealt secrets, the speaker, and finally on the
whole `CoreState`.

## 5. Order of the setup (what `start_game` does, each step the typed twin of today's string step)

1. checks (seat count <= 8, one faction per seat, Thunders Edge / FULL refused, card set known);
2. owners = sorted player ids; seats; players in the order given; `Player::new` with the opening tokens;
3. decks (2.), reveal the top two public objectives, deal one secret per seat (in seat order);
4. strategy: the eight cards of the set, initiative per card, cards per player by seat count;
5. per seat: `deploy` (home system + planets control, the starting fleet with the faction's own mech/flagship,
   starting technologies, the Nekro pair, Sorrow breach, leaders (agent readied, others locked));
6. promissory notes of every seat (Mahact without Alliance, the Empyrean extra), the Tribuni check;
7. map slice and frontier tokens (a planetless system gets one, 35.5), lanes on;
8. map feasibility: unsatisfiable objectives leave the decks, the revealed two are re-drawn, hands re-filled from the top;
9. speaker (seat or seeded); marks of the private "seen" baselines (`staging_enabled`, written by `Game::with_table`);
10. starting technology: queue the picks (Keleres last), settle those that need no answer (`settle_core`).

The text tables are minted in the order of the old load (code 0 the diplomacy rules tag, code 1 the empty presentation
phase), so the tables compare equal.

## 6. What stays on the string path and why

* Start presets (`preset::apply`, 17 of them with suffixes) and the 15+ scenarios: they are hand edits of a finished
  string position (cards in hands, units on the board, custom decks). Dev/test tables; kept as they are, with
  `Codec::load` at the first step. They are 1 153 of the 1 405 golden rows; the 252 plain rows go native.
* `Codec::load` stays for stored games (recovery, replay, resume) and for the fork-and-edit paths of the host.
* The oracle: `ti4_engine::setup::start_game_seeded_with_card_set` + `seating::deploy` + `map_feasibility::apply` +
  `starting_tech::open` are the reference the parity test compares with.

## 7. Proof plan

* `crates/ti4-engine-core-bridge/tests/setup_native_parity.rs`: for the 252 plain host rows of `host_create_golden`
  (non-TE: 2-8 seats x 3 card sets x starting tech x speaker x 3 seeds) and the golden/bench setups of
  `GameSpec`: `start_game` core == `Codec::load(old setup)` by `==` on the whole state (and the hash of both Debug texts),
  `materialize(new core) == old GameState` (serialised and Debug), identical first decision/question and the identical first
  few steps of two games; Thunder's Edge rows refuse.
* `host_create.golden` unchanged, still passing through the rewired `create`.
* quick fingerprint `538dc68f23dddcc9`, `perf_ab` smart sweep chain hashes unchanged, rules version 36.
* mutation check on the new files where cheap (`tools/engine_core/mutants.sh`).
