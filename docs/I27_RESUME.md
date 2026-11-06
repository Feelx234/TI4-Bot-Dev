# I27 resume
Branch `core-integration-27` = `core-heavyfix-1` (`b7ac383a9`: int-26 `d7adf5fe8` + stale Rin test, 4 re-recorded Jol-Nar/Rin bench games,
`Tape::take_unknown` fix, docs) + own commits (lint base `heavy-base-int27` in `end_of_d1.sh`, heavy plan header, this file and
`docs/INTEGRATION27.md`). Nothing else merged (`core-theory-fork-rng` doc-only, left out; the mech investigation is lane `core-jolnar-mech-1`).
Rules version 36. Not pushed. Local tag `heavy-base-int27` = the tip. Cheap matrix green (fingerprint `538dc68f23dddcc9`, suite hash
`775f4136e60a99d5`, clippy 41 = baseline, wasm 6.60/2.10/1.51 MB); logs `/root/deploytmp/i27-logs/`.
Deployed to the alpha (user-approved): release `20261106T055127Z-be97102e` (commit `be97102eb`, code = the tip), live 2026-11-06 06:59 Berlin;
preflight and smoke OK (INTEGRATION27.md section 4). Rollback: `deploy/public/deploy.sh rollback` (back to `20261105T061956Z-3e7a099a`, int-25).
