# Replayer: a click answers the occurrence it was made on — 2026-09-30

Closes both P1 findings in `plans/ASTRA_REVIEW_RESPONSE_2026-09-30.md` §2.

## The defect

`ChoiceFingerprint` binds actor, prompt, ordered `(id, kind)` pairs and typed context, and
deliberately excludes the frame and ask ordinal so a rebuild can match a recorded answer to the offer
it was made against. The engine legitimately asks identical-looking questions in succession — a seat
six units over capacity is asked "remove a unit: over capacity in 14" six times in one step, each with
the single option `remove|0` and no context — so the fingerprint cannot say which occurrence a click
was aimed at. Three consequences:

1. **A stale click consumed the next identical ask.** `ManualSubmission` carried only the fingerprint
   and option id. Reproduced by review: publish ask 1, accept `a`, publish an identical ask 2, resubmit
   the old click → `Accepted { option_id: "a" }` and ask 2 removed.
   (`examples/astra_stale_submission_20260930.rs`, exit 101 before this change.)
2. **The network host could leave a remote player holding a dead panel.** `net/host.rs` resent a
   client's panel only when the pending *fingerprint* changed. An offer answered and replaced by an
   identical one inside one 50 ms pump tick never passes through `None` from the pump's view, so the
   new offer was never sent.
3. **The host's `Delegate` had a check-then-act gap.** It read `gate.pending()` under one lock
   acquisition and called `gate.delegate_pending()` under another; a newer offer arriving between
   them was delegated instead.

An earlier fix (`a7aa7942`) keyed only the *app's* panel suppression on `(fingerprint, frame, ask)`.
Review found it correct but incomplete, and refuted its claim that a double click was always refused.

## The fix

- **`control::OfferId`** — a `u64` from a process-wide counter, allocated in
  `PendingManualChoice::new`, which every offer passes through. Never reused within a process, so a new
  ask, a new branch and a new table all get fresh ones; a branch id alone would not do, since a new
  table restarts branch numbering. Not part of the fingerprint; replay matching is unchanged, as review
  required.
- **Captured when the offer is drawn.** `ManualSubmission` gains `offer`; `ManualSubmission::to` builds
  one from the offer on screen. The GUI's choice panel, the remote GUI and the network client all bind
  the click to the offer they drew, never to whatever is pending when the click is handled.
- **Checked under the gate's lock.** `PendingManualChoice::validate` refuses an occurrence mismatch as
  `Stale` before looking at the fingerprint. `ManualControl::delegate_pending_once(offer)` delegates only
  the named occurrence, which also closes the host's check-then-act gap, because the check now happens
  inside the locked call.
- **Deduplicated by occurrence.** `ReplayApp`'s `answered` and the host's `pending_sent` are both
  `Option<OfferId>`. The host's decision is factored into the pure `net::host::pending_changed` so it can
  be tested without a timing-dependent network run.
- **Wire protocol bumped** to `ti4-online-v2`: `Submit` and `Delegate` now name an occurrence.
  `ChoiceFingerprint` is no longer used anywhere in the network layer.

`Stale` is preferred to `Duplicate` whenever an offer is pending, because it names the current offer
and that is what lets a panel that drew an old question redraw the new one. An intermediate version of
this change checked `answered` first and returned `Duplicate` for an old click; the engine test
`a_stale_or_invented_answer_is_refused_and_the_panel_stays_up` caught the lost redraw contract, and
the original ordering was restored, keyed on `OfferId`.

## Regressions

Review's five required cases, plus delegation:

| test | covers |
|---|---|
| `control::an_old_click_does_not_answer_the_next_identical_offer` | 1–3: second offer answerable; replaying the first click refused without consumption; a fresh click accepted |
| `control::an_older_click_is_stale_and_consumes_nothing` | a click two offers old |
| `control::a_click_from_a_replaced_table_is_refused` | 4: same branch numbering, same shape, different table |
| `net::host::a_replaced_identical_offer_is_sent_without_an_intervening_none` | 5: two offers in one pump tick |
| `control::a_delegation_for_an_old_offer_does_not_delegate_the_new_one` | delegation, including no leftover one-shot |
| `control::fingerprint_is_stable_across_rebuilds_of_the_same_offer` | extended: identical offers are distinct occurrences |
| `online.rs` (real TCP host and client) | extended: the occurrence survives the wire |

Kept unchanged, as review asked: `app.rs::an_identical_looking_ask_at_the_next_ordinal_is_a_new_question`
and `live.rs`'s ordinal-uniqueness assertion.

## Mutation check

A passing test proves nothing unless it can fail. Each path of the fix was broken deliberately and the
suite re-run; every mutant was restored afterwards.

| mutant | caught by |
|---|---|
| `validate` checks the fingerprint only | `an_old_click…`, `an_older_click…`, `a_click_from_a_replaced_table…` |
| `delegate_pending_once` ignores the offer | `a_delegation_for_an_old_offer…` |
| `pending_changed` compares presence only (how a fingerprint compare behaves on lookalikes) | `a_replaced_identical_offer_is_sent…` |

In the intermediate version `an_old_click…` did **not** catch the `validate` mutant — the `Duplicate`
short-circuit answered before `validate` ran — so it would have shown green over a broken fix. Restoring
the pending-first ordering made it bite.

## Checks

```text
cargo run -p ti4-replayer --example astra_stale_submission_20260930
    -> OLD_CLICK_AT_NEXT_ASK Stale { current: ... }; pending_remaining=true   (exit 0; was 101)
cargo test -p ti4-replayer -j 4 --no-fail-fast
    -> lib 48, app 21, feed 6, live 9, online 1, project 23, rebuild 12, shell 4, table 6: all pass
cargo clippy -p ti4-replayer --all-targets -> no findings in ti4-replayer
```

## Not done

- **Build identity for a released client** (review §4) is untouched: `ti4-review/build.rs` stamps the
  real `git rev-parse HEAD` and dirty state, and `net/host.rs` refuses a mismatched commit. A client
  package still has to be built from a clean tree at a published commit.
- `park` still stamps every offer `BranchId::SOURCE`. With occurrence identity that no longer affects
  correctness, only what the panel displays.
