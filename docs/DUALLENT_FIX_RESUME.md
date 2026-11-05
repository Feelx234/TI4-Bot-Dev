# dual-lent: the strict panic of heavy run A, and what the lent set shows behind it (lane core-duallent-fix, 2026-11-05)

Branch `core-duallent-fix` from `core-integration-25` (3e7a099ab). Nothing pushed or deployed.

## 1. The failure and its root cause

Heavy run A (`end_of_d1.sh`, out `/root/end-of-d1-20261105`): the step `dual-lent` failed, all 3 shards panicked at `core_sync/unmapped.rs:119`:
`unmapped (strict mode): Decks: exploration trait "MR"` (shard 0) and `"FACTION"` (shards 1, 2); the merge refused (no shard file).

* `MR` and `FACTION` are PLANET TYPES (`planets.json` `planetType`: `FACTION` on every home planet, `MR` on Mecatol Rex `mr` and `ordinianc4`; Thunder's Edge adds `FAKE`,
  `SPACESTATION`, `LIGHTNING`). They are not exploration traits and no deck has such a key (`exploration_decks` has exactly CULTURAL, FRONTIER, HAZARDOUS, INDUSTRIAL).
* Who sends them: the Crown of Emphidia. `relics::crown_of_emphidia_choose_text` offers every controlled planet with `planet_type().is_some()` (a home planet, Mecatol Rex), and
  `crown_of_emphidia_explore_with` explores into `record.planet_type()` (the FIRST planet type). The core offers the same (`CrownExplore::options`: `!traits.is_empty()`, and `traits` includes
  `Faction`/`Mr`/..: `docs/ENGINE_KNOWN_ISSUES.md` B5-copy-04, "an equivalent mutant of the string filter"). Picking such a planet exhausts the Crown and explores nothing: the string's
  `draw` finds no deck; the core's tail (`driver/windows/tactical/tail.rs`: `traits_of(..).first()` is `None` -> `Flow::Done`) opens no window.
  So this is NOT a string bug, NOT a core bug and NOT a rules-version-36 regression: both engines agree, on purpose (documented before). Nothing goes into `STRING_ENGINE_BUGS.md`.
* Why it only shows now: the dual adapters `explore_write::draw`, `exploring_dual::explore_planet` (and `open_record`, `choose_deck`, `distant_suns`) turn the deck TEXT into an `ExploreType` with
  `conv::explore_type(..).ok()?`, which COUNTS a refusal of the codecs (`unmapped::note`) and PANICS in strict mode (the gate binaries). The probe was meant as "the text is no deck: bail", but it
  is counted as an "unmapped value of the state". It needs a Crown in play and a seat that picks a planet without a trait, which the 20 golden games never reach (all passed) and the lent set
  does (3 of 3 shards; the soak's `old_100000_6_wide` slice too).
* A long-standing gap that strict mode exposed (the strict mode of the gate binaries is older than the Crown window: B5), not a v36 regression.

## 2. The fix

* `core_sync/conv.rs`: `try_explore_type(text) -> Option<ExploreType>` (does not count, does not panic); `explore_type` (the codecs' reading of a state) keeps counting and is built on it.
* `explore_write::draw`: a text that is no deck, while the string's state holds no deck under that key, is the answer `Some(None)` ("nothing to draw", compared; the string's `draw_text` finds no deck either).
  A string state that DOES hold a deck under a key the tables do not know still bails (`UNKNOWN_ID`).
* `exploring_dual::explore_planet`: the same text gives a window with no question and the value `None` (the string explores nothing; the product's tail opens no window).
* The documented bail test `explore_write::tests::the_writes_bail_where_the_core_holds_less` was updated: "WEIRD"/"FACTION" without a deck in the state is `Some(None)`, with a deck of that key it is the bail.
* Tests, `relics::relic_window_tests`: `the_crown_of_emphidia_on_a_planet_without_a_trait_runs_in_lockstep_and_strict` (seven positions, Dual, strict on the thread: picks of the home planet, Mecatol Rex, a traited
  planet, the decline; no difference, no bail, no unmapped value, the Crown spent or not as the string does) and `a_deck_text_that_is_no_deck_is_a_probe_and_draws_nothing`.
  RED without the fix: both fail with `unmapped (strict mode): Decks: exploration trait "FACTION"` (the exact message of the run); green with it (197 passed in `crown emphidia explor relics:: dual_window deck_text`).
* Decision streams: only Dual-run adapters changed (plus `explore_type` re-expressed through the probe, same result and same counting); `dual_run` prints `hashes equal: true` for all 19 lent games.
  Quick fingerprint `538dc68f23dddcc9`: `tests/wrapper_native` green (section 5).

## 3. dual-lent standalone, after the fix

Command (as `end_of_d1.sh`, slot 2, own target dir `CARGO_TARGET_DIR=/root/deploytmp/duallent`, `nice -n 15`, `CARGO_INCREMENTAL=0`):

    RUST_BACKTRACE=1 TI4_BAIL_PROBE=$O/bail-probe-lent.tsv TI4_DUAL_FAIL_FAST=1 SHARDS=3 tools/engine_core/dual_sharded.sh $T/gate/dual_run tools/rulebot/golden-lent.json 20 all=dual $O/dual-lent-shards

Result: no panic any more; 19 games played, `hashes equal: true`, but **195 differences in 13 of the 19 games (rc 1)**. 6 games are clean (3, 5, 8, 10, 11, 13). None involves the Crown. The step does NOT pass yet; the
differences are all behind the strict panic (the lent set was never clean: `docs/LENT_CENSUS.md` "Differences the lent set found" lists OPEN rows, and its W0 table fixed the others in 2026; these are new classes or regressions).
Logs: `/root/deploytmp/duallent-out/dual-lent.log`, `dual-lent-shards/`, one-game repros with `TI4_DUAL_TRACE=1 TI4_NESTED_LOG=1` in `/root/deploytmp/duallent-out/repro/game-N.log`
(`dual_run tools/rulebot/golden-lent.json 20 all=dual --game N`).

| family | games | what the trace shows | verdict |
|---|---|---|---|
| `game::close_vote` (agenda; 40 `action_card_deck`, 26 `trade_goods`, 18 `.questions`, 13 staged `TRADE_GOODS_GAINED pillage`, 20 `resolver.state`, `nested.events` `[PILLAGE_USED, TRADE_GOODS_GAINED]` vs `[TRADE_GOODS_GAINED]`) | 0, 2, 4, 6, 14, 15, 17, 18 | After the vote's trade goods the string asks, in a chain, the Nomad agent Artuno (`leader:nomad:nomadagentartuno:TRADE_GOODS_GAINED:after`, seat a), Mentak's Pillage (`ability:mentak:pillage:TRADE_GOODS_GAINED:after`, e/d), Suffi An (`suffi_an`), Ssruu's copy of the Mentak agent (f), Scheming's discard and the Nomad reaction to `ACTION_CARD_DISCARDED`: 6 questions. The core window asks the first (Artuno) with the same options and then ends (1 question): the Pillage reaction after Artuno's answer is never asked; the string state then differs by the pillaged trade good, the staged event and the action cards Suffi An draws (the deck shifts by 5). | OPEN, a core gap in the nested reaction chain started by an Artuno use inside `close_vote` (D2 agenda/reaction lane). Not a string bug. |
| `exploration::explore` (2 `questions`, `exploration_log[].result` "gained 1 trade good", staged pillage) | 14, 16 | same chain after a trade good gained by an exploration card (`cm3`: remove 1 infantry); the core asks the card question and stops | OPEN, same cause |
| `invasion::finish_control_gain` (`.questions` 2 vs 1, `exploration_decks` shifted, `exhausted_planets`, a Naaz mech) | 1, 2 | string: Naaz commander reaction (`leader:naaz:naazcommander:PLANET_CONTROL_GAINED:after`, answered) then "Distant Suns: which exploration card to resolve" (`dmz`/`hrf6`); core asks the reaction and stops | OPEN, same shape: the core does not follow the effect of the answered Naaz commander reaction |
| `technology::end_turn` `details.reinforcements` string n, core n+1 (22 questions "Predictive Intelligence: redistribute command tokens") | 7, 9 | the core counts one more reinforcement than the string at the seat's Predictive Intelligence question; same options | OPEN, not analysed (compare `strategy_cards::te6_warfare`, FIXED by a counted bail for "a command token in a Mahact fleet pool": a Mahact seat is at those tables) |
| `combat::settle` Skilled Retreat "withdraw to which system": string offers `42`, core does not | 17 | the row "Skilled Retreat destination 42 missing" of `docs/LENT_CENSUS.md` (FIXED with notes keyed by combat round) shows again in `combat::settle` | OPEN, regression or second path |
| `leaders::jolnarhero` `type_id` `jolnar_mech` vs `mech` | 12 | the known OPEN row W0-open-01 | OPEN (B7) |

The soak of run A (`/root/end-of-d1-20261105/soak`) shows the same families in ordinary games (std slice 85 differences, `invasion::ground_round_hits` with the staged pillage, `game::close_vote` with `trade_rider`
discards, `resolver.state`), so they are not specific to the lent set.

## 4. Do the later steps of run A depend on this?

Run A finished 08:48 (END OF D1 RUN FAILED: dual-lent zero-bail soak d4gate). Checked in its logs:

* `board-t`, `board-g`, `sync`, `bench-*`, `strict` (the golden games and the driver dual, no Crown): independent, passed.
* `census`: passed, but its lent table is empty (`bail-probe-lent.tsv` was never written because the shards died), so the census has the golden corpus only.
* `zero-bail`: failed (enforce) on the bails of `dual-all` (39,747 bails/gates in 44 causes, e.g. `d1::step tactical` 7,815); it read `dual-lent.log` as a report with no lent data, so it did not count the lent corpus. It is independent of this fix.
* `soak`: affected. Slice `old_100000_6_wide`: shards 1 and 2 died with the same `Decks: exploration trait "FACTION"` / `"MR"`, shard 0 with `Transient: owner not in the table "neutral"` (separate, not analysed; a lane of its own); `te_100000_6_driver` died on the same `"neutral"` panic and showed `turn::menu` differences; the five ordinary slices ran with 85 / 11 / 4 / 3 differences (the families above). So a soak rerun needs the fixed binary, and it will
  still fail on the open families and on `"neutral"`.
* `d4gate`: failed for its own reasons (11 windows neither covered nor exempt, stage-variant coverage below 80% in `driver::agenda`, `driver::turn`, `driver::windows`, `rules::token_gain`); unrelated.
* `lint`/`prod-graph`/`clippy`/`tests`/`dual-all`/`dual-drv`/`dual-wide`: passed.

## 5. Checks and cleanup

* Unit tests green (197). The quick fingerprint pin `tests/wrapper_native.rs` `538dc68f23dddcc9`: `cargo test --profile lockstep -p ti4-engine-core-bridge --test wrapper_native` green (rc 0, 10:13).
* Not run: the full engine lib suite on Dual (48 min), clippy, the other heavy steps (this lane's step was `dual-lent`).
* The target dir `/root/deploytmp/duallent` is deleted at the end; the logs stay in `/root/deploytmp/duallent-out`.

## 6. To resume

1. Pick a family of section 3 (the `close_vote` chain after Artuno is 8 of 13 games): `dual_run tools/rulebot/golden-lent.json 20 all=dual --game 0` with `TI4_DUAL_TRACE=1 TI4_NESTED_LOG=1`; find why the core window stops after the first answered reaction.
2. `tools/engine_core/dual_sharded.sh ... golden-lent.json 20 all=dual` again until 0 differences; then rerun run A's `census`, `zero-bail`, `soak`.
