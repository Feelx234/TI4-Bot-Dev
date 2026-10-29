# B12 (combat and invasion area, item 4 + 5 of `ENGINE_PACKAGE_PREREQS.md`): state

Branch `core-b12` (from `core-integration-3` @ 1e55ab28; `core-dual-shards` and `core-b8c` @ 8fa4ddb2 merged).

## Status

The area is closed in the sense of the definition of done: every combat/invasion window and nested reason of the pre-B12 census
(`docs/ENGINE_REACTIONS.md` 10.4) is ported or a documented, tested bail (10.1, 10.1b, 10.1c, 10.2). The one gate run on the final
commit is in the final report of the session (`/root/deploytmp/core-b12-gate`, `summary.txt`); if it is missing, the gate has not run.

## Decisions

* 5c (Bastion/Crimson typed reads) was **dropped by decision** (user): the Last Bastion, the Crimson Rebellion, structured
  diplomacy and the turn-redo deck reserve are being disabled at the product edges and the core will panic if they are reached.
  What was committed before stays; nothing more is ported for them; whole-window bails for them stay as they are.
* 5a `FinalizingControl` and 5b `finish_committing`'s Research Team questions / `add_forced_combats` are documented bails
  (string windows inside the dual run), tests in 10.1c.

## Still string (documented, tested bails; plan in `ENGINE_REACTIONS.md` 10.2)

Scramble Frequency (the barrage/cannon/bombard windows need the dice-stream plumbing of `roll_tail`), Courageous to the End and
Reflective Shielding (child casualty flows; 9.2), Reveal Prototype and the cost the core does not judge (research as a callable piece),
Technological Singularity with the Z token, Ambuscade, the Winnu commander unlock, a window lent no dice stream.

## Mutation survivors (documented)

Glue with no observable effect in the tested windows: Dynamo's flagship-owner filter (two Empyrean seats), `pay_in`'s up-front guard,
Salvage's restore after a failed payment, the `lent`/`borrowable` glue of the Ssruu copies (the pure `borrowable` is tested), the
type filter of `extra_die_for`, Fire Team's `rerolled` bit and the delta removal (only the Crown of Thalnos reads them), its ground-kind
guard, the Sardakk mech's `unit_ability` flag, the Icon's `system != producing` guard and ship filter.

## Tools

`/root/deploytmp/core-b12-agent/mutate.py` (one `cargo test` per mutation; `MUT_ROOT`/`MUT_TARGET` run it in a second worktree so
development and mutation do not share files; filters are `a|b` substrings), `batch*.sh`, `census.out` (pre-B12 census).
Private targets are deleted at the end (rebuild about 4 minutes at `CARGO_BUILD_JOBS=3`).
