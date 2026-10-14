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
builders, two card-set modules, 2–3 layers of re-export shims (with stale docs), a separate
`ti4-view` crate that only `ti4-host`'s users use, and two wasm game hosts speaking different
protocols. Goal: one implementation per concept, one crate between engine and hosts
(`ti4-host`), items imported from where they are defined.

Decisions (owner): **old offline and web2 saves may break** (no legacy path); **include the wasm
hosts**; **fold `ti4-view` into `ti4-host`**.

## Findings

**A. Two game-setup paths**
| | `ti4_host::setup::{draw, create}` | `ti4_host::map::create_game_with_factions` |
|---|---|---|
| Users | server lobby (no preset), web2 local | server lobby **with preset** (`ti4-server/src/session/registry.rs:3416`), web/ offline (`ti4-wasm/src/host_setup.rs:203`) |
| Seats | drawn (`seating::lobby_seat_order`), then factions for that order | factions by join index, no seat draw |
| Counts | `PLAYER_COUNTS = 2..=8` | `MIN/MAX_PLAYERS = 3..=6` |
| Rule bot id | `bot_kind::RULE = "smart"` | `"rule"` |

**B. Three setup-option builders**: `ti4_host::setup::setup_options` (typed), its server wrapper
`session::table::setup_options` (fine), and `ti4-wasm/src/host_setup.rs::describe()`, a
hand-written `json!` copy (card-set names duplicate `setup::card_set::name`).

**C. Card sets**: `ti4_host::setup::card_set` (constants) and `ti4_host::card_set` (re-export +
server-record `LEGACY`/`resolve`).

**D. Re-export chains** view → host → server: `ti4_host::protocol::{status,view}`,
`ti4_host::projection` (`pub use ti4_view::projection::*` + 4 wrappers), `ti4_host::planning`,
`ti4_host::map`, `ti4_host::maps`; `ti4_server::{map, card_set, preset, projection, protocol,
planning, maps}`, `ti4_server::session::reaction_modes`. ~300 call sites use the server paths.
Stale doc: `ti4-server/src/planning.rs` says "defined in `ti4-view`".

**E. `create_game_with_*` explosion**: 10 variants in `ti4-view/src/map.rs` + 4 in
`ti4-host/src/map.rs`, all ending in `create_game_with_template_seated`.

**F. Two wasm hosts** on the same `play` loop / `Host` checkpoint struct in `ti4-wasm/src/lib.rs`:
- web2: bespoke C-ABI (`ti4_play*`, `ti4_draft`, `ti4_plan`, `ti4_journal`, `ti4_replay`,
  `ti4_switch_seats`, `ti4_set_stepping`, `ti4_set_rule_seats`, update buffer of view+choice);
  consumed by `web2/src/session/wasmTransport.ts`.
- web/: `ti4_host_start/command/query` → `HostSession` → `ServerMessage` JSON (same as the server).
web2 already has `wsTransport.ts` decoding `ServerMessage` behind the same `transport.ts`
interface, so web2 can drive the `ServerMessage` host too.

**G. `ti4-view` no longer needs to be its own crate.** Its only dependents are `ti4-host`,
`ti4-server` and `ti4-wasm`, and the last two depend on `ti4-host` anyway (`ti4-engine` only
mentions it in a comment, `timing.rs:1188`). Its dependencies are `ti4-host`'s plus `ti4-calc`, and
both are wasm-clean. It has no `tests/` dir; its only asset is
`src/maps/community_maps.json` (+ `include_str!` of `ti4-content/content/map_templates.json`,
same relative depth after the move). Half of D (the host-side shims) exists only because of the
crate split.

## Plan (one PR per step, in order)

### Step 1: fold `ti4-view` into `ti4-host` (G, host half of D)
1. Mechanical move: `git mv crates/ti4-view/src/* crates/ti4-host/src/view/`, `lib.rs` →
   `view/mod.rs` (crate doc → module doc, keep `pub mod …` list). `ti4-host/src/lib.rs` gets
   `pub mod view;`. Add `ti4-calc` to `ti4-host/Cargo.toml`; drop `ti4-view` from the workspace
   `members`/`[workspace.dependencies]`, and from `ti4-server`/`ti4-wasm` `Cargo.toml`.
   Rewrite paths: `ti4_view::` → `ti4_host::view::` (server, wasm), `ti4_view::` →
   `crate::view::` (inside host); `crate::` inside the moved files → `crate::view::` where it named
   a ti4-view module. Fix the `include_str!` paths if the depth changed (`view/maps/…` is one
   level deeper: `../../../../ti4-content/…`).
   Commit this alone, so `git log --follow` keeps the history.
2. Merge the pairs that the split had made into one module each, and delete the shims:
   - `view::status`, `view::view` are the wire DTOs: `protocol/status.rs` and `protocol/view.rs`
     become the real files (move them in, delete the `pub use` shims).
   - `view::projection` (1249 lines, views) + `projection.rs` (message wrappers, 224): one
     `projection` module (`projection/mod.rs` + `projection/messages.rs`).
   - `view::planning/` + `planning.rs` (wire DTOs): one `planning/` (`wire.rs` for the DTOs).
   - `view::map` + `map.rs`: one `map.rs` (after Step 3 it is small).
   - `view::maps::{build, template_loader}` + `maps::catalog`: one `maps/`.
   - The rest stays as `ti4_host::view::{action_cards, agendas, components, exploration,
     reaction, reaction_modes, scenario, score, seats, secondary, secret_objectives, tactical}`:
     the facts of the update the UI reads.
3. `.cargo/config.toml`: in `c-web`/`l-web`/`t-web` replace `-p ti4-view` with `-p ti4-host` (the
   package count stays the same; the host's own tests now run in `t-web`).
4. Docs: `ti4-host/README.md` (module table, "thin layer over ti4-view"), `crates/ti4-wasm/README.md`,
   `web2/AGENTS.md` (crate list back to eight; "a fact the UI needs is added in `ti4-view`" →
   "in `ti4-host/src/view/`"; paths `crates/ti4-view/src/scenario.rs`, `ti4_view::seats`,
   `ti4_view::reaction_modes`, `ti4_view::components`, `script_answer in ti4-view`), `web2/NEXT.md`,
   the engine comment. Old plans in `web2/plans/` stay as written (history).

### Step 2: one setup path (A, B, C)
- `TableSetup` gains `preset: Option<String>`. `setup::create` applies the preset logic lifted
  from `map::create_game_with_factions` (`preset::rotation`, `roster`, `seat_roster`, `apply`);
  `setup::draw` uses `SeatOrder::Join` + preset roster when a preset is set. `SpeakerOrder` moves
  from `map` to `setup`, next to `SeatOrder`.
- `registry.rs` lobby start: drop the preset branch; always `setup::draw` + `setup::create`.
  Server games are unaffected on replay (the init record stores the opening state).
- `ti4-wasm/src/host_setup.rs`: `Setup` becomes a thin request type → `TableSetup` + chosen factions
  → `setup::draw/create`. `describe()` = `serde_json::to_value(setup::setup_options("The page", 0))`
  plus the `maps`/`presets`/`content_digest`/`engine_build` extras. Player range from
  `setup::PLAYER_COUNTS`, bot id `bot_kind::RULE` (`"smart"`; accept `"rule"` via serde alias for
  the web/ form). Update `web/src/offline/local/hostApi.ts` types + form if fields change.
- `ti4_host::card_set::{LEGACY, resolve}` → `ti4-server` (storage concern; `storage.rs` or
  `session/table.rs`). `setup::card_set` is the only set module.
- Delete `map::create_game_with_{preset,options,speaker,factions}`; keep `rebuild_galaxy`.
  Migrate their test/fixture callers to `TableSetup` or `create_game_with_template_seated`.

### Step 3: collapse `create_game_with_*` (E)
Keep `create_game_with_template_seated` (the real builder) and one
`create_game_with_template(content, ids, seed, template)` for tests/fixtures; replace
`_rotated`, `_and_card_set`, `_map_*` variants at their call sites (mostly `ti4-server/tests`,
`fixtures.rs`, `dev/scenarios.rs`; grep the workspace).

### Step 4: remove the server re-export layer (server half of D)
- Delete `ti4-server/src/session/reaction_modes.rs`, the `pub use ti4_host::…` block of
  `ti4-server/src/lib.rs`, `ti4-server/src/planning.rs`'s re-exports (keep `runner`),
  `ti4-server/src/maps/` re-exports (move `variant_seed` next to its HTTP caller; keep
  `community_tests`).
- Mechanical rewrite: `crate::{protocol,projection,map,maps,card_set,preset}::` and
  `ti4_server::{…}::` → `ti4_host::…::`, then `cargo clippy --fix`. Fix stale module docs.

### Step 5: one wasm host (F)
1. Extend `HostSession` to what web2 local play needs that it lacks: several local human seats
   + seat switching (`ti4_switch_seats`/`humans` mask), rule-seat mask, stepping/pause between
   bot decisions, the wait of the bots, scenario start (already in `Setup`), draft/plan
   (`run_draft`/`apply_draft`, planning commands → `ti4_host::planning`). Expose them as
   `ClientMessage` local commands (`HostSession::local`) rather than new C exports.
2. Rewrite `web2/src/session/wasmTransport.ts` on `ti4_host_start/command/query` + the
   `ServerMessage` decoding already used by `wsTransport.ts` (share it). Keep
   `transportContract.test.ts` / `wasmTransport.test.ts` green; web2 saves switch to the
   `HostSession::save_state` format (old web2 saves break). `localTable.ts` uses the setup
   exports that remain (`ti4_setup_options/preview/draw` stay, on `ti4_host::setup`).
3. Delete the bespoke exports and their state from `ti4-wasm/src/lib.rs` (`ti4_play*`,
   `ti4_draft*`, `ti4_plan`, `ti4_journal`, `ti4_replay`, `ti4_saved_option`, the update buffer).
   The `play` loop and checkpointing stay, owned only by `HostSession`'s path.
4. Update `ti4-wasm/README.md`, `ti4-host/README.md`, `web2/AGENTS.md` ("Local play": saved game
   format, `ti4_journal`, `ti4_set_stepping`).

(Not in scope: moving server `SessionShared` onto `HostSession`; `server_parity` already guards it.)

## Verification
Builds only through the aliases of `.cargo/config.toml` (no `-p` subsets, no `--workspace`; see
`web2/AGENTS.md`, "Rust builds"); one build or suite at a time.
- While working: `cargo l-web`, `cargo t-web <filter>` (e.g. `both_hosts`, `bot_lobby_factions`,
  `map_selection`, `player_lobby_admission`, `table_in_game`, `dev_scenarios`, `server_parity`).
- Before each commit: `cargo fmt --all -- --check`, `cargo l-web`, `cargo t-web`;
  `scripts/build-wasm.sh` + `npm test` in `web2/` (host/wasm changed); `cargo b-web` +
  `npm test` (server changed, transport suite). Before the push: `cargo t-web-slow`.
- ti4-host contract: `cargo clippy -p ti4-host --target wasm32-unknown-unknown` (another target,
  so it does not disturb the alias build); CI runs it too. After Step 1 it covers the former
  ti4-view code too.
- Step 1 is a pure move: `cargo t-web` has the same test count (+ the host's own tests, now
  included), and `scripts/build-wasm.sh` gives a wasm of about the same size (size gate).
- New test (Step 2): same `TableSetup` + seed through the server lobby start and through wasm
  `Setup::build` → identical opening `GameState` (extend `crates/ti4-server/tests/it/both_hosts.rs`).
- Decision-purity gate (ti4-host README): `hash_survey` on a copy of `/var/lib/ti4-public`,
  byte-identical before/after each step (built with the alias package set).
- Step 5: `npm test` in `web2/` and `web/`; a web2 local game in the browser (start a table,
  hot-seat switch, undo, draft, reload from save); web/ offline e2e.
