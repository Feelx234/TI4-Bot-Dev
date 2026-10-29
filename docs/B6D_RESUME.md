# B6d resume note (branch core-b6-action-cards)

State: 29 of the 33 cards (31 with branch `b6d-hits`) are ported with a lockstep test through the announce window and mutation checks (list and conditions in
`docs/B6_CARDS.md`, sections B6d, Exploration Probe, Strategize x4 and Overrule; issue lines B6d-* in `docs/ENGINE_KNOWN_ISSUES.md`).
The block is NOT complete, so the heavy gate (`flock /root/.ti4-heavy.lock`, `tools/engine_core/verify_package.sh action_cards`,
`SHARDS=4`, `SKIP_AB=1 SKIP_WASM=1`, `--profile gate`; golden dual incl. `driver=dual` and the wide mask; `board_equivalence`;
`bench` + `bench-v1..v5`; clippy) has NOT been run.

## Open

* The layer gate is fixed (branch `b6d-layers`, merged): the card windows reach other areas through `kernel::children` (erased child windows, `AbilityTable::links`) registered in `driver::children`; `check_layers.py core` 0 violations, `lint_core.sh` 0 hits.
* Still without a core effect (`BAILS`): Fire Team (ported on `core-b12`, bb26913b, not merged here; remove it from `BAILS` when the
  lines are merged) and Scramble Frequency (a documented bail on `core-b12` too). Courageous To The End and Experimental Battlestation
  are ported in branch `b6d-hits` (play window only; from inside a reaction window they stay the string's, `docs/B6_CARDS.md`, last section).
* The playable-hand reads still answer `None` for Focused Research, Impersonation, Plagiarize, Mercenary Contract, Pirate Fleet,
  Reveal Prototype (`cost_is_payable`, B6-bail-02) and for Harness/Infiltrate `reaction_can_matter` (B6-bail-03).
* The mutation batch of the Diplomacy primary of Strategize/Overrule (`/root/deploytmp/b6d/batch-strategize2.sh`) had no verdicts
  at the time of the merge.

## Tools

`/root/deploytmp/b6d/mutate.py FILE FILTER OLD NEW [PKG]` (one textual mutation, `MUT_TARGET` = a private target dir) and the
`batch*.sh` scripts next to it; run them in a scratch worktree (`git worktree add --detach /root/deploytmp/<name> <commit>`).

## Resume

`cd /root/TI4-Bot-Dev/.claude/worktrees/core-b6 && git log --oneline | head`, then `CARGO_TARGET_DIR=/root/deploytmp/core-b6d
CARGO_BUILD_JOBS=2 cargo test -p ti4-engine --lib b6d_tests` and `cargo test -p ti4-engine-core --lib`.
