# Why the core engine was slower than the string engine, and what to do about it

Lane `core-perf-why`, branch `core-perf-why` cut from `core-botstops-1` (6b884bc84), 2026-11-04. Follows `docs/PERF_AB.md` (core alone 3.9x to 5.7x slower per
game, rule-bot games 1.4x to 1.75x slower per decision). Question: is that inherent in the typed design, or is it the thin layer around the core? Raw data and
the tools are in `docs/perf_why_data/`.

## Short answer

**It was the thin layer, not the typed kernel.** For a random-pick 4p game (seed 1, 1165 core steps) the wrapper `Game` took 1.50 to 1.60 s, the core kernel alone
(`NativeGame`: the same loop without the mirror, the asker and the table) 0.80 to 0.85 s, the string engine 0.29 to 0.32 s. Inside the wrapper's 1.56 s:

* **a third (515 ms) was one bug-shaped loop**: for every seat a reaction window passed by (7.6 times per step) the wrapper built the whole string `GameState` (55 us, 1190
  allocations) to hand it to a window observer that exists only in the server's bluff banner. In a bot game, in a test, and in every seat without the banner the observer is
  `None` and the state was thrown away.
* **a quarter (410 ms) was `env_with_plays`**: the ability registry and its text ids rebuilt from scratch at every step (hundreds of ids formatted with `format!` and
  looked up in the code table, plus about a hundred more code lookups for the table's codes). The string engine arms its resolver **once when the game is seated**
  (`reactions::arm`: "Called once when the game is seated").
* **the kernel proper (the rules) was 28 % (438 ms)**, which is 1.3x to 1.5x the string engine's whole game. That is the part that can be called inherent, and it is small.
* the per-question view (7 %) and the per-step mirror (5 %) are the remaining thin-layer costs.

Two changes (below, both in this branch) took the wrapper from 1.56 s to 0.66 to 0.70 s and the kernel alone from 0.82 s to 0.43 s; **every one of 24 random games
(3p to 6p, seeds 1 to 6) plays the identical game (chain hash, steps, decisions), the quick game fingerprint is unchanged (74d5675ac754ac81), and 9 rule-bot/replay games of 4p
give equal chains too.** The core is now about 2.2x the string engine on this game (was about 5x); with the rule bot 1.1x to 2.2x per game (was 1.8x to 2.9x). The ranked plan
for the rest is at the end: about 1.4x is reachable without touching the rules, the floor is the kernel at about 1.3x.

## Method

* Tools (all in this branch): `perf_why` (`crates/ti4-engine-core-bridge/src/bin/perf_why.rs`, gate build): `layers` plays the same random-pick game as the kernel
  (`NativeGame`), as the wrapper (`Game`) and as the string engine, with a **counting allocator** (`crates/ti4-perf-alloc`, its own crate because the workspace forbids
  `unsafe`) for allocation counts; `micro` times the per-step pieces on the live core after N steps; `docs/perf_why_data/probe.patch` is the **timing probe** (Instant
  counters around each phase of `step_native`/`native_step`/`TableAsker::ask`, plus a `probe` command that runs the wrapper and prints them; not part of the product).
  Profiles: `perf record --call-graph fp` on a gate build with line tables, frame pointers and no strip (`CARGO_PROFILE_GATE_DEBUG=line-tables-only`,
  `RUSTFLAGS=-C force-frame-pointers=yes`, `CARGO_PROFILE_GATE_STRIP=false`), aggregated by inclusive share of samples with a stack that contains a frame.
* Games: 4p `lobby_spec("sol", 4, seed)` as in PERF_AB. Representative ones: 4p seed 1 random-pick (the layered budget), 4p seed 2 replay of its own recorded smart game (engine only
  on a real trajectory) and 4p seed 2 with the rule bot (the shipped decider).
* Load: **the machine was never idle** (another lane kept 1 to 4 cores busy: the 1-minute load was 1.2 to 4 at the starts). Layer-to-layer comparisons are inside one process back to back
  (same load), shares come from one run each, and the before/after timings against PERF_AB's data (taken at load 1.1 to 1.2) are therefore conservative for the after side.
  Read absolute numbers as +-10 %; the shares and the allocation counts are not noisy.

## Per-layer time budget (4p seed 1, random-pick bot, 1165 steps)

Timing probe on the wrapper `Game` (`probe_before.txt`, `probe_after.txt`); "kernel rules" is `live_step` without the question hand-off, plus the reconcile hooks.

| layer | before ms (share) | after ms (share) | what it is |
|---|---|---|---|
| kernel rules (typed core: windows, resolver, driver, queries) | 438 (28 %) | 416 (59 %) | inherent part; 1.3x to 1.4x the string engine's whole game |
| passed-by seats notified (`window_skipped` loop) | 515 (33 %) | 0 | one string state per seat, to an observer that is `None` |
| env / registry (`env_with_plays`) | 410 (26 %) | 69 (10 %) | registry rebuilt per step, text ids formatted and interned, table codes interned |
| asker (`TableAsker::ask`: one string state + `Observed` per question) | 111 (7 %) | 111 (16 %) | 87 ms of it is the string state, 8 ms the table's `ask_seeing` and journal |
| mirror (`materialized` + drop of the old state, rolls, secondary) | 83 (5 %) | 104 (15 %) | the string `GameState` the host reads, rebuilt in full after every step |
| total (wrapper) | 1557 | 700 | string engine: about 300 |

Same game by layer, wall time and allocations (`layers_before.txt`, `layers_after.txt`; three repetitions each, the middle value):

| layer | before | after | string engine |
|---|---|---|---|
| kernel (`NativeGame`) | 0.82 s, 5.0 M allocs, 387 MB | 0.44 s, 0.87 M allocs, 284 MB | |
| kernel + mirror every step | 0.93 s, 6.3 M allocs | 0.56 s, 2.2 M allocs | |
| kernel + mirror + one string state per question | 0.98 s, 7.8 M allocs | 0.64 s, 3.6 M allocs | |
| wrapper `Game` | 1.51 s, 18.2 M allocs, 1450 MB | 0.66 to 0.74 s, 3.75 M allocs, 524 MB | 0.30 s, 1.67 M allocs, 132 MB |

What the numbers say about the questions of the brief:

* **Cloning the position: not a cost.** `CoreState::clone` is 0.9 us and 4 allocations (persistent chunks); the two clones per step are 2 ms of 1560. A `GameState` clone (what a string-shaped step has to avoid) is 12 us and 214 allocations.
* **Codec load of the whole position per step: does not happen.** `Codec::load` runs once per game in the wrapper (64 us). The string-shaped **store** (`materialize`, 55 us, 1190 allocations, 96 KB:
  Board 15 us, Players 8, Decks 4, Cards 3, Trade 3, Explore 2.5, Transient 1.3, the rest under 1 us each) is the expensive crossing, and it was called about ten times per step:
  once for the mirror, once per question, and once per passed-by seat (7.6).
* **Per-step rebuilding of derived data: yes, and it was the second biggest cost** (`env_with_plays`, 300 us and 3850 allocations per step, 47 % of the kernel loop on its own).
* **Journaling and the table:** `Table::ask_seeing` plus the journal are 8 ms of 1560 (0.5 %).
* The kernel allocates half as often as the string engine (0.87 M against 1.67 M) once the env is cached; its remaining bytes (284 MB against 132 MB) are mostly the
  per-step `AbilityTable` clone (123 KB) and the mirror.

Rule-bot game (4p seed 2, `perf record`, before the changes): the bot is 51 % of the run (`choose_seeing`); `Codec::load` is 18.5 % (14.5 % `CoreView::of <- ProdView::of <- production::available`,
3.7 % `SeenOwned::from_parts`); mirror and passed-by seats 17.8 %; env 10.3 %; dropping the old string states 4.6 %. Replay of the same game (no bot): mirror and passed-by seats 37.9 %, env 21 %, live step 30 %,
dropping string states 9.2 %.

## For each big cost: what the string engine does, and the cheapest faithful equivalent

| cost | string engine | cheapest faithful equivalent for the core |
|---|---|---|
| string state per passed-by seat | has no such step: it keeps the string state itself and the observer reads it in place | ask the table first (`Table::has_window_observer`) and build the state once per step. **Done.** |
| registry and ids per step | `reactions::arm` once at seating (one standing slot per seat and window; conditions are evaluated lazily when an event fires, "the resolver has no unregister") | memoize the typed id per (rule, seat, lender, factions) in the `Render` (its code table only grows, so a code never changes); cache the table's codes per `Render`. **Done.** Next: cache the registry keyed by what `applies.holds` reads (techs, relics, leaders, factions), or derive it only when those change |
| mirror after every step | none: `state` is the truth | mirror only the slices that changed (compare each slice with the `before` clone the step already takes, or by chunk identity), or make `state` lazy (needs the public field `Game::state` to become a method) |
| string state per question | none | build the `Observed` lazily: only the view-reading deciders and the few enrichments (`about_invasion`, `details.agenda`) need `seen.state` |
| bot queries (`Codec::load` of the whole position per query) | the same `Observed` code runs on the string engine too, so this cost is shared | one `CoreView`/`Full` per `Observed` in a `OnceCell` (the position is immutable there) |
| planet scans in the kernel | the board is a map system -> planets, so "planets of a system" and "planets I control" are local | an index system -> planets and owner -> planets (or a static table for the first); `Board::planet` is 14 % of the kernel's samples, from `planets()` filters in `mech_destinations` (Ghost), `infantry_sites`, `planets_of` |

## What was changed in this branch (both keep every fingerprint)

1. **Passed-by seats** (`crates/ti4-engine-next/src/game/mod.rs`, `wire/choice.rs`): `Table::has_window_observer()`; the loop in `native_step` stops at once when nobody listens and otherwise builds the string
   state once per step instead of once per seat (`event_text` only reads the code table, so skipping it changes nothing). Effect: the wrapper -33 % (random 4p seed 1: 1.56 s to 1.02 s; allocations 18.2 M to 7.9 M).
2. **Ability ids and table codes memoized in the `Render`** (`render/mod.rs`, `live/env.rs`, `ties/reactions_core.rs`): `Render::ability_id_memo` keyed by a packed (slot, owner, lender, owner faction,
   lender faction) with a multiplicative hasher (the first version with std SipHash spent 4.6 % of the kernel in hashing), and `cached_table_codes` for `set_table_codes`. Validity: the text of an id is a function of the key and the `Render`'s tables, and the code table
   only grows within a `Render`; a failed step and a fork make a fresh `Render` (empty memos), and `sync_codes` only ever replaces a table by one that extends it. `env_with_plays`: 410 ms to 64 to 69 ms; the kernel alone 0.82 s to 0.44 s; the wrapper
   1.02 s to 0.66 to 0.70 s.

Checks (no heavy gate): `native_play --quick` fingerprint 74d5675ac754ac81 unchanged; all 24 random games of PERF_AB (`random_24_after.jsonl`) equal in chain, steps, decisions and finish to `docs/perf_ab_data/after.jsonl`
(37.8 s of loop time before, 15.0 s after); the 9 4p games (random, rule-bot record, replay of that record, seeds 1 to 3: `sweep_after_4p.jsonl`) equal in chain and steps; the smart 4p seed 2 chain `e3df2ee60f34f5d9` equal on record and replay;
`cargo test --profile lockstep`: `wrapper_native` 9 of 9, `kernel_env` 3 of 3, `ti4-server` `it bluff_reaction` 11 of 11 (the window observer path). Not run: the gates, and `ti4-engine --lib reactions_core` (the package is `ti4-engine`; the memo does not touch what it checks).

Timings of the 9 games against PERF_AB's `after.jsonl` (the same games; before at load 1.1 to 1.2, now at load 1.4 to 3.8; legacy from `after.jsonl`):

| mode, seed | core before ms | core now ms | speedup | legacy ms | now / legacy | before / legacy |
|---|---|---|---|---|---|---|
| random 1 | 1461 | 712 | 2.05x | 311 | 2.3x | 4.7x |
| random 2 | 1357 | 614 | 2.21x | 264 | 2.3x | 5.1x |
| random 3 | 1524 | 643 | 2.37x | 402 | 1.6x | 3.8x |
| replay 1 | 3893 | 1657 | 2.35x | 675 | 2.5x | 5.8x |
| replay 2 | 2796 | 1159 | 2.41x | 371 | 3.1x | 7.5x |
| replay 3 | 2230 | 856 | 2.60x | 568 | 1.5x | 3.9x |
| rule bot 1 | 8793 | 6183 | 1.42x | 4009 | 1.5x | 2.2x |
| rule bot 2 | 5787 | 4392 | 1.32x | 1996 | 2.2x | 2.9x |
| rule bot 3 | 4112 | 2655 | 1.55x | 2340 | 1.1x | 1.8x |

(The rule-bot games differ between the engines: compare per decision. Rule bot 4p seed 2 per decision: core 2.84 ms before, 2.15 ms now, string 1.33 ms. The bot itself is unchanged: 2.9 to 3.1 s.)

## Ranked plan (gain on the current wrapper, 4p seed 1 random: 0.66 to 0.70 s; string engine 0.30 s)

| # | item | gain | risk | note |
|---|---|---|---|---|
| 1 | done: skip the passed-by loop | -33 % of the original wrapper | very low | |
| 2 | done: ids and table codes memoized | -22 % of the original wrapper | low | invalidation = lifetime of the `Render` |
| 3 | one `CoreView` per `Observed` (`OnceCell`) for the bot's queries (`production::available`, `SeenOwned::from_parts`, `tech_preps`) | -15 % to -18 % of a rule-bot game, on both engines | low to medium | the same pattern as PERF_AB's Mahact fix; shrinks the bot's 51 % share |
| 4 | the asker builds the string state only when the decider wants a view or the context needs it (`about_invasion`, agenda details) | -12 % of the current wrapper (87 of 700 ms) | medium | `Table::ask_seeing` reads `seen.state` for the enrichment and the `observed_offer` callback |
| 5 | mirror by changed slices (equality against the `before` clone, or chunk identity) or a lazy `state` | -7 % to -12 % (104 ms of 700) | medium (lazy: an API change of a public field read by the host after each step) | Board 15 us and Players 8 us dominate a store; most steps change one to three slices |
| 6 | env remainder: `table()` clone (123 KB per step) and `applies.holds` over specs x seats; cache the whole `Env` keyed by the facts the specs read | -6 % to -9 % | medium | needs an invalidation story (tech, relic, leader, faction changes) |
| 7 | index system -> planets and owner -> planets in the board; fix the O(159) `planets()` filters | -5 % to -8 % (14 % of the kernel's samples) | low per site, many sites | Ghost `mech_destinations`, `infantry_sites`, `planets_of` |
| | total of 4 to 7 | wrapper about 0.42 to 0.45 s, about 1.4x the string engine | | the floor is the kernel rules at about 1.3x; with item 3 a rule-bot game gets close to parity (1.0x to 1.2x per decision) |

By gain per risk: 3, then 4, 7, 5, 6. Items 3 and 7 pay on the server and in self-play too (bot seats), 4 and 5 only in per-step cost.

## Caveats

* One representative random game was layered in detail (4p seed 1); the 24-game check shows the same direction everywhere (total loop time 37.8 s to 15.0 s), the shares per player count were not remeasured.
* `perf` with frame pointers attributes inlined code coarsely; the per-phase numbers above come from the timing probe, not from `perf`. The probe adds an `Instant` pair per phase (it showed the same total as the unprobed run within 3 %).
* The `micro` timings are tight-loop (hot cache) numbers: `materialize` is 55 to 60 us there and about 80 us per call in the real loop.
* `docs/perf_ab_data` numbers for the string engine were not re-run (the string engine is unchanged, but the thin `Observed` is shared).
