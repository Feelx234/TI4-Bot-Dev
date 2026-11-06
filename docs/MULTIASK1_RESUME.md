# multiask-1: sequences of order-independent questions as ONE question (core-multiask-1, 2026-11-06)

Branch `core-multiask-1` from `core-integration-28` (`f22ece5e8`). Not pushed, not deployed, live branch untouched. Theory:
`git show core-theory-multiask:docs/MULTI_ASK_THEORY.md`. User decisions (final): the conversion order below, the status-phase gain
deleted, Politics with all six orders, payments and removals/discards on a validated multi-pick type on the wire, the bot may change its
set picks, `BOT_VERSION` and `RULES_VERSION` (36) unchanged, one re-baseline at the end, the production build/pay/place reorder out of
scope.

## 1. What changed, per conversion

| # | Conversion | Core | Answer shape (wire id) | Proof against the old loop |
|---|---|---|---|---|
| 1 | Gain pools (A/B/E): Leadership's 3, Warfare's 1, Summit's 2, explore, Lead from the Front, Starlancer, Propagation, Empyrean | `SOpt::Gain{tactic,fleet,strategic}` summing to `left` (`gain_pool_choice`, `gain_answer`, `gain_pools`); the tokens are placed tactic, fleet, strategic | `{t}\|{f}\|{s}`, kind `gain` | (pure split of `left`) |
| 1' | Status-phase gain | `TokenGain` asks NOTHING: every owed token goes to `STATUS_GAIN_POOL` (Tactic), plain `COMMAND_TOKEN_GAINED:Tactic` per token, then only the redistribution | none | the redistribution's options depend on the sheet total and the floor only |
| 2 | Predictive Intelligence (F) | `TOpt::Split` over every final split, floor 0, no `Done`; the current split keeps the card ready | `{t}\|{f}\|{s}`, kind `redistribute`, details mode `redistribute` | `technology_free_windows::the_split_question_offers_exactly_what_the_move_loop_could_reach` |
| 3 | Politics | `SAsk::PoliticsAgendas{first,second}`, `SOpt::Stack` (six orders; one agenda left keeps the old top/bottom) | `{top ids}/{bottom ids}`, kind `agenda_order`, details.agendas | `strategy_primaries::politics_reaches_every_order_of_the_two_agendas_the_rules_allow`: written first, FAILED on the old code (4 of 6 orders), passes now. String limit recorded as S10 |
| 4 | Payments (D) | `PAsk::Pay` options are the atoms; the answer `POpt::Set(PaySet{faces bitmask, goods, commodities, buy})`, checked by `choice::production::set_worth` (`sum >= owed && sum - max < owed`); lone LEADING atoms are still taken unasked (so the string asks wherever the core asks) | atom ids joined by `+` (`exhaust\|jol+trade_good+trade_good`); a single atom id = that one first, then the canonical order (`Choice::pick`); raw `set:{faces}:{g}:{c}:{buy}` | `pay.rs set_tests::the_one_payment_question_reaches_exactly_what_the_loop_reached` (brute force, >500 positions, both currencies, `paid` 0..2) |
| 4' | Leadership's buy loop (C) | ONE `PAsk::Purchase(PurchaseAsk{pay, each 3, min, max})` (secondary: min 1), answer `PaySet{buy: n, ..}` checked by `purchase_worth` (`n == 0` spends nothing, else `sum >= 3n && sum - max < 3n`), then ONE gain question for the n tokens | `buy:N+atoms` (`buy:0`) | `strategy_cards.rs purchase_tests::the_one_purchase_question_reaches_exactly_what_the_buy_loop_reached` (credit carry, primary and secondary) |
| 5a | Hand-limit discard (G) | `CAsk::HandLimit{held, discard}`, `COpt::Discard{index, card, copies}`, answer `COpt::DiscardSet{counts}` (four bits per option, `cards::Counts`), `discard_set_ok`: exactly `discard`, a name at most its copies; the first copies go | option ids joined by `+`, once per card; raw `counts:{n}` | `draw.rs the_one_discard_answer_reaches_exactly_what_the_card_by_card_loop_reached` |
| 5b | Fleet supply / capacity removal (H) | one question per stage; `TOpt::RemoveSet{counts}` checked by simulation (`Enforce::removal_ok`: the excess is gone and some unit was still needed); an option alone = that unit first, then the option the question would offer first, until the fleet fits (`lead_set`) | option ids joined by `+`, once per unit; raw `removals:{n}` | `fleet_enforce::the_one_removal_answer_reaches_exactly_what_the_unit_by_unit_loop_reached` |

Shared machinery: `Choice::is_multi_pick`, `accepts` (the driver, nestq's `live::is_option` and the step machine use it), `pick(i)` (an atom
leads a set), `default_pick`, `answers(limit)`, `answer_with(opt)`; wire `Choice.multi_pick: Option<MultiPick{kind, owed, repeat, buy}>`
and `MULTI_PICK_SEPARATOR` `+`; `answer_for` accepts '+'-joined listed ids; thin `live::pick_of` parses sets
(`render::production::{set_of, set_text, multi_pick}`).

Adapters: rule bot `multi_answer` (payments: the old least-waste atom rule repeated on what is left; purchases: each token decided as the old
yes/no was, on the pools and spare influence the tokens before leave; `answer_text`), Politics with the old verdict per agenda, the gain
split token by token, Predictive Intelligence keeps the split. Host batch: `gain_split_answer` (pool steps), `multi_pick_answer` (payment
steps up to the bill; a purchase's yes/no and payment steps, leaving the pools for the gain question). Legacy renderer copies updated.

## 2. Payment sites (audit asked by the user)

Every site that ASKS a payment goes through the one-set question; the guard in `tools/engine_core/lint_core.sh` fails on a payment
question built, or a single payment atom matched, outside the shared machinery (`choice/production.rs`, `choice/mod.rs`,
`rules/production/{mod,pay,window,one,triggered,strategy_cards}.rs`; probed: a planted file is reported).

| Site | File | Converted | How / reason |
|---|---|---|---|
| `PayStep` (the shared step) | rules/production/pay.rs | yes | one set for the rest of the bill; lone leading atoms unasked |
| `PayBill` (a whole bill) | rules/production/pay.rs | yes | over `PayStep` |
| `pay_in` (inside an ability's effect) | rules/production/pay.rs | yes | `eff.ask` of the set question |
| Production window, Paying stage | rules/production/window.rs | yes | `apply_set` over the same atoms; credit carried as before |
| Leadership purchase (primary, secondary) | rules/production/strategy_cards.rs | yes | `PAsk::Purchase` (C) |
| Technology research (primary/secondary, Research window) | rules/production/strategy_cards.rs (`pay_step`) | yes | `PayStep` |
| Thunder's Edge Construction's paid structure | rules/production/strategy_cards.rs (`PayBill`, COST) | yes | `PayBill` |
| Produce one unit (Chaos Mapping, Freelancers, Minister of Industry singles) | rules/production/one.rs | yes | `PayStep::answer_set` |
| Sling Relay, Integrated Economy | rules/production/triggered.rs | yes | `PayStep::answer_set` |
| `Spawn::Bill` children (orbital mech, action card costs, Reaper) | driver/children.rs; rules/faction_component.rs; rules/action_cards/effects.rs; rules/invasion/begin.rs | yes | `PayBill` |
| Custodians, Reaper resumes | driver/windows/invasion/ops.rs, fight.rs | yes | `PayBill::new/resume` |
| Relics (6 resources) | driver/turn/relic.rs | yes | `PayBill` |
| Yin, Mentak, Empyrean timing costs | factions cd_registry/yin, cc_ground/mentak, ce_movement/empyrean | yes | `pay_in` |
| Objective costs, Inheritance Systems, research waivers, trade-good costs, secret payments | kernel/fx_pay.rs (`pay_cost`, `pay_first_plan`, `pay_trade_goods`, `pay_secret`) | n/a | no question: the string pays the first plan / a fixed amount unasked; nothing to convert |
| Production build loop (choose, pay, place interleaved) | rules/production/window.rs | partly | its PAY step is converted; the build/pay/place reorder is out of scope (user) |
| Agenda votes (exhaust planets for votes) | rules/agenda | no | not a payment (a vote loop with its own events) |

Count: 14 of 14 question-asking payment sites converted; 4 automatic payment helpers ask nothing; the build loop's reorder out of scope.

## 3. Shadow (native-led dual) and the other oracles

`native_play --shadow`: each native multi-ask answer carries its string expansion (`multiask::legacy_answers`): `Scripted::Seq` (gains,
restack moves + `done`, Politics' two answers), `Set` (payment atoms, smallest worth first), `Leadership` (yes/no + atoms; the pools pass
to the gain entry behind it), `Labels` (discard by card name: the string's ids are hand places), `Removal` (the lead unit, then the first
option). The follower answers per string question with `Verdict` Answer / Pass / Drop; entries are bound to their seat and an entry the
string's step ended inside is carried into the next step (`CARRIED`). The string's questions of an expansion are compared loosely (same
seat, offered, from the expansion). The status phase's string gain questions are phantom native questions (not in the chain). Politics'
two orders the string cannot reach are S10 (`docs/STRING_ENGINE_BUGS.md`); the harness seat picks the reachable order of the same piles.
`NativeSubject` (string-led) lends the token arm and steps that pay / discard / remove (counted, with the reason).

## 4. Results (merged tip, gate profile)

| Check | Result |
|---|---|
| core lib + core-tests + factions (debug) | 2,729 passed, 0 failed |
| web-set Rust tests (thin, host, rule bot, server, wasm; helper branch `core-multiask-1-tests`) | 0 failures (slow splice tests not rerun after the last merge) |
| web `tsc` / `vitest` | clean / 2,757-2,758 passed (only `launcher.test.ts`: `fake-indexeddb` missing from the copied node_modules, environment) |
| `check_layers` core / thin / setup | 0 / 0 / 0 violations |
| `lint_core.sh` | 0 hits, 8 allowed, 0 multi-ask guard hits |
| `clippy_core.sh` | ok, engine warnings 41 = baseline |
| `check_production_graph.sh` | 0 violations |
| shadow sweep `native_play --factions all --players 3,4,5,6 --shadow` | 96 games, 0 gaps, 0 first differences, 22 known string bugs (16 S1, 1 S2, 5 S5); base `f22ece5e8`: 0 differences, 18 known |
| `native_play --quick --shadow --twice` | 1,302 steps, whole game shadowed (933 compared), zero differences, deterministic |
| bridge lib | 10 pinned-game tests and `golden::the_smoke_subset_reproduces` fail: they pin games/steps the conversion changed on purpose (list in the helpers' reports); re-pin with the re-baseline |
| **wasm (real build, wasm-opt)** | **6,653,942 raw / 2,120,394 gzip / 1,526,740 brotli: OVER the budget 6,630,000 / 2,110,000 / 1,520,000** (by 23,942 / 10,394 / 6,740 bytes; int-28 was 6,624,713 / 2,106,859 / 1,516,834) |

Per the brief the wasm overrun stops the lane here: no trim was attempted, and the one re-baseline (goldens, bench, pins) was NOT done,
because a trim changes the code again and the re-baseline must happen once, on the final tree.

## 5. Ask counts and fingerprints

14-game suite (`PERF_KERNEL_KINDS=1 PERF_KERNEL_NOSTATE=1 perf_kernel suite 1`; games diverge after the first changed question, so the
per-kind numbers are approximate):

| Kind | base `f22ece5e8` | multiask-1 |
|---|---|---|
| gain_command_token | 1,495 | 263 |
| status redistribute | 496 | 496 |
| Predictive Intelligence | 208 | 39 |
| Politics agenda placement | 194 | 103 |
| payments (pay_resources / pay_influence) | 607 | 241 |
| buy_token_with_influence (follow + purchase) | 297 | 323 |
| hand-limit discard | 386 | 288 |
| fleet / capacity removal | 237 | 168 |
| **all decisions** | **14,485** (14,738 steps) | **12,388** (13,630 steps): -14.5 % |

New fingerprints (deliberate: the decision stream changed): quick game `538dc68f23dddcc9` -> **`8b758f435a636511`**; `perf_kernel suite 1`
`suite_hash` `775f4136e60a99d5` -> **`a45e96e8a2d96295`**. Not yet re-pinned in `tests/wrapper_native.rs` / `native_play.rs`.

## Remaining

1. Trim the wasm under budget (about 24 KB raw, 10.4 KB gzip, 6.7 KB brotli). Candidates: the rule bot's `multi_answer`, the host batch
   multi-pick mapping, the four `Choice` helpers, render `set_text`/`set_of`; measure with the i28 `wasm_syms.py`.
2. Then the one re-baseline: re-pin quick fingerprint and suite hash, re-pin the 10 bridge game tests, `golden bless` (v36-std, ladder
   g036; `--expect` gain_command_token, status_redistribute_tokens, predictive_intelligence_redistribute, politics_place_agenda,
   pay_resources, pay_influence, buy_token_with_influence, discard_over_hand_limit, remove unit), re-record the 77 bench entries.
3. Open from the helpers: (a) `Choice::pick(0)` on the PRIMARY purchase buys one token (old first option was "no"): random/first-option
   bots now buy; `default_pick` buys the minimum. (b) Live state vs replay of the export disagrees on some answer counts of the lobby
   table (61-63, 65, 68, 69 answers; 65/69 even decision counts): looks related to the documented staged-event issue
   (`docs/D28E_RESUME.md`) but worse; not investigated. (c) The rule bot's removal now takes the first options after its lead (its
   cheapest-unit rule applies to the lead only). (d) `cargo wire-web` alias fails here ("Unrecognized option: 'config'"); the helper ran
   the same tests with `TI4_WRITE_WIRE=1`.
4. UI: screenshot folder `web/e2e/screenshots/HP-multi-pick-questions` (24 captures), private Artifact
   https://claude.ai/artifact/6XytDany5oALwGRGo5gVD4.

## 6. Join notes

* Merge after `core-integration-28`; touches core choice/rules/driver, factions (Propagation, Empyrean), thin render/live/wire, legacy
  render copies, host batch/server, rule bot, bridge (multiask, oracle, native_play, native_subject, perf_kernel), web, docs.
* Every fingerprint over the decision stream changes on purpose (see section 5); goldens and bench re-recorded once.
* Wire: `Choice.multi_pick` is new (skipped when `None`, so old JSON is unchanged); answers may be '+'-joined ids.
