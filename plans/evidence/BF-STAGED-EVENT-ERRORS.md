# BF staged-event error propagation

`supply::try_flush_staged_events` is the strict staged-event API for callers that can propagate
`TimingError`. It resolves queued typed events in deterministic key order, including nested events
staged by reactions. If a reaction fails, it restores the pre-event `GameState`, dice history, and
RNG state and leaves that event queued for retry. With no timing handle it returns `Ok(0)` and
leaves the queue untouched. Malformed staged records are discarded, preserving the existing queue
validation behavior.

The existing `supply::flush_staged_events` signature remains for compatibility and retains its
legacy swallowed-error behavior through `hooks_economy::emit`; new production callers should use
the strict API. The game-driver migration is coordinated separately.

Focused tests:

- `supply::tests::strict_flush_restores_failed_reaction_and_keeps_event_for_retry`
- `supply::tests::strict_flush_processes_nested_events_after_already_waiting_rows`
- Existing compatibility coverage: `supply::tests::staged_gains_flush_in_order_and_only_with_a_timing_handle`

Tests were not run in this bounded package because the coordinator owns Cargo builds for the shared
working tree.

## Root Game integration and review resolution

Game now uses strict supply delivery and strict hooks_ground::try_announce_staged_events. Both restore per-event state, dice, RNG, EventSequence, and all Resolver journal/scope/registry fields on failure, preserving the failed row for retry. The external decider input stream cannot be rewound. Legacy callers retain their original APIs. Supplemental Luna review identified the ground path swallowing errors before supply delivery; root closed it and added an actual GROUND_FORCE_DESTROYED reaction failure/retry fixture. The latest engine library run passes all three strict regressions, including nested queue ordering (1,985 passed, 1 ignored overall). Required independent tier-C acceptance of root changes remains pending.
