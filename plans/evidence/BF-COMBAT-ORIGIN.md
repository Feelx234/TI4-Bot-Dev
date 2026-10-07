# BF-COMBAT-ORIGIN — the combat-origin seam, and the wave it was left inside

Date: 2026-10-04
Branch: `wp/base-factions`
Inherited HEAD: `a13f185b` ("Seed inventory baselines without replaying held card gains",
2026-10-03 20:51), on top of `cdab67c7` and `ec8e957c`
Inherited working tree: 21 modified paths, engine **not compiling**
Package class: engine implementation + repair of inherited uncommitted work
Reviewer tier: **not run** — see *Limits*.

## Objective

`crates/ti4-engine/src/combat.rs` in the inherited working tree had introduced a combat-origin
distinction for Naaz's Eidolon Maximum — hits that come from a unit ability rather than from a
combat roll — as `pub enum HitOrigin { CombatRoll, UnitAbility }`, and had changed
`absorb_hits_seeing_with` to take it. Two call sites were never updated, so the branch did not
build. The package was: finish the seam, make `ti4-engine` green, register the decision sites the
wave added, re-validate the base-faction claim ledger and the behaviour point, and commit the result
without touching the separate ML session's uncommitted work.

## What was actually wrong

```text
error[E0061]: this function takes 7 arguments but 6 arguments supplied
  --> crates/ti4-engine/src/game.rs:256   (space-cannon absorption)
  --> crates/ti4-engine/src/combat.rs:5764 (unit test for the same helper)
```

Both are hits that no combat roll produced: a space cannon fires because the system card was used,
and the test injects a hit directly. `HitOrigin::UnitAbility` is the correct label for both, and it
is the label Eidolon immunity depends on — `unit_ability_hits` is what Naaz's
`EIDOLON_IMMUNITY` suppresses (`crates/ti4-engine/src/factions/naaz.rs:244-252`), and the wave's
`absorb_hits_seeing_with_origin` routes it through `absorb_hits_with_origin`
(`crates/ti4-engine/src/combat.rs:2136-2160`).

## Changes

1. **`crates/ti4-engine/src/game.rs`** — the space-cannon call passes
   `crate::combat::HitOrigin::UnitAbility`. This is the only engine `game.rs` hunk in the commit;
   the same file also carries another session's round-income experiment (`Game::with_round_income`,
   `STATUS_INCOME_TRADE_GOODS`) and it stays uncommitted. The `SHIP_MOVED` payload hunk that adds
   `path` **is** committed: it is the producer for Muaat's breakthrough, which reads
   `event.text("path")` (`crates/ti4-engine/src/factions/muaat.rs:1163-1167`).
2. **`crates/ti4-engine/src/combat.rs`** — the test call passes `HitOrigin::UnitAbility`; the
   `# Errors` doc section required by `missing_errors_doc` was added to
   `absorb_hits_seeing_with_origin`.
3. **`crates/ti4-engine/src/technology.rs`** — a real test defect, not a compile error.
   `bf_f3_tests::a_module_waiver_requires_a_selected_payment_and_is_paid_once` hardcoded waiver
   index `0`. `research_waiver_offers` numbers only the tables that register a research-waiver hook,
   and the wave registered Yin's Brother Omar (`crates/ti4-engine/src/factions/yin.rs:144-150`), so
   the test's own hook moved to index `1` and the test paid the wrong waiver. The test now derives
   the index from `research_waiver_offers`, which is the same contract the production caller uses
   (`crates/ti4-engine/src/strategy_cards.rs:1470-1490`). The assertion is unchanged: a selected
   payment is paid once, and an unselected waiver is not paid.
4. **`crates/ti4-engine/tests/decision_delivery_inventory.rs`** — three decision sites the wave
   created were never registered, so the registry test was red:
   - `strategy_cards.rs::resolve_research` — 2 choices (which waiver, which payment), delivered as
     `ObservedVia("strategy_cards.rs::ask")`;
   - `yssaril.rs::action_card_draw_requested` — 1 choice, `ObservedHere`;
   - `OBSERVED_ASKS` gained `("yssaril.rs", "action_card_draw_requested", 1)` and
     `("yssaril.rs", "deepgloom_transaction", 1)`.
5. **`crates/ti4-engine/src/planets.rs`** — `move_placed` (new in the wave) needed its `# Panics`
   doc section.

`cargo fmt -p ti4-engine` was run once. Its whole diff was confined to files already inside the
inherited wave (`action_cards.rs`, `combat.rs`, `factions/hooks_economy.rs`, `factions/muaat.rs`,
`factions/naaz.rs`, `game.rs`); no already-clean committed file was reformatted.

## Commands and exact results

All engine/test commands ran with `CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0`; the soak ran as
twelve concurrent single-threaded slices on a 32-core host.

Parallelism note: `link.exe` failed transiently (`exit code: 1102`) when the engine suite was linked
with `-j 8` on a machine already carrying twelve soak processes plus other sessions. The identical
command succeeded at `-j 4`. Compile-only work at `-j 8`/`-j 16` was fine. Practical rule for this
host: run heavy link steps at `-j 4`, and keep soak/diagnostic processes parallel — that is where the
throughput actually comes from.

```text
cargo check -p ti4-engine --all-targets -j 8      -> Finished, exit 0
cargo test -p ti4-engine -q --no-fail-fast -j 8   -> 1928 passed / 0 failed / 1 ignored (lib)
                                                     1 passed (integration), 4 passed
                                                     (decision_delivery_inventory), 5 passed,
                                                     all doctests ran, 0 failed
cargo test -p ti4-engine --lib -q                 -> 1928 passed / 0 failed / 1 ignored in 3.50s
cargo test -p ti4-sim --lib -q                    -> 51 passed / 0 failed / 1 ignored in 10.88s
cargo check --release -p ti4-policy -p ti4-sim --all-targets -j 8 -> Finished, exit 0
```

The one ignored `ti4-engine` test is `factions::tests::print_faction_ledger` (the ledger printer,
confirmed with `cargo test -p ti4-engine --lib -- --list --ignored`). The one ignored `ti4-sim`
test is the retired behaviour gate
`the_suite_reproduces_and_stays_within_the_recorded_bounds`.

### Lints

`cargo clippy -p ti4-engine --all-targets` before: 22 warning lines, of which 3 pointed at files
inside the inherited wave (`combat.rs:2140` missing `# Errors`, `combat.rs:4412`
`unnecessary_map_or`, `planets.rs:61` missing `# Panics`). After the fixes **no warning points at a
file inside the wave**: the same command reports `ti4-engine` (lib) 14 and `ti4-engine` (lib test) 18
with 14 duplicates — 18 distinct engine warnings — plus `ti4-model` (lib) 1. All are pre-existing
(`type_complexity`, `too_many_arguments`, `derivable_impls`, `needless_range_loop`,
`assigning_clones`, `unnecessary_sort_by`, `manual_checked_times`) and were left alone.

`cargo clippy -p ti4-sim --all-targets`: `ti4-sim` (lib) 16 warnings, `base_faction_soak` example 3,
`ti4-policy` (lib) 7, `ti4-model` (lib) 1. No `ti4-sim`, `ti4-policy`, or `ti4-model` file is part
of the inherited wave (`git status` shows none modified), so none of these are in-wave; they were
not touched.

### Base-faction ledger

`cargo test -p ti4-engine --lib print_faction_ledger -- --ignored --nocapture`

```text
arborec 12/12   argent 12/13   ghost 12/12   mentak 12/12
muaat   13/13   naalu  11/13   naaz  11/13   saar  12/13
sardakk 12/12   winnu  10/11   yin    9/11   yssaril 10/12
total 136/147
```

The denominator moved from 145 to 147 because the wave registered further base-faction assets
(including `naaz_voltron` and the Muaat breakthrough parts), so 136/147 and 133/145 are not the
same measurement. Still unclaimed: `argentagent`; `naaluagent-te`, `naalucommander`;
`naaz_voltron`, `naazbt`; `saarbt`; `winnuhero`; `yinhero`, `yinbt`; `yssarilagent`, `yssarilbt`.

### Seated soak, parallelised

```bash
BIN=target/release/examples/base_faction_soak.exe
for f in arborec argent ghost mentak muaat naalu naaz saar sardakk winnu yin yssaril; do
  "$BIN" "$f" 0 200 > "target/soak/$f.log" 2>&1 &
done; wait
```

Twelve concurrent slices, 200 seeds each, each seed played twice (play + replay comparison):
**2,400 games, 0 failures** — no engine error, no refusal, no replay divergence. Wall clock
**7 m 06 s** for the whole campaign (measured on the second, timed run: 00:51:43 → 00:58:50).
Run twice back to back with identical results.

### Behaviour point

`cargo run --release -q -p ti4-sim --example rebaseline_behavior -j 4` (six original in-scope
factions, 30 seeds, authored bot, frozen maps):

```text
completion 1.000000        faction_differentiation 0.615464   score_spread 1.950651
share_INVASION_RESOLVED 0.014055   share_PRODUCTION_RESOLVED 0.025814
share_SHIP_MOVED 0.046439          share_SPACE_COMBAT_RESOLVED 0.003217
share_SYSTEM_ACTIVATED 0.051628    share_TACTICAL_ACTION_BEGAN 0.025229
vp_pace 0.472222
```

Every value is **identical to the recorded `c5632448` point**, and the recomputed 95% intervals
match the recorded ones (`share_SHIP_MOVED` 0.039444–0.053434, `vp_pace` 0.444444–0.500000). The
five metrics that sit outside the retired v45 bounds are unchanged and remain a separate
re-baselining decision. This batch does not include Naaz, so it does not measure the Eidolon change;
that change is covered by the `naaz` soak slice, not by this diagnostic.

## Decisions

- **`HitOrigin::UnitAbility` at the space-cannon site** rather than relabelling the helper: a space
  cannon hit is not produced by a combat roll, and Eidolon immunity is defined against ability hits.
- **Derive the waiver index instead of hardcoding it.** The fixture was correct; the assumption that
  the test's hook is the first waiver in the game was not. No fixture was regenerated.
- **Commit the inherited engine wave, not only this package's own edits.** A green HEAD requires it:
  the wave's `combat.rs`, `strategy_cards.rs`, `transactions.rs`, `status.rs`, `invasion.rs`,
  `supply.rs`, `planets.rs`, `action_cards.rs`, `hooks_*.rs` and the Muaat/Naalu/Naaz/Yin/Yssaril
  files are one interdependent unit, and committing only the new edits would leave HEAD
  non-compiling. The wave's own evidence (`BF-naalu.md`, `BF-yin.md`) is committed with it.
- **Exclude the other session's work.** `game.rs` is staged partially: the `HitOrigin` hunk and the
  `SHIP_MOVED` `path` hunk are committed; the round-income experiment is not. `ti4-mlp`,
  `ti4-policy`, `ti4-training`, `ti4-replayer`, `ti4-review`, `scripts/`, `plans/INDEX.md` and
  `target-cuda-repack/` are untouched and uncommitted. Verified safe: `git show
  HEAD:crates/ti4-training/src/rollout.rs` contains no `with_round_income` reference, so HEAD does
  not depend on the excluded experiment.
- **Environment note for later sessions.** The pinned `out/libtorch-2.9.1-cpu/lib` has no `include`
  directory, so a *fresh* `torch-sys` C++ build fails (`fatal error C1083: Cannot open include
  file: 'torch/torch.h'`). Setting `LIBTORCH` in the environment invalidates the cached fingerprint
  and triggers that rebuild. With `LIBTORCH` unset the cached debug artifacts are used and
  `cargo test -p ti4-sim --lib` passes; `--release` builds work. A whole-workspace debug build is
  therefore not available in this environment right now — set `LIBTORCH` only for the release
  diagnostics that need it, and unset it afterwards.

## Limits and open findings

1. **No independent review was run.** No review peer was available in this session, and the
   reviewing agent is not the implementer. The changes above are self-reviewed. Tier C/D review is
   still owed for the wave as a whole.
2. **The wave's implementation was authored in the previous session's uncommitted working tree**, not
   by this package. This package completed it (call sites, test index, registry, lints) and
   re-validated it; it does not re-derive its design decisions.
3. **11 base-faction assets remain unclaimed** (listed above), including `naaz_voltron` and the
   Muaat/Naaz/Yssaril/Saar breakthroughs that the wave partly implements.
4. **The Naaz Eidolon behaviour change is not measured**, only soaked: no fixture asserts a Naaz
   combat where an ability hit is suppressed and a roll hit is not. That is the natural next test.
5. **The retired behaviour bounds are untouched** — five metrics remain outside them, unchanged by
   this package.
6. **`ti4-sim` integration examples were not run** beyond `rebaseline_behavior` and the soak; the
   `ti4-sim` integration tests (1/1, 4/4, 5/5) were run as part of the engine/sim package suites.

## Verification that the committed tree is green

The staged `game.rs` (HEAD plus the two committed hunks, without the income experiment) was written
into the working tree from the index (`git show :crates/ti4-engine/src/game.rs`), built with
`cargo check -p ti4-engine --all-targets -j 8` -> `Finished`, exit 0, and then restored; the working
file's blob hash is identical before and after (`cc0f6d41d966eff70b233c1a97f70b1e6366129b`), so the
swap proved the commit compiles without disturbing the other session's file. An isolated
`git checkout-index` copy of the whole index was also built; that attempt failed on unrelated
toolchain noise in a cold target directory (`only metadata stub found for rlib dependency core`,
`link.exe` failures) and was discarded rather than chased.

## Next safe action

Add the missing Naaz fixture: a ground combat where a `HitOrigin::UnitAbility` hit is suppressed by
Eidolon immunity and a `HitOrigin::CombatRoll` hit is not, then claim `naaz_voltron` in the ledger
if the Voltron unit behaviour the wave implemented is real. After that the remaining base-faction
gaps are eight leaders/breakthroughs, which are the wave-D queue items the previous handover
already listed.

## Subsequent independent review, 2026-10-04

See [BF-CODEX-INHERITED-REVIEW-2026-10-04.md](BF-CODEX-INHERITED-REVIEW-2026-10-04.md) for findings and corrections to producer taxonomy, placement/release gap claims, atomic-package scope, and the limits of aggregate compatibility diagnostics. This preserves the original implementation record; later WIP is separately reviewable.
