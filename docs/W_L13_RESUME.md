# W lane 13 resume note, branch `core-w-l13` (from `core-w-l9` c639bdfc, merged with `core-w-l10` 959b95d1)

Task: the lent bodies still left after lane 9 (TE strategy primaries in the heroes, the counted bails of lane 9, nested reaction windows in the heroes,
bail-as-failure tests). Pure refactor: `RULES_VERSION`, serde output and fixtures unchanged. No heavy work (no golden/wide dual, bench, soak, ratchet,
workspace clippy, census).

## Merge note
`core-w-l10` was merged first. Conflicts: `dual_window.rs` (l9 extracted `build_settle_env` out of `settle_run`; l10's edits to the body were already in
the extracted function, the header was taken from l9, `waylay_check(gs, script)` kept), X-89 in the committed fight (`FightFacts::x89`, the Yin gate for X-89 is gone),
`tests/combat_fight.rs` (new `Env` fields). `core-w-l11` / `core-w-l12` had no commits beyond l10's tip when this lane started.

(see the end of the file for the status: ported, open, checks)
