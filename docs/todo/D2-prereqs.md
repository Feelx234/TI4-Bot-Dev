# TODO D2-prereqs: everything D2 needs that is not a window (2026-10-29)

Theory document (read-only analysis, no builds, no tests). Branch `core-todo-d2`, cut from `core-integration-3` @ `1e55ab28`. Only this file changed.
Written for an implementation agent that executes it without re-deriving the design. Where I could not decide from the code, the text says
**VERIFY** (a 10-minute check the agent does first) or points to an open question (section 9).

**Not covered here (other theory agent, branch `core-todo`):** `docs/todo/A2b.md` (tactical/movement execution), `docs/todo/D1b.md` (action phase and
window composition), `docs/todo/P1.md` (plain events). I reference them; I do not repeat them. D1a is in `docs/ENGINE_AREA_STATUS.md` 4.1.

**Sources.** `docs/ENGINE_CORE_STATE.md` (3.2 mechanism, 5 gating, 6.5 process, 7 briefs D1/D2, 9 final structure), `docs/ENGINE_AREA_STATUS.md` (branch
`core-area-status` @ `5c02b4db`: 2.2 lend rule, 2.4 N1-N11, 2.5, 4 roadmap, 5 risks), `docs/DISABLE_XF123.md` (branch `disable-xf123` @ `f472fd4e`, section 5),
`docs/ENGINE_CORE_DRIVER.md`, `docs/ENGINE_KNOWN_ISSUES.md` (D-table, A6), and the code of this branch. Line numbers are from `core-integration-3`
(re-grep before editing; later integration branches shift them).

**Labels.** *Measured* = I counted it in the tree or in `/var/lib/ti4-public` for this document. *Cited* = from a document. *Estimated* = my judgement.

---

## 0. Summary and findings that change the plan

1. **D2 split into nine sub-packages D2-1..D2-9** (section 2). Four can start now (D2-1 fixtures/totality/alpha round trip, D2-3 text tables, D2-4a resolver sync,
   D2-6a allow-list mechanism); D2-2 and D2-5 wait for the integration refresh that carries B12 and B6c (their bail labels are final then); D2-7 (crate move),
   D2-8 (flip) and D2-9 (production strip + final gate) wait for the D1b freeze (`core-integration-7`). Estimated 45-65 agent-h plus two gates (2 h + 4 h).
   That is more than the 20-30 h line of `ENGINE_AREA_STATUS` 4.1 because that line folded fixtures, the alpha harness, the oracle harness, the disabled-feature
   panics and the crate move into one row; the critical path after int-7 is ~27-30 h wall (section 2.2).
2. **Conflict inside the decisions (needs your answer, question 1).** `ENGINE_CORE_STATE.md` 9.1/9.4 say the legacy engine is *never* built for server or wasm.
   `ENGINE_AREA_STATUS` 2.5 says leaf bails (Ssruu copies, Dynamo, Watcher, faction O/W bodies, card bodies) stay lent through D2 by `with_string`. A lend runs
   string code, so **the string code must be in the production binary as long as one lend is reachable.** Plan: two-phase D2. D2-8 flips with legacy as an
   optional dependency behind a feature `lend-string` (on in server and wasm); D2-9b switches it off once the bail census over production inputs is zero.
3. **Today `Ctx::of(gs)` is built fresh for every lend** (44 call sites, measured; `core_sync/ctx.rs:155`), so the "persistent for the game" text interner of
   `ctx.rs:83-90` and `keys/text.rs:59` is *documentation, not behaviour*. Codes are only valid inside one lend. After the flip a `CoreState` lives for the
   whole game, so the tables must be part of it (D2-3). Also `Blobs::intern` is a linear scan (`keys/text.rs:66`).
4. **`Unmapped` is invisible today**: 40+ call sites do `Ctx::of(..).ok()?` / `.ok()` and bail silently (measured: `grep "Ctx::of" -> 44`). There is no counter. D2-1 adds
   one at the single construction point `core_sync/error.rs:21 bad()`.
5. **The alpha corpus is small** (measured, count and sizes only): `/var/lib/ti4-public` has 24 `game_*` directories, 5.36 MB in total (largest 1.13 MB); 14 have
   `init.json` + `decisions.jsonl` (603 recorded decisions in all, largest game 184, median 29), 6 have `snapshot.json` + `history.json`. All 14 seat only real
   lobby factions, 4-8 players, player ids `player_<64 hex>`; none seats Bastion/Crimson, none enables diplomacy (`docs/DISABLE_XF123.md` 1.3, cited; re-checked
   faction lists). It proves format compatibility, not coverage. Coverage comes from the golden/bench/template corpora. Question 2.
6. **The string resolver's `initiative_order` is set once at `Game::new`** (`game.rs:1485`, `Resolver::new(state.initiative_order(), ..)`) and
   `sync_timing_context` (`game.rs:7469`) never refreshes it, while `player_order` (`timing.rs:1563`) uses it in every non-speaker phase. A resolver rebuilt for a lent
   window from the *current* state (`state.initiative_order()` changes after strategy cards are chosen) would order reactions differently. The sync image must carry
   the resolver's own `initiative_order` (D2-4). Same family: `arm` (`reactions.rs:1060`) reads only factions of the seats (borrowed-agent and Ssruu registrations),
   so re-arming from the current state reproduces the registry, but that is a **VERIFY** with a test (I2 in D2-4).
7. **No cargo feature named `core-dual` exists** (docs 6.3/6.4 name one). The "core-dual wiring" of today is: default feature `typed-core` of `ti4-engine`
   (`crates/ti4-engine/Cargo.toml:30`, forced on for every dependent because the workspace dependency has default features), `dual.rs` (1 599 lines),
   `dual_window.rs` (3 602), `vote_dual.rs`, `production_window_dual.rs`, `kernel_sync/`, 91 `dual!`, 165 `dual_typed!`, 72 `dual_window!`, 2 `dual_query!` and ~18
   `dual_hook!` call sites (measured), the runtime `AreaMask`, and eight `string-<area>` features. D2-9 removes all of it from the production crate.
8. **Paths are hard-coded in tests and tools**: `tests/decision_delivery_inventory.rs` scans `CARGO_MANIFEST_DIR/src` and keys its registry by (file name, function,
   operation); `tests/content_ids_resolve.rs` scans `src`; `crates/xtask/src/content_gen/mod.rs:56` writes `crates/ti4-engine/src/generated`; `.cargo/config.toml` aliases list
   `-p ti4-engine`; `tools/engine_core/*` read `crates/ti4-engine/src`. The crate move (D2-7) must update all of them in the same commit (section 5, trap T10).

---

## 1. Facts the plan stands on (measured unless marked)

| Fact | Evidence |
|---|---|
| `CoreState` has 17 slices (`state/mod.rs:150`), derives `Clone`; `Rng` streams, decks, players are `Cow`/`Arc`; `Code(u16)` at `state/mod.rs:61`; `Atom::Blob(u32)` at `keys/atom.rs:44` | code |
| Text tables live in `Ctx` (`ctx.rs:142`): `owners: Owners` (sorted `Vec<String>`, max 16, `ctx.rs:14-60`), `texts: Texts` (`Vec<Box<str>>` + `HashMap`, max 65 535, `ctx.rs:90-136`), `keys: KeysCodec` (owner table in key form, `None` if one id is a proper prefix of another, plus `Blobs`) | code |
| `Code` users: 39 (core), `RngDomain::Dyn(u16,u32)` interns names outside the 22 literals (`codec/rng.rs:52-64`) | grep |
| `Unmapped` is built only by `bad()` (`core_sync/error.rs:21`); ~30 `bad(` sites in `codec/*.rs` + `conv.rs` (`conv.rs:12,23,30,53`) + `ctx.rs:25 (>16 owners)` + `keys` parse failures that end in `None` | grep |
| Production player ids: `player_<64 hex>` (server), `seat_NN` (`ti4-host/src/setup.rs:487`), `p{i}` (`setup.rs:580`, max 8): no id is a prefix of another | code + alpha init files |
| String `Resolver` (`timing.rs:446`): private fields `registry` (closures), `initiative_order`, `seating_order`, `active_player`, `speaker`, `phase`, `table`, `log`, `applied_events`, `relation_being_resolved`, `emission_stack`, `maximum_depth`, `used: UsedKeys` (`timing.rs:415`, `(String id, FrequencyScope)` + insertion order), `round_number`, `turn_number`, `participation`. No public accessor for `used`, counters or the emission stack | code |
| Core `Resolver` (`kernel/resolver.rs:339`) has `spend` (`:440`), `used_keys` (`:818`), `set_counters`, `set_emission_stack` (`:388`), `set_initiative_order` (`:407`), `checkpoint/restore` (`:544/:568`); `Driver` (`driver/mod.rs:84`) owns `resolver`, `table: AbilityTable`, `next_id`, `seeds` | code |
| Registrations happen only in `reactions::arm` (`reactions.rs:1060`, called once at `game.rs:1490`); no production `forbid` call outside tests | grep |
| `dual_window!` sites per file: combat 14, invasion 15, strategy_cards 12, relics 7, leaders 6, production 4, exploration 3, 2 each in action_cards/laws/game, 1 each in agenda_effects/keleres/reactions/secrets (72 in all) | grep |
| Core-sync gate bin `core_sync_gate CONFIG GAMES STRIDE [--strict]` exists (`ti4-engine-core-bridge/src/bin/core_sync_gate.rs`), is **not** called by `tools/engine_core/verify_package.sh` (it says the round trip "runs inside 1: F2's tests") | code |
| Round trip uses `slice_of_key` (`core_sync_gate.rs:30-60`): the map JSON key -> slice. `Map` and `Rng` codecs store nothing into `GameState` except `rng_seed` (`codec/map.rs:33`, `codec/rng.rs:96`): the typed map comes from the string `Galaxy`, the stream positions from `GameRng` | code |
| Replay API without engine changes: `ti4_host::replay::replay_session(&initial_state, galaxy, &records) -> ReplayReport{final_state,..}` (`ti4-host/src/replay.rs:94`); `ti4_server::storage::Storage::{new, load_init, load_decisions, load_history, load_snapshot}` (`storage.rs:736, 945, 964, 557`); recovery and snapshot checksum at `storage.rs:1034-1240, 1561` | code |
| Alpha preflight: `deploy/public/deploy.sh --preflight-only --ref <ref>` -> `recovery-check.sh preflight --bin SERVER --web-dir DIST --data-dir LIVE [--allow-failed ID]...` (copies LIVE without `feedback/`) | code |
| Crates that depend on `ti4-engine` today (module counts = distinct `ti4_engine::<module>` names): **production** host (48 modules, 321 uses), server (27, 157), wasm (5, 25), rule-bot (16, 190, linked into wasm), selfplay (6, 13); **tools/other** policy (28, 453), mlp (19, 273), training (13, 182), sim (11, 68), review (14, 54), replayer (6, 27), calc (4, 7), bridge (2, 4), advisor (3, 4), legacy-stub `ti4-legacy` (2, 2), cli (Cargo dep, 0 uses), engine-core-bridge (36, 97). `ti4-bot-agent` and `xtask` do not depend on it (xtask only writes into its `generated/`) | grep |
| Reads of `game.state` outside the engine: host 45, server 42, wasm 29, sim 29, training 67, mlp 90, review 24, selfplay 20; writes: host 1, wasm 1; reads of `game.timing`: host 2, server 4, review 4, training 1; `game.events`: ~30 | grep |
| Core test count in `#[test]`: engine 4 199, core+factions+bridge 270 (grep of the attribute only; proptests and generated tests not counted) | grep |

---

## 2. Sub-packages, dependencies, merge points, estimates

### 2.1 The split

Branch names `core-d2-<n>-<name>`, cut from the **newest `core-integration-N`** that exists when the package starts (it must contain `core-dual-shards`;
today only the area branches do, so a package started before int-4 asks the coordinator). One agent per package, own worktree.

| Pkg | Name | Goal in one line | Needs | Est. agent-h | Gate |
|---|---|---|---|---:|---|
| **D2-1** | `fixtures-totality` | `seated_dealt`; `Unmapped` counted per run and strict in gates; totality audit of every refusal; alpha-log round trip tool | int-4 | 4-6 | in G-prep |
| **D2-2** | `disabled-panics` | one central check for diplomacy / deck reserve / Bastion / Crimson; panics at the typed entry; sites list worked | `disable-xf123` merged; int-5 | 2-3 | in G-prep |
| **D2-3** | `text-tables` | `Code(u16)` and `Atom::Blob(u32)` tables and the owner table travel with `CoreState` (state-owned append-only interner) | int-4 | 3-5 | in G-prep |
| **D2-4a** | `resolver-sync` | `ResolverImage` export/rebuild/import between the string `Resolver` and core terms; churn test (rebuild every step) | int-4 | 3-4 | in G-prep |
| **D2-4b** | `resolver-in-state` | `ResolverState` in `CoreState` (L1), driver copies in/out, trace-id counter in state | D1a merged, D2-4a, D2-3 | 3-5 | in G-prep |
| **D2-5** | `reverse-lend` | `materialize`, `with_string`, bail table (label, footprint), wrappers per bailing label, footprint audit, churn mode | D2-3, D2-4a, int-5 (B12+B6c bail labels final) | 6-9 | G-prep |
| **D2-6a** | `allow-list` | allow-list file + matcher + stale-entry check on the existing `Difference` type | int-4 | 2-3 | in G-prep |
| **D2-6b** | `oracle` | legacy-vs-core harness in `ti4-engine-core-bridge` (golden, benches, alpha replays) using the allow-list | D2-7, D2-8 | 4-6 | G7 |
| **D2-7** | `legacy-split` | `git mv` of the string engine to `ti4-engine-legacy`, facade `ti4-engine`, Cargo/features/workspace, path-bound tests and tools | int-7 freeze (D1b + G6 merged) | 6-10 | cheap checks + bench hashes |
| **D2-8** | `flip` | `Game` wrapper owns `CoreState`; checkpoints = clone; `Knobs`; call-site rewiring; `Unmapped` becomes an error; disabled panics on | D2-1..D2-5, D2-7, D1b | 8-12 | G7 |
| **D2-9** | `prod-strip-final-gate` | remove typed-core/AreaMask/dual wiring from the production crate, wasm size, features, G7 incl. alpha preflight | D2-8, D2-6b | 4-6 (+ gate 3-4) | **G7** |

D2-1..D2-5 and D2-6a are the **prep block** (nothing in them changes behaviour; every step is checkable against the string engine). D2-7..D2-9 are the **flip block**.

### 2.2 Dependency graph, lanes, merge points

```text
now          int-5 (B12+B6c gated)                 int-7 (D1b/G6, freeze)
 |             |                                      |
 lane A: D2-1 -+--> D2-2 ---------------------------->|                       D2-7 -> D2-8 ---> D2-9 -> G7
 lane B: D2-3 -> D2-4a -> (D1a merged) -> D2-4b -> D2-5 --> G-prep (1 run) --->|      \-> D2-6b -/
 lane C: D2-6a ------------------------------------>|
```

* **Prep merges (M-prep):** D2-1, D2-3, D2-4a, D2-6a merge into the integration line as soon as each passes its cheap checks (they add code and tests, no behaviour). They
  touch disjoint files: D2-1 `fixtures.rs`, `core_sync/error.rs`, bridge bins; D2-3 `core_sync/ctx.rs`, `codec/*`, `keys/*`, core `containers/` and `state/mod.rs`; D2-4a new
  `resolver_sync.rs` and small accessors in `timing.rs`/`event.rs`; D2-6a new bridge file. D2-1 and D2-3 both touch `core_sync/` (see T12 for the order).
* **G-prep (one heavy run)** after D2-1, D2-3, D2-4a/b, D2-5, D2-6a are merged on one branch: `verify_package.sh none ...` with `SHARDS=4` under `flock /root/.ti4-heavy.lock`, plus
  the churn bench run (D2-5 item 14) and `core_sync_gate --strict` + `alpha_roundtrip` (D2-1). Pass criteria: 0 differences, `unmapped: 0`, bench 200/200 x6 equal with churn on, alpha round trip 0 differences.
  G-prep is optional in the user's gate rule ("gate when a block is complete") but the prep block is exactly such a block, and its defects are 10x cheaper to find before the flip.
* **Freeze (M-freeze):** `core-integration-7` = D1b/G6 + all earlier areas. From then no one but D2 touches `crates/ti4-engine/src`; nightly fixers fix the string side *after* the move in
  `crates/ti4-engine-legacy/src` and the package owner re-applies them to the core (policy 6.5 of `ENGINE_CORE_STATE.md`). Announce it when D2-7 starts, not before.
* **Critical path after int-7** (estimated, 1 agent per box, wall): D2-7 (8) -> D2-8 (10) with D2-6b (5) in parallel after D2-7 -> D2-9 (5) + G7 (4) = **~27-30 h**. D2-1..D2-6a (~33 agent-h)
  run in three lanes during D1b and are off the critical path if D1b really takes 20-30 h.
* **Calibration** (measured, `ENGINE_AREA_STATUS` header): window/stage work ran at 0.2-0.35 of document estimates, leaf work at 0.5-0.7, gates 1.5-2 h. The numbers above are
  already scaled; the least certain are D2-5 (one wrapper per bailing label: 15-40 labels expected) and D2-8 (touches `Game`, 138 methods; mostly mechanical once D1b has moved the window fields).

---

## 3. Sub-package checklists

Format: `file:line` is on `core-integration-3`. Every item ends with its test. "NOT" lists what to leave alone.

### D2-1 `core-d2-1-fixtures-totality`

**Definition of done.** (a) `fixtures::seated_dealt` exists and the three hand-built workarounds are gone. (b) `Unmapped` is counted by (slice, reason) and printed by every gate
tool; gate tools run strict and exit 1 on a nonzero count. (c) Every refusal site is classified *unreachable for production inputs* with a test, or fixed. (d) `alpha_roundtrip`
runs `store(load(g)) == g` over a copy of the alpha logs and prints 0 differences, 0 unmapped. (e) Player ids that the key codec cannot represent are refused at game creation.

1. **`fixtures::seated_dealt(seats, sources)`** in `crates/ti4-engine/src/fixtures.rs` after `seated_game` (`:227`): `let mut s = seated_game(seats, sources); promissory::deal(&mut s, ContentStore::embedded(), sources); s`
   (`promissory.rs:138`). Also `fixtures::real_game(players: &[&str]) -> GameState`: `fixtures::game` (`:25`, faction `generic`, `ti4-model/src/state.rs:686`) with the first N entries
   of `Faction::ALL` seated and notes dealt; lift the hand-written loop from `core_sync_gate.rs:~84-100` (function `fixtures`) into it and call it from there.
   Test: `Codec::load(.., SliceSet::ALL)` is `Ok` and `store(load(g)) == g` for `seated_dealt` with each of the 24 `LOBBY_FACTIONS` (3 seat tables), in `core_sync/tests.rs`.
2. Replace the workarounds that build generic notes by hand: `action_cards.rs:8143`, `vote.rs:2429`, `explore_write.rs:650` (PS-2 of `docs/ENGINE_PACKAGE_PREREQS.md`; re-grep `promissory_notes.clear\|generic`).
   Their tests must still assert the core half ran: `dual::thread_window_counts(label)` (`#[cfg(test)]`, `dual.rs:~1158`) -> `compared >= 1` (B6-design-02/07).
3. **Unmapped counter.** New `core_sync/unmapped.rs`: `pub struct UnmappedCounts { pub total: u64, pub by_reason: BTreeMap<(u8 /*Slice as u8*/, &'static str), u64> }`,
   `pub fn note(&Unmapped)`, `pub fn take() -> UnmappedCounts` (process-wide `Mutex`, for gate bins), `pub fn take_thread() -> UnmappedCounts` (thread-local, for tests; **T6**).
   `bad()` (`error.rs:21`) calls `note`. `core_adapter::Unmapped` (enum, `core_adapter.rs:39`) gets `note_adapter(&'static str)` at its constructors.
   `DualReport` (`dual.rs:392`) gets `unmapped: BTreeMap<String, u64>` merged in `merge` (`dual.rs:464`) and printed in `Display`; the shard merge of `dual_run --merge`
   (branch `core-dual-shards`) must sum it. Test: a deliberately refused state (`fixtures::game(&["a","b"])` with its `generic` faction) increments `by_reason[(Players,"faction")]`.
4. **Strict mode.** `core_sync::set_strict(bool)` (default off; `TI4_UNMAPPED=strict` env read once, native only). Strict makes the *first* `Unmapped` in the process panic with slice, reason, text (so the
   backtrace names the call site). `dual_run`, `core_sync_gate`, `board_equivalence`, `decision_corpus`, `alpha_roundtrip` call `set_strict(true)` first. Unit tests that refuse on purpose use
   `core_sync::allow_unmapped(|| ..)` (thread-local guard). **NOT**: do not change any `.ok()?` bail logic in the adapters; the counters observe it.
5. **Refusal audit.** For every `bad(` site list (slice, reason, reachable?) as a table in the module doc of `core_sync/error.rs` (not a new .md). Expected classification (my reading, **VERIFY** each):
   * `ctx.rs:25` >16 owners: unreachable (max 8 players, owners are players). Test: `Owners::of` on an 8-seat game with plots, captured units and support holders has <= 9 entries.
   * `codec/seats.rs:19-22` >8 players: unreachable (`ti4-host` `PLAYER_COUNTS = 2..=8`, `setup.rs:24`, `max_players` at `:395`). Add `const _: () = assert!(MAX_PLAYERS <= 8)` next to the host constant.
   * owner id a proper prefix of another (`ctx.rs:~155 key_owners`): unreachable for generated ids; **make it unreachable by construction**: new `core_sync::ids_supported(&[PlayerId]) -> Result<(), String>` (<= 8,
     no proper prefix, all ASCII) called from `ti4_host::setup::create` (the single creation point of server lobby start and wasm new game; check `ti4-wasm/src/host_setup.rs` for a second) and from `ti4_server` lobby validation (`storage.rs:325`).
   * faction `generic` (`conv::id::<Faction>`): reachable only through `GameState::new`/`fixtures::game`. Add a check in the future `Game::with_state` entry (D2-8) and today in `ti4_host::setup::create`: every
     seated faction is in `Faction::ALL`.
   * `codec/marks.rs:23,25` mark not typed / not canonical: **the one open set.** Add `tools/engine_core/marks_census.py` (text-based like `module_graph.py`): collect every
     `faction_marks.insert|entry|remove` key template in `crates/ti4-engine/src` and check it against the patterns of `keys/marks.rs`; print unknown templates, hand-review. The golden/alpha runs are the dynamic check.
   * `codec/agenda.rs:23,31,76`, `cards.rs:20-23`, `explore.rs:43`, `board.rs:82,170,173`, `transient.rs:83,185`, `ledger.rs:29`, `pending.rs:21`, `conv.rs:23,30`: custom content / corrupted input only.
     Each gets one test that feeds the offending input and expects the specific reason, plus a line in the table saying "unreachable for generated content".
   * `codec/map.rs:22-24` (`typed_map(galaxy)` None): **VERIFY** hyperlane maps and map edits (`laws::apply_to_galaxy`, `movement::replay_map_edits`) are expressible: `template_openings()` (`workload.rs:176`) returns
     `(positions, refused)`; require `refused.is_empty()` for every template of `ti4_host::maps::template_loader`, also with Thunder's Edge.
   * `codec/cards.rs:44-47` deck reserve and `codec/trade.rs:416,423` diplomacy: disabled features (D2-2).
6. **Alpha round trip.** Refactor the per-position check of `core_sync_gate.rs` (steps 1-3 of its header) into a lib module `ti4_engine_core_bridge::roundtrip::{check_position, Stats}`; `core_sync_gate` calls it (same output).
   New bin `ti4-engine-core-bridge/src/bin/alpha_roundtrip.rs`, args `DIR [--every K] [--strict]`, add `ti4-server.workspace = true` to the bridge dependencies (no cycle: server does not depend on the bridge).
   Procedure per `game_*` in DIR (**DIR is always a copy**: `D=$(mktemp -d); rsync -a --exclude feedback /var/lib/ti4-public/ $D/` mirrors `recovery-check.sh`; the tool refuses a DIR that is
   `/var/lib/ti4-public` itself):
   1. `Storage::new(DIR)`, `load_init(id)`, `load_decisions(id)`, `load_history(id)` if present; skip games whose history has non-empty `rng_marks` (turn redo, disabled; print them as skipped).
   2. Galaxy as `recover_from_init` builds it (`storage.rs:~1050-1190`: from `map_tiles`/`map_template`; **VERIFY** the helper to call).
   3. For `k` in `0, K, 2K, .., n` (default K=1; total work is sum n^2/2 ~ 29 000 replayed decisions for today's 14 games, seconds): `replay_session(&init.initial_state, galaxy.as_ref(), &decisions[..k])`
      -> `final_state` -> `check_position`. Also check `snapshot.json`'s state when present.
   4. Print per game: positions, differences, unmapped by reason, max owners, text-table and blob-table high-water marks (D2-3), rules version; total line; exit 1 on any difference or (strict) unmapped.
   Test: a unit test over a tiny synthetic data dir (one `init.json` written by `Storage::save_init` of a `seated_dealt` game and 3 recorded decisions).
7. **Ratchet for later:** the same tool runs in G-prep and G7 on a *fresh* copy (the live directory grows).

**NOT touched:** `ti4-model/src/state.rs`, any rule behaviour, `RULES_*` constants, the existing `.ok()?` fallbacks, `fixtures::game` itself (393 users; it stays `generic`).

### D2-2 `core-d2-2-disabled-panics`

**Definition of done.** One function decides "this position uses a switched-off feature"; the typed entry points refuse such a position with a panic naming the family and
`docs/DISABLE_XF123.md`; all core and adapter sites listed in `DISABLE_XF123.md` section 5 are either unreachable-by-construction (comment + `debug_assert!`) or removed in the
flip; no golden/bench/template/alpha position trips it.

1. Base must contain `disable-xf123` (the product edges) **and** the B12/B6c merge (int-5), else the site list below is stale. Re-run the greps of `DISABLE_XF123.md` 5 and diff against this list.
2. New `core_sync/unsupported.rs`: `pub enum Disabled { Diplomacy, DeckReserve, LastBastion, Crimson }`, `pub fn find(gs: &GameState) -> Option<Disabled>`: `gs.diplomacy.enabled`,
   `gs.deck_reserve.is_some()`, any `p.faction` in `{"bastion","crimson"}`. `pub fn refuse(gs)`: `panic!("{:?} is switched off (docs/DISABLE_XF123.md); use ti4-engine-legacy", d)`.
   Policy switch `core_sync::set_disabled_policy(Policy::{Bail, Panic})`, default **Bail** until D2-8 flips the default (so dev tools `ti4-training --diplomacy`, `ti4-review`, `--roster wide`
   keep working on the string engine with the lends bailing as today). Gate bins set `Panic` (a golden position must never trip it; `DISABLE_XF123.md` 1.2 shows they cannot).
3. Call `unsupported::check(gs)` at the top of `Lend::with_core`, `Lend::region`, `Lend::view` (`lend.rs:88,102,117`), `CoreView::of` (`:153`), `Codec::load` (`codec/mod.rs`) and
   `dual_window::tables`/`lend` (`dual_window.rs:761,775`) -> `Bail`: return `Err(Unmapped{reason:"disabled feature"})` (counted); `Panic`: panic.
4. Sites from `DISABLE_XF123.md` 5 (all on the core side or string-side guards that become dead):
   * XF-3: `production_view.rs:39 unported_economy` (callers `:64,:84,:106,:293`), `supply_core.rs:28`, `production_window_dual.rs:259`, `dual_window.rs:~1611`; core `kernel/hooks/direct.rs:68-127`
     (`bastion_*`, `crimson_*`), `ti4-engine-core-factions/src/ca_combat/{bastion,crimson}/*`, `ca_combat/mod.rs:13-21`, `cb_economy/winnu/query.rs:111`, `cards/mod.rs` (Skilled Retreat with Last Bastion).
   * XF-2: `core_sync/codec/cards.rs:44-47`; `explore_write.rs:49,152,301`, `dual_window.rs:919,990`, `objectives_write.rs:90,98,106`, `strategy_cards_core.rs:461,486,550`;
     core `rules/action_cards/effects.rs:208` (`Fx::Veto && cards().reserve.is_some()`), `:779`.
   * XF-1: `explore_write.rs:304`, `dual_window.rs:788,1547,2325`, `reactions_core.rs:261`, `leaders_core.rs:141`, `transactions.rs:1193,2134`, `game.rs:2381,5855` (string side stays);
     core `query/trade.rs:209`, `rules/diplomacy.rs:94,148`, `rules::diplomacy_book`, the `DiplomacyWindow` in `core-b4-trade`.
   Rule for this package: **do not delete** the gated core code (diplomacy rules, Bastion/Crimson rows); add `debug_assert!(unsupported::find(..).is_none())` at the entry of the three
   biggest (`rules/diplomacy.rs`, `direct.rs` bastion/crimson group, `effects.rs:208`) and a doc line "dead after D2-8; delete in D4". Deletion removes tested, gated code and is a D4 clean-up.
5. Tests: one per family, `Policy::Panic` + `#[should_panic(expected = "DeckReserve")]` etc.; one per family in `Bail` mode asserting the counted `Unmapped`. The ~30 engine test files that
   build such tables with `seated_game(.., "bastion")` / `diplomacy ... true` / `deck_reserve = Some` (list: `grep -rl` on `"bastion"|"crimson"|enabled = true|deck_reserve = Some` in `ti4-engine/src`,
   `ti4-engine-core*`) keep running on the string side because the default is `Bail`; they move to `ti4-engine-legacy` untouched at D2-7. In core crates, tests that construct the typed form
   directly (`Faction::Bastion`, `Diplomacy { enabled: true }`) are unaffected unless they go through `Codec::load`.
6. Golden check (cheap, no gate): `core_sync_gate` and `dual_run` in `Panic` policy over profile-20 (20 games) and the template openings: 0 panics.

**NOT touched:** `ti4_host::disabled` and the product edges (already on `disable-xf123`), selfplay/training/mlp/review options, `seating::wide_roster`.

### D2-3 `core-d2-3-text-tables`

**Definition of done.** `CoreState` carries `texts: Texts { codes: Interner, blobs: Interner }` and the owner table `owners: Arc<OwnerTable>` (ids as bytes, text order); the codecs read and write
them through `core`, not through `Ctx`; `Ctx` keeps only the *derived* parts (`Owners`/`keys.owners` are rebuilt from `core.owners`); a clone of `CoreState` shares the base of the tables;
a code minted in one lineage resolves in that lineage and nowhere else; `Blobs::intern` is no longer linear; the string-visible behaviour is bit-identical.

Design (decided): **state-owned, append-only, base + local suffix**, not a process-global shared table.
```rust
// ti4-engine-core/src/containers/interner.rs   (L0; bytes, not str: lint_core forbids String/&str)
#[derive(Clone, Default)]
pub struct Interner { base: Arc<Vec<Box<[u8]>>>, local: Vec<Box<[u8]>> }
impl Interner {
    pub fn intern(&mut self, bytes: &[u8]) -> Option<u32>;      // None when the table is full (caller chooses the max)
    pub fn get(&self, i: u32) -> Option<&[u8]>;                 // base then local
    pub fn len(&self) -> usize;
    pub fn compact(&mut self);                                   // local -> new base Arc; call at a step boundary when local.len() > 64
}
impl PartialEq for Interner { /* by content in order, not by pointer */ }
```
Reason: a global table shared by forks (rollouts, rule-bot search) grows with every fork's novel text and has nondeterministic code order across threads; state-owned makes code assignment a function of the
state's own history, equality sound, clone cost = `Arc` bump + a short suffix. `Code(u16)` = index into `codes` (max 65 535 as today, `ctx.rs:116`); `Blob(u32)` = index into `blobs`.

1. Add `Interner` + proptests (clone then diverge: both lineages resolve their own and the shared base; equal histories give equal state; `compact` keeps indices) in `containers/`. Lint: no `String`, no `format!`.
2. `state/mod.rs:150`: add `pub texts: Texts` (always valid, **not** a `Slice`, so `loaded`/footprints are untouched; accessor without the `debug_assert`). `Default` pre-interns `Code(0)` = `DIPLOMACY_RULES_V1`
   (`ctx.rs:95-101`; keep: a default `Diplomacy` must equal a loaded default `DiplomacyState`, even though diplomacy is disabled). Also add `owners: Arc<OwnerTable>`; `OwnerTable` = sorted ids as bytes, set by `Seats` load, immutable afterwards.
3. Move `Texts` (`ctx.rs:83-136`) and `Blobs` (`keys/text.rs:59-88`) onto the core type; `Ctx.texts` and `KeysCodec.blobs` are removed. Signatures change mechanically:
   `Texts::intern(slice, text)` -> `core.texts.codes.intern(..)`; `KeysCodec::parse_*(.., blobs: &mut Interner)`, `render_*(.., blobs: &Interner)` (`keys/mark_values.rs`, `marks.rs`, `other.rs`, `text.rs:199,242`);
   `codec/rng.rs:52-64` `domain(cx, text)` -> `domain(core, text)`, `TextSeeds(&Ctx)` -> `TextSeeds(Interner clone)` (`lend.rs:17-20`). Codec `load` already gets `core: &mut CoreState`; `store` gets `&CoreState`.
4. `Ctx::of`/`refresh` (`ctx.rs:155,167`): owner table now comes from `core.owners` when a core exists. Today's per-lend `Ctx::of(gs)` (44 sites) keeps working: it builds the table for a fresh lend and the first `Codec::load`
   fills `core.owners`/`core.texts`; no call site changes in this package.
5. A mid-game new owner (`refresh` finding `owners != self.owners`, `ctx.rs:167`) becomes an `Unmapped` ("owner not in the game's table") instead of silently rebuilding: players are fixed for a game.
6. Tests: (a) `store(load(g)) == g` and `load(store(x)) == x` unchanged over the existing `core_sync/tests.rs` + `core_sync_gate` golden positions; (b) **cross-lend stability**: load g, run a ported mutation that
   interns a text, `store`, `load` again from the stored state with the *same* `CoreState.texts` carried over: same code for the same text; (c) clone divergence; (d) 65 535 limit returns `Unmapped`, not a panic;
   (e) high-water report: `alpha_roundtrip` and `core_sync_gate` print `texts.len()` / `blobs.len()` maxima; expect both < 1 000 over a full golden game; **a bigger number means some text embeds a game-varying number and the
   set is not closed: stop and report** (T8).
7. Trap to test explicitly: `Kernel.seeds` holds a *clone* of the interner taken before the call, so a `Dyn` rng domain minted *during* a call resolves to an empty text and silently draws from the wrong seed
   (`codec/rng.rs:66 domain_text(..).unwrap_or_default()`). Production rules never mint `Dyn` (all 22 domains are literals; **VERIFY** with grep for `RngDomain::Dyn(` outside `codec/` = 0, measured: 0). Add a debug-assert in `Rng::stream` that
   a `Dyn` domain's code exists in the seeds' table.

**NOT touched:** `Code` users' semantics (X1 closes sets later), the `Slice` enum, `ti4-model`.

### D2-4a `core-d2-4a-resolver-sync`  (the most delicate piece)

**Definition of done.** For every golden/bench/template/alpha game, rebuilding the string `Resolver` from its core-term image before *every* `Game::step` leaves all 200+200 bench decision hashes and the
20 golden dual runs unchanged ("churn test"), and an explicit comparison of the image before/after a lent window shows the same used keys, counters, emission stack, log delta and applied events.

What a lent string window can read or change in the resolver (read off `timing.rs:446-463` and `game.rs:7469 sync_timing_context`):

| Field | Source of truth after the flip | In (core -> string) | Out (string -> core) |
|---|---|---|---|
| `registry` (closures) | derived: `reactions::arm(&mut r, &state)` from the materialised state | rebuild | nothing |
| `initiative_order` | **its own value from `Game::new`** (T4); not `state.initiative_order()` | copy | nothing (never changes) |
| `seating_order`, `active_player`, `speaker`, `phase`, `round_number`, `turn_number` | the state (`sync_timing_context`: `set_phase`, `sync_lifecycle(round, turn_seq)`, `set_seating_order`, `set_active_player`, `set_speaker`) | derive exactly as `game.rs:7469` | nothing |
| `used` (ordered `(ability id, Trigger(event id) / Turn(n) / Round(n))`) | image | `spend` each key in order | the ordered suffix added during the window |
| `emission_stack`, base depth | image (a window opened inside an emission) | `set_emission_stack` | nothing (restored) |
| `applied_events` | image (read by `supply.rs:1921,2024`) | copy | delta |
| `log` | not synced in; the lend gets an empty one | empty | delta, handed to the events view (see P1) |
| `relation_being_resolved`, `maximum_depth`, `participation` | defaults | defaults (**VERIFY** where `set_participation` is called: `grep -rn set_participation crates/ti4-engine/src` shows only `timing.rs`, so it is `None` in production) | nothing |
| `table: Table` (deciders, decision log) | stays in the wrapper | move the `Table` out and back, never rebuilt | n/a |
| event id counter (`EventSequence`, `event.rs`) | image | seed the string `EventSequence` | read back | 

1. **String-side accessors (small, additive, in `timing.rs`/`event.rs`; legacy code stays frozen after D2-7):** `Resolver::used_keys() -> Vec<(String, FrequencyScope)>` (ordered), `spend_key(String, FrequencyScope)`,
   `emission_stack() -> &[(String,u64)]`, `set_emission_stack_text`, `initiative_order() -> &[PlayerId]`, `set_initiative_order`, `round_turn() -> (u64,u64)`, `registry_ids() -> Vec<String>` (ordered, for I2),
   `EventSequence::{peek, set}`. These are `pub(crate)`; no behaviour change. Mind the `ResolverSnapshot` FORK RULE (`timing.rs:465`): the snapshot destructures every field without `..`; the new accessors add no field.
2. **New `crates/ti4-engine/src/resolver_sync.rs`** (typed-core only):
   ```rust
   pub struct ResolverImage { pub used: Vec<(AbilityId, UsedScope, u64)>, pub initiative: Vec<Seat>, pub emission_stack: Vec<(EventKind, u64)>, pub applied: Vec<EventRec>, pub next_event_id: u64 }
   pub fn export(r: &timing::Resolver, seq: &EventSequence, render: &Render) -> Result<ResolverImage, Unmapped>;
   pub fn rebuild(state: &GameState, img: &ResolverImage, table: Table, render: &Render) -> timing::Resolver; // Resolver::new(initiative..) + reactions::arm + sync as game.rs:7469 + spend in order + stack
   pub fn delta(before: &ResolverImage, after: &timing::Resolver, ..) -> ResolverDelta;                       // used suffix, applied delta, log delta, next id
   ```
   Ability-id text <-> `AbilityId` uses the existing `kernel_sync/shim.rs` renderers (`ability_text`, `event_text`; the dual shim already prints `used` as `"{ability}|{scope}({n})"`, `shim.rs:229`).
3. **Tests (all must be written before the churn mode):**
   * I1 round trip: `export(rebuild(export(r))) == export(r)` on the resolver of every golden position (stride 10) and after nested-question rollbacks (`game.rs:3277-3325`).
   * I2 registry identity: `rebuild(..).registry_ids() == long_lived.registry_ids()` at every golden position; any difference is a blocker (it would mean `arm` depends on mid-game state).
   * I3 order identity: for 200 random positions, `player_order()` of rebuilt equals long-lived, per phase (catches T4).
   * I4 Trigger-scope keys: ids come from the carried `next_event_id` (a fresh counter would collide with spent `Trigger(n)` keys).
   * I5 checkpoint: rebuild only at step boundaries (no `Checkpoint` is outstanding there); test that `checkpoint()`/`restore()` on a rebuilt resolver undoes only keys added after the rebuild.
4. **Churn hook** (`Game::step`, `game.rs:2056`, first statement): `if churn::resolver() { self.timing = resolver_sync::rebuild(&self.state, &export(..), take_table(&mut self.timing), ..) }`, switched by
   `TI4_CHURN=resolver` (native env, read once; wasm: compile-time false). Acceptance: bench `tools/engine_profile/verify_benches.sh` 200/200 x6 identical with `TI4_CHURN=resolver`, and the 20 golden games on `dual_run` with all areas `dual`: 0 differences.
5. Document the table above in the module doc of `resolver_sync.rs` (it is the contract D2-4b and D2-5 use).

**NOT touched:** window ordering rules, `timing.rs` algorithms, the core `Resolver` algorithm; `Driver::new` signature (D2-4b).

### D2-4b `core-d2-4b-resolver-in-state`  (after D1a is merged)

**Definition of done.** The core resolver's persistent parts are in `CoreState`; a driver step reads them in and writes them back; the golden dual compares the *state's* resolver image with the
string's after every window (so D1's per-step comparison extends to `used`).

1. `state/resolver.rs` (**L1**; the layer checker forbids `state` importing `kernel`): `ResolverState { used: SmallList/Vec<(AbilityId, UsedScope, u64)>, initiative: SmallList<Seat,8>, next_id: u64, emission_stack: Vec<(EventKind,u64)> }`.
   `AbilityId` is in `choice::reaction` (L1) and `EventKind` in `state/ledger.rs`: both allowed. `UsedScope` currently in `kernel/resolver.rs:~830`: move to `state/resolver.rs`, re-export from the kernel (keep paths).
2. `Resolver::{from_state(&ResolverState, Registry/Cow<Registry>) , into_state(self) -> ResolverState}` in `kernel/resolver.rs`; the `Resolver` struct is unchanged internally (copy in/out per window; `used` is short).
   The registry stays **derived** (`AbilityTable::registry(state, order, name)`, `kernel/ability.rs:425`), cached in the `Driver` per game, rebuilt on a seat/faction change (never mid-game).
3. `Driver::new(window, resolver, table, next_id)` (`driver/mod.rs`): callers pass `Resolver::from_state(&state.resolver, ..)` and `state.resolver.next_id`; `Driver::finish` stores them back. `ENGINE_CORE_DRIVER.md` 2 "D2 moves them into the state" is this item.
4. `Codec::load/store` for the new field: it is part of `Slice::Pending` (D1 owns `Pending`): add to that codec, with the image of D2-4a as the string side. Footprint of every window that emits adds `Pending`.
5. Tests: after each ported window in the golden dual `state.resolver == export(game.timing)` (plug into `dual_window::run_dual_window`); `CoreState::clone` equality includes it.

**NOT touched:** window logic, `AbilityTable`, event kinds.

### D2-5 `core-d2-5-reverse-lend`

**Definition of done.** `Game::materialize()`, `Game::with_string(fp, label, f)` and a generated bail table exist; for every window label that bails in the final golden/bench dual (expected 15-40 labels), a
wrapper calls the legacy function with the lent state; the **churn mode** (`TI4_CHURN=state`: `state := materialize(load(state))` plus rebuilt resolver, rng, galaxy before every step) keeps all bench hashes; a footprint
audit proves no lent body writes outside its declared slices.

1. **`materialize`.** `core_sync::materialize(core: &CoreState, cx: &Ctx, base: &GameState) -> R<GameState>`: `base` is the cached last materialisation (or the `init` state at game start); `Codec::store` of all slices into a clone.
   Not covered by `GameState`: the typed map (`Map` stores nothing) -> `materialize_galaxy(core.map) -> Galaxy` (**VERIFY** a typed-to-string galaxy exists; if not, the wrapper keeps the string `Galaxy` it already re-derives each step,
   `game.rs` `galaxy` field + `laws::apply_to_galaxy` + `replay_map_edits`, and re-derives it from the materialised state: that is what `Game::step` does today); stream positions: `codec::rng::positions(cx, &core.rng) -> RngPositions`
   for `GameRng::set_positions`; dice history = `ledger.dice`.
2. **Cache + revision (D8).** The wrapper holds `materialized: OnceLock<Arc<GameState>>`, invalidated by every `&mut self` method (`step`, `answer`, `with_string`). `state(&self) -> &GameState` lazily fills it. This replaces the public field
   `Game::state` (~350 read sites outside the engine, 2 writes; D2-8 sweeps them). `OnceLock`, not `OnceCell`: the server shares `&Game` across a worker (`Sync`).
3. **`with_string`.** `fn with_string<T>(&mut self, fp: Footprint, label: &'static str, f: impl FnOnce(&mut StringCtx<'_>) -> T) -> Result<T, Unmapped>` with
   `StringCtx { state: &mut GameState, galaxy: Option<&Galaxy>, timing: timing::Resolver, rng: GameRng, dice: Dice, seq: EventSequence, table: &mut Table }`.
   Steps: clone the cached `GameState`; `resolver_sync::rebuild` (D2-4a); `GameRng` from positions; run `f`; `Codec::load_slice` for every slice in `fp.writes` back into `core` (codecs already exist); apply `ResolverDelta`, rng
   positions, dice and event deltas. **Correctness does not depend on the footprint**: the string function sees the *whole* materialised state; the footprint only limits what is loaded back. An unknown label uses `Footprint::ALL`.
4. **Questions inside a lent body.** A string body that asks cannot suspend. Use a tape decider (`StringTape: Decider`) set as the `Table` default; precedent `ReplayDecider` (`ti4-host/src/replay.rs:41-80`) and the `Recording` of `dual_window.rs`.
   When the tape is dry the decider returns `IllegalChoice::DeciderFailed`; `with_string` rolls back (drop the clones), returns `Lent::Asking(choice)`; the driver rolls back to the window start, reports the question and re-runs with the
   answer appended (`ENGINE_CORE_DRIVER.md` 1 "Two kinds of question"). Determinism requirement 3 of `ENGINE_AREA_STATUS` 2.2: the body reads nothing outside (state, window struct, tape): the observers (`aftermath_observer`,
   `strategic_observer`), `blocked`, `GameRng` side channel (`rng_force`, host-only) are not reachable inside a lent body; if one is, the label is not lendable (report it).
5. **Bail table.** `crates/ti4-engine/src/lend_table.rs` (hand-checked, partly generated): `pub struct Bail { label: &'static str, area: Area, writes: SliceSet, reads: SliceSet, ids: &'static [&'static str] /* e.g. "B12-bail-02" */, test: &'static str }`, `pub const BAILS: &[Bail]`.
   Generation: env `TI4_BAIL_PROBE=1` in `run_dual_window`/`run_dual` (`dual.rs`): when the core side bails (None) after the string closure ran on its copy, record `(label, slices whose JSON keys changed between the pre and post copy)` using `slice_of_key`
   (lift it from `core_sync_gate.rs:30` into `core_sync::slice_of_field`). Run the sharded golden dual once (it is part of G-prep anyway); a small bin `bail_table` merges shard outputs (union per label) and prints the Rust table.
   Observed writes are a lower bound, so the **audit below is the safety net**, and unknown/extra writes widen the entry by hand. Tests: `every_bailing_label_has_an_entry` (compares `DualReport.windows` with `bailed > 0` against `BAILS`) and
   `no_entry_is_stale`.
6. **Wrappers.** One per label in `BAILS` with `bailed > 0`: `lent::<label>(game, window_args) -> Lent<T>` calls the legacy *enclosing* function with the dual mask forced to `String` (`dual::with_mask(AreaMask::STRING, ..)`) so no extraction of the
   closure inside `dual_window!` is needed. The argument builder is the part that costs time: the windows with plain-data state (the `Fields` of B1g, the `Snap` of B4d) are cheap (~10 min); windows whose state is a `Game` field with
   nested stage enums are D1b's job (not lendable, `ENGINE_AREA_STATUS` 2.2 test 2). Emit the list of labels that need an argument builder in the PR description with the estimated minutes.
7. **Footprint audit** (debug and `Dual`/churn runs): after the lent body, `load` *all* slices from the post-state and require the slices not in `fp.writes` to be equal to before (compare per slice with `slice_of_field`). A violation panics with label + slice.
8. **Churn mode `state`** (`TI4_CHURN=state`, native env, same hook as D2-4a item 4): at the start of `Game::step`, `self.state = materialize(load(self.state))`, positions of `GameRng` through the codec, galaxy re-derived. Acceptance: bench 200/200 x6 identical,
   `dual_run` golden 0 differences, and `alpha_roundtrip` still 0. This is "the flip, simulated": the string engine runs on state that went through the core every step.
9. **Events produced by a lent body** (`Game::events` pushes, resolver log lines) are returned as text and appended to the events view as already-rendered lines; their relation to typed events is the P1 package's design (`docs/todo/P1.md`): until P1
   lands, the dual compares nothing here. Add a TODO marker referencing P1; do not invent a second event model.

**NOT touched:** legacy rule bodies (call them, do not edit them), the dual comparison rules, `Footprint` semantics for ported windows.

### D2-6a `core-d2-6a-allow-list`

**Definition of done.** `allow_list.json` + matcher + stale-entry check exist and are unit-tested on synthetic differences; the file is empty (no intended difference exists before D2; question 4 of `ENGINE_AREA_STATUS` and `A6` of KNOWN_ISSUES).

1. `crates/ti4-engine-core-bridge/src/allow_list.rs` and `tools/engine_core/allow_list.json`:
   ```json
   { "version": 1, "entries": [ { "id": "AL-001", "reason": "...", "ticket": "B1", "rules_version_min": 36, "rules_version_max": null,
       "area": "combat", "label": "combat::roll_*", "path": "players[*].relics*", "scenario": "*" } ] }
   ```
   Every field except `rules_version_max`/`scenario` is required; `reason` and `ticket` non-empty; `rules_version_min` mandatory (a stale entry hides regressions, KNOWN_ISSUES D13).
2. Matching (`fn matches(&Difference, rules_version, scenario) -> Option<&Entry>`): area equal, label glob (`*` at the end or whole), JSON-path glob (`[*]` = any index, `*` = any key suffix) against `Difference.path` (`dual.rs:340`),
   version in `[min, max)`. A difference that matches is counted per entry and does not fail; any other fails.
3. **Stale check:** `--check-stale` (used in full gate runs, not in partial runs) fails when an entry matched 0 differences; the full-run summary prints hits per entry. A unit test proves both directions.
4. Where the oracle reads it: D2-6b. Where the first entries come from (not now): the three copied string bugs, Heart of Ixth (KNOWN_ISSUES B1), Harrow (B2-K7), Xxekir Grom (B7-copy-01), each only together with a `RULES_VERSION` bump after D2.
5. Not an allow-list item: configurations the core refuses by policy (diplomacy on, reserve, Bastion/Crimson). The oracle *skips* those scenarios (a filter in the harness, counted and printed), it does not allow-list differences for them.

### D2-6b `core-d2-6b-oracle`  (after D2-7 and D2-8)

**Definition of done.** `oracle` bin in `ti4-engine-core-bridge` plays each scenario on the new engine and replays the same answers (by option id) on `ti4_engine_legacy`; after every step compares the choice (ids, prompt, `DecisionContext::canonical`),
the materialised `GameState` JSON, the new events, rng positions and the decision hashes V1/V2; reports through the `DualReport` format so `dual_sharded.sh` / `--shard i/N` / `--merge` keep working; applies the allow-list; exits 1 on an
unallowed difference or (full run) a stale entry.

1. Cargo: `ti4-engine-legacy = { path = "../ti4-engine-legacy" }` as a normal dependency of the bridge (legacy compiled with its own `typed-core` code but run with `AreaMask::STRING`, see D2-7 item 5).
2. Corpora: golden 20 (`tools/engine_profile/profile-20.json`), bench configs `tools/rulebot/golden*.json` (v1..v5, rules versions 1-5, 23, current), template openings, `decision_corpus` replay, **alpha replays** (`replay_session` on both engines
   from the alpha copy; the 14 replayable games).
3. Scenario filter: skip + count scenarios with `unsupported::find(..).is_some()` (D2-2).
4. Replacement of today's `dual_run`: keep the binary name and shard protocol; `AREAS` argument is dropped (there is only one comparison now).
5. Tests: the harness compared with itself (legacy vs legacy) reports 0; an injected one-field difference is reported with its path; an allow-listed one is not.

### D2-7 `core-d2-7-legacy-split`  (flip block, after the freeze)

**Definition of done.** The string engine lives in `crates/ti4-engine-legacy` (package `ti4-engine-legacy`, lib `ti4_engine_legacy`), history preserved (`git mv`); a new `crates/ti4-engine` is a facade that re-exports it; **every dependent compiles
unchanged**; all path-bound tests and tools point at the new path; bench hashes, `cargo xtask gen-cards --check`, `check_layers.py`, `lint_core.sh` unchanged. One commit for the move, one for the facade, one per path fix; no rule change.

Design (decided, question 3): **whole-crate move + facade first, shrink later**, not per-area module moves. Reason: 118 modules reference each other through `Game`, `GameState`, `Choice` and `TimingContext`; moving areas one by one
needs the dependency direction between the halves before either half exists. The facade makes the move mechanical and keeps the product building at every commit; D2-8/D2-9 then replace facade re-exports by wrapper code module by module.

1. **Inventory (1-2 h, produces the decisions):** `python3 -I tools/engine_core/boundary_inventory.py --json $SCRATCH/inv.json`; classify the 118 `pub mod`/`mod` lines of `lib.rs` into (a) **wrapper-owned** (wire/glue: `choice`, `decision_context`, `fingerprint`, `event`, `rng`, `dice`, `observation`, `render`, `core_sync`, `fixtures`,
   `registry`, `generated`), (b) **string helpers production consumers call on a `GameState`** (host 48, server 27, rule-bot 16, selfplay 6, wasm 5 modules; e.g. `objectives`, `production`, `technology`, `tactical`, `fleet_strength`, `valuation`, `planets`, `seating`, `setup`, `movement`),
   (c) **rule bodies only the game loop calls** (`combat`, `invasion`, `strategy_cards`, `action_cards`, `agenda*`, `relics`, `exploration`, `leaders`, `factions/*`, `laws`, `reactions`, `timing`, `transit`, `vote`, ...). (b) stays available in production until D3 (bots keep the string interface),
   so those modules are the ones that must be *copied* out of legacy in D2-9 if legacy leaves production. Record the classification in `crates/ti4-engine/src/lib.rs` as a table comment; do not create a document.
2. `git mv crates/ti4-engine crates/ti4-engine-legacy`; edit its `Cargo.toml` (`name = "ti4-engine-legacy"`); workspace `Cargo.toml`: members and `[workspace.dependencies]` get `ti4-engine-legacy`; dependents unchanged for now.
3. New `crates/ti4-engine/` with `Cargo.toml` (`name = "ti4-engine"`, dependency `ti4-engine-legacy`, features forwarded: `default = ["typed-core"]`, `typed-core`, `ts = ["ti4-engine-legacy/ts"]`, the eight `string-*`) and `src/lib.rs` = `pub use ti4_engine_legacy::*;`. **VERIFY** that `#[macro_export]` macros (`dual!`, `dual_window!`, ...) and all `pub mod` paths used by
   dependents resolve through the glob (`cargo check --workspace` is the test); add explicit `pub use ti4_engine_legacy::{dual, dual_typed, dual_query, dual_window, dual_hook};` if not.
4. Path-bound artifacts (T10): `tests/decision_delivery_inventory.rs` and `tests/content_ids_resolve.rs` moved with the crate keep scanning their own `src` (now legacy): unchanged and green. The new `ti4-engine` has no `Choice` producers yet, so it needs no registry until D2-8.
   Update: `crates/xtask/src/content_gen/mod.rs:56` (`adapter_out`) and `emit.rs:91` comment; `crates/ti4-engine-core/Cargo.toml:13` comment and `tests/adapter.rs`; `.cargo/config.toml` aliases (`c-web`, `l-web`, `t-web`, `t-web-slow`: add `-p ti4-engine-legacy` to the test aliases;
   keep the checking aliases on `ti4-engine`); `tools/engine_core/{verify_package.sh,check_layers.py,boundary_inventory.py,lint_core.sh,state_size.sh}`; `tools/engine_profile/module_graph.py`; `deploy/`; `web/e2e` only mentions paths in manifests (do not edit generated manifests).
   `git grep -n "crates/ti4-engine/"` outside `docs/`, `plans/`, `web/plans/` must be empty at the end (docs may keep history).
5. **Make the oracle copy pure.** Legacy keeps its `typed-core` wiring (340 macro sites; editing them is out of the question for a frozen crate) but the harness and tests force `AreaMask::STRING` (`dual::set_default_mask(AreaMask::all(Backend::String))`);
   `ti4-engine-legacy`'s `build_default()` changes to `STRING` unconditionally behind a feature `oracle` (default on in legacy): one small edit in `dual.rs:165`. The facade's `typed-core` feature forwards to it so production behaviour is unchanged until D2-8.
6. Checks (cheap, runner): `cargo check --workspace --all-targets` (excluding torch crates as the alias does), `cargo test -p ti4-engine-legacy -p ti4-engine-core-bridge --no-run`, bench hashes 200/200 x6 with `selfplay` built from the facade, wasm builds. Gate: none (pure move); one heavy run is the bench in D2-9.
7. Announce the new path to nightly fixers/other agents the moment the move merges (CLAUDE.md nightly rule: the sweep covers whatever lands before 20:30 Berlin; do not merge the move in the hour before it).

**NOT touched:** any `.rs` body inside the moved crate (except `dual.rs` `build_default` and the `Cargo.toml`), generated files, `ti4-model`, web.

### D2-8 `core-d2-8-flip`

**Definition of done.** `ti4_engine::Game` is a wrapper that owns a `CoreState`; `step`/`answer`/choices come from the core driver; `Game::state()` is the cached materialisation; checkpoint = `CoreState::clone` + the decision log; leftovers run through `with_string`;
disabled features panic; `Unmapped` is an error, not a fallback; the bots, host, server and wasm see the same API and the same bytes.

1. **Entry:** `Game::new/with_table/with_state(state: GameState, ..)` loads once: `ids_supported`, `unsupported::refuse`, `Codec::load(ALL)` with strict totality (`Err` -> `GameError::Unmapped(Unmapped)` surfaced to the host; the server marks the game
   unrecoverable and keeps the files as for `StorageError::Disabled`). Setup stays string: `setup.rs`, `opening.rs`, `starting_tech.rs` run in legacy and end in this `load` (N4 of `ENGINE_AREA_STATUS`).
2. **Fields that stay in the wrapper** (inventory of `Game`, `game.rs:919-1012`; the FORK RULE destructure-without-`..` of `snapshot/instantiate/into_game` stays, now over the new struct): `table` (deciders + `DecisionLog`), `content`, `sources`, `strategy_cards` (initial list), `events` view (P1),
   `rng_force` side channel `GameRng::observation/sync` (host-only; reverse-lend bodies must not reach it), `aftermath_observer`, `strategic_observer`, `round_income_*` (experiments) as `Knobs`. Everything else is `CoreState` (D1 moved the 15 window options into `Pending`).
3. **Checkpoints.** `Game::checkpoint()` = `(CoreState::clone(), DecisionLog len/clone, events len)`; `restore`. `GameSnapshot` = the same plus the wrapper fields, plain data, `Send`. The shared `deck_reserve` cursor of `game.rs:1070 detached_state` disappears with XF-2 (disabled).
   Measure the bucket with `state_size` (gate 10) and report: target `ENGINE_CORE_STATE.md` 6.6: checkpoints 18.7% -> ~2% of a game.
4. **Resolver and tables** live in the state (D2-4b, D2-3); the native registry is built for all seats with `AbilityTable::registry` (`kernel/ability.rs:425`); `mirror_timing_log` (`game.rs:3345`) is replaced by P1's rendering from the typed journal.
5. **Per-step reconcile hooks** of `Game::step` (`starting_tech::settle`, `announce_staged_ground_events`, `announce_gains`, `space_stations::reconcile_all`, `nexus_is_triggered`, `legendary::settle_control_points`, `keleres::reconcile`, `crimson::enforce_sundered`,
   `laws::apply_to_galaxy`, `movement::replay_map_edits`; `ENGINE_AREA_STATUS` 2.1) are D1a/D1b's; here only verify that none is still called on the string state.
6. **Call-site rewiring:** the 340 macro sites live in legacy and stop being compiled into the production game loop; the core driver calls the core rule and, for a bailing label, `lent::<label>` (D2-5). A label without a wrapper is a hard `Err(Unmapped)` with the label named.
7. **Sweep the public surface:** `game.state` (field) -> `game.state()`; the 2 host/wasm writes (`grep "state_mut()\|game.state\\.[a-z_.]* *="`) move into a `Game::edit(|gs| ..)` that materialises, edits, loads back (used for setup tools only).
   `game.timing` readers (host 2, server 4, review 4, training 1) read `game.resolver_image()`.
8. **`decision_delivery_inventory` for the new crate** (T10): the facade has no scanner; add a copy of the scanner test for the wrapper's `Choice` producers (`render/*`) when they exist; legacy's own registry stays.
9. Set `set_disabled_policy(Panic)` as the default for the wrapper (D2-2), `set_strict(true)` for `Unmapped`.
10. Tests: `fork_equivalence` (tests dir, FORK RULE) and the 20 golden games through the wrapper; `GameSnapshot` instantiate/continue equals the original; checkpoint/restore around a nested question (the `game.rs:3277` path) equals no-checkpoint.

**NOT touched:** wire/TS types, `DecisionRecord`/fingerprint V1/V2, saves, `RULES_VERSION`, web.

### D2-9 `core-d2-9-prod-strip-final-gate`

**Definition of done.** Production crates (`ti4-server`, `ti4-wasm`, `ti4-host`, `ti4-rule-bot`, `ti4-selfplay`) contain no dual wiring; the production dependency tree contains `ti4-engine-legacy` only behind `lend-string` (phase a) or not at all (phase b, only if the bail census over
production inputs is zero); G7 passes.

1. Remove from the production `ti4-engine`: feature `typed-core` (core and factions become plain dependencies), the eight `string-*` features, `AreaMask`/`Backend`/`dual*.rs`/`kernel_sync/`/`*_dual.rs`, `set_default_mask`, `TI4_AREAS`, `TI4_DUAL_FAIL_FAST`. Their homes after D2-7 are legacy (frozen) and the bridge.
   `crates/ti4-engine-core-bridge/Cargo.toml:12` loses `features = ["typed-core"]`. `cargo tree -p ti4-wasm -e normal | grep -c legacy` and the same for `ti4-server` must print 0 (phase b) or the feature-gated count (phase a).
2. `lend-string` feature on `ti4-engine`: `dep:ti4-engine-legacy`; enabled by `ti4-server` and `ti4-wasm` in phase a. **Census gate for phase b:** `Lent` counters (label, count) exported next to `unmapped`; over G7's whole corpus every label count must be 0 for production-reachable
   scenarios; any nonzero label stays in phase a and is a named follow-up port. The user's final structure (9.1/9.4) is reached exactly when this census is empty (question 1).
3. wasm: `build-wasm.mjs` flow, `scripts/wasm-size-gate.mjs`; report raw/gzip/brotli before/after against 6.3 of `ENGINE_CORE_STATE.md` (budget 2.4 MB gzip with a ratchet lowered at each deletion; this is the first measurement with the string engine gone from the binary in phase b).
4. `.cargo/config.toml` aliases and `tools/engine_core/verify_package.sh` package lists: add `ti4-engine-legacy`, drop `AREA` modes (`TI4_AREAS=...` lines); `check` for `oracle` replaces `dual-golden`.
5. **G7 (final gate)**, one run, `SHARDS=4`, under `flock /root/.ti4-heavy.lock`, mapped to `ENGINE_CORE_STATE.md` section 5:

   | # | Check | Command / criterion |
   |---|---|---|
   | 1 | tests | `cargo test -p ti4-model -p ti4-content -p ti4-engine -p ti4-engine-legacy -p ti4-host -p ti4-rule-bot -p ti4-selfplay -p ti4-engine-core -p ti4-engine-core-factions -p ti4-engine-core-bridge -p xtask`; `cargo test -p ti4-wasm --features ts`; `cargo xtask gen-cards --check` |
   | 2 | immutable surfaces | `git diff --exit-code <int-7> -- web/ docs/RULES_VERSION.md crates/ti4-model/src/state.rs 'crates/ti4-wasm/**/fixtures'` and generated TS: empty |
   | 3 | oracle | `oracle` over golden 20, bench configs v1..v5, templates, decision corpus, **alpha replays**: 0 unallowed differences, 0 stale allow-list entries, skipped-scenario count printed |
   | 4 | round trip | `core_sync_gate profile-20.json 20 10 --strict` (>= 3 000 positions) and `alpha_roundtrip` on a fresh copy: 0 differences, `unmapped: 0` |
   | 5 | benches | `verify_benches.sh`: `bench`, `bench-v1..v5` 200/200 identical on the new engine (and under `TI4_CHURN` on the pre-flip build, once, in G-prep) |
   | 6 | golden hashes | 20/20 `profile-20.json` hashes = `tools/rulebot/baseline.json` |
   | 7 | CPU A/B | `tools/engine_profile/ab_time.sh` vs the int-7 selfplay; must not be worse than +1.5%; report the gain (expected from checkpoints) |
   | 8 | wasm size | item 3 |
   | 9 | hygiene | `check_layers.py core`, `lint_core.sh`, `clippy`; new: production `cargo tree` check of item 1 |
   | 10 | `state_size` | `CoreState::clone` ns/allocations, checkpoint bucket |
   | 11 | compile time | release builds of `ti4-engine`, `ti4-engine-core` |
   | 12 | **alpha replay preflight** | `deploy/public/deploy.sh --preflight-only --ref <head> --allow-failed <ids>` against a copy of `/var/lib/ti4-public`. Expected failures today: `game_a36f40db3b5cc50fabaabcd66a6bad01` (known, "canonical hashes differ", cited `STARTING_TECH_CHOICES.md`) and the turn-redo saves `game_935f5618...`, `game_d26c07f1...` (disabled, `DISABLE_XF123.md` 1.3). Any other failure blocks. List recovered/failed counts |
6. After G7: delete nothing. `ti4-engine-legacy` stays (decision 9.1); D3 starts one nightly later (`ENGINE_CORE_STATE.md` 8.5).

---

## 4. Order of work inside the prep block (what to do first)

1. D2-1 items 3-4 (counters, strict): 1 h, unblocks measuring everything else.
2. D2-3 (text tables) before D2-1 item 6's high-water output (T12).
3. D2-4a accessors and I1-I3 tests before the churn hook; churn hook before D2-5.
4. D2-5 item 5 (bail probe) needs the final labels: run it once after int-5, then freeze the table.

---

## 5. Known traps (read before coding)

| # | Trap | Where | What to do |
|---|---|---|---|
| T1 | Fixtures with faction `generic` (`ti4-model/src/state.rs:686`, `fixtures::game`) are refused by `conv::id::<Faction>` and the Players/Cards codecs; the core half **silently bails** and the test is green having compared nothing (B6-design-02) | 393 users of `fixtures::game(`, 365 of `seated_game(` | `seated_dealt`/`real_game`; every lockstep test asserts `compared >= 1`; the new engine refuses `generic` at `Game::new` |
| T2 | Players codec refusals: `codec/players.rs` (`FRAGMENT_KEYS`, plots `:189-201`, faction `:149`), `cards.rs:17-23` promissory ids `"{alias}:{faction}"`, `seats.rs:19` | D2-1 item 5 table | classify, test, do not widen the codec to accept junk |
| T3 | Delivery-inventory registry: `tests/decision_delivery_inventory.rs` keys sites by (file name, function, operation) and scans `CARGO_MANIFEST_DIR/src`. Renaming a function or moving/splitting a file (e.g. `game.rs` in D2-8) fails it | `expected_sites()` at `:1796` | update the registry rows *in the same commit* and list them in the commit message; question 4 asks to pre-approve rename-only edits |
| T4 | String resolver `initiative_order` fixed at `Game::new`, not refreshed by `sync_timing_context` | `game.rs:1485`, `timing.rs:1563` | carry it in the image (D2-4a); I3 test |
| T5 | `arm` must be called once; no unregister; borrowed-agent registrations depend on the seated factions | `reactions.rs:1060` | I2 registry-identity test |
| T6 | Dual report races: the report is process-wide and `take_report()` in parallel tests steals counts (B6-design-07, `dual.rs` `Serial`). New counters (`unmapped`, lent labels) need a thread-local variant for tests and a process-wide one for gate bins | D2-1 item 3, D2-5 | `take_thread()`; tests that read the shared report hold `serial()` (there is a test that enforces it, `dual.rs` tests) |
| T7 | Content-id lint on test/production literals: `tests/content_ids_resolve.rs` scans production lines that mention `alias`, `.id()`, `PlanetId::new(`, `SystemId::new(`, `planets_in(`, `home_system(` and requires every id-shaped literal to be in the corpus (test modules skipped). New code in `fixtures.rs` / wrappers that spells faction or planet ids on such lines must use corpus ids (use the generated enums, not literals) | D2-1 item 1, D2-5 wrappers | run `cargo test -p ti4-engine content_ids_resolve` before committing |
| T8 | `Code` sets that are not closed would overflow `u16` over a long game or many forks; `Code` derives `Ord` and must not be used where text order is observable | D2-3 item 6e | high-water report; never sort by `Code` |
| T9 | `Kernel.seeds` clone cannot resolve a `Dyn` domain minted during the call -> wrong seed, silent RNG divergence | D2-3 item 7 | debug-assert; production mints none |
| T10 | Path-bound tests/tools: `decision_delivery_inventory.rs`, `content_ids_resolve.rs`, `xtask content_gen/mod.rs:56`, `.cargo/config.toml` aliases, `tools/engine_core/*`, `module_graph.py` | D2-7 item 4 | one commit per fix, `git grep -n "crates/ti4-engine/"` empty at the end |
| T11 | Scratchpad / target collisions: parallel runners writing to the same `CARGO_TARGET_DIR`, `/tmp` names or the alpha copy; tests with fixed temp file names | every package | each runner: own scratch worktree, own `CARGO_TARGET_DIR` under the scratchpad, `mktemp -d` for data copies, delete on stop (E4/E5 of `ENGINE_AREA_STATUS`) |
| T12 | Two packages edit `core_sync/` (D2-1 `error.rs`, `tests.rs`; D2-3 `ctx.rs`, `codec/*`, `keys/*`) | merge conflicts | D2-1 item 3-4 (counter, strict) merges first; D2-3 rebases onto it |
| T13 | The live checkout / nightly rule: whatever lands before 20:30 Berlin is part of that night's sweep. The prep block changes only the typed side and adds accessors in `timing.rs`; keep it out of the sweep window unless the cheap checks and the bench hashes are green | CLAUDE.md Time | merge prep work in the morning; the move (D2-7) never in the last hour |
| T14 | `Map` and `Rng` codecs do not store into `GameState`; `materialize` needs the galaxy and `GameRng` positions from elsewhere | `codec/map.rs:33`, `codec/rng.rs:96` | D2-5 item 1 |
| T15 | The golden games never seat Bastion/Crimson, never enable diplomacy, never attach a reserve; the disabled panics cannot fire there, but a *dev* test that builds such a table through the core entry will | `DISABLE_XF123.md` 1.2 | `Policy::Bail` until D2-8 |
| T16 | `core_sync_gate` is not part of `verify_package.sh`; the round-trip criterion of gate 4 was never enforced by the script | `verify_package.sh:21` | add it to the script in D2-1 (strict) |
| T17 | Rules versions: golden runs use `rules_version` 23 without `sources`; version 35 and TE are covered by unit tests only (R3) | D2-6b corpora | include the bench configs of v1..v5; PS-2 profiles if they exist by then |

---

## 6. Standing rules (identical in every brief)

```text
STANDING RULES
1. Work in your own git worktree and branch (name given below), cut from the newest core-integration-N. Never push. Commit small, one item per commit, message ends with the
   attribution lines of the session. Do not commit anything outside the files your package owns.
2. Gate only when a whole area/block is complete: ONE heavy run, SHARDS=4, under `flock /root/.ti4-heavy.lock`. Everything else is a cheap check (check/clippy of the touched crates,
   the unit tests of the touched module, lint_core.sh, check_layers.py). No bench, no full dual run, no CPU A/B outside the gate.
3. Every failure gets a regression test before the fix (name it after the failure). A found string bug is reported in docs/ENGINE_KNOWN_ISSUES.md style (id, file:line, one line), not fixed
   in the port; the port copies it.
4. One monitor per wait: when you must wait for a background result, start exactly one Monitor on it; do not poll, do not sleep in a loop.
5. Stop and report when the five-hour usage reaches 90% (the weekly usage is not a criterion). Report what is done, what is not, the last commit, and the exact next item.
6. No behaviour change: RULES_VERSION and every RULES_* constant, wire/TS types, saves, fingerprints, web/, tools/rulebot baselines stay byte-identical. No test assertion is edited
   (new tests only); a necessary registry/path edit is listed in the commit message and the report.
7. Live checkout rule: never touch /root/TI4-Bot-Dev's checked-out branch; nightly sweep cut-off 20:30 Berlin (`TZ=Europe/Berlin date`).

WORKFLOW RULE (do not stop or idle while checks run)
 After each item: commit a WIP snapshot (`git commit -am "wip: <item>"`). Delegate every compile, test, mutation or bench run to a Haiku runner agent (Agent tool, model haiku):
 it checks out THAT commit in a separate scratch worktree (`git worktree add <scratchpad>/run-<n> <sha>`), uses its own CARGO_TARGET_DIR under the scratchpad and CARGO_BUILD_JOBS=2, runs the
 command, and reports ONLY the failing test names and the first error of each failure (plus the final counts), then removes the worktree and target dir. Continue with the next item
 while it runs; fold a runner's report in when it arrives (regression test first, fix second). Fallback if the Agent tool is unavailable: `run_in_background` shells with the same isolation.
 Runner brief template: "In a new git worktree of <repo> at <sha> (scratch: <dir>), set CARGO_TARGET_DIR=<dir>/target CARGO_BUILD_JOBS=2 and run: <command>. Report only: pass/fail counts, each
 failing test name, and the first compiler or panic message of each failure (max 15 lines each). Do not fix anything. Remove the worktree and target dir at the end. Heavy gates are not yours."
```

---

## 7. Ready-to-paste briefs

Each brief is complete on its own; paste the STANDING RULES block of section 6 above it.

### Brief D2-1 (`core-d2-1-fixtures-totality`)

> Package D2-1, branch `core-d2-1-fixtures-totality`, worktree `.claude/worktrees/core-d2-1`, cut from the newest `core-integration-N`. Read `docs/todo/D2-prereqs.md` sections 0, 1, 3 (D2-1), 5 (T1, T2, T6, T7, T12, T16) and
> `docs/ENGINE_CORE_STATE.md` 5. Goal: (1) `fixtures::seated_dealt` and `fixtures::real_game`, three hand-built workarounds replaced; (2) `Unmapped` counted at `core_sync/error.rs bad()` by (slice, reason), printed in `DualReport` and every gate tool,
> `set_strict` for gate tools; (3) refusal audit table in the module doc of `core_sync/error.rs` with a test per reachable refusal and `ids_supported` called from `ti4_host::setup::create`; (4) `marks_census.py`; (5) `roundtrip` lib module and
> `alpha_roundtrip` bin working on a COPY of `/var/lib/ti4-public` (never the live directory), 0 differences; (6) `core_sync_gate --strict` added to `verify_package.sh`. Do items in the order of the checklist; item 3-4 first and merge them early (D2-3 rebases onto them).
> Do not change rule behaviour, `.ok()?` fallbacks, `fixtures::game`, or any test assertion. Definition of done: section 3 D2-1; hand back the branch tip, the unmapped table of one `core_sync_gate` run over profile-20 (counts by reason), the
> `alpha_roundtrip` summary line, and the list of refusal sites you could not classify as unreachable.

### Brief D2-2 (`core-d2-2-disabled-panics`)

> Package D2-2, branch `core-d2-2-disabled-panics`, worktree `.claude/worktrees/core-d2-2`. Start only when the base contains `disable-xf123` and the B12/B6c merge; check with `git branch --contains`. Read `docs/DISABLE_XF123.md` (all, especially section 5) and
> `docs/todo/D2-prereqs.md` D2-2, T15. Goal: `core_sync::unsupported` (`Disabled`, `find`, `refuse`, `Policy::{Bail, Panic}` default Bail), calls at the six typed entry points, `debug_assert`s at the three core hot spots, the site list re-grepped
> and annotated, tests per family in both policies, golden check: `core_sync_gate` and `dual_run` over profile-20 and the template openings in `Panic` policy trip 0 times. Do not delete gated core code, do not touch the product edges, `seating::wide_roster`
> or dev tools. Hand back: the re-grepped site list (differences from the doc) and the count of test files that seat a disabled feature.

### Brief D2-3 (`core-d2-3-text-tables`)

> Package D2-3, branch `core-d2-3-text-tables`, worktree `.claude/worktrees/core-d2-3`. Read `docs/todo/D2-prereqs.md` D2-3, T8, T9, T12 and `crates/ti4-engine/src/core_sync/{ctx.rs,keys/text.rs,codec/rng.rs,lend.rs}`.
> Goal: `containers::Interner` (base `Arc` + local suffix, bytes not str, content equality), `CoreState.texts` and `owners`, `Ctx.texts`/`KeysCodec.blobs` removed, codecs and `keys/*` signatures changed mechanically, cross-lend stability and clone
> divergence tests, high-water output. Output of `core_sync_gate` must be byte-identical before and after. Core hygiene: no `String`/`&str`/`format!` in the core file (`tools/engine_core/lint_core.sh`), layer L0 for the container. Hand back: before/after round-trip
> summaries, the texts/blobs maxima over profile-20, and a statement whether any set looked open (growing with the game).

### Brief D2-4a (`core-d2-4a-resolver-sync`)

> Package D2-4a, branch `core-d2-4a-resolver-sync`, worktree `.claude/worktrees/core-d2-4a`. Read `docs/todo/D2-prereqs.md` D2-4a, T4, T5, `crates/ti4-engine/src/timing.rs` (446-463, 1563-1600), `game.rs` (1485-1490, 3277-3325, 7469), `reactions.rs:1060`,
> `kernel/resolver.rs`, `kernel_sync/shim.rs`. This is the most delicate piece of D2: build the tests I1-I5 first, then `resolver_sync.rs`, then the churn hook `TI4_CHURN=resolver`. Acceptance: bench 200/200 x6 identical with the churn on (ONE run, in
> the G-prep or by the coordinator, not yours), and your cheap proof: the I1-I5 tests on every golden position at stride 10 plus a 5-game `selfplay` run with the churn on whose decision hashes equal the off run. If I2 or I3 fails, stop and report: it means the
> design (re-arm from state) is wrong. Only additive `pub(crate)` accessors in `timing.rs`/`event.rs`. Hand back: the image contract table as implemented, and any resolver field not in the table that a lent window reads.

### Brief D2-4b (`core-d2-4b-resolver-in-state`)

> Package D2-4b, branch `core-d2-4b-resolver-in-state`, worktree `.claude/worktrees/core-d2-4b`; start after D1a is merged and D2-3, D2-4a are on your base. Read `docs/todo/D2-prereqs.md` D2-4b, `docs/ENGINE_CORE_DRIVER.md` 2, `kernel/resolver.rs`,
> `driver/mod.rs`. Goal: `state/resolver.rs` (L1), `Resolver::from_state/into_state`, `Driver` copy in/out, the `Pending` codec extended with the image of D2-4a, per-window assertion in the dual that the state's resolver equals the string image. Layer rule:
> `check_layers.py core` must pass (no `state` -> `kernel` import). Hand back: files touched, the list of drivers/windows constructing `Driver::new`, the dual assertion count.

### Brief D2-5 (`core-d2-5-reverse-lend`)

> Package D2-5, branch `core-d2-5-reverse-lend`, worktree `.claude/worktrees/core-d2-5`; start when the base contains B12 and B6c (int-5), D2-3 and D2-4a. Read `docs/todo/D2-prereqs.md` D2-5, T14, `docs/ENGINE_CORE_STATE.md` 3.2, `docs/ENGINE_AREA_STATUS.md` 2.2 and 2.5,
> `core_sync/{lend.rs,codec/mod.rs,codec/map.rs,codec/rng.rs}`, `dual.rs` (`run_dual_window`, macros at 1309). Goal: `materialize` + cache, `with_string`, `StringTape`, `lend_table.rs` with the probe (`TI4_BAIL_PROBE`) and the two table tests, wrappers for
> every label with bails in the golden dual, footprint audit, churn mode `TI4_CHURN=state`. An unknown label lends `Footprint::ALL`; correctness never depends on the table. Labels whose state is a nested `Game` field cannot be lent: list them for D1b instead
> of building a window-kind codec. Hand back: the label table (label, bails per 1000 calls, writes, wrapper status), the labels that need an argument builder with minutes estimate, and the churn proof on a 5-game run (hashes equal). The heavy G-prep run is
> started by the coordinator when D2-1..D2-6a are merged.

### Brief D2-6a (`core-d2-6a-allow-list`)

> Package D2-6a, branch `core-d2-6a-allow-list`, worktree `.claude/worktrees/core-d2-6a`. Read `docs/todo/D2-prereqs.md` D2-6a and `crates/ti4-engine/src/dual.rs` (`Difference` at 340, `DualReport` at 392). Goal: `allow_list.rs` + `tools/engine_core/allow_list.json` (empty)
> + matcher + `--check-stale` + unit tests (match, version bounds, glob paths, stale detection, malformed entries rejected). No harness yet. Hand back the file schema as implemented and the test names.

### Brief D2-6b (`core-d2-6b-oracle`)

> Package D2-6b, branch `core-d2-6b-oracle`, worktree `.claude/worktrees/core-d2-6b`; start after D2-7 (legacy crate exists) and while D2-8 runs. Read `docs/todo/D2-prereqs.md` D2-6b, `docs/ENGINE_CORE_STATE.md` 9.3, `crates/ti4-engine-core-bridge/src/{workload.rs,bin/dual_run.rs}`,
> `tools/engine_core/dual_sharded.sh` (on `core-dual-shards`). Goal: the `oracle` bin (legacy vs new engine, per-step comparison, shards, allow-list, skip filter for disabled scenarios), tests (self-comparison 0, injected difference found, allow-listed difference passes). Corpora:
> golden 20, bench configs v1..v5, templates, decision corpus, alpha replays from a COPY. Hand back a sample report and the per-corpus step counts.

### Brief D2-7 (`core-d2-7-legacy-split`)

> Package D2-7, branch `core-d2-7-legacy-split`, worktree `.claude/worktrees/core-d2-7`; start only when the coordinator announces the freeze (`core-integration-7` merged, nobody else editing `crates/ti4-engine`). Not in the last hour before 20:30 Berlin. Read `docs/todo/D2-prereqs.md`
> D2-7, T3, T10, `docs/ENGINE_CORE_STATE.md` 9, and run `tools/engine_core/boundary_inventory.py` first. Goal: `git mv crates/ti4-engine crates/ti4-engine-legacy`, rename the package, facade crate `crates/ti4-engine` (`pub use ti4_engine_legacy::*;` + forwarded features), workspace
> members and dependencies, all path-bound tests and tools (list in item 4), `oracle` feature (legacy defaults to `AreaMask::STRING`), the module classification table in the facade `lib.rs`. Commits: move; facade; one per path fix. Checks through runners: `cargo check --workspace --all-targets`
> (alias package sets), `cargo test -p ti4-engine-legacy --no-run`, `selfplay` built from the facade gives 200/200 bench hashes (ONE run at the end, via the coordinator). `git grep -n "crates/ti4-engine/"` outside docs/plans must be empty. Hand back: the classification
> table (a/b/c per module), the commits, and any dependent that needed more than a Cargo edit.

### Brief D2-8 (`core-d2-8-flip`)

> Package D2-8, branch `core-d2-8-flip`, worktree `.claude/worktrees/core-d2-8`; start after D2-7 and D1b are merged. Read `docs/todo/D2-prereqs.md` D2-8, `docs/ENGINE_CORE_STATE.md` 3.2 and 6.5, `docs/ENGINE_CORE_DRIVER.md`, D1a/D1b hand-back notes (`docs/todo/D1b.md`, plain events `docs/todo/P1.md`).
> Goal: the wrapper `Game` owning `CoreState`, `state()` cache, checkpoints = clone, `Knobs`, `Unmapped` as `GameError`, disabled panics default on, public-surface sweep (`game.state` -> `game.state()`), `fork_equivalence` and the 20 golden games through the wrapper. Work in this order: entry/load, fields inventory, checkpoint/snapshot,
> driver wiring, surface sweep, panics. Keep legacy as an optional dependency behind `lend-string` (on for server and wasm). Hand back: the fields inventory as implemented, every call site that still reaches `with_string` (labels with counts from a 20-game run), and the checkpoint cost numbers from `state_size`.

### Brief D2-9 (`core-d2-9-prod-strip-final-gate`)

> Package D2-9, branch `core-d2-9-final`, worktree `.claude/worktrees/core-d2-9`; start after D2-8 and D2-6b are merged. Read `docs/todo/D2-prereqs.md` D2-9 and `docs/ENGINE_CORE_STATE.md` 5 and 6.3. Goal: remove typed-core/AreaMask/dual wiring from the production crate, `lend-string` census gate, aliases and
> `verify_package.sh`, wasm size report, then ONE G7 run under `flock /root/.ti4-heavy.lock` with `SHARDS=4` covering rows 1-12 of the table in D2-9 item 5, including the alpha replay preflight on a fresh copy. Hand back: the G7 summary table, the census (labels with counts; empty means phase b may switch `lend-string` off),
> wasm raw/gzip/brotli before/after, the CPU A/B ratio, the preflight line (recovered/failed/expected failures).

---

## 8. What is deliberately not in D2

* Window ownership, tactical/movement execution, phase flow, plain events: D1a/D1b/A2b/P1 (`docs/todo/`).
* Rule fixes (Heart of Ixth, Harrow, Xxekir Grom): after D2 with a `RULES_VERSION` bump and allow-list entries (D2-6a item 4).
* D3 typed bots, D4 oracle-free tests and deletion of the gated diplomacy/Bastion/Crimson core code.
* Porting the leaf bails (Ssruu copies, Dynamo, Watcher, ...): they are lent through D2 and are the work list of the phase-b census.

## 9. Open questions for you (each with my recommendation)

1. **Is legacy allowed in the production binary while lent bails are reachable?** Section 9.1/9.4 of `ENGINE_CORE_STATE.md` say never; `ENGINE_AREA_STATUS` 2.5 keeps leaf bails lent through D2, and a lend is string code.
   *Recommend:* two-phase D2: phase a ships `ti4-engine-legacy` behind feature `lend-string` (server, wasm), phase b removes it when the lent-label census over the G7 corpus plus one nightly is empty. Cost: wasm size stays up in phase a. The alternative (port every reachable bail
   before D2-8) is the 40-70 h of B12-like work the status document already declined.
2. **Alpha corpus is 14 replayable games (603 decisions).** *Recommend:* use it as the format/regression check (round trip, oracle replay, preflight) at G-prep and G7 on a fresh copy each time, and rely on golden 20 + bench v1..v5 + templates for coverage; do not spend time on synthetic alpha-like logs.
3. **How to cut the string engine.** The docs say "move per area, one commit per area". *Recommend:* whole-crate `git mv` to `ti4-engine-legacy` plus a facade, then shrink the facade by wrapper code (D2-7 reasoning). Per-area moves need the dependency direction between the halves first. Say no if you want per-area commits; D2-7 then becomes 14-20 h.
4. **May the move and flip commits edit `decision_delivery_inventory.rs` registry rows and `content_ids_resolve.rs` scan roots (rename-only, no assertion change)?** CLAUDE.md/gate 1 says no test is edited except to add. *Recommend:* yes for rename/path-only edits, each listed in the commit message and the report.
5. **Strict `Unmapped` in production.** After D2-8 a refused position is a hard error (the game is marked unrecoverable and its files kept, like `StorageError::Disabled`), not a silent string fallback (none exists). *Recommend:* yes, plus the creation-time guards of D2-1 item 5, so a refusal can only come from corrupted saves.
