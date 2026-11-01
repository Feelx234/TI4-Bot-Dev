# D2 lane G5 (`core-g5-abilities`): the research waiver questions, the card-move consumer, the revealed-card view -- resume note (2026-11-01)

Branch `core-g5-abilities`, cut from `core-g4-abilities` (`d0c218b22`). Never pushed; the live branch is untouched. Read `docs/G4_RESUME.md` first (its "Not done" section is task 1 here).
Rules kept: pure refactor (RULES_VERSION, serde, fixtures, fingerprints unchanged); every new behaviour has a test that fails on the pre-G5 code.

## 1. The research window asks the waiver and its payment (Cabal Riftmeld, Yin Brother Omar)

A pick of a technology only a faction waiver opens no longer ends the window `failed`. `rules::production::strategy_cards::Research` now plays `strategy_cards::resolve_research`:

| piece | where | string |
|---|---|---|
| `Ask::ResearchWaiver { tech, offered: [Option<Waiver>; 2] }`, `Ask::ResearchWaiverPayment { tech, waiver }`, `Opt::ResearchWaiver(u8)`, `Opt::ResearchPayment(Waiver)`, `Subtype::{ResearchWaiver, ResearchWaiverPayment}`, `waiver_ctx` | `choice/strategy.rs` (appended at the END of each enum, away from H4's `Sucaban*` variants) | `resolve_research` |
| `Content::ResearchWaiver` (the source text `research_waiver`) | `choice/generic.rs`, `render/generic.rs` | `DecisionSource::FactionAbility("research_waiver")` |
| `Waiver` moved to `choice::strategy` (re-exported from `kernel::hooks`: layer rule, the question names it) | `choice/strategy.rs`, `kernel/hooks/sites.rs` | |
| `StrategyHooks::research_waiver_offers` (EVERY module's offer, in module order; the `first` slot stays for `waiver_offer`) | `kernel/hooks/strategy.rs` | `hooks_strategy::research_waiver_offers` |
| stages `Waive`, `WaivePay`; fields `declined`, `waiving`, `before`; `try_settle -> Settled::{Landed, Failed, Waiver}` | the window | `offer_research_inner`, `paid_research` |
| renderer: prompt `research {tech}: choose a prerequisite waiver` / `{waiver label}: choose the payment`, options `waiver|{i}` (kind `research_waiver`), the payment ids `captured|{owner}|{unit}` (Cabal) and `ground|{system}|{planet}` / `space|{system}` (Yin), both offer cards, the decline captions | `render/strategy.rs` (helpers at the end of the file, one arm each in `prompt`/`subtype`/`option`/`parse`/`details`) | |
| decode helpers `payment_model` (Cabal), `payment_place` / `place_code` / `PaymentPlace` (Yin) | `cb_economy/cabal/offer.rs`, `cd_registry/yin/query.rs` | |

Behaviour (each fixed by a lockstep and a core test):

* Free research (`Free`): no bill; a declined waiver or payment puts the technology on `declined` (skipped by `open`), the technology question is asked anew, an empty offer ends the free research and the paid one begins (`declined` is cleared at that point: each of the string's two loops has its own list).
* Paid research (`Choose`): the position is saved when the technology is picked (`before`), the bill is paid, THEN the waiver is asked; a decline of either question (or a payment the module refuses) restores the saved position, as the string's `*state = before`, and the technology question comes back without it.
* One question per module: `waiver|{i}` is the place in `research_waiver_offers` (a Cabal seat holding Yin's commander through an Alliance sees two). More than two offers leave the path (`failed`: a counted bail, not silent).
* The payment list is not capped: `SmallList` spills to the heap (the G4 note's point 4 is a non-issue). The Cabal provider lists a captured (owner, form) held twice once; the string lists it twice with the same id (the lockstep avoids that corner: the answer is the same either way).
* Unit ids: every unit of the content catalogue parses with `Unit::from_id` and prints back the same (`render::strategy::tests`), so `captured|{owner}|{unit}` is the string's for faction units too.

Tests: `ti4-engine-core-tests/tests/research_waiver.rs` (6), the two rewritten bail tests in `strategy_cards.rs` / `action_cards/effects.rs` (now "asks the waiver"), engine `strategy_research_waiver_tests.rs` (6 lockstep tests under the Dual backend: Riftmeld, declines, the paid research and its roll-back, Yin's space and ground payments, two modules), `render::strategy::tests` (3).
On the pre-G5 code the lockstep tests count `(0, 1)` (a bail) and the core tests find the window `failed`.

### Join points with lane H4 (`core-h4-leftovers`, Doctor Sucaban)

H4 also edits the Research window (uncommitted when this was written: staged files `strategy_cards.rs`, `choice/strategy.rs`, `render/strategy.rs`, `strategy_cards_core.rs`, `strategy_research_tests.rs`). The edits were kept additive; expect these textual joins:

* `RStage`: H4 adds `Offer`, `Trade` between `Free` and `Choose`; G5 adds `Waive`, `WaivePay` after `Pay`.
* `Research` struct and `new`: both append fields (`sucaban`, `offer_at`, `checkpoint` / `declined`, `waiving`, `before`): keep all, one line each in the constructor.
* `begin_paid`: H4 rewrites it (Sucaban's offers first) and its signature takes `&mut CoreState`; G5 only CALLS it (`self.begin_paid(sx.state, sx.kernel)` compiles with either). G5 does not touch its body.
* `land`: G5 matches `try_settle` (Landed / Failed / Waiver); H4 ends it with `end_paid(s, landed)`. Merge: keep G5's match, and call H4's `end_paid(s, true)` where G5 sets `RStage::Done` after a landing, `end_paid(s, false)` is not needed on the waiver paths (a decline returns to `Choose`, where the empty-offer branch already calls `end_paid`). `waive_and_pay` sets `Done` directly after a landing: replace by `end_paid(sx.state, true)` (the Ssruu checkpoint is then dropped without a restore, as in H4's landed case).
* `answer`: G5 adds three arms before `(Choose, Decline)` and snapshots `before` at the top of the `(Choose, Research(t))` arm; H4's arms are `Offer`/`Trade`. No overlap of patterns.
* The two snapshots are independent: H4's `checkpoint` is taken before a Ssruu offer (restored when no technology lands), G5's `before` after the Sucaban trade, before the bill.
* `choice/strategy.rs`, `render/strategy.rs`: H4 inserts its variants after `ResearchTechnology`; G5 appends at the end of each enum and adds its helpers at the end of the renderer file.
* `strategy_cards_core.rs`: G5 changed two comment/bail texts (`research`'s doc and the `note_bail_here` text); H4's change is in `research_unported`'s doc.
* Tests: G5's lockstep tests are in a NEW file (`strategy_research_waiver_tests.rs`, one line in `lib.rs`) to avoid `strategy_research_tests.rs`, which H4 edits. H4's Sucaban lockstep (`Jol-Nar` agent) and G5's waiver lockstep never share a table.

## 2. Native consumer of the `CardsStaged` rows

`kernel/fx_cards.rs` (new): `announce_staged_cards(sx)` drains the rows oldest first (a malformed row is dropped), a staged discard goes on the pile (`cards().discarded_action`), names `last_action_discarded` and emits `ACTION_CARD_DISCARDED {player, card}`; a take emits `ACTION_CARD_TAKEN {player, from}` with no card; the events are mirrored once as a batch (`Live::announce_staged_cards`). The string's `announce_discard` Data Skimmer branch is a counted bail (`Unknown::DataSkimmer`) when another seat holds the breakthrough, has not passed, in the action phase (Thunder's Edge content: no core rule).
Wiring: `Sx::emit_mirrored` calls it after a TOP-LEVEL typed event (`Live::emit_typed` announces "now rather than at the next component action"); rows left by a window that emits plain `sx.emit` wait for the next typed event of the game, as in the string. NOT wired (no native step exists yet): the string's calls after a faction / leader component action (`play_faction_action`, `perform_leader_action`) and `agenda_reveal`: the component-action step of the future native turn calls `fx_cards::announce_staged_cards` after the action, before `COMPONENT_ACTION_RESOLVED`.
Evidence: `ti4-engine-core-tests/tests/staged_cards.rs` (4), lockstep `silent4_live_tests::{the_holder_of_spy_net_takes_..., spy_net_over_the_hand_limit_...}`: the harness no longer strips the native row and counts it; native and string now carry the same `ACTION_CARD_TAKEN` events and no `cards:staged:*` row. The discard half has core tests only (no native prelude path stages a discard: Scheming / So Ata / Kyver / Mageon are windows of other flows; their string tests run under the Dual backend).

## 3. Naalu `may_view_hand`: the consumer

`Seen::revealed_cards` (`seen/hand.rs`, with `Revealed`, `Shown`): the typed twin of `SeatObservation::revealed_cards`: the `CardsReveal` rows of the bound seat as viewer (`from` is the viewer, `to` the owner), merged per (owner, kind), a card the owner no longer holds dropped, a note turned faceup dropped; then the standing `may_view_hand` permission of the hook table for the promissory notes of each other seat (merged into the owner's row or appended in seating order). Order caveats (documented on the accessor): rows are ordered by owner SEAT (the string orders by the owner's id text), the notes of a standing permission by the note table (the string by id text). `Slice`s read: all in `Seen::READS`.
Evidence: `ti4-engine-core-tests/tests/seen_revealed.rs` (3), engine `silent4_live_tests::the_typed_seen_reports_the_revealed_cards_like_the_seat_observation` (6 positions against the string observation, M'aban included, order-insensitive within a row).
Nobody calls it yet: the string's only callers are the observation tests and a harness bot (`action_cards.rs`); the rule bot's `Seen` users may take it.

## 4. Checks

STATE AT HAND-OVER (checks NOT finished; the session was told to hand back):

* Private `CARGO_TARGET_DIR=/root/deploytmp/core-g5` (~0.7 GB, delete at the end), scratch worktree `.claude/worktrees/core-g5-run` (detached at the branch tip; remove with `git worktree remove`).
* First engine lib run (tip `8a02ebbd3`): 5060 passed, 21 failed. 17 of the 21 (agenda_close_tests 2, agenda_reveal_tests 15) plus `game::tests::deadly_plot_effect_...` were MY regression: `fx_cards::has_staged_cards` read `marks()` in `Sx::emit_mirrored` where the footprint does not load the Marks slice (debug assert). Fixed in `1fd92c5ba` (guard on `loaded`, test `staged_cards::a_position_without_the_marks_slice_has_nothing_staged`). The remaining 3 are the expected M6d-owned ones (two `combat_live`, one `tactical_live`); NOT yet re-run on the fixed tip.
* `ti4-engine-core --lib` 474 ok, `ti4-engine-core-factions --lib` 146 ok (tip `8a02ebbd3`).
* Running when handed back: `/tmp/.../scratchpad/g5_run1.sh` (bridge lib, the named core-tests binaries, engine lib under `TI4_AREAS=all=dual`) on the fixed tip; logs in the scratchpad `g5_run1/`. The normal engine lib run on the fixed tip is still to do.
* Mutation: NOT run. Script ready: `scratchpad/g5_mut.sh` (fx_cards, seen/hand, the Research window lines 694-1204, hooks/strategy.rs, emit_mirrored lines 155-175, cabal/yin helpers, renderer helpers and arms), each with core tests and engine lockstep, `D4_COVERS_FILE=/dev/null`.
* Not run: clippy_core.sh, check_layers (0 violations, 528 files) and lint_core (0 hits) were run at `8a02ebbd3`.

