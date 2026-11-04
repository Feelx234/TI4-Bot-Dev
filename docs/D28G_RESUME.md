# D2-8 lane G: the `replay_export` hang (acknowledgement of a declared card)

Branch `core-d28g-ackhang`, cut from `core-d28d-wrapper` (`3904d80cb`). Not pushed. `RULES_VERSION` 35, serde, fixtures and wire TS unchanged.

## Root cause

Not a rules-timing difference. With the Sabotage timing (rules version 3) the string engine ALSO announced a card declared in a window only when the window closes, and the worker (`on_observed_offer`, `ti4-server/src/session/worker.rs`) holds the declaring answer back until `state.action_card_plays` shows the play. In the string, the first question after the announcement (the Rider's own "predict the agenda outcome") was asked in the live position, which already held the play, so the reply to `submit_choice` was sent before the owner answered that question.

In the native wrapper the announcement runs inline at the close of the window (`announce_declared` -> `play_inline` -> `Inline::ask`, whose facts are `None`). Such a question comes back as `Asking { nested: None, at: Some(position) }`: not "nested" (no facts) but asked from inside a step that the driver rolled back. `game/asker.rs` chose the position to show the table with `asking.nested.or(core)` and ignored `asking.at`, so the table saw the position the step STARTED from (the play not yet pushed: `has=false`, 14 plays present). The `live/mod.rs` renderer already used `nested.or(at).unwrap_or(core)`; the asker did not.

## Fix

`crates/ti4-engine-next/src/game/asker.rs`: one helper `position_of(q)` = `nested` position, else `asking.at`, else the live core, used for the observed position of the question and for the `window_skipped` position. No change in the core, the server, the wire messages, their order or the persistence rule (the reply is still sent only after the decision is persisted; it is simply sent at the same boundary as in the string).

## Tests

* New: `ti4-server/tests/it/declared_card_reply.rs::a_declared_rider_is_acknowledged_before_its_owner_is_asked_the_prediction`. A 3-seat lobby game on the `agenda` start preset (agenda phase from round 1, riders dealt), first option answered on a thread (a client never blocks on the reply). When a seat is asked its Rider's prediction, its `after AGENDA_REVEALED` answer must already be acknowledged. Fails on the unfixed code ("the answer of \"after AGENDA_REVEALED\" was not acknowledged when \"Warfare Rider: predict the agenda outcome\" was asked", 5 s), passes fixed (0.4 s).
* `replay_export` (4 of 4, 6 s; before: hung), `fork_equivalence` 2, `crash_recovery` 5, `random_bots` 7 of 8, `bluff_reaction` 9 of 11.
* Failing before AND after (checked on the unfixed binary, not caused by this lane): `random_bots::the_host_can_redo_a_bot_seats_turn_and_the_bot_plays_the_new_turn_itself` (`Conflict: the saved history no longer replays under the strict matcher ... EngineError at decision 0`), `bluff_reaction::pass_ends_the_hold_early_and_stop_interrupts_it_at_once` and `bluff_reaction::the_default_hold_lasts_two_to_four_seconds` (hold timing). The D28D note said bluff_reaction was 11/11 in the dev profile; these two were run here in the `gate` profile only.

Binary: `cargo test --profile gate -p ti4-server --test it` (private target dir, deleted at the end).
