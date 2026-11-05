# Where the typed core spends its time and memory (kernel only)

Lane `core-perf-kernel`, branch `core-perf-kernel` from `core-integration-25` (3e7a099ab), 2026-11-05. Follows `docs/PERF_AB.md` (engine A/B) and `docs/PERF_WHY.md`
(wrapper vs kernel vs string: the thin layer was the cost; the kernel was 0.42 s for 4p seed 1, 1.3x to 1.5x the string engine). This lane profiles **only the core**
(`ti4-engine-core` + `ti4-engine-core-factions`), driven directly through its driver by a new bench bin, and answers: where does the kernel's time go, who allocates
its bytes, where does its memory go, what can be indexed. Raw data and the analysis scripts are in `docs/perf_kernel_data/`.

## Short answer

* **The kernel is compute-bound (IPC 3.1, 1.7 cache misses per 1000 instructions) and its cost is algorithmic, not allocation or copying.** The state clone is not a cost
  (`CoreState::clone` 0.8 us, 3 allocations, 0.6 KB; 1 to 1.5 % of instructions); the allocator is 1 to 3 % of the samples; **a third of all instructions were three
  whole-board scans** (37 % of instructions in `planets_of` and its callers) and another tenth was an O(table) comparison in `Render::sync_codes` / `Interner::extends`.
* **Three small, behaviour-neutral changes (prototyped, kept in this branch) cut the kernel's instructions by 46 % and its wall time by 40 %** (4p seed 1: 334 ms to 188 ms;
  the 14-game suite: 4.84 s to 2.90 s, 3.05 k to 5.08 k steps/s), with **identical chain hash and final-state hash for all 14 games** and the quick-game fingerprint
  `538dc68f23dddcc9` unchanged. Allocated bytes -21 % (3133 MB to 2479 MB for the suite).
* After them the remaining kernel time is: **the per-step `Env` rebuild 25 %** (registry, ability ids, codes), **the play-facts provider 11 %** (computed in every
  `table_at`, read only when an action card is played), **board / fleet / tech scans 15 to 20 %** (owner and system indexes would remove them), the rest spread thin.
* **Memory is a non-issue for the core:** a whole game is 1.0 to 1.1 MB of heap growth, 94 % of it the output-only event queue (`ledger.events`); the position proper is about
  20 KB; the peak transient overhead of the clones in flight is about 0.1 MB. The "284 MB allocated" is churn (peak live over the start: 0.8 to 1.3 MB), of which 57 % was the
  per-step `Env` build (the `AbilityTable`/registry vectors, mostly growth by doubling and clones of a 10.7 KB table, about five times per step).

## What was already applied (not re-proposed here) and the overlaps

Already in the base (`core-integration-25`): PERF_AB's Mahact guard and the one-pass `space_cannon_kinds`; PERF_WHY's skip of the passed-by seats loop (`Table::has_window_observer`) and the
memoized ability ids and table codes in the `Render`. Items from PERF_WHY's ranked plan that were **chosen but not done** and that this lane touches: its **#6** ("env remainder: `table()` clone and
`applies.holds` over specs x seats; cache the whole `Env`") is finding R1 below (now measured: 25 % of the kernel after the prototypes), its **#7** ("index system -> planets and owner -> planets; fix the
O(159) `planets()` filters", "14 % of the kernel's samples") is finding R4 / done item D1 (measured 37 % of instructions, call-site fix done, the index is still open). Its #3, #4, #5 (one `CoreView` per
`Observed`, lazy string state in the asker, incremental mirror) are wrapper/bot items, not kernel, and are not touched.

## Method

* **Tool: `perf_kernel`** (`crates/ti4-engine-core-bridge/src/bin/perf_kernel.rs`, a dev bin; the production graph is untouched: `tools/engine_core/check_production_graph.sh` 0 violations,
  `python3 -I tools/engine_core/check_layers.py core` 0 violations). It plays whole games by driving the core's driver directly: `step_arm` / `live_arm` / `ask_arm_fn` / `advance` with
  `Pick`s chosen from the **typed** `Choice.opts` by a seeded splitmix64 weighted draw (opening a transaction weighs 0.1, as the shipped bot's random pick; the draft's last card is not a decision).
  It is `NativeGame::step_inner` / `live::run_live` minus: the wrapper `Game`, the string mirror, `TableAsker`, `Observed`, the journal, the question rendering (`render_asking`: option ids, labels,
  display facts), the event text (`pushed_since`), the decider, the per-call rollback clones and the table. What the core cannot be driven without stays and is timed apart as "adapter":
  `live::env::env_with_plays` (builds the `Env`: `AbilityTable`, registry with a text id per ability, code table), `Cx::texts` (`combat_texts`) and `Render::sync_codes`. **Setup is not timed and not the core's
  own**: the core has no setup (opening deal, map, factions are the string engine's `GameSpec::build`, loaded by `Codec::load`); the lobby spec is `native_play::lobby_spec` (TE off, card set `te`).
* Commands: `perf_kernel game <players> <seed> [faction]`, `suite [reps]` (14 games, 3p to 6p, 14 faction mixes: the seat-0 faction plus the lobby rotation by seed; Ghost once, no Empyrean),
  `loop <s>`, `mem`, `micro`, `setup`. Per game it reports steps, decisions, loop wall ms, steps/s, allocations and bytes (counting allocator, `crates/ti4-perf-alloc`, now also live and peak bytes), a
  per-phase split (env / texts / env_clone / ask / advance / arm / pick), a **chain hash** (over actor, option count and the picked typed option) and a **state hash** (FNV of `Debug` of the final `CoreState`). The build without
  `--features perf-count` uses plain mimalloc, so profiles have no counter atomics (the counting build is for the allocation numbers only).
* Determinism: two suite runs, and the base and the prototype builds, give the same `suite_hash` `775f4136e60a99d5`; all 14 games finish (14 738 steps, 14 485 decisions).
* Profiles: gate build, `CARGO_PROFILE_GATE_DEBUG=line-tables-only`, `-C force-frame-pointers=yes`, `strip=false`, `perf record --call-graph fp`; **two event types**: `cpu-clock` at 10 kHz on
  4p seed 1, 3p seed 2, 5p seed 3 (Ghost), 6p seed 2 (`profile_cpu_base_*.txt`) and **`instructions:u` every 2 M instructions** on the whole suite (`profile_instr_suite_{base,proto}.txt`; the shares are noise-free, so this
  is the main source). Shares are of samples whose stack contains `Game::play`. Inline frames are kept (`perf script` resolves them); "self" is the leaf frame, "demand" the nearest non-helper project function
  (L0/L1 helpers like `Board::planet` are charged to whoever asked), "layer" the innermost project frame. Scripts: `prof.py`, `chains.py`.
* Allocation call sites: `perf probe` uprobes on mimalloc's `mi_malloc_aligned` / `mi_zalloc_aligned` / `mi_realloc_aligned` (size argument) + `perf record --call-graph fp` on 4p seed 1 (`allocprof.py`,
  `allocdetail.py`), no valgrind/heaptrack on this machine. Counts match the counting allocator (688 861 events vs 617 077 allocations: the setup is in the first, filtered by `Game::play`).
* **Noise.** The machine was never quiet (load average 12 to 14 on 8 cores, another lane building and benchmarking in slot 2, heavy run A in slot 1). Wall times: min of 4 interleaved runs (base, prototype,
  base, ...) per game; repeated single runs of one game differ by 2 to 4 %, a single suite pass by 5 to 10 %; across a load change a single run moved by 25 %. **Quote instruction counts and sample shares;
  read wall times as +-5 %.** `perf stat` works here (hardware counters available): instructions are stable to 0.1 % between runs.

## Results: whole games, kernel only

Suite of 14 games (steps, decisions, wall = min of 4 interleaved runs, instructions from `perf stat -e instructions:u` of the whole process, about 45 M of it setup; allocations from the counting build):

| game | steps | dec | base ms | steps/s | proto ms | steps/s | instr M base | instr M proto | allocs k base -> proto | alloc MB base -> proto | peak live MB |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 3p s1 sol | 660 | 681 | 156 | 4244 | 105 | 6274 | 1710 | 1095 | 301 -> 292 | 111 -> 98 | 0.77 |
| 3p s2 hacan | 943 | 1025 | 243 | 3881 | 142 | 6632 | 2802 | 1436 | 466 -> 453 | 156 -> 137 | 0.88 |
| 3p s3 yssaril | 780 | 745 | 194 | 4029 | 117 | 6678 | 2198 | 1213 | 368 -> 357 | 128 -> 112 | 0.86 |
| 4p s1 sol | 1251 | 1250 | 340 | 3685 | 188 | 6647 | 3713 | 1811 | 614 -> 594 | 264 -> 196 | 1.14 |
| 4p s2 letnev | 1099 | 1104 | 349 | 3150 | 194 | 5650 | 3981 | 1961 | 572 -> 554 | 236 -> 176 | 1.07 |
| 4p s3 jolnar | 1193 | 1140 | 365 | 3271 | 207 | 5763 | 3924 | 2011 | 665 -> 646 | 261 -> 196 | 1.21 |
| 4p s4 mahact | 1140 | 1140 | 350 | 3253 | 225 | 5060 | 3841 | 2211 | 584 -> 565 | 252 -> 190 | 1.20 |
| 4p s5 nekro | 1157 | 1098 | 299 | 3867 | 216 | 5344 | 3152 | 2150 | 579 -> 561 | 250 -> 186 | 1.18 |
| 5p s1 xxcha | 1153 | 1060 | 406 | 2837 | 266 | 4330 | 4540 | 2745 | 665 -> 647 | 254 -> 199 | 1.14 |
| 5p s2 l1z1x | 995 | 952 | 389 | 2557 | 208 | 4779 | 4326 | 2079 | 615 -> 599 | 219 -> 172 | 1.08 |
| 5p s3 ghost | 874 | 839 | 284 | 3076 | 180 | 4850 | 3236 | 1773 | 523 -> 509 | 196 -> 154 | 0.99 |
| 6p s1 arborec | 1162 | 1099 | 513 | 2266 | 306 | 3791 | 6117 | 3304 | 702 -> 683 | 265 -> 217 | 1.21 |
| 6p s2 muaat | 1173 | 1243 | 509 | 2303 | 287 | 4081 | 5866 | 2856 | 743 -> 724 | 275 -> 227 | 1.27 |
| 6p s3 naalu | 1158 | 1109 | 439 | 2640 | 255 | 4547 | 4963 | 2477 | 678 -> 660 | 266 -> 218 | 1.25 |
| **total** | 14 738 | 14 485 | **4835** | **3048** | **2899** | **5084** | **54 370** | **29 120** | 8075 -> 7844 | 3133 -> 2479 | max 1.27 |

(base = `core-integration-25` + the bench; proto = + the three changes below. Cycles 17.6 G -> 11.3 G (-36 %); IPC 3.08 -> 2.57, cache misses per 1000 instructions 1.7 -> 3.0 (the same misses over fewer
instructions: the engine is now less compute-bound). Steps/s fall with the player count (4.2 k at 3p to 2.3 k at 6p base): per-step cost grows with the seats, the registry has one row per spec per seat, 618 rows at 4p, 930 at 6p.)
The kernel-only 4p seed 1 game is 334 ms here against 0.43 s for `NativeGame` in PERF_WHY: the question rendering and the asker are about 20 % of that figure.

## Time budget

### By phase of a step (Instant probes, counting build, base, whole suite 4.9 s; the sum is 99.3 % of the loop)

| phase | ms | share | allocs | MB | what |
|---|---|---|---|---|---|
| `advance` | 3216 | 65.7 % | 2.87 M | 1067 | the rules: windows, resolver, driver, hooks, queries (the core proper), incl. `table_at` / play facts |
| `env` | 825 | 16.9 % | 4.98 M | 1778 | `env_with_plays`: `AbilityTable` clone, registry rows, ability ids (memoized), codes, roster; 62 % of all allocations, 57 % of all bytes |
| `texts` | 623 | 12.7 % | 0.05 M | 1 | `combat_texts` + `Render::sync_codes` (9.5 % was one O(table) text comparison, fixed) |
| `ask` | 176 | 3.6 % | 0.1 M | 17 | the question the arm's open window waits on |
| `env_clone` | 15 | 0.3 % | 0.08 M | 271 | `Env::clone` per live call (10.7 KB `AbilityTable`, memcpy only) |
| arm choice, pick+hash | 8 | 0.2 % | 0 | 0 | the bench's own cost |

After the three changes (same probes): advance 1919 ms (60 %), env 850 (27 %), texts 174 (5.5 %), ask 184 (5.8 %): **`env` is now the largest adapter-side item and `advance` fell 40 %**.

### By core layer (instruction samples, innermost project frame; "demand" = who causes the layer's cost)

| layer / area | base | proto | note |
|---|---|---|---|
| L0 data (`hex`, `galaxy`, tables) | 0.4 % | 0.7 % | |
| **L1 state / containers** | **35.9 %** | **34.3 %** | `Board::planet` 23 % -> 9 % (cell reads of whole-board scans), `Interner` 5 % (text compare, `intern`), `SmallList::as_slice` 3.7 %, `keys` 1.2 %, `Players::by_seat` |
| L2 queries | 10.1 % | 18.3 % | `query::tech` 3.7 %, `fleet` 2.2 %, `board` 2.2 %, `trade` 0.9 %; the share grows because the scans above vanished |
| L3 kernel (resolver, hooks, ability table) | 4.2 % | 7.6 % | `ability::AbilityTable::registry` 2.9 %, `resolver` 0.5 %, hooks 0.3 %, seam 0.2 % |
| L4 rule areas | 14.0 % | 9.6 % | `rules::production` 12.9 % (`infantry_sites` 13.9 % demand) -> small; `technology_windows` 0.9 % |
| L5 driver | 1.6 % | 2.9 % | `driver::turn`, `live`, `windows`, `settle` (cheap; the replay `Driver::reask` is 18.5 % / 13.7 % inclusive) |
| L6 factions | 13.3 % | 4.3 % | `ce_movement` (Empyrean watcher) 6.5 %, `cc_ground` (Ghost mechs) 5.7 %, `silent` 0.7 %, `cb_economy` 0.2 % |
| adapter (render, env, plays) | 18.8 % | 18.8 % | `Render::sync_codes` 10.4 % -> 0, `env_for` 7 %, `Render::code` 2.6 % -> 5.3 % |
| allocator (inside the counting build, per sample leaf) | 1.1 % | | the plain build: malloc/free are 1 to 3 % of samples; `memcpy` 13 % (of which `sync_codes` 8.6 %) |

(Inclusive, base -> proto: `Driver::settle` 60 % -> 46 %, `live::advance` 51 % -> 52 %, `Resolver::run_window` 25.7 % -> 6.9 %, `RulesHost::condition` 24.7 % -> 6.1 %, `env_with_plays` 13.6 % -> 25.5 %,
`planets_of*` 37.0 % -> 14.2 % (the `_in` form), `Board::planet` 20.8 % -> 10.1 %, `play_facts` 6.2 % -> 11.2 %, `action_menu::sources` 6.2 % -> 11.5 %, `CoreState::clone` 0.9 % -> 1.4 %.)

### Top 15 self-time functions (base, instruction share, leaf frame <- project function that owns it)

| % | leaf <- owner |
|---|---|
| 18.2 | `Board::planet` <- itself (the cell read of every board scan) |
| 8.6 | `memcpy`/`memcmp` (libc, stripped) <- `Render::sync_codes` (`Interner::extends`: compares all ~1000 texts byte by byte at every call) |
| 3.0 | `eq` <- `infantry_sites` |
| 3.0 | `eq` <- `empyrean::watcher_window` |
| 2.9 | `SmallList::as_slice` <- itself |
| 2.4 | `eq` <- `ghost::mech_destinations` |
| 2.4 | `get` <- `Board::planet` |
| 2.1 | `FnMut::call_mut` <- `infantry_sites` (the `planets_of` filter) |
| 1.9 | closure <- `infantry_sites` |
| 1.8 | closure <- `watcher_window` |
| 1.6 | `eq` <- `live::env::env_for` (id memo / spec compare) |
| 1.5 | `Board::sys` <- itself |
| 1.2 | `memcpy` <- `Interner::intern` |
| 1.2 | `get` <- `Interner::extends` |
| 1.2 | `try_fold` <- `infantry_sites` |

After the changes the top is `Board::planet` 8.9 %, `SmallList::as_slice` 5.2 %, `eq` in `env_for` 2.9 %, `Board::sys` 2.8 %, `memcpy` in `Interner::intern` 2.6 %, `env_for` closure 2.3 %,
`then_some` in `query::tech::unlocked_upgrade` 2.0 %, `MarkKey::cmp` 1.3 %.

### Top inclusive call chains (base)

| % (inclusive) | chain (root last) |
|---|---|
| 13.9 | `infantry_sites` <- `Research::begin_paid` / `step` / `ask_of` <- `strategic::perform::run` <- `run_begin` / `SecondaryWin::follow` (Doctor Sucaban: "does this seat have infantry anywhere?", asked for every research) |
| 11.4 | `empyrean::watcher_window` <- `watcher_when` <- `RulesHost::condition` <- `Resolver::run_window` <- `Sx::emit` <- `Announce::advance` / `play_inline` (every action card played, every seat) |
| 11.5 | `ghost::mech_destinations` <- `mech_when` <- `RulesHost::condition` <- `run_window` <- `Sx::emit_mirrored` <- `tactical::Machine::emit` <- `activate::activated` (every system activation, every seat) |
| 11.0 | `Render::sync_codes` <- `combat_live::combat_texts` (4.3 %) and the `live_step` syncs (5.1 %) |
| 6.2 | `plays::play_facts` <- `provider` <- `driver::live::table_at` <- `driver_of` (every `advance`) |
| 6.2 | `action_menu::sources` <- `menu_cards::transactions` <- `open_partners` <- `may_transact` <- `are_neighbours` (every menu question) |
| 5.7 | `AbilityTable::registry` <- `env_for` <- `env_with_plays` |
| 18.5 | `Driver::reask` (the replay-from-the-start device for nested questions; inherent to the design) |

Why Ghost's and Empyrean's scans run in games without those factions: **154 of the 186 ability specs are `Applies::EverySeat`, all 154 with a condition function**, so the registry has a row per seat for each
(618 rows at 4 seats) and `RulesHost::condition` evaluates the Watcher's and Icarus Drive's conditions (a board scan) for every seat on every matching window. Only 26 specs are `Applies::Faction`, 6 `Applies::Held`.
In the suite, Ghost sits in one game and Empyrean in none.

## Allocations: who allocates the bytes (4p seed 1, base, `allocprof.py`, 272 MB / 689 k events)

| share of bytes | count | site | what |
|---|---|---|---|
| 33.6 % (88.7 MB) | 270 k | `live::env::env_for` | registry rows pushed into a `Vec` grown by doubling (`Registry::register` 51 MB / 11 k allocs), `Registry` clone (18 MB), bucket `Vec`s: ~300 small allocations per call |
| 19.4 % (51.2 MB) | 11 k | `AbilityTable::registry` | the `Vec<AbilityDef>` of the rows, grown from empty by doubling (nine reallocations per call) |
| 10.1 % (26.6 MB) | 10 k | `driver::live::table_at` | `AbilityTable::clone` (186 specs x 56 B = 10.4 KB), twice per step |
| 8.7 % (23.6 MB) | 8 k | `Env::clone` in `run_live` | the same table again (1.75 per step) |
| 8.6 % (22.7 MB) | 6 k | `containers::log::Log::push` | the open chunk of the ledger: `Vec::clone` of the shared tail (exact capacity) and then a growth, once per first append after a clone |
| 5.1 % (13.4 MB) | 3.8 k | `AbilityTable::clone` (elsewhere) | `table()` per step |
| 3.6 % (9.5 MB) | 39 k | `CoreState::clone` | 3 allocations and 0.6 KB each; 1.2 per step beyond those above |
| 1.2 % each | | `plays::play_facts`, `live_common::table_at` | |
| rest | | `Tape::pick`, `fx::journal_recs`, `Sx::emit`, `asking_of` (a `Box<CoreState>` per nested question), `transit::activatable`, `search_path` | under 1 % each |

By size class: 69 % of the bytes are in blocks of 8 to 32 KB (the table clones and the registry vectors), 90 % of the allocations are under 128 B. The `AbilityTable` is cloned about **4.7 times per step**
(once in `table()`, 1.75 in `Env::clone`, 2 in `table_at`) = 24 % of all bytes but 0.4 us each (a 10.7 KB memcpy): a byte problem, not a time problem. After the prototypes: 204 MB in the same game, `env_for` 53 MB,
`AbilityTable::clone` 37 MB, `table_at` 27 MB, `Log::push` 23 MB, `registry` 19 MB (`alloc_proto_summary.txt`).

## Memory (`perf_kernel mem`, counting build; `micro`)

| quantity | 3p s2 hacan | 4p s1 sol | 6p s2 muaat |
|---|---|---|---|
| heap live at process start / after setup | 0 / 4.27 MB | 0 / 4.64 MB | 0 / 4.63 MB |
| heap live at step 250 / 500 / 750 / 1000 / end | 4.57 / 4.76 / 4.95 / - / 5.11 MB | 4.96 / 5.15 / 5.34 / 5.53 / 5.73 MB | 5.02 / 5.24 / 5.46 / 5.67 / 5.84 MB |
| growth over the game | +0.84 MB | +1.09 MB | +1.20 MB |
| of which `ledger.events` (output queue) | 0.72 MB | 0.96 MB | 1.02 MB |
| peak live over the start of the loop | 0.90 MB | 1.17 MB | 1.30 MB |
| RSS after setup -> end (VmHWM, setup and the string game included) | 19.5 -> 23.2 MB | 21.4 -> 23.2 MB | 21.6 -> 25.2 MB |
| events in the ledger / bytes per event | 1615 / 447 | 2162 / 445 | 2314 / 441 |

* **Size of the state.** `size_of::<CoreState>()` 2880 B; the heap behind it at the end of a game (what each field owns alone, measured by moving it out and dropping it): `board` 11.7 KB, `players` 3.0 KB, `pending` 1.6 KB,
  `decks` 0.9 KB, `trade` 0.7 KB, `texts` 0.6 KB, `cards` 0.5 KB, `transient` 0.5 KB, `rng` 0.4 KB, `explore` 0.4 KB, `ledger.dice` 1.4 KB, the rest 0; **`ledger.events` 0.96 MB**. Without the event queue the position is about 20 KB.
  `Emit` is 232 B inline (`PlainRec`/`EventRec` payloads add ~215 B of heap each). The code table holds ~1060 texts (an `Interner`: a shared sealed base plus a 64-entry local suffix).
* **Live copies at once.** The persistent containers make a clone 3 allocations and 0.6 KB (`Chunked` cells and `Cow` are `Arc`s; a board edit copies one 16-cell chunk). The checkpoints in a step: `advance`'s `before`, the driver's rebuild from the position, the `reask` scratch,
  the `Box<CoreState>` of each nested question (1.3 MB allocated per game in total, freed at once). Measured peak live over the loop start (0.9 to 1.3 MB) minus the end-of-game growth (0.84 to 1.2 MB) = **about 0.1 MB of transient copies**; an in-flight fork or undo is as cheap, one tail chunk plus the edited cells.
* **Unbounded or long-kept data.** Only the ledger grows: `ledger.events` (output only, "no codec loads or stores it") is 94 % of the growth and is never trimmed; `dice` and `vp` are tens of rows. Everything else is bounded by the game
  (cards, planets, units). The 4.6 MB "static" heap (the embedded `ContentStore`, the kernels, the ability table) is per process, not per game. RSS is dominated by what is not the core: 7.7 MB binary and libc at start, +12 to 14 MB for the setup path
  (the string `GameState`, the content store), +2 to 4 MB mimalloc retention during the game.
* **Against the string engine** (`docs/perf_ab_data/after.jsonl`, random-pick bot, peak RSS of the whole process): string engine 38.6 to 46.3 MB, core behind the wrapper 27.8 to 33.7 MB, core kernel-only here 23.2 to 25.2 MB (the setup is the string engine's in all three).
* **Memory-reduction opportunities** (all small in absolute terms; the order is gain per risk):

| # | item | gain | risk |
|---|---|---|---|
| M1 | `Emit` queue: drain what the host has announced (or store `PlainRec` compactly) | up to 1.0 MB per game (85 to 94 % of the growth); RSS -4 % | medium: the queue is the string-parity output; needs the host's cursor |
| M2 | `AbilityTable` specs behind an `Arc<[Spec]>` (clone = refcount) | -24 % of allocated bytes (51 KB per step), no time | low: `register*` take `Arc::make_mut`; clone sites unchanged |
| M3 | `Log::push`: reserve the chunk when the shared tail is copied (one allocation instead of clone + growth) | -8 % of bytes | very low |
| M4 | registry rows with the capacity known (`Vec::with_capacity`, done) and the buckets as one flat `Vec<u32>` | done: `env_with_plays` 123 KB -> 69 KB per call; flat buckets: -300 allocations per step | low to medium |
| M5 | `Box<CoreState>` per nested question (`Asking.at`) is a clone kept for the renderer only | 1.3 MB per game churn | medium (the renderer reads it) |

## Repeated recomputation and O(n) that could be indexed

| id | what | how often | measured |
|---|---|---|---|
| D1 (done) | `planets_of(core, system)` = printed planets plus a **scan of all 159 planets for `placed_in == Some(system)`**, called in loops over all 231 systems by `infantry_sites`, `ghost::mech_destinations`, `empyrean::sites` (systems x planets = 36 k cell reads per call) | per research, per system activation, per action card, per seat | 37 % of instructions; call sites now hoist the placed list (`placed_planets` once) |
| D2 (done) | `Render::sync_codes` -> `Interner::extends`: byte comparison of every text of the table, at least twice per live call | 2 to 5 per step | 10.7 % of instructions; identity shortcut (same base, same `Arc`s) |
| D3 (done) | `Registry`/`AbilityTable::registry` vectors grown from empty by doubling | 1 per step | 123 KB -> 69 KB per `env_with_plays`, time neutral |
| R1 | `env_with_plays` rebuilt for every step: 186 specs x seats through `applies.holds`, one row, id (memo hit) and bucket insert each, `exploring_dual::codes` (3 % alone: interns ~30 texts), `table()` clone, `Resolver::register` | 1 per step | 25.5 % inclusive of the proto (13.6 % of base) |
| R2 | `plays::play_facts` (the "play facts": what a seat places, Reveal Prototype's research set, Mahact pools, blanked laws) computed in **every** `table_at`, i.e. every `advance`, though only a card played inside a window reads them | 2 to 4 per step | 11.2 % of the proto (`query::tech::unlocked_upgrade` 4.8 %, `reveal_payable`, `seat_more`) |
| R3 | conditions of `EverySeat` specs: Watcher, Icarus Drive and others are evaluated for seats that cannot have the unit | per window event x seat | `RulesHost::condition` 6.1 % of the proto after D1 (24.7 % of base) |
| R4 | scans of the whole board per query: `sorted_planets` (4.2 %), `controlled_planets` / `controls_a_planet_in` (4.2 %), `systems_with_units_of` (1.6 %), `mitosis::moves` (2 %), `are_neighbours` (3.7 %), `standing_in` / `held_on_board` / `remaining_all` (8.2 %), `unlocked_upgrade` (4.8 %), `open_to_faction` (1.5 %) | every menu question and production window | 20 % of the proto (demand view) |
| R5 | `Research::begin_paid`: `infantry_sites(..).is_empty()` builds the whole site list to ask a yes/no | per research | 1.2 % of the proto (13.9 % of base) |
| R6 | `Driver::reask` / `resume`: a nested question replays its step from the start | per nested question | 13.7 % inclusive of the proto (inherent to the replay design; the saving is in what the replay recomputes, R1/R2) |
| R7 | `Arc` refcount traffic of `Cow`/`Chunked`/`Interner` (atomics although the engine is single-threaded) | everywhere | under 2 %; not worth a type change |

## What was changed in this branch (all keep every hash)

1. **`query::board::placed_planets` + `planets_of_in`** (new), used by the three hot whole-board loops: `rules::production::strategy_cards::infantry_sites`, `ghost::timing::mech_destinations`, `empyrean::timing::sites`.
   Same planets in the same order (printed planets without `placed_in`, then the placed ones in planet order); test `planets_of_in_lists_what_planets_of_lists` (every system, three placed planets).
2. **`Interner::extends`**: the common case of two tables of one lineage (same sealed base, same suffix `Arc`s) is decided by pointer identity and only the texts past the shared prefix are compared; test
   `extends_agrees_with_the_comparison_by_content` against the old definition over 200 snapshots, compactions and diverged clones.
3. **Registry capacity**: `AbilityTable::registry` reserves `specs x seats`, `Registry::register` reserves the iterator's size.
4. **The bench** (`perf_kernel`), the `perf-count` feature of the bridge crate, and `ti4-perf-alloc` now counts live and peak bytes (`live()`, `peak()`, `reset_peak()`).

Checks: suite hash `775f4136e60a99d5` (chain and final-state hashes of all 14 games) equal between base and prototype builds, and between two runs; `native_play --quick` fingerprint `538dc68f23dddcc9` equal before and after
(1165 steps, 0.54 s -> 0.28 s of that bin); `cargo test --profile lockstep -p ti4-engine-core --lib` interner (13 of 13) and `planets_of_in` (1 of 1), `-p ti4-engine-core-factions --lib` ghost/empyrean/watcher (8 of 8),
`-p ti4-engine-core-bridge --test wrapper_native` 9 of 9. Not run (as asked, no heavy gate): the dual, the goldens, the soak; the changes touch no rule (a scan is hoisted out of a loop, a comparison is shortened by identity, a vector is
pre-sized), but `Applies`/registry changes (R3) would need the gate.

## Ranked plan (what is left, on the current code; share = instructions in the prototype profile; "overlap" = already in PERF_AB / PERF_WHY plans)

| # | item | gain (of current instructions) | risk | notes |
|---|---|---|---|---|
| 1 | **R2 lazy play facts**: compute `facts.plays` only when a window reads it (a `OnceCell` behind a method; ~20 reader sites read the public field) | 8 to 10 % | medium: the field becomes an accessor; the provider must stay a pure function of the position (it is, per its doc) | new |
| 2 | **R1 env**: (a) memoize `exploring_dual::codes(r)` in the `Render` like the table codes (2.9 %); (b) keep the registry rows across steps, rebuilt only when what `applies.holds` / the `Held` predicates read changes (techs, relics, leaders, factions, promissory) or the seating changes; (c) flat registry buckets | (a) 2 to 3 %, (b) 10 to 15 %, (c) 2 to 3 % | (a) low, (b) medium: invalidation, test by comparing against a fresh build at every step in a debug run, (c) low | overlaps PERF_WHY #6 |
| 3 | **R4 indexes**: system -> planets as a static table plus the placed list (`planets_of` O(1)), owner -> planets and owner -> systems with units in the board (maintained in `planet_mut`/`sys_mut` writers or recomputed per position by a `OnceCell`), fleet queries by owner | 6 to 10 % | low per site, medium for a maintained index (the `placed_in` writers are 6 sites plus the codecs) | overlaps PERF_WHY #7 (the part left after D1) |
| 4 | **R3 gate the conditioned specs** by faction (`Applies::Faction` for Watcher, Icarus Drive and the other faction-only units registered `EverySeat`) or give them a cheap precondition (does the owner have a unit of that kind: the supply count) | 3 to 6 % (also fewer registry rows: 154 of 186 specs are `EverySeat`) | medium to high: changes the registry (ids, order, passed-by logs); needs the dual and the goldens | new |
| 5 | **M2/M3** `AbilityTable` specs in an `Arc`, `Log::push` capacity | 0 % time, -30 % allocated bytes | low | new (bytes only) |
| 6 | **R5** existence check instead of the list in `begin_paid` | 1 % | very low | new |
| 7 | **M1** drain the event queue | 0 % time, -1 MB per game | medium | new |
| 8 | R6 `reask`: make the replay cheaper by caching what it recomputes (the same queries per replay) | 3 to 6 % | high | new, only after 1 to 3 |
| | **total of 1 to 3 and 5, 6** | **about 25 to 35 %** more (4p seed 1 would go from 188 ms to about 125 ms; about 8 k steps/s) | | the floor is the rules proper (`advance` without conditions and scans), roughly 40 % of today |

Cumulative with the done items: 334 ms -> about 125 ms for the 4p game. For scale only (different random games, different pickers): PERF_WHY measured the string engine at 0.30 s and the kernel behind `NativeGame` at 0.42 s for 4p seed 1;
the kernel-only loop here is already 0.19 s, so the "typed kernel is inherently 1.3x to 1.5x the string engine" of PERF_WHY was mostly three O(board) scans plus the code-table comparison, not the typed design.

## Caveats

* The suite plays random-pick games (the shipped bot's weights); real bot games spend most of their time in the bot (PERF_AB), not in the engine. Faction mix matters: Ghost and Empyrean seats make the board-scanning conditions *real* (they
  find a unit and go on), so their games pay more after D1 than the table shows; a `lobby` with Empyrean was not in the suite (the rotation by seed never seats it).
* The per-phase Instant probes cost about 25 ns each (about 10 per step); the counting allocator adds an atomic per allocation (about 3 % of samples), so profiles come from the plain build.
* "Layer" shares charge a cost to the innermost project frame; the demand view charges the nearest non-helper function. Neither is exact for inlined generic code (`eq`, `get`, `try_fold` are folded into the project function that owns them, shown after `<-`).
* The chain hash of `perf_kernel` is its own (typed options), not the string chain of `native_play`; the two bots pick differently, so the games differ from PERF_WHY's (4p seed 1: 1251 steps here, 1165 there).
* The setup (string engine) is outside the timing but inside `perf stat` (about 45 M instructions of 1.7 to 6.1 G).
* No TE-on games: the product refuses Thunder's Edge; `lobby_spec` plays TE off (the card set is `te`), as the nightly does.

## Reproducing

```text
CARGO_TARGET_DIR=... RUSTFLAGS="-C force-frame-pointers=yes" CARGO_PROFILE_GATE_DEBUG=line-tables-only CARGO_PROFILE_GATE_STRIP=false \
  cargo build --profile gate -p ti4-engine-core-bridge --bin perf_kernel [--features perf-count]
perf_kernel suite 1 | tail -1                         # suite_hash 775f4136e60a99d5
PERF_KERNEL_NOSTATE=1 perf stat -e instructions:u perf_kernel game 4 1 sol
perf record -e instructions:u -c 2000000 --call-graph fp -o x.data perf_kernel suite 1 ; python3 docs/perf_kernel_data/prof.py x.data all 40
perf_kernel mem 4 1 sol ; perf_kernel micro 4 1 sol 600
```
