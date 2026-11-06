# chance-2: randomness as a question (lanes C1b, C2, C3, C3b, C4)

Branch `core-chance-2` from `core-integration-28` (`f22ece5e8`). Not pushed, not deployed, live checkout untouched. Design:
`git show core-theory-fork-rng:docs/FORK_RNG_THEORY.md`, decisions D1-D4 (memory `project_rng_chance_design_decisions`).
Targets used: `/root/deploytmp/ch2-gate` (gate builds), `/root/deploytmp/ch2-dev2` (debug tests), `/root/deploytmp/ch2-basesrc` +
`ch2-basetgt` (the int-28 base, M0 census build), `/root/deploytmp/ch2-wasm`. Logs and scripts: `/root/deploytmp/ch2-logs/`
(`gate.sh`, `wasm.sh`, `m0_patch.py`, `m0-draws.tsv`).

## What was done, per lane

### C1b (`ac2ff8f11`): ti4-chance without `Dyn` and reseeding

* `Domain::Dyn`, `Refused::Dyn`, `Streams::reseed`, `reseed_domain` removed (D2/D3). `Domain::text()` is total (`&'static str`).
* Property test `another_seed_gives_independent_streams` replaces the reseed test.

### C2 (`53b3d0178`): the core's streams are `ti4_chance::Streams`

* `state::Rng` / `RngDomain` are aliases of `ti4_chance::{Streams, Domain}`; the core's `rand*` deps are gone; it depends on
  `ti4-chance` only. The setup shuffles and draws the speaker with ti4-chance (`ti4-engine-core-setup/src/rng.rs`).
* The `SeedSource` indirection is gone everywhere: `NoSeeds`, `TextSeeds`, `Literals`, `with_seeds`, `is_seeded`,
  `Env::seeds`, `Sx::seeds`, `RulesHost::seeds`, `Runner::with_seeds`, `lend::Kernel::seeds` (thin and legacy copies).
* Codec: `domain_in` refuses a stream name that is no literal (D3); `TextSeeds` gone; test
  `a_stream_name_that_is_no_literal_is_refused_at_load`.
* `Streams`' `Debug` prints the old `Rng { seed, streams: Cow([..]) }` text: perf_kernel's state hash is a hash of the position's
  `Debug` text, and without this it moved (`5123ce6f84221010`) although no draw changed. With it the pin holds.
* ti4-chance's bit-identity tests compare against a FROZEN copy of the old implementation (`crates/ti4-chance/tests/reference/mod.rs`);
  its dev-dependencies on the core and setup are gone (they would now compare the crate with itself and make a dependency cycle).
* Tests: a seeded rig keys its own streams at the position's stream positions and writes the positions back (what `KitSeeds` did);
  `kit_rng(seed)`, `kit_at(seed, rng)` in `ti4-engine-core-tests::rig`. An unseeded driver now draws from the position's real
  streams (before: all-zero keys): one snapshot re-blessed (`action_cards_announce/spy`: another card is taken), a few tests that
  relied on particular faces made robust (Fire Team large group, Cabal hero order, a retreat test given 160 seeds instead of 40).
* Legacy dual: "a window lent no stream" is now "the core's streams have no positions" (`dual_window.rs`).
* `.cargo/config.toml` web aliases name `ti4-chance` (and since C3 `ti4-engine-chance`).

### C3 (`daaf90512`): the core always asks; the resolver crate `ti4-engine-chance`

M0 first (on the int-28 base, `m0_patch.py`: the census records each draw's face; perf_kernel dumps the committed draws):

| perf_kernel suite 1 (14 games, 14,738 steps) | value |
|---|---|
| draws made (incl. rolled back) | 652 |
| committed draws | 350 (53.7 %) |
| draws thrown away by dry rollbacks | 302 (46.3 %); 0 by error rollbacks |
| dry rollbacks (runs) | 3,613 |
| chance requests after C3 (roll groups) | 260 (1.35 faces per request) |
| quick perf game 4p 1 sol: window runs (`PERF_KERNEL_RUNS`) | 7,400 -> 7,423 (+23 = its 23 requests: exactly one extra run each) |

Cost of always-ask (gate profile, `perf stat -e instructions:u`, `PERF_KERNEL_NOSTATE=1`, suite 1):

| build | instructions |
|---|---|
| int-28 base | 6,066.5 M |
| C2 | 6,035.9 M (-0.5 %: no `dyn SeedSource` per draw) |
| C3 always-ask | 6,121.0 M (+0.9 % vs base, +1.4 % vs C2) |

Below the C3x threshold (5 % suite), so no cost cuts beyond the two that come for free were made: one request per side's whole roll
when no Munitions Reserves reroll interleaves (space combat), and one request per `n` dice everywhere a loop drew `n` dice back to back
(`Sx::dice`, groups of at most `MAX_FACES` = 12 since the wasm trim). The space-combat "whole side in one request" batch was reverted
again in the wasm trim (code size); it saved little. Wall time is not comparable on this machine today (load 8-12 from other agents).
NOT measured: a dedicated combat-heavy setup (the theory's 6p l1z1x/jolnar/sardakk preset); the suite's heaviest games (4p jolnar
63 committed draws, 6p muaat 56) are in the number above.

The change:

* `choice::generic::Ask::Chance(Request)`, `generic::Subtype::Chance`, `Pick::Chance(Faces)`; `ti4_engine_core::chance` re-exports
  the types. `Request`/`Kind`/`Faces` derive `Ord` (the `Ask` enum does).
* `Tape::chance(state, req)`: the next tape answer, else the request becomes the run's question (when no earlier question did) and
  the run goes on with a fixed placeholder; `Tape::dice`, `die`, `index`; `Sx::{die, dice, index, chance}`, `Eff::die/roll`,
  `RulesHost::die`; `Play::apply` / `advance` / `start_questions` get the tape (the action-card effects without an `Sx`);
  `roll_bombard`, `roll_fleet`, `roll_barrage` take a die closure (the last two are the oracle's helpers only).
* Every draw site converted (inventory #1-#28 of the theory): combat side rolls (batched), Munitions Reserves, rerolls, barrage,
  space cannon offense/defense, bombardment, ground combat, Harrow, Ambush/produce, rifts, Cabal hero (batched), Kyver (Index),
  Neuraloop (Index), Spy/Sanctions (DieMod in C3, Index in C3b), Plague, Battlestation, Fracture, Courageous, card redraws, crater,
  Spec Ops (batched), Silver Flame, the Jol-Nar commander walk, Infantry II / Strike Wing / Arborec (`Eff::roll`), Scramble
  Frequency / Fire Team (factions).
* The core never touches the streams: `check_layers.py core` has a no-draw rule (`rng_mut(`, stream draws, `answer*`, `rand`,
  `ChaCha`, `random_range` in non-test code of the core and the factions). 0 violations.
* `driver::live::recall` compares the remembered position with the streams set aside (`same_but_streams`): the resolver writes the
  advanced streams before it resumes, and the memo of nestq-1 must still be trusted (else every chance answer costs a re-ask run).
  `as_replayed` and the park's `as_called` write the call's streams into their copies.
* `crates/ti4-engine-chance` (new): `Mode { Play, Search }`, `advance_resolved` / `advance_observed` / `advance_in` / `resolved(f)`
  around any `LiveFn`, `chance_of` / `chance_asked`, and the driver-level `settle` / `settle_observed` / `settle_from` (tests, the
  oracle's window duals, a search with its own streams). Play: writes the streams into the position BEFORE the resumed call; on an
  error puts them back. Search: calls once, answers nothing.
* Producers through it: the thin live loop (`live_step_inner`, both copies: thin and legacy), perf_kernel, the C0 fork tests, the
  core-tests (rig + the direct `advance_*` calls), the legacy `drive_windowed`. A pending chance question left by a search is resolved
  by the thin loop like any other (it never reaches an asker).
* Placeholder: every face the HIGHEST (`sides`), not all 1s as the theory proposed. Found by the core tests: Dannel of the Tenth (the
  Yin hero's inline ground combat) loops round after round inside one operation; with every die a 1 nobody hits and the speculative
  rest never ends. With every die hitting, the loop ends. Any "roll until" loop in the inventory ends at the highest face too (rifts
  stop at the first loss only after the last rift).
* Census (`probe-counters`) is redone for chance: requests met, faces, from tape, asked, placeholders, dry/error rollbacks.
* `check_layers.py resolver` (new mode): the resolver names only `ti4-engine-core` and `ti4-chance`, no clock/thread/entropy/unsafe.
  `check_production_graph.sh` and `clippy_core.sh` know the crate.

T3 (the proof of C3): quick game `538dc68f23dddcc9` (deterministic), suite `775f4136e60a99d5` (state hash on), and the
answered-request sequence of suite 1 (`PERF_KERNEL_DRAWS`, 350 faces in 260 requests, per game, per domain, `domain sides face`) is
byte-identical to the M0 dump of the base's committed draws.

### C3b (own commit): Spy and Sanctions unbiased (D4), and the wasm trim

* Spy (`rules/action_cards/effects.rs`) and Sanctions (`rules/agenda/effects.rs`) pick the card with `Index { n: held }` (one die with as
  many sides as cards, `index = face - 1`); `Kind::DieMod` and `ModForm` are deleted from ti4-chance. The legacy oracle makes the same
  change (`action_cards.rs` Spy, `agenda_effects.rs` Sanctions: `rng.die(DICE, held)`, recorded with the new `Dice::record_rolled`), so the
  shadow stays at zero differences.
* **DELIBERATE RE-RECORD (flagged):** policy as the user set it: no `RULES_VERSION` bump. Reason: the rules say "random"; the d10 modulo
  the hand was an engine bug (biased whenever the hand size does not divide 10), and D3 already drops old games.

  | pin | before | after |
  |---|---|---|
  | quick game (`native_play --quick --twice`) | `538dc68f23dddcc9` | `43c985a35397f2c9` (1,165 steps, deterministic; `--shadow`: ZERO GAP) |
  | perf_kernel suite 1 `suite_hash` | `775f4136e60a99d5` | `c8bd543a5d560b91` (14,788 steps; NOSTATE `6626db6287e0b664`) |
  | golden `v36-std` (4 games) | | `golden diff v36-std`: every game reproduces (none plays Spy or Sanctions): nothing to re-bless |
  | snapshot `action_cards_announce/spy` | | re-blessed (another card is taken) |
  | rule-bot bench baselines (`tools/rulebot/baseline-v*.json`), perf_ab chains, nightly baselines | | NOT re-recorded (heavy runs skipped by the user): games that play Spy or Sanctions will differ from those baselines; re-record the entries a bench verify names |
* Wasm trim (the C3 build was 6,638,662 raw, budget 6,630,000): `Faces` 13 bytes (`MAX_FACES` 12) so `Pick` stays 16 bytes; no
  `state_at`/`facts` writes in `Tape::chance`; the thin `GameRng::derive_seed` delegates to ti4-chance; `as_replayed` writes the streams
  in debug builds only (only the debug check reads them); static panic texts in ti4-chance; in the browser build only, `Request`, `Kind`,
  `Faces`, `Domain` and `Streams` print a short fixed text (native builds derive `Debug`; perf_kernel's state hash reads it there);
  `advance_resolved` has its own loop (the observer form is not linked into the browser); the side batch and the Cabal batch reverted;
  `Tape::dice` a plain loop.

### C4: Search mode tests (in `crates/ti4-engine-core-bridge/tests/fork_core.rs`, committed with C3)

* `Play` gets a Search mode: the chance questions are answered from the TEST's own copy of the starting streams
  (`advance_search`), the position's streams are asserted untouched after every call, and with `fork_every` every call is also made on a
  copy (must give the same `(Live, Out)` and position) and every chance question is also answered on a copy with OTHER faces (that copy's
  streams must not move either).
* `a_search_answers_every_chance_question_itself_never_moves_the_streams_and_forks_anywhere`: the 14 games of perf_kernel suite 1, each
  played in Play mode and in Search mode with a fork at every call and every chance question. Required and passing: the searched game's
  streams never move; same answer chain, outputs, decisions, steps; the same final position with the streams set aside; the played
  game's streams end exactly where the search's own copy ends; as many chance questions answered.
* `a_copy_taken_at_a_chance_question_resumes_later_to_the_same_game`: copies taken right after chance questions (two combat-heavy games)
  play 40 steps to the same position and chain.
* The C0 tests (T1 69 forks, T2, census) pass unchanged in meaning (the census tallies are now chance requests).

## Join notes (merging with core-multiask-1, core-search-thin-1)

* Shared files touched most: `kernel/ability.rs` (Tape), `kernel/stage.rs` (Sx), `driver/mod.rs`, `driver/live.rs` (`is_option`,
  `recall`, `as_replayed`, park closure), `choice/mod.rs` (`Pick::Chance`), `choice/generic.rs`, thin `live/mod.rs`
  (`live_step_inner`), `rules/action_cards/effects.rs` (`apply` signature), perf_kernel, fork_core.rs.
* Any new `match pick { Pick::Opt, Pick::Number }` needs a `Pick::Chance(_)` arm. Any new `Ask` area code that matches
  `generic::Ask` exhaustively needs `Chance(_)`.
* Any new caller of a `LiveFn` (a new arm runner, a search position) must go through `ti4_engine_chance::advance_resolved` (play) or
  answer the chance questions itself (search). A new random event in the core must be `sx.die/dice/index/chance` (the lint fails
  otherwise). Never draw in the core.
* New tests that drive a `Driver` directly over a window that rolls: wrap `run`/`resume` with `ti4_engine_chance::settle` (outside the
  core) or `run_r`/`resume_r` (inside the core's own tests, `driver::test_chance`).
* The thin step state lane (core-search-thin-1) should keep `ti4_engine_chance` as the only place that turns chance questions into
  answers; V1/V2 are unchanged by this branch.

## Numbers at the tip

| check | result |
|---|---|
| quick game `native_play --quick --twice` | `43c985a35397f2c9` deterministic (C3b re-record; `538dc68f23dddcc9` through C3); `--shadow` ZERO GAP |
| perf_kernel suite 1 | `c8bd543a5d560b91` (NOSTATE `6626db6287e0b664`); C1b-C3: `775f4136e60a99d5` unchanged |
| instructions suite 1 (NOSTATE) | base 6,066.5 M; C2 6,035.9 M; C3 6,116.0 M (+0.8 %); tip (C3b + trims) 6,129.1 M (other game: 14,788 steps) |
| chance requests suite 1 | C3 260 requests / 350 faces (= the base's committed draws, byte-identical sequence); tip 252 / 335 |
| tests (debug) | chance, resolver, core, factions, core-tests, thin, bridge (incl. C0 + C4 fork tests): 3,417 pass; legacy lib 5,346 (one failure fixed after the run: the lent-stream flag, re-run green); golden smoke green |
| `golden diff v36-std` | every game reproduces |
| check_layers core / resolver / chance / setup / thin, lint_core, production graph, clippy_core | 0 / 0 / 0 / 0 / 0, 0 hits, 0 violations, ok (41 = baseline) |
| **wasm (real build)** | **6,630,643 raw (budget 6,630,000: 643 bytes OVER)**, gzip 2,106,541 (ok), brotli 1,516,110 (ok). int-28: 6,624,713. Budget not raised |

## Open / remaining

* **Wasm raw 643 bytes over budget** (gzip and brotli under). Attribution (`/root/deploytmp/ch2-logs/attr/*.named.wasm`,
  `wasm_syms.py`): the growth left is `turn::inline::run` (+4.3 KB, the inline windows' monomorphs), the drop glue of
  `Box<CoreState>` (+2.8 KB, an inlining change), `SmallList::push` (+2.5 KB), ti4-chance (+3 KB, mostly moved from core/setup/thin),
  the resolver (+0.9 KB). Candidates not tried: route `GameRng::die/shuffle` of the thin crate through ti4-chance; a `Live` variant for
  chance questions without a boxed `Asking` (no `Asking` drop in the resolver loop); merge `Tape::die/index` into `chance`.
* Bench baselines (`tools/rulebot/baseline-v*.json`), perf_ab chains and nightly baselines are NOT re-recorded for C3b (heavy runs
  skipped by request); games that play Spy or Executive Sanctions will differ there.
* Combat-heavy cost measurement (a dedicated preset) not made; the suite's +0.8 % is far below the 5 % threshold.
* Next lanes: C5 (thin step state; core-search-thin-1), C6 (search API over `Mode::Search` / `settle_from`), C7, C8.

## Join notes (C3b)

* Legacy oracle: `crate::action_cards::UNBIASED_PICKS` (36) gates the string engine's new Spy/Sanctions draw; older rules versions keep the
  d10 modulo (the frozen string engine replays their golden sets: `legacy-bench/g021` diverged without the gate).
* `Kind::DieMod` / `ModForm` are gone: any branch that used them must use `Kind::Index`.
