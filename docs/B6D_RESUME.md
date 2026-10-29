# B6d resume note (branch core-b6-action-cards)

State at the last commit: 25 of the 33 cards are ported and mutation-checked (list in `docs/B6_CARDS.md`, section B6d). The block is
NOT complete, so the heavy gate (`flock /root/.ti4-heavy.lock`, `tools/engine_core/verify_package.sh action_cards`, golden dual
incl. `driver=dual` and the wide mask, `board_equivalence`, `bench` + `bench-v1..v5`, clippy) has NOT been run.

## Left

* Strategize x4 / Overrule: after the pick (`perform_strategy_card`: options `holder's card`, detail `holders`, kind `strategy card`)
  run the chosen card's ability as a child window by kind: secondary: Leadership (`Leadership::new(who, false)`), Diplomacy,
  Politics (`Politics::new`), Construction, Trade (replenish; `replenish_causes_events` bail), Warfare (`home_production`),
  Technology (`Research::new(who, false, 4, scope)`), Imperial (`draw_secret` + `HandLimit`); primary: the primaries of
  `strategy_primaries.rs` (TE variants `te6warfare`, `te4construction` are string-first). A kind not carried marks the run
  `unsupported` (the adapter declines it) with a test per kind. The guards of each kind are those of
  `strategy_core::*` / `strategy_cards_core::*` adapters.
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
