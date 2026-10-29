# P1 / D1a-remainder resume note (branch core-p1, worktree .claude/worktrees/core-p1)

Done (WIP commits, last d4c87f93): P1-1 harness (Recording.order/Pushed, Run::with_order, `.order` compare), P1-2 types
(state/plain.rs), P1-3 API (fx::plain, journal_mark, mirror_journal, Sx::plain, Sx::emit_mirrored), P1-4 agenda windows write
the queue (PlainEv deleted), P1-5 render/events.rs, docs section 9 of ENGINE_CORE_DRIVER.md, D1a remainder typed windows
(LeaderFollowers, Tokens, Voting, Trade; state/pending_windows.rs, d1_windows.rs) and the W3/W4 census (d1::pending.open/close,
d1::reconcile). core-compute tools merged. Verified: core lib tests (state:: rules::agenda kernel::) 95 passed.
NOT verified yet: ti4-engine lib tests (run eng1 on d4c87f93 was queued), mutants, gate.

Next: read /root/deploytmp/core-p1-eng1.log; fix failures with regression tests; mutate (tools/engine_core/mutants.sh) fx.rs, rules/agenda/{reveal,close,committee}.rs,
render/events.rs, d1_windows.rs; add P1/D1a lines to docs/ENGINE_KNOWN_ISSUES.md (core-integration worktree); P1-6 accessor deferred (D2); P1-7 not started;
then the single gate: merge core-dual-shards (already contained), flock /root/.ti4-heavy.lock, SHARDS=4 tools/engine_core/verify_package.sh driver ... EXTRA_DUAL=...
Runner helper: /root/deploytmp/core-p1-build.sh COMMIT NAME -- cargo-args (target /root/deploytmp/core-p1, scratch worktree /root/deploytmp/core-p1-run); delete both when done.
