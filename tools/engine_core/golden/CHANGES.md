# Golden bless log

Append-only, written by `golden bless` / `golden bless-new`. Every changed subtype below must be named in the commit message of the rule fix.

## 2026-10-30 09:35 Berlin  commit 85e65f62  NEW set v35-std
- tier Slim, rules version Some(35), 3 games
- reason: new set

## 2026-11-04 23:27 Berlin  commit 8b014915e  NEW set v36-std
- tier Slim, rules version Some(36), 4 games
- reason: new set

## 2026-11-06 14:57 Berlin  commit ddcabc87d  set v36-std
- games changed (1): g040.tsv.zst
- changed subtypes: load_cargo 3, propose_transaction 1
- reason: Rearmament Agreement For places each seat's own mech from its reinforcements (core-jolnar-mech-1, known string bug S9); user accepted the in-place change at RULES_VERSION 36. Proof: with only rules/agenda/effects/placements.rs reverted the set reproduces (4/4)
- issue: -
- bugfix of a frozen set: no

