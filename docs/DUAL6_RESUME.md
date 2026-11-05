# Dual game 6 (B6) and the probe of the other 19 golden dual games (lane core-dual6, 2026-11-05)

Branch `core-dual6` from `core-integration-24` (`df7845b21`). Not pushed, not deployed. One commit: the fix below (harness only, no engine rule changed).

## 1. B6: the extra roll group `(0,[8,7])` of `combat::settle` in game 6

**Root cause: a scope gap of the dual harness, neither engine is wrong.** The two dice are those of a **Courageous to the End** (reason `courageous to the end`,
2 dice, no hit number, seat `d`, system 30) that the holder played inside a window of the settle (its only ship was destroyed by a forced casualty, so there was no
question in between: the card is played inside `settle`, not inside the answer of a casualty question).

* `CombatWindow::settle_machine_dual` built the string side's list of the dice of the settle from `ctx.dice.history()` since the settle began: every roll, the ability's two dice included
  (`hits_on` None shows as 0, hence `(0,[8,7])`).
* The core's `SettleOut` lists the dice of the machine only (`w.drawn`). The core plays the card in its own ability (its dice are ledger rows of purpose `Ability`) and the dual compares those separately,
  as `nested.ability_dice`, which was EQUAL in game 6 (the only difference of the game was the settle row).
* Not a version 36 effect (same at core-botstops-1), not S1-S7, not a core bug, not a new string bug: no S8 for this, no engine change, decision streams of all games unchanged by construction.

**Fix** (harness only): `dual_window::note_dice` now also keeps the place of each effect roll in the dice history (`Recording::dice_at`, `effect_roll_places()`), and the settle leaves exactly those places out of its list
(`combat.rs`, `settle_machine_dual`). `Dice::roll` passes the index (`dice.rs`).

**Regression test**: `combat::tests::a_courageous_to_the_end_played_inside_a_settle_adds_no_die_to_the_machines_dice` (the holder has one destroyer against three dreadnoughts, so the casualty is forced; 30 seeds, both holders). Without the fix it fails with
exactly the game 6 difference (`[..., (9,[1]), (0,[6,10])]` against the core without the last group); with it it passes. The existing `courageous_*` and `combat::`, `dual_window::`, `dice::` lib tests: 245 passed, 0 failed
(`cargo test --profile lockstep -p ti4-engine-legacy --lib -- combat:: dual_window:: dice:: courageous`).

**Check on the real game**: `dual_run tools/engine_profile/profile-20.json 20 all=dual --game 6` (release binary, about 55 s): before 1 difference, after **0 differences, exit 0, hashes equal**.
Game 4 also had this settle row (it lost it with the fix: 97 -> 96 differences).

## 2. Probe of the 20 games

`dual_run tools/engine_profile/profile-20.json 20 all=dual --game N` one game at a time, `TI4_DUAL_FAIL_FAST` unset (nothing stops at the first difference; the report prints at most 200 differences per game),
niced, single process. 11 of the 20 games differ; **the heavy dual steps (dual-all, dual-drv, dual-wide, dual-lent, strict) will still stop early** unless the classes below are dealt with or recognised.
Game index = `--game N` (seed, map, factions: `dual_run --specs ...`). Times are wall seconds of the probe under load 4-9.

| game | seed, map | differences | classes |
|---|---|---|---|
| 0, 1, 2, 5, 6 (fixed), 8, 11, 15, 17, 18 | | 0 | clean |
| 3 | 4, 4pOuterRim naaz letnev mentak yssaril | 1 | C |
| 4 | 5, 4pHyperlanes argent cabal sol naaz | 96 (after the fix) | A, B, C, naaz hero menu order (F) |
| 7 | 8, 4pOuterRim yssaril naalu muaat nomad | 2 | C, E1 |
| 9 | 10, 4pOuterRim empyrean winnu naaz mahact | 504 | A, E2 |
| 10 | 1, 6pStandard sardakk cabal nomad nekro winnu ghost | 71 | B, D |
| 12 | 3, 6pStandard hacan yin naaz mentak ghost naalu | 7 | D, E3, E4 |
| 13 | 4, 6pOuterRim naaz letnev mentak yssaril ghost argent | 2 | D |
| 14 | 5, 6pStandard argent cabal sol naaz titans ghost | 123 | B, E4 |
| 16 | 7, 6pStandard winnu titans mentak ghost arborec jolnar | 6 | E5 |
| 19 | 10, 6pOuterRim empyrean winnu naaz mahact sardakk jolnar | 127 | A, E6 |

Classes (first look only, none is root-caused beyond what is written; the label is what the report prints):

* **A. Nano-Forge** (games 4, 9, 19): `[exploration/Value] relics::action_relics` string `["stellarconverter"]`, core `[..., "nanoforge"]`, and the cascade `[driver/Decision] turn::menu.choice`: the core's menu has the option
  `relic|nanoforge` the string's lacks (the other options shift by one, so ids, labels, canonical text and both decision hashes differ). This is the documented string gap (`docs/NANOFORGE_RESUME.md`: "the string engine does not offer or play it";
  the Z-gate never held it). The core is right (user decision 2026-11-03/04: Nano-Forge port). **Candidate S8** (not recorded in STRING_ENGINE_BUGS.md: it was not the cause of B6). A narrow recognition would be two filters on the CORE side of the dual only:
  drop `Relic::NanoForge` in `explore_core::...action_relics` and the `Opt::UseRelic(NanoForge)` in `menu_sources_view::native().relics`, with a test. Not done here (outside the ask).
* **B. Cabal Vortex** (games 4, 10, 14): the core's menu offers `faction|cabal|vortex` ("Vortex: exhaust to capture another player's unit type from their reinforcements") where the string's offers nothing (26 to 34 menus per game).
  Both lists are "a non-structure unit of another player next to one of the Cabal's space docks with a model in the box"; the string reads `galaxy.adjacent`, the core `board::neighbours` of the core map. Suspect: adjacency (hyperlanes / wormhole tokens / laws, the same family as the
  commit `7c28b67fe` about the setup copy of the map) or the unit-in-box rule, but not checked. Unknown which side is right.
* **C. Experimental Battlestation dice not in the core's ledger** (games 3, 4, 7): `[driver/Value] nested.ability_dice` string `[(Some(5), [a, b, c])]`, core `[]`. Three dice on a 5 can only be Experimental Battlestation's (`experimental_battlestation_space_cannon`; inferred from the value, the roll reason was not printed). It is played inline in a reaction window after a move; no other difference shows in the window (so the core rolls the same dice) but the core writes no ledger row for them: `Play::ability_roll` (`ti4-engine-core/src/rules/action_cards/effects.rs`) only knows Courageous (its comment says "Experimental Battlestation is not played from a window", which these games contradict), and the row has no hit number
  (`announce.rs::play_inline` writes `hits_on: None`; the string has `Some(5)`). Same family as B6 but on the core side: the ledger bookkeeping of an inline play. Game 7 also shows `[movement/Decision] tactical::move context.hit` (string `{"cause":"other","destroy":false,"producer":"c","restriction":"any"}`, core absent) at the move after it.
* **D. Ghost wormhole tokens** (games 10, 12, 13): `[movement/State] tactical::activate wormhole_tokens.ALPHA / .BETA` string `"71"`/`"35"`/`"62"`/`"72"`, core absent or another system (`"29"` against `"76"`, `"72"` against `"80"`). All three tables hold the Ghost.
* **E. One-off or small:** E1 game 7 `tactical::move context.hit` (see C). E2 game 9 `aftermath::fight_over` `details.fleet_limit` string 4, core 3. E3 game 12 `aftermath::fight_over` the core offers `support:naaz` (a Support for the Throne to give, kind `give_note`), the string does not.
  E4 games 12 and 14 `invasion::finish_control_gain.questions` string 1, core 0 (game 14 also `players[N].commodities`, and one `resolver.state`). E5 game 16 `invasion::bombard_rerolls` (the Jol-Nar commander's reroll question): the string's context has `invasion_seq` 112, the core's has none.
  E6 game 19 `leaders/Event factions::mahact::hero`.
* **F.** Game 4 also shows the Naaz hero and fabrication-token options in another order (`component|leader|naazhero` against `faction|naaz|fabrication_token`): the cascade of A (the nano-forge option) or an order difference; not separated.

Order to take them (my suggestion): A (decided, mechanical, 3 games), C (core ledger or harness, 3 games), B and D (probably one adjacency/token family each, 3 games each), then the singles. Each is one agent, one game:
`dual_run tools/engine_profile/profile-20.json 20 all=dual --game N` (about 1 to 3 minutes, one core; `--seed S` also works).

## 3. How it was run

Private target `/root/deploytmp/dual6-target` (`CARGO_INCREMENTAL=0`), release build of `dual_run` (13 minutes from scratch, 6 minutes incremental) and the `lockstep` profile for the lib tests, deleted at the end. No heavy slot was taken (single games, one core each, niced; `/root/.ti4-heavy.lock` was held by core-bench-rv36, `.lock.2` free).
Probe outputs: `/root/deploytmp/dual6-probe/{base,fix1}/gN.out|err`, `summary.txt` (kept, small).
