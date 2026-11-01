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

## 4. Checks (final, tip after this commit)

* `ti4-engine --lib`: 5078 pass, 3 fail, with or without `TI4_AREAS=all=dual`: exactly the three M6d-owned tests (`combat_live::{a_space_fight_of_each_faction..., an_undefended_system_with_a_space_dock...}`, `tactical_live::a_tactical_action_of_each_faction...`). Core lib 474, factions lib 146, bridge lib 3 + kernel_env, 32 named `ti4-engine-core-tests` binaries all pass.
* **Not G5's: `turn_jobs2` (2 tests)** `a_free_leader_of_the_closing_menu_is_asked_and_its_dispatch_is_a_counted_gap` and `a_leader_printed_during_the_action_phase_is_a_free_option_of_the_closing_menu` failed with `TurnGap::MenuSources` on the G4 base too: Hacan's Guild Ships (`rules/trade.rs`, lane G `d5a6abc1a`) reach every seat, so the closing menu has an open trade partner. Fixed in the tests (the partner is marked as already transacted with); no assertion loosened.
* Fixed on the way: `has_staged_cards` read the Marks slice where the footprint does not load it (`1fd92c5ba`); clippy findings of the G5 test code (unused import, `mut`, complex type).
* `clippy_core.sh`: remaining findings are not G5's (`g2_abilities.rs:734` logic-bug error, `silent/g3/ragh.rs:26`, `native_play.rs:1084/1097`).
* `check_layers.py core`: 528 files, 0 violations; `lint_core.sh`: 530 files, 0 hits.
* Mutation (`g5_mut.sh`, `D4_COVERS_FILE=/dev/null`): fx_cards 17/17 and seen/hand 30/30 killed by the core tests (after the survivor tests); hooks/strategy.rs 1/1, emit_mirrored 2/2, renderer helpers 4/4, cabal/yin helpers killed but for `unwrap_or(0)` fallbacks of a u16 index (`// mutants:skip`, unreachable). The waiver cap `min(2)` is skipped too (`start_waiver` leaves the path above two offers). Research window (lines 694-1204, whole window): union of core and lockstep leaves 18 survivors, all in code G5 did not write (payment steps, `rider`, `research_chosen`, `Settled` plumbing); every mutation point on G5's lines (waiver questions, `start_waiver`, `settle`, `try_settle`) is killed by the union. Renderer arms: survivors only in other families' arms.
* Not run: golden / wide dual, benches, soak, ratchet.
