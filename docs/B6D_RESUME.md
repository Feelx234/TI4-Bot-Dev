# B6d resume note (branch core-b6-action-cards)

State at the last commit of B6d proper: 25 of the 33 cards are ported and mutation-checked (Strategize x4 and Overrule follow on `b6d-strategize`) (list in `docs/B6_CARDS.md`, section B6d). The block is
NOT complete, so the heavy gate (`flock /root/.ti4-heavy.lock`, `tools/engine_core/verify_package.sh action_cards`, golden dual
incl. `driver=dual` and the wide mask, `board_equivalence`, `bench` + `bench-v1..v5`, clippy) has NOT been run.

## Left

* Strategize x4 / Overrule: DONE on branch `b6d-strategize` (see `docs/B6_CARDS.md`, "Strategize x4 and Overrule"): the pick and the
  abilities as child windows by kind. Not carried (marked `unsupported`, a test each): the Warfare secondary, the Imperial primary,
  the Thunder's Edge primaries, and the guards listed there. Open: the layer violations of the child windows
  (`check_layers.py core`: 10 in `rules/action_cards`).
* Exploration Probe: needs the exploration resolution as a window (`explore_with`, frontier deck, rewards/attachments).
* Courageous To The End, Experimental Battlestation: child hit flow (`AssignHits`/`SustainOffer` as a `Child`), the combat
  agent's area (`core-b12`, ENGINE_REACTIONS 9.2). Fire Team is ported on `core-b12`; Scramble Frequency is a bail there.
* The playable-hand reads still answer `None` for Focused Research, Impersonation, Plagiarize, Mercenary Contract, Pirate Fleet,
  Reveal Prototype (`cost_is_payable`, B6-bail-02) and Harness/Infiltrate `reaction_can_matter` (B6-bail-03); `Child<W>`, `PayBill`
  and the facts make them reachable.
* After the last card: remove it from `BAILS`, run the gate once on the final commit and the mutation batches
  (`/root/deploytmp/b6d/batch*.sh`, driven by `/root/deploytmp/b6d/mutate.py`; run them in a scratch worktree with a private
  `MUT_TARGET`).

## Resume

`cd /root/TI4-Bot-Dev/.claude/worktrees/core-b6 && git log --oneline | head` then `CARGO_TARGET_DIR=/root/deploytmp/core-b6d
CARGO_BUILD_JOBS=2 cargo test -p ti4-engine --lib b6d_tests` and `cargo test -p ti4-engine-core --lib`.
