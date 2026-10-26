# Engine optimization study (2026-10-26)

Where the engine (`ti4-engine`, with `ti4-model`, `ti4-content` and `ti4-host`) spends its time, how it
is structured, and what would make it faster. Branch `engine-optimization-study`, based on
`build-size-and-time` (= `consolidated-2026-10-25` plus its seven dependency and build cleanups; the
engine source is the same as in `consolidated-2026-10-25`).

**Measured** means a number from a run described below. **Estimated** means derived from a profile
share (an upper bound of what removing that work could save) and is labelled as such.

## Summary

| Workload | Before | After | Change | Same games |
|---|---:|---:|---:|---|
| 20 golden games (10 4p + 10 6p), CPU, median of 3 interleaved runs | 85.9 s | 52.7 s | **-38.7%** | 20/20 hashes |
| `bench` (200 golden games), CPU | 1146 s* | 681 s | -41% | 200/200 |
| `bench-v1` / `-v2` / `-v3`, CPU | 639 / 651 / 664 s* | 331 / 363 / 355 s | -45..-48% | 200/200 each |
| `bench-v4` / `-v5`, CPU | 1050 / 1056 s* | 637 / 634 s | -39..-40% | 200/200 each |
| Solo bench native (`ti4_run_seeded` 3, 2000 steps, 8 seats), CPU | 2.16-2.28 s | 1.76 s | -21% | same 2519 decisions |
| Solo bench in wasm (Node 24, `js/run.mjs ... 3 2000 8`) | 6.0 s | 3.9 s | **-35%** | same sha256 of the result |
| Browser resume, 1472 decisions, wasm (Node) | 2.04 s | 1.74-1.84 s | -12% | resume check ok |
| Browser resume, same save, native | 1.20-1.23 s | 1.18-1.23 s | 0% | ok |
| Server views, 4 games x 5 viewers, native (one run each) | 10.1 s | 8.5 s | -15% | same JSON size |

\* The "before" bench runs had 5 workers on a machine that was also building; the "after" runs had 6
workers on a quieter machine. CPU time is less sensitive to that than wall time but not immune
(SMT and cache contention): the interleaved A/B row is the clean comparison.

All six benches (`bench`, `bench-v1` .. `bench-v5`) are 200/200 identical with the final binary
(`tools/engine_profile/verify_benches.sh`), and the engine (3918), content (146), host, rule bot
(190), selfplay and wasm (`--features ts`) test suites pass.

What was changed (all behaviour-neutral, see "Implemented"):

1. `ti4_content::units::catalogue` returns a borrowed `Cow` of its cached map instead of a copy
   (asked about 120 times per decision).
2. `MovementRules::with_laws` uses a cached system index (`galaxy::systems_index`) and copies the map
   only when a faction module really adds a wormhole (it copied it for every set of rules).
3. `galaxy::planets_in` reads a per-scope tile index instead of scanning the whole planet corpus.
4. `transactions::may_transact` / `neighbours`: the same answer without listing every seat's
   neighbours (a board scan per seat and pair).
5. `ti4_host::projection::placed_galaxy` keeps its last answer per thread (pure function of the tiles).
6. The `selfplay` binary uses mimalloc (glibc malloc/free was 40% of a bot game's CPU).

The biggest remaining items, ranked (details under "Proposals"): speculative checkpoints that copy
whole logs (quadratic; 8-40% depending on workload), the bot's per-destination movement searches
(~28% of a bot game now), string ids and string-keyed maps (memcmp is 23% of a bot game now),
per-viewer recomputation in views, typed content records.

## 1. Where the time goes

### Method (reproducible)

Everything is in `tools/engine_profile/` (entry point `profile.sh`, see its header):

- Builds: release profile with `CARGO_PROFILE_RELEASE_STRIP=false CARGO_PROFILE_RELEASE_DEBUG=line-tables-only`
  (symbols, same code), once plain for timing and once with `RUSTFLAGS="-C force-frame-pointers=yes"` so
  `perf record -g` gets whole stacks cheaply (`profile.sh build`, private target dirs).
- Profiles: `perf record -F 499 -g` (999 for short runs), exported with
  `perf script --show-mmap-events -F tid,ip,sym --no-inline` and attributed by `categorize.py`:
  - **bucket**: the exclusive split asked for bot-only games (first match wins, in this order:
    harness statistics, decision hash, state cloning/checkpoints, content index copies, view building,
    serialization, bot (movement queries / objective evaluation / other), decision record, option
    generation, applying answers, other engine work).
  - **kind**: what the leaf frame does: malloc/free, memcmp, memcpy, BTreeMap code, clone, drop,
    content lookups, serde, formatting, sorting, other code.
  - **inclusive**: share of samples whose stack contains a named function.
  - glibc has no symbols on this machine; its hot addresses were identified by disassembly
    (`objdump -d`): offset `0x1a73c0` is `memcmp_evex` (`vpcmpnequb`, `bzhi`), `0x1b0a00` is
    `memmove` (`vmovdqu64` load), `0xb0000-0xb6000` are the malloc internals next to
    `__default_morecore`. Another libc needs other ranges (`LIBC_RANGES` in the script).
- Call counts: `count_calls.sh` puts uprobes (tracefs) on mangled symbols and counts with `perf stat`.
- Timing counters: `crates/ti4-selfplay/examples/engine_profile.rs`:
  - `game CONFIG [N]`: plays the config's games exactly as `selfplay run` does (the printed hash is the
    bench's), and at every question builds what a server sends (one `StateUpdate` per seat plus a
    spectator, `project_state_update_with_map`) and its JSON; then replays the decisions through the
    server's `ti4_host::replay::replay_session` and through a bare `Scripted` table.
  - `solo SEED STEPS PLAYERS`: `ti4_wasm::ti4_run_seeded` natively (random seats; the same call as
    `crates/ti4-wasm/js/run.mjs`).
  - `resume SAVE`: the browser host's resume of an offline save (`{resume: save}` to `ti4_host_start`),
    natively; `wasm_resume.mjs` times the same in a real WebAssembly instance (synchronous `ask`, no JSPI).
- A/B: `ab_time.sh A B [config] [repeats] [workers]` runs both binaries interleaved, compares
  user+sys CPU (`/usr/bin/time`) and every game's decision hash. `verify_benches.sh BIN` runs all six
  benches with CPU time.
- The machine has 8 cores and is shared: every comparison is CPU time, interleaved and repeated.

Profiled workloads: 10 golden 4p games (`profile-4p.json`, seeds 1-10, 4pHyperlanes/4pOuterRim,
starting tech, rules 23), 10 golden 6p games (`profile-6p.json`), the solo bench (seed 3, 8 seats,
2000 steps), the resume of a 1472-decision 8-seat offline save from the alpha feedback, and 3 games
with views.

### Bot-only games (the bench workload)

A "typical 4-player full game" here: golden 4p, 5-8 rounds, 1200-2500 decisions; 1.0-1.2 ms of CPU
per decision after the changes, 1.6-2.4 s per game.

**Exclusive split, 4p (perf, 17 553 samples before / 9 968 after; share of the game's CPU):**

| Bucket | Before | After |
|---|---:|---:|
| Bot: movement/reach queries (engine code the bot calls: `movable_into`, `can_reach`, rift exits, `MovementRules`) | 22.8% | 27.6% |
| Bot: other decision logic (option scoring, plans, `targets`, prep) | 15.9% | 16.8% |
| Bot: objective/goal evaluation (`objectives::goals`, imagined progress) | 10.2% | 14.2% |
| State cloning / speculative checkpoints (`Resolver::checkpoint`, log, state, galaxy, registry copies) | 20.8% | 15.5% |
| Engine: automatic steps (windows, combat, phases, per-step reconcile) | 16.0% | 14.4% |
| Engine: option generation (`legal_options`) | 3.4% | 4.2% |
| Engine: applying answers (`apply_choice`) | 3.9% | 3.3% |
| Content index copies (`units::catalogue`, `all_systems`) | 4.3% | 0.3% |
| Harness: timeline recorder, rift census | 0.9% | 1.4% |
| Logging: decision record + decision hash | 0.4% | 0.6% |
| View building / serialization | 0.1% / 0.0% | 0.1% / 0.0% |
| Other (kernel, unattributed) | 1.3% | 1.5% |

The "after" column is of a smaller total (-39% CPU), so a share that grew did not get slower. Rolled
up after the changes: **bot ~59%, engine ~22% (+15% cloning/checkpoints, mostly engine-side), harness
and logging ~2%, views and serialization ~0** (selfplay builds no views and writes JSON once per game).
The 6p split is the same within 2-4 points (bot 45.6%, cloning 21.7%, engine 26.2% before).

The bot has no lookahead or simulation of the engine (no forks); its cost is reading the position:
`backward::verdict` (44%, inclusive) → `PlanCtx::new` (38%): `steps()` asks `activation_value` for
every system, which asks `movable_into` (15%), which builds a fresh `MovementRules::with_laws`
(12% before, 8% after) and runs one path search per ship (`search_path` 11-14%). Counted with uprobes on
10 4p games (17 514 decisions): `PlanCtx::new` 2 618 calls (about 5 ms each), `movable_into` 92 114,
`MovementRules::with_laws` 253 749 (14.5 per decision), `units::catalogue` 2 149 866 (123 per decision).

**Kind (what the CPU does), 4p:**

| Kind | Before (glibc) | After (mimalloc) |
|---|---:|---:|
| malloc/free/realloc | 39.6% | 18.2% |
| memcmp (string compares: `String` ids, string-keyed maps, content field lookups) | 15.9% | 22.7% |
| engine/model code | 10.8% | 16.3% |
| content lookups (`ti4_content`: `ContentStore::get`, `resolve_id`, `Record` fields) | 8.1% | 9.2% |
| clone | 5.5% | 7.1% |
| memcpy/memmove | 4.6% | 5.8% |
| BTreeMap/BTreeSet code | 3.2% | 4.3% |
| drop | 2.5% | 2.3% |
| other (iterators, sorting, formatting, kernel, bot code) | 9.8% | 14.1% |
| serde_json | 0.0% | 0.1% |

**Scaling** (final binary, 8 games per size, `--starting-tech`, recommended maps, 6 workers; wall per game):

| Seats | Rounds | Decisions | ms per game | ms per decision | Bot share (wall) |
|---:|---:|---:|---:|---:|---:|
| 3 | 5.2 | 1125 | 1039 | 0.92 | 59% |
| 4 | 5.4 | 1571 | 1631 | 1.04 | 58% |
| 5 | 7.1 | 2213 | 2846 | 1.29 | 54% |
| 6 | 6.9 | 2690 | 3987 | 1.48 | 56% |
| 7 | 6.6 | 3158 | 6241 | 1.98 | 60% |
| 8 | 7.1 | 3944 | 9563 | 2.43 | 57% |

- Decisions grow linearly with seats and so does the cost of each decision (every seat's units are on
  the board the bot and the engine scan), so a game costs roughly seats squared: 8 seats cost 9x a 3-seat game.
- With game length the cost per decision grows too. Golden games split in thirds by length: 4p
  1.16 → 1.25 → 1.39 ms per decision (1645 → 2431 decisions), 6p 1.33 → 1.46 → 1.68 ms (2220 → 3306).
  Part of that is a fuller board, part the speculative checkpoints, which copy logs that grow with the
  game (quadratic in total).
- 89-91% of a game's wall time is in the action phase, 5-8% strategy, 1.5-2% agenda, under 1% status.

### Engine alone: the solo bench (random seats)

Native, seed 3, 8 seats, 2000 steps (profile taken before change 4):

| Bucket | Share |
|---|---:|
| Engine: option generation | 33.3% (of which `may_transact` 21%: the presence of every seat recomputed per pair) |
| State cloning / checkpoints | 28.6% (`DecisionRecord` clones alone 13%) |
| Engine: automatic steps | 26.7% |
| Engine: applying answers | 8.7% |

Kinds: malloc/free 33%, memcmp 17%, memcpy 10%, clone 8%. Change 4 removed most of `may_transact`:
2.2 → 1.76 s natively and 6.0 → 3.9 s in wasm, where allocation is costlier (dlmalloc in a
`opt-level = "z"` build). In wasm the same game takes 2.2x the native time after the changes (2.7x before).

### Replay and resume

- **Server replay** (`ti4_host::replay::replay_session`, which the server runs on recovery): 0.59 ms
  per decision for bot games, the same as a bare `Scripted` replay (0.59-0.60 ms), so the canonical
  hashing costs nothing measurable. A replay costs about as much as the engine part of playing the game
  (4.2 s replay vs 4.6 s engine steps for the same 4 games); only the bot's share is saved.
- **Browser resume** of a 1472-decision 8-seat save: 1.2 s native, 1.7-2.0 s in wasm. Profile
  (native): state cloning/checkpoints **39.5%** (logs `Vec` clones 17.6%, `Resolver::checkpoint` 11.7%,
  `GameState` clones 9.5%, `Game::fork` 8.2% for the host's undo checkpoints), automatic steps 35%,
  decision record 5%, views 2%. The resume is checkpoint-bound.

### View building (`ti4-host`)

`engine_profile game`, 4 golden 4p games, 4556 questions, 5 viewers per question (4 seats +
spectator), as a server with one subscriber per seat builds them:

| Part | Before | After |
|---|---:|---:|
| Bot | 7122 ms | 5493 ms |
| Engine steps | 5055 ms | 4593 ms |
| Views (5 per question) | 10078 ms (2.2 ms per question per viewer) | 8518 ms (1.9 ms) |
| JSON of the views | 2564 ms (300 KiB per question, 60 KiB per viewer) | 1913 ms |

With every seat subscribed, building and serializing the views costs more than the engine and the bot
together. Inside a view (perf, share of view time): `project_table_view_with_map` 43% (objective progress
of every seat: `neighbour_planet_counts` 20%, `transactions::neighbours` 14%), `redacted_state` 17%
(a `GameState` clone per viewer, 14%), `view_for` 13.5%, `placed_galaxy` 12.6% (a `Galaxy` rebuilt from
the tiles each time; now cached), `fill_facts` 15% (tactical facts 5%, exploration 6.5%). Most of the
table view is the same for every viewer and is computed once per viewer.

### Hot spots, ranked (before the changes, bot-only 4p unless noted)

| # | Hot spot | Share | Kind |
|---|---|---:|---|
| 1 | Allocation (malloc/free) across everything | 39.6% (leaf) | allocation |
| 2 | Bot movement queries: `movable_into` → `with_laws` + `search_path`, per destination | 22.8% | algorithm, rebuilds |
| 3 | Speculative checkpoints: `Resolver::checkpoint` 9.1%, log/state/galaxy copies in `step_aftermath`, `advance_turn`, `try_flush_staged_events`, `step_tactical` | 20.8% (solo 28.6%, resume 39.5%) | state cloning, quadratic |
| 4 | String compares (memcmp) under id lookups and string-keyed maps | 15.9% (leaf) | String ids |
| 5 | Content: `ContentStore::get`/`resolve_id`/`Record` field reads through `serde_json::Map` | 8.1% (leaf) | content representation |
| 6 | `units::catalogue` map copies (2.1 M per 10 games) | 4.3% | needless clone (fixed) |
| 7 | `Galaxy::adjacent` (a `BTreeSet` built per call) | 3.8% | recomputation |
| 8 | `galaxy::planets_in` full corpus scan (bot `targets`) | 3.4% | linear scan (fixed) |
| 9 | Galaxy copy in `with_laws` for a hook that adds nothing | 2.8% | needless clone (fixed) |
| 10 | `argent::blocks_passage` per system per seat in `with_laws` | 2.7% | hook evaluated per system |
| 11 | `transactions::may_transact` (solo: 21%) | 1.0% | O(seats²) board scans (fixed) |
| - | serde_json (engine) | 0.0-0.1% | not a factor outside views |
| - | Harness (timeline, census, hash) | 1.1% | not a factor |

## 2. Structure and complexity

Measured with `tools/engine_profile/module_graph.py` (text-based: `crate::module` paths, test code excluded).

- **Size**: `ti4-engine` 271k lines (about half tests), `ti4-host` 31k, `ti4-rule-bot` 21k, `ti4-model` 6.7k,
  `ti4-content` 5.7k. Largest engine modules (non-test lines): `factions` 41.9k (48 files, `cabal.rs`
  4.6k), `action_cards` 7.8k, `game` 7.4k (+11.1k tests in the same file), `combat` 6.6k, `invasion` 5.0k,
  `production` 3.8k, `diplomacy` 3.7k, `choice` 2.8k.
- **Largest functions**: `agenda_effects::resolve_with` 906 lines, `leaders::dispatch_leader` 817,
  `Game::apply_choice` 438, `reaction_preview::preview_for` 422, `invasion::resolve_ground_round` 330,
  `exploration::resolve_instant` 330, `vote::pending_choice_bare` 316, `combat::pending_choice` 277,
  `Game::step` 207. `game.rs` has 373 functions; `Game` has 38 fields, 20 of them optional windows.
- **Cycles**: 55 of the engine's 75 top-level modules form one strongly connected component (620
  module-to-module edges); only leaf helpers (`rng`, `dice`, `fingerprint`, `fixtures`, ...) are outside
  it. Fan-out: `game` 46 modules, `factions` 40, `invasion` 29, `action_cards` 29. Fan-in: `choice` 43,
  `factions` 37, `decision_context` 36, `supply` 28. In `ti4-host`, 13 of 20 modules are one cycle
  (`projection`, `view`, `protocol`, `planning`, `batch`, `replay`, ...). `ti4-model` has no cycles.
  Consequence: no part of the engine compiles, tests or can be optimised in isolation, and a change to
  `choice` or `game` recompiles everything (one codegen unit in release: 4.5 min for `ti4-engine`).
- **Window precedence twice**: `Game::legal_options` and `Game::step` each list the ~15 optional windows
  in the same order; the order is the rule and is kept in sync by hand.
- **External dependencies are small**: the engine uses `serde`/`serde_json` (content records,
  `ChoiceOption.payload: BTreeMap<String, Value>`, `Choice.details`, typed event payloads), `rand` +
  `rand_chacha` (seeded dice and decks), `sha2` (fingerprints, hashes), `thiserror`; `ti4-model` adds
  `hex`, `strum`, `enumset`; `ti4-content` the same. 41 crates in the engine's normal tree. None of them is
  a hot spot except `serde_json::Map` as the content record store (row 5 above).
- **State representation**:
  - Every id is a `String` newtype (`PlayerId`, `SystemId`, `PlanetId`, `UnitTypeId`, `ObjectiveId`, ...);
    `Unit` is `{type_id: String, owner: String, 2 bools}`; the board is
    `BTreeMap<SystemId, SystemState>` with `planet_units: BTreeMap<PlanetId, Vec<Unit>>`. Every lookup
    and every equality is a string compare: memcmp is 16-23% of the CPU.
  - Stringly-typed state: `faction_marks: BTreeMap<String, String>` (548 uses, keys built with
    `format!`, e.g. `"private:#moved:{seq}"`), `laws: BTreeMap<String, String>`, `exploration_decks:
    BTreeMap<String, Vec<String>>`, `Game::events: Vec<String>` (`"LEADER_UNLOCKED:..."`), option payloads
    as JSON values (300 `payload.get/insert` sites). 14 of `GameState`'s 106 fields are string-keyed.
  - Content records are `serde_json::Map<String, Value>`; a field read is a string-keyed `BTreeMap` lookup
    and a type check, many times per decision (`Record::text("tileId")`, `int`, `strings`).
  - Copies: 422 `state.clone()` sites in engine non-test code; speculative checkpoints clone the
    `GameState` (37 KiB as JSON), the `Galaxy`, the timing registry, and three logs that grow all game
    (`Table.log`, `Resolver.table.log`, `Game.events`) at every activation, aftermath step and turn change.
  - Derived data is recomputed instead of indexed: `Galaxy::adjacent` builds a set per call, presence of a
    seat is a board scan, `all_systems`/`all_planets` build maps of the whole corpus (159 + 96 call sites).

## 3. Proposals, ranked

Gain is per workload; "est." means derived from profile shares, not measured.

| # | Proposal | Est. gain | Effort | Behaviour risk | Status |
|---|---|---|---|---|---|
| 1 | mimalloc in the `selfplay` binary | **measured -28% CPU** (bot games) | trivial | none (allocator only) | **done** |
| 2 | `units::catalogue` borrowed `Cow` | measured -8.3% with #3 | small | none | **done** |
| 3 | `with_laws`: cached system index, copy the map only when a hook adds a wormhole | (in #2) | small | none | **done** |
| 4 | `planets_in` tile index | measured -3.8% bot games | small | none | **done** |
| 5 | `may_transact`/`neighbours` without listing all neighbours | measured -21% solo native, -35% solo wasm, ~-2% bot games | small | none (same boolean, derivation in the code comment) | **done** |
| 6 | `placed_galaxy` memo | measured ~-15% of view time (with #5) | small | none (pure function) | **done** |
| 7 | mimalloc (or another allocator) in `ti4-server` | est. -20..-30% of server CPU (replay, views: both allocation-bound, 30-42% malloc/free) | trivial | none; a deployment decision | proposed |
| 8 | Speculative checkpoints without copying logs: keep a length and truncate on restore (logs are append-only between a checkpoint and its restore), or chunked `Arc` logs; an undo journal for `GameState` instead of a full copy at each activation | est. -8..-15% bot games, -20..-28% solo, -30..-40% resume; removes the quadratic term | medium | medium: every restore path must keep the prefix property; verify with a debug mode that compares against the full copy, the fork-equivalence tests and all benches | describe only |
| 9 | Bot reachability per position: one search per (seat, ship, origin) giving every reachable destination, cached in `Observed`, instead of `movable_into` per destination with a fresh `MovementRules` each | est. -20..-25% bot games (movement queries are 28% now) | medium | medium: `MovementRules` depends on the destination (active-system rules, `PassSite`); needs an equivalence test over all positions of the benches | describe only |
| 10 | Reuse one `MovementRules` per (seat, position) across destinations in `movable_into` (retarget `active_system`) | est. -6..-8% bot games | small-medium | low-medium (same caveat as 9, smaller) | describe only |
| 11 | Views: compute the viewer-independent parts (table view and objective progress, board, exploration facts) once per update and share them across subscribers; build the redacted view from `&state` instead of cloning `GameState` per viewer | est. -40..-60% of view time with 5 viewers | medium | low (pure projections; wire unchanged) | describe only |
| 12 | Other id representations (section 4): stage 0 `Arc<str>` ids + `&str` probes | measured -7.6% bot, -22% views, -17% resume | small | low | **done (section 4)**; later stages described |
| 13 | Typed content records (structs parsed once from the corpus) instead of `serde_json::Map` reads | est. -5..-9% | high | low | describe only |
| 14 | Cache `Galaxy::adjacent` per galaxy version (adjacency table rebuilt when the map changes) | est. -3..-6% bot, -5% solo | medium (Galaxy has pub fields mutated in place) | low-medium | describe only |
| 15 | Hooks evaluated per system in `with_laws` (`blocks_passage` for every system and seat): ask once whether any seat has the technology | est. -2..-3% bot | small | low | describe only |
| 16 | Typed state for `faction_marks`, `laws`, `events`, option payloads | est. small for speed, large for clarity | high | medium; changes the wire/save format unless kept as a serde view | describe only |
| 17 | One window stack (enum) instead of ~15 `Option` fields with the precedence listed twice | no speed gain; removes a class of ordering bugs | medium | medium | describe only |
| 18 | Break the 55-module cycle (rules core / faction modules / window drivers behind traits) | compile time, testability | high | low | describe only |
| 19 | wasm: try `opt-level = 3` or `"s"` for the engine (README: 36% faster at +50% size), and an allocator measured against dlmalloc | est. -20..-35% wasm time | small | none (size/download trade-off) | describe only |

### Bot-only play specifically

- Done: mimalloc (#1) and the content/movement copies (#2-#4): **-38.7% CPU** on the golden games,
  every hash identical.
- Next by size: #9 (bot reachability per position, ~25%), #8 (checkpoints, ~10%), #12 (ids).
- `PlanCtx::new` is called about once per seven decisions (2618 times in 17 514 decisions) and costs
  about 5 ms each; it is not called repeatedly for the same decision, so a per-decision memo saves
  nothing. A memo across decisions of the same position (same seat, same turn, no state change) would,
  but needs a position key; not attempted.
- The harness (timeline recorder, rift census, decision hash, usage counters) is 1-2%: not worth
  touching. Selfplay builds no views and serializes once per game.
- Parallelism is already per game (`--workers`); with mimalloc the per-thread heaps also remove
  allocator contention between workers.

## Implemented changes (all behaviour-neutral)

Before/after on the same workload, CPU time, A/B interleaved, 20 golden games (`profile-20.json`, 4 workers
unless noted), every game's hash identical in every run:

| Step | Change | Before → after (median of 3) |
|---|---|---|
| 1 | catalogue `Cow`, `systems_index`, lazy galaxy copy | 88.7 → 81.3 s (-8.3%) |
| 2 | `planets_in` tile index | 78.1 → 75.1 s (-3.8%) |
| 3 | mimalloc in `selfplay` | 75.3 → 54.3 s (-27.9%) |
| 4 | `may_transact`/`neighbours`, `placed_galaxy` memo | 55.6 → 54.6 s (-1.8%, noise level for bot games; solo -21%) |
| all | base → final (6 workers) | 85.9 → 52.7 s (-38.7%) |

Why each is identical:

- `catalogue` / `systems_index` / `planets_in`: the cached value is built by the same function from the
  same immutable compiled-in corpus, per source scope; other stores still build fresh. `Cow` derefs to
  the same map; no caller mutates it (all 200+ call sites compile unchanged).
- Lazy galaxy copy: `adds_extra_wormholes` is true exactly when `apply_extra_wormholes` would return
  true; when false the old code dropped its copy.
- `may_transact`: `partners(x)` is "everyone but x" for a seat that reaches past adjacency and
  `neighbours(x)` otherwise, so `partners(p).contains(q)` is one seat test; `neighbours` computes the
  asking seat's presence once. The derivation is in the code comment.
- `placed_galaxy`: pure function of the tiles (and the embedded corpus); the memo compares the tiles.
- mimalloc: only the allocator of the `selfplay` binary (the library, tests and examples keep the system
  allocator). Nothing decides from addresses.

## 4. String ids: other representations (2026-10-26, follow-up)

Question: could the engine use interned integers, enums or cheap-clone strings for its ids
internally and keep the old string ids everywhere outside (wire, saves, replays, option ids, JSON
views, bots, UI, wasm API, fixtures)? Before = branch head `799194e7` (sections 1-3 applied);
after = the prototype commit on this branch. Same machine, CPU time, interleaved A/B.

### 4.1 Inventory (measured)

- **24 newtypes** in `crates/ti4-model/src/id.rs`, all `struct XId(String)` with
  `#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]`,
  `#[serde(transparent)]`, `new(impl Into<String>)`, `as_str()`, `Display`; `PlanetId` alone had
  `Borrow<str>`. 16 of them carry `ts_rs::TS` (TypeScript `string`). One `UNSEATED` const (unused).
  Outside `id.rs` there is one more (`ti4-server` `LobbySlotId(pub String)`).
- Mentions (whole workspace, all crates, tests included) / `::new(` constructions in source:

  | Type | Mentions | `::new(` sites | Kind |
  |---|---:|---:|---|
  | PlayerId | 8868 | 2931 | dynamic (seat names `a`..`h`; server seat ids) |
  | SystemId | 3589 | 1627 | static tiles, plus generated (hyperlanes, Fracture, created systems) |
  | PlanetId | 1674 | 702 | static, plus created planets (`placed_planets`) |
  | FactionId | 970 | 337 | static |
  | LeaderId | 844 | 640 | static |
  | TechnologyId | 691 | 483 | static (with TE replacements / faction variants) |
  | UnitTypeId | 677 | 502 | static (generic + faction units) |
  | ActionCardId | 457 | 310 | static |
  | ObjectiveId / SecretObjectiveId | 310 / 261 | 218 / 190 | static |
  | StrategyCardId | 298 | 130 | static |
  | BreakthroughId / RelicId | 129 / 116 | 104 / 76 | static |
  | CommandId, and 9 more (Token, Agenda, Law, Event, Choice, Option, Fragment, ExplorationCard, ExpeditionTile, PromissoryNote) | 3-20 each | 0 | nearly unused |

  Per crate (all id types): engine 13 488, training 728, rule bot 716, policy 685, mlp 554, host 832,
  server 758, model 478, review 197, replayer 166, others under 100.
- **Many ids are not typed at all**: `ChoiceOption.id/kind/label`, `Choice.prompt`,
  `DecisionRecord.{chosen, offered}`, option payload keys, `faction_marks` keys (548 uses, built with
  `format!`), `laws`, `exploration_decks`, `agenda_deck`, `Game::events`, content ids inside
  `ti4-content` (`BTreeMap<String, usize>` per category) and every `Record` field name
  (`serde_json::Map<String, Value>`). The newtypes are only part of the string traffic.
- **Constructions at run time (measured)**: a uprobe on the constructor of a prototype that interned
  every id counted **40.1 M id constructions in 10 golden 4p games, 2290 per decision**. Most are
  temporaries made to look something up (`seat.technologies.contains(&TechnologyId::new("amd"))`,
  `board.get(&SystemId::new(text))`): 428 such lookup sites in engine, rule bot and host.
- **Where the string compares are (estimated from the fp profile, memcmp = 15.9% of a bot game before
  sections 1-3, 22.7% after)**: the leaf `memcmp` loses its direct caller under frame-pointer unwinding,
  so the frame above it is the function doing the lookup; grouped by the key type those functions
  compare: system ids (movement search, adjacency, `with_laws`, neighbours) ~19% of the memcmp,
  content ids and record field names (`resolve_id`, `Record::text`, the old `planets_in`) ~14%,
  technology ids (`has_technology_text`, `can_research`) ~10%, player ids (`unit.owner == player`
  filters, presence) ~8%, planet ids ~6%, sorting by id ~6%, the remaining ~37% spread over functions
  under 0.15% each. Player ids are one letter, so `==` on two different seats still calls `memcmp`
  (equal lengths).

### 4.2 Options

**(c) Cheap-clone strings (`Arc<str>`) with string semantics — prototyped.**
`struct XId(Arc<str>)`; `Eq`, `Ord`, `Hash` are the text's (with a pointer-equality shortcut, which
cannot change a result); `Debug` prints `XId("a")` exactly as before; serde writes and reads a plain
string (hand-written impls replace `#[serde(transparent)]`, same JSON); `Borrow<str>` on every id so
maps and sets are probed with `&str`. `Arc`, not `Rc`: games, snapshots and states cross threads
(server session workers, `fork_for_worker`, rayon). Ordering, iteration order of every `BTreeMap`,
fingerprints and replays are unchanged by construction. Costs: `new()` copies the text once (as
before); a clone is an atomic increment instead of a heap copy; an id is 16 bytes instead of 24
(`Unit` 40 instead of 56 bytes).

**(a) Interned ids (global or thread-local interner), u32 handles or canonical `Arc<str>`.**
- Equality becomes a pointer or integer compare. Ordering must stay the text's: interning order is
  first-come and differs between runs and processes, so an `Ord` by handle would reorder every
  `BTreeMap<XId, _>` (board, players' sets, scored objectives, ...) and with it iteration-dependent
  decisions, option order, fingerprints and replays. So `Ord` must still compare text (or a rank, see b).
- A thread-local interner breaks equality across threads (a state built on one thread compared on
  another); it must be global, which means a lock or a concurrent map on every construction.
- Measured: interning every construction (thread-local cache in front of a global `Mutex<HashSet>`)
  made the 20 golden games **26% slower** (53.2 → 67.0 s CPU, hashes identical) because of the 2290
  constructions per decision. Interning only pays after the temporaries are gone (stage 1 below).
- wasm: single-threaded, a global interner is fine; never freed memory is bounded by the id universe
  (content names, seats, generated tiles), but the server is long-running and seat ids come from lobbies.

**(b) Compile-time enums or generated constants for static content ids.**
- `build.rs` of `ti4-content` can emit one constant per id of the corpus, in sorted text order, so an
  index compare equals the text compare (`rank(a) < rank(b)` iff `a < b`) for every corpus id.
- Not closed: faction and TE variants, generated systems/planets, and tests that build their own
  `ContentStore` (not the embedded one) need an `Other(Arc<str>)` fallback, and a mixed compare must
  fall back to text; a property test must prove `Ord` = text order over all pairs.
- Serialization, `Display`, ts types: through the text (a table lookup), so the wire is unchanged.
- Biggest win (memcmp mostly disappears for content ids, maps can become arrays/bitsets indexed by
  rank), biggest effort: every `match` on id text, every `format!` that builds an id, the content
  loader, the bots' feature code.

### 4.3 Prototype (stage 0): `Arc<str>` ids + `&str` probes — measured, all benches identical

Changes (`crates/ti4-model/src/id.rs` and 94 files of mechanical call-site edits):
- the 24 newtypes hold `Arc<str>`; `new(impl AsRef<str>)` (every existing caller compiles unchanged);
  `Eq`/`Ord` with a pointer shortcut, `Hash` of the text, serde as plain string, `Borrow<str>` for all;
- 428 lookups `map.get(&XId::new(text))` / `set.contains(&XId::new(text))` became
  `map.get(ti4_model::id::text(&text))` (sites whose collection is a `Vec` keep the old form);
- the unused `UNSEATED` const is gone (an `Arc` cannot be `const`).

Before (head `799194e7`) → after, interleaved where noted:

| Workload | Before | After | Change |
|---|---:|---:|---:|
| 20 golden games, CPU, median of 3 interleaved | 53.7 s | 49.6 s | **-7.6%** |
| of that: `Arc<str>` alone | 54.8 s | 52.3 s | -4.6% |
| of that: pointer shortcut on top | 52.3 s | 51.3 s | -1.8% (noise level) |
| Solo bench native (3 runs each) | 1.75-1.88 s | 1.53-1.58 s | -12% |
| Browser resume, native (3 runs each) | 1.17-1.23 s | 0.96-1.01 s | -17% |
| Server replay `replay_session`, 4 games (2 runs) | 4.19-4.41 s | 3.70-3.76 s | -12% |
| Server views, 4 games x 5 viewers (2 runs) | 8.51-8.64 s | 6.68-6.72 s | **-22%** |
| JSON of those views | 1.90-1.94 s, 300 KiB/question | 1.87-1.90 s, 300 KiB/question | 0 (same bytes) |
| Solo bench wasm (Node, 2 runs) | 3.91-4.03 s | 3.70-3.79 s | -5% (same sha256) |
| Browser resume wasm (3 runs) | 1.79-2.08 s | 1.72-1.90 s | -2..-4% (resume check ok) |
| Peak RSS, 10 4p games 1 worker / resume | 62.5 / 104.4 MB | 59.2 / 99.7 MB | -5% |
| wasm file size | 7 096 776 B | 7 147 998 B | +0.7% |
| `bench`, `bench-v1..v5` | 200/200 each | **200/200 each** | identical |

Tests: model 94, content 146, engine 3918, host 299, rule bot 190, selfplay, wasm `--features ts`
(10 lib + 49 integration, the TypeScript wire types included) pass. Not compiled here (libtorch):
`ti4-mlp`, `ti4-tensor`, `ti4-advisor`, `ti4-replayer`, `ti4-review`; a grep finds none of the patterns
that broke elsewhere (`XId::new(x.into())`, `UNSEATED`), but `Borrow<str>` can make a
`contains(&&id)` ambiguous (one such test line needed a `*` here), so they need a compile before a merge.

Wasm gains less than native: atomics are plain operations there, but the remaining cost is dlmalloc
and the engine's own work, and the views are not on the wasm hot path.

### 4.4 Migration plan

| Stage | What | Effort | Risk | Gain |
|---|---|---|---|---|
| 0 | `Arc<str>` newtypes with string `Ord`/`Hash`/serde, `Borrow<str>`, `&str` probes (this prototype) | done (1 day incl. review) | low; wire, order and hashes unchanged by construction, all benches identical | measured: bot -7.6%, views -22%, resume -17%, replay -12%, solo -12% native / -5% wasm, memory -5% |
| 1 | Remove the remaining temporary ids and `format!` keys on hot paths (2290 constructions per decision before stage 0; count again with `count_calls.sh` on `XId::new` after stage 0); typed keys for `faction_marks` lookups | 2-4 days | low | est. -5..-10% |
| 2 | Generated rank tables for static content ids (sorted, so rank order = text order) with an `Other(Arc<str>)` fallback; `Ord` by rank when both are corpus ids, else text; property test over all pairs | 1-2 weeks | medium (custom stores in tests, TE/faction variants, every text `match`) | est. -10..-15% (most of memcmp's 23%), enables array/bitset maps |
| 3 | `PlayerId` as a seat index where the table is fixed (`a`..`h`), text at the boundary | 1 week | medium (server seat ids, lobby) | est. -3..-5% |
| 4 | Typed content records (no `serde_json::Map` field lookups) | 1-2 weeks | low | est. -5..-9% |

Not recommended: interning at every construction (measured +26% CPU), a thread-local interner
(cross-thread equality), `Ord` by intern handle (changes `BTreeMap` order and therefore play).

**Recommendation**: take stage 0 (it is on this branch, behaviour-identical, measured gains on every
workload; compile the libtorch crates once before merging). Do stage 1 next. Stage 2 only after the
larger, cheaper items of section 3 (checkpoint log copies #8, bot reachability #9, shared view parts
#11), because those remove more time per day of work; revisit ids when memcmp is the top item again.

## Files

- `tools/engine_profile/profile.sh` (entry point), `categorize.py`, `count_calls.sh`, `ab_time.sh`,
  `verify_benches.sh`, `module_graph.py`, `wasm_resume.mjs`, `profile-4p.json`, `profile-6p.json`,
  `profile-20.json`.
- `crates/ti4-selfplay/examples/engine_profile.rs` (game with views and replays, solo, resume).
- Section 4: `crates/ti4-model/src/id.rs` (the `Arc<str>` ids); call counts of id constructions with
  `COUNT_FUNCS="<mangled fn>" tools/engine_profile/count_calls.sh BIN -- BIN run ...`.
