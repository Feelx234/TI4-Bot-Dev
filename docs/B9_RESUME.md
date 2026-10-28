# B9 agenda: status (branch core-b9-agenda, worktree .claude/worktrees/core-b9)

Done and gated (see the final report of the session): 24 agenda effects as core windows (`rules/agenda/table_effects.rs`,
`effects.rs`, `effects/questions.rs`), `laws::offer_discard` (`rules/agenda/offer_discard.rs`), Covert Legislation's
face-down draw (`rules/agenda/covert.rs`), questions NameTied / Structure / DiscardLaw (`choice/agenda.rs`,
`render/agenda.rs`). Bails and design lines: docs/ENGINE_KNOWN_ISSUES.md "B9 agenda" (core-integration worktree).

Not ported (B9-bail-08): the agenda reveal chain (`reveal_agenda`, Quash / Political Favor / Political Secret offers,
Veto replacement), `close_vote` and `resolve_agenda_outcome` as core Stage machines; also Sanctions, Artifact, Rearmament
For, Plowshares Against, Redistribution, Unconventional For, Antiquities, Grant Reallocation, Crisis.

Gate command (private target dir deleted afterwards):
  CARGO_TARGET_DIR=<dir> CARGO_BUILD_JOBS=3 SKIP_AB=1 SKIP_WASM=1 tools/engine_core/verify_package.sh agenda <out> /bin/true 3
