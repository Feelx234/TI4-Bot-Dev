# Integration 20 resume (2026-11-04)

Branch `core-integration-20` (cut from `core-integration-19` `fc910308d`), worktree `/root/wt/core-integration-20`. Not pushed. Details: `docs/INTEGRATION20.md`.

* Done: merge of `core-silent-fix-18` (no textual conflicts, both Sh'val literals kept). Z-gate 768 games: 768 finished, 0 gaps, 0 differences of the core, 152 known string bugs (73 S1, 34 S2, 1 S3, 1 S4, 43 new S5); plain 96 / 288 unchanged; quick 1165 steps `74d5675ac754ac81`.
* New string bug S5 (a first-scored secret unlocks no leader in the string, the core does at once, 51.7): recorded, recognised by the harness. Question Q2 for the user: fix the string behind a RULES_VERSION (restores the shadow's reach in 43 games and three guard tests) or keep.
* Next: heavy gates of INTEGRATION18 section 8.4; lane D wrapper joins.
