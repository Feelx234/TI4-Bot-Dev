# zero-bail and d4gate of heavy run A (lane core-zerobail, 2026-11-05)

Branch `core-zerobail` from `core-duallent-fix` (e8fab2758). Nothing pushed or deployed. Rules version stays 36; no decision stream changed
(quick fingerprint `538dc68f23dddcc9`, see section 5). Heavy lock slot 1, `nice -n 10`, own target `/root/deploytmp/zerobail`, outputs in
`/root/deploytmp/zerobail-out`.

Run A (`/root/end-of-d1-20261105`) failed two steps of this lane: `zero-bail` (39,747 bails) and `d4gate` (11 windows uncovered, stage
variants below 80 %). Both are fixed; the cause of each is below.

## 1. zero-bail: what the 39,747 were

The step counted the `[bail: ...]` / `[gate: ...]` rows of the dual reports (`dual-all.log`; `dual-lent.log` was the log of a dead step and
counted nothing, which the step did not notice). Those rows are counters of the DUAL, which plays the string engine and lends windows to the
core. None of them says that a game stopped. Classified by reading the code behind every label (the full table with the evidence per row is
`tools/engine_core/zero_bail_classes.tsv`):

| class | causes / rows of run A | what it is | native proof |
|---|---:|---|---|
| census | 20 causes, 38,179 | `d1::step [bail: <arm>]` (the string driver's step arms, `check_arm` in `d1_pending.rs` counts every arm the dual does not lend whole), `d1::reconcile [bail: announce_*, nexus_unlocked]`, `turn::hooks [bail: minister_of_war / technology::end_turn / legendary::end_turn]` (`note_lent_hook`, the doc of `turn_flow::Hook` called them "still string bodies": they have been jobs of `driver::turn::end` for a long time) | `driver::live::live_arm` has a live function for every arm; lockstep tests per arm |
| adapter | 24 causes, 1,568 | a lent WINDOW the dual's adapter declines: hook chain of an activation (Scanlink, Starlancer, Ceasefire, Nullification, Minister of Peace, Dominus Orb), Titans' Coalescence and Terragenesis, Nova Seed, the Sardakk hero, Cabal end notes, Integrated Economy nested in a control gain, the Construction production of some seats, card announces (`Announce::of(.., registered = false)`), `nested (an action card the core does not play)` (the dual's default ability table has no plays provider) | the same windows run in the native path (`driver/windows/tactical/hooks.rs`, `rules/invasion/forced.rs`, `cd_registry/titans/offer.rs`, ...) and each has a lockstep test string against native (`tactical_live.rs`, `invasion_live.rs`, `combat_live.rs`, ...) or a core test |
| gap | 0 | a position a game can reach that the core does not play | none found |
| mis-count | 1 (4 rows in the lent log) | `game::close_vote [bail: NO REASON RECORDED]`: an adapter `return None` without a reason (agenda_close_dual.rs, the lent set), not classifiable | belongs to lane core-lent-gaps (the same Artuno chain); the check now FAILS on such a row (see 2) |

So: (a) real core gaps: none found among the 44 causes; the native playthrough finds none either (section 2). (b) by design: 38 of 44
causes are the dual's census or a decline of its adapters; they cannot go to zero while the dual drives a string `Game`, and they stop
being "bails" the moment a game is played by the wrapper (which is what the product and the benches do). (c) mis-counting: two real ones,
fixed in the acceptance: a log of a dead step was read as a clean corpus (`dual-lent.log`), and the check never looked at the thing the
rule is about, a whole game on the core alone.

One cause is a double count of one event: `tactical::activate [bail: a run the driver refused]` follows
`nested (an action card the core does not play)` in the same activation (golden game 13; the nested bail is counted, then the window
driver returns `NotAsking`). Left as it is; both are classified.

### What the step does now (`tools/engine_core/zero_bail.py`, `zero_bail_classes.tsv`, `end_of_d1.sh` step `zero-bail`)

1. NATIVE evidence, the rule itself (enforce requires it): `native_play --factions all --players 3,4,5,6 --seeds 1` (96 games) and
   `--first-seed 2 --seeds 3` (288 games), all lobby factions, `te` card set: every game finished, 0 gaps, last line `ZERO GAP`
   (measured today: 96/96 and 288/288); and the bench results of the steps `bench-*` (rule bot WITH the view on the native wrapper, 6 configs
   x 4 churn modes x 200 games): every game a `victory`, `problems` empty (agenda phases included: `native_play`'s bot is the blind random
   pick and never reaches an agenda, the benches do). Six distinct bench configs are required.
2. Every dual cause must match a row of `zero_bail_classes.tsv` (class `census` / `adapter` / `gap`, first match wins, whole-string regexes).
   Unclassified, silent (`NO REASON RECORDED`), `gap`: fail. A log that is not a complete dual report (no `games N | areas on dual` header
   or no `dual: N calls` line) fails instead of counting nothing.
3. Every row cites its evidence as `path::token`; the step checks that the file and the token exist (a renamed lockstep test breaks the
   step). `tools/engine_core/test_zero_bail.py` (18 tests) pins all of this, including that the cited evidence of the real table holds.

Standalone, exact command of the step with `ZERO_BAIL=enforce` on a fresh `dual-all` of this branch (0 differences, same 34,626,595 calls,
39,747 bails) and the bench results of run A: rc 0, `/root/deploytmp/zerobail-out/zero-bail.log` (2 min 47 s, mostly `native_play`):
`native 96 + 288 games ZERO GAP, 24 bench files x 200 victories, census 38,179 + adapter 1,568, gap 0, unclassified 0, silent 0`.
With the current (unfinished) lent log of lane core-lent-gaps (`/root/deploytmp/duallent-out/dual-lent.log`) the step fails on exactly the
4 silent `game::close_vote` rows; it passes the moment that lane's fix lands and the lent shards are rerun.

### Overlap with lane core-lent-gaps

* `game::close_vote` (silent rows, and the `a rider or draw the core does not pay` class) and the differences of the Artuno chain are theirs.
  I did not touch `agenda_close_dual.rs`.
* Any new cause their fix makes visible in the lent set is unclassified until somebody reads it and writes a row: that is intended.

### Not done / limits

* The class `adapter` rests on reading each guard in the dual adapters and on the lockstep tests the rows cite; I did not re-prove every
  row by a separate native run of the exact position. The Ceasefire / E-Res Siphons / Minister of Peace hooks have a core test or a
  lockstep test, but no test that asserts a bail-free native game with that card in play (they are rare in the bench seeds).
* The doc comments of `turn_flow::Hook` and `EWhy::Terragenesis` were stale ("still string bodies", "not a rule of the core table"); corrected.

## 2. d4gate

`coverage.py --check` joins the inventory of `impl Window for X` with the covers file the D4 test crate writes. The covers come from
`covers!` declarations and from the stage trace of the `Rig`. The windows of the driver (`TurnFlowWin`, `VotingWin`, `CombatLive`, the status
and strategic windows, ...) are run by the live path (`driver::live::advance`, the jobs of the turn flow), not by a `Rig`, so a test that
drove them recorded no stage: `driver::turn` 0 %, `driver::agenda` 27 %, `driver::windows` 20 %. They are also the windows added after the
last baseline (lanes B..F of the native driver), so the 11 uncovered ones had tests that were never counted.

Fix:

* A STAGE PROBE in the core (`driver/probe.rs`, one thread-local `Cell<Option<fn>>`, `None` in every product thread): `Driver::once` and the end of
  `Driver::settle`, and `inline::run` (windows run inside a job) report the type name and the stage of the window that runs. The test kit
  installs it on the thread of every test that builds a position or a `Rig` (`covers::install_probe`, deduplicated per process). Hygiene:
  `lint_core.sh` 0 hits (two `lint-allow(core)` lines for the static type name), `check_layers.py core` 0 violations, a unit test in the module.
* New tests for windows that really had none: `Z'eu` (`leader_windows_agents.rs`, + snapshot `zeu.snap`), `SchemingDiscard`, `Redistribute`,
  `HookedDraw`, `OccurrenceScoring` (`driver_status_windows.rs`), and the held vote's trade goods and Predictive Intelligence stages
  (`agenda_flow.rs`).
* No exemption added, no threshold touched. `covers_exempt.txt` is unchanged (3 rows: Keleres, Firmament, diplomacy).

Result: see the end of section 3.

## 3. Reproduce

    cd <worktree>; export CARGO_TARGET_DIR=/root/deploytmp/zerobail CARGO_INCREMENTAL=0
    python3 -I tools/engine_core/test_zero_bail.py
    # zero-bail, exact step command (needs the dual-all, dual-lent and bench steps' outputs in --out):
    ZERO_BAIL=enforce tools/engine_core/end_of_d1.sh --only zero-bail --out <out>
    # d4gate:
    rm -f $CARGO_TARGET_DIR/d4-covers.txt
    cargo test --profile lockstep -p ti4-engine-core-tests --no-fail-fast <the D4FLAGS of end_of_d1.sh>
    tools/engine_core/d4_gate.sh 3 4 5
