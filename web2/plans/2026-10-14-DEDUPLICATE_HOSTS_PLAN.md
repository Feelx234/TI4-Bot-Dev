# Deduplicate ti4-view / ti4-host / ti4-server and the two wasm hosts (2026-10-14)

Status: planned, not started.

## Context

Two lineages were merged into `dev` on 2026-10-14:

- **Lobby lineage (Felix, "lobby phase 5/7")** put game setup into `ti4-view` → now
  `ti4_host::setup` (`TableSetup`, `draw`, `create`, `setup_options`, `SeatOrder`). Used by the
  server lobby start and by web2's local play (wasm `ti4_setup_*`, `ti4_play_setup`).
- **Offline lineage (T3–T10)** created `ti4-host` by moving code out of `ti4-server`, with
  `ti4_host::map::create_game_with_factions` as "what a lobby start calls", plus wasm
  `host_setup::Setup`/`describe()` and `HostSession` (`ti4_host_*`, `ServerMessage` JSON) for web/'s
  offline mode.

Result: two setup paths that **give different games for the same seed**, three "setup options"
builders, two card-set modules, two seat/speaker enums, 2–3 layers of re-export shims (with stale
docs), and two wasm game hosts speaking different protocols. Goal: one implementation per
concept, in the lowest crate that needs it, imported from where it is defined.

Decisions (user): **old offline saves may break** (no legacy path); **include the wasm hosts**.

## Findings

**A. Two game-setup paths**
| | `ti4_host::setup::{draw, create}` | `ti4_host::map::create_game_with_factions` |
|---|---|---|
| Users | server lobby (no preset), web2 local | server lobby **with preset** (`ti4-server/src/session/registry.rs:3416`), web/ offline (`ti4-wasm/src/host_setup.rs:203`) |
| Seats | drawn (`seating::lobby_seat_order`), then factions for that order | factions by join index, no seat draw |
| Counts | `PLAYER_COUNTS = 2..=8` | `MIN/MAX_PLAYERS = 3..=6` |
| Rule bot id | `bot_kind::RULE = "smart"` | `"rule"` |

**B. Three setup-option builders**: `ti4_host::setup::setup_options` (typed), its server wrapper
`session::table::setup_options` (fine), and `ti4-wasm/src/host_setup.rs::describe()` — a
hand-written `json!` copy (card-set names duplicate `setup::card_set::name`).

**C. Card sets**: `ti4_host::setup::card_set` (constants) and `ti4_host::card_set` (re-export +
server-record `LEGACY`/`resolve`).

**D. Re-export chains** view → host → server: `ti4_host::protocol::{status,view}`,
`ti4_host::projection` (`pub use ti4_view::projection::*` + 4 wrappers), `ti4_host::planning`,
`ti4_host::map`, `ti4_host::maps`; `ti4_server::{map, card_set, preset, projection, protocol,
planning, maps}`, `ti4_server::session::reaction_modes`. ~300 call sites use the server paths.
Stale doc: `ti4-server/src/planning.rs` says "defined in `ti4-view`".

**E. `create_game_with_*` explosion**: 10 variants in `ti4-view/src/map.rs` + 4 in
`ti4-host/src/map.rs`, all ending in `create_game_with_template_seated`. Also `SpeakerOrder`
(`ti4_view::map`) vs `SeatOrder` (`ti4_host::setup`) — distinct concepts, keep both, but both live
next to `TableSetup`.

**F. Two wasm hosts** on the same `play` loop / `Host` checkpoint struct in `ti4-wasm/src/lib.rs`:
- web2: bespoke C-ABI (`ti4_play*`, `ti4_draft`, `ti4_plan`, `ti4_journal`, `ti4_replay`,
  `ti4_switch_seats`, `ti4_set_stepping`, `ti4_set_rule_seats`, update buffer of view+choice);
  consumed by `web2/src/session/wasmTransport.ts`.
- web/: `ti4_host_start/command/query` → `HostSession` → `ServerMessage` JSON (same as the server).
web2 already has `wsTransport.ts` decoding `ServerMessage` behind the same `transport.ts`
interface, so web2 can drive the `ServerMessage` host too.

## Plan (one PR per step, in order)
### Step 0 — `ti4-host` in the cargo aliases
`ti4-host` is built by `l-web`/`t-web`/`b-web` only as a dependency, so its unit tests
(`setup.rs`, `map.rs`, `card_set.rs`, …) never run. Add `-p ti4-host` to all of
`c-web`/`l-web`/`t-web` in `.cargo/config.toml` (one package set for all aliases, so they keep
sharing one build of the engine), and update the crate list in `web2/AGENTS.md`.


### Step 1 — one setup path (A, B, C)
- `TableSetup` gains `preset: Option<String>`. `setup::create` applies the preset logic lifted
  from `map::create_game_with_factions` (`preset::rotation`, `roster`, `seat_roster`, `apply`);
  `setup::draw` uses `SeatOrder::Join` + preset roster when a preset is set.
  `TableSetup` also owns the `SpeakerOrder` (move the enum from `ti4_view::map` to `ti4_host::setup`
  if nothing in ti4-view needs it; otherwise just re-home the import).
- `registry.rs` lobby start: drop the preset branch; always `setup::draw` + `setup::create`.
  Server games are unaffected on replay (the init record stores the opening state).
- `ti4-wasm/src/host_setup.rs`: `Setup` becomes a thin request type → `TableSetup` + chosen factions
  → `setup::draw/create`. `describe()` = `serde_json::to_value(setup::setup_options("The page", 0))`
  plus the `maps`/`presets`/`content_digest`/`engine_build` extras. Player range from
  `setup::PLAYER_COUNTS`, bot id `bot_kind::RULE` (`"smart"`; accept `"rule"` via serde alias for
  the web/ form). Update `web/src/offline/local/hostApi.ts` types + form if fields change.
- `ti4_host::card_set::{LEGACY, resolve}` → `ti4-server` (storage concern; e.g. `ti4-server/src/storage.rs`
  or a `session/table.rs` helper). `setup::card_set` is the only set module.
- Delete `map::create_game_with_{preset,options,speaker,factions}`; keep `rebuild_galaxy`.
  Migrate their test/fixture callers to `TableSetup` or `create_game_with_template_seated`.

### Step 2 — collapse `create_game_with_*` (E)
Keep `create_game_with_template_seated` (the real builder) and one
`create_game_with_template(content, ids, seed, template)` for tests/fixtures; replace
`_rotated`, `_and_card_set`, `_map_*` variants at their call sites (mostly `ti4-server/tests`,
`fixtures.rs`, `dev/scenarios.rs`, `ti4-cli`/`ti4-sim` if any — grep the workspace).

### Step 3 — remove re-export layers (D)
- Import every item from its defining crate: `ti4_view::{status, view, projection, map, maps,
  planning, reaction_modes, seats}`, `ti4_host::{protocol, setup, preset, journal, …}`.
- Delete shim modules `ti4-host/src/protocol/{status,view}.rs`, the `pub use ti4_view::…::*`
  lines in `ti4-host/src/{projection,planning,map}.rs` and `maps/mod.rs`; delete
  `ti4-server/src/{planning.rs (keep `runner` as `planning/runner.rs` under a plain `mod`),
  maps/catalog.rs (move `variant_seed` next to its HTTP caller)}`,
  `ti4-server/src/session/reaction_modes.rs`, and the `pub use ti4_host::…` block of
  `ti4-server/src/lib.rs`.
- Mechanical rewrite with `sed` per path (`crate::protocol::` → `ti4_host::protocol::`,
  `ti4_server::map::` → `ti4_view::map::`/`ti4_host::setup::`, …), then `cargo clippy --fix`.
  Fix the stale module docs while there.

### Step 4 — one wasm host (F)
1. Extend `HostSession` to what web2 local play needs that it lacks: several local human seats
   + seat switching (`ti4_switch_seats`/`humans` mask), rule-seat mask, stepping/pause between
   bot decisions, scenario start (already in `Setup`), draft/plan (`run_draft`/`apply_draft`,
   planning commands — map to `ti4_view::planning` as the README describes). Expose them as
   `ClientMessage` local commands (`HostSession::local`) rather than new C exports.
2. Rewrite `web2/src/session/wasmTransport.ts` on `ti4_host_start/command/query` + the
   `ServerMessage` decoding already used by `wsTransport.ts` (share it). Keep
   `transportContract.test.ts` / `wasmTransport.test.ts` green; web2 saves switch to the
   `HostSession::save_state` format (old web2 saves break, consistent with the decision above).
3. Delete the bespoke exports and their state from `ti4-wasm/src/lib.rs` (`ti4_play*`,
   `ti4_draft*`, `ti4_plan`, `ti4_journal`, `ti4_replay`, `ti4_saved_option`, update buffer,
   `TableRequest`/`TableOptions`/`TableDraw` once `host_setup` covers them). The `play` loop and
   checkpointing stay, owned only by `HostSession`'s path.
4. Update `ti4-wasm/README.md`, `ti4-host/README.md` (setup row, "two hosts" text).

(Not in scope: moving server `SessionShared` onto `HostSession`; `server_parity` already guards it.)

## Verification
Builds only through the aliases of `.cargo/config.toml` (no `-p` subsets, no `--workspace`; see
`web2/AGENTS.md`, "Rust builds"); one build or suite at a time.
- While working: `cargo l-web`, `cargo t-web <filter>` (e.g. `both_hosts`, `bot_lobby_factions`,
  `map_selection`, `player_lobby_admission`, `table_in_game`, `dev_scenarios`, `server_parity`).
- Before each commit: `cargo fmt --all -- --check`, `cargo l-web`, `cargo t-web`;
  `scripts/build-wasm.sh` + `npm test` in `web2/` (ti4-view/ti4-wasm changed); `cargo b-web` +
  `npm test` (server changed, transport suite). Before the push: `cargo t-web-slow`.
- ti4-host contract: `cargo clippy -p ti4-host --target wasm32-unknown-unknown` (a different
  target, so it does not disturb the alias build); CI runs it too.
- New test (Step 1): same `TableSetup` + seed through the server lobby start and through wasm
  `Setup::build` → identical opening `GameState` (extend `crates/ti4-server/tests/it/both_hosts.rs`).
- Decision-purity gate (ti4-host README): `hash_survey` on a copy of `/var/lib/ti4-public`,
  byte-identical before/after each step (run its build with the alias package set).
- Step 4: `npm test` in `web2/` and `web/`; a web2 local game in the browser (start a table,
  hot-seat switch, undo, draft, reload from save); web/ offline e2e.
