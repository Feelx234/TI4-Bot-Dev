# I21 resume

Branch `core-integration-21` (tip: `git log -1`). Read `docs/INTEGRATION21.md`. Not pushed.

State: merges (gaps 19, d28d+d28f, d28g) and part 2 (payment note, invasion texts, turn-redo refusal tests, module run, matrices, checks) are done.
Open: `wrapper_props` (4 of 6 red, not triaged), the three red tests (INTEGRATION21 section 5; question Q3 about the snapshot fixtures), the Thundarian ground-roll port, `MAX_TYPES` naming for fights with more than ten kinds.
Private target `/root/deploytmp/core-i21` and the logs under `/root/deploytmp/i21-logs` are removed at the end of the lane.
Lesson: the env file `/root/deploytmp/i21env.sh` ends in a `cd` into the worktree, so a `cd` placed before sourcing it is ignored; a bisect started that way ran in the lane worktree and left it on a detached head until restored. Source first, `cd` after.
