# BF-YIN-BT-GRANTS — Yin Ascendant acquisition

Status: scoped implementation present; root validation and independent review pending. `yinbt`
remains unclaimed in `MODULE` until root closes the route and the unsupported granted-effect
limitation is reconciled.

## Printed rule and pool

Thunder's Edge corpus text for Yin Ascendant is: “When you gain this card or score a public
objective, gain the alliance ability of a random, unused faction.” The implementation listens to
`BREAKTHROUGH_GAINED` (`player`, `breakthrough`) and `PUBLIC_OBJECTIVE_SCORED` (`player`,
`objective`). The public-score event is staged only after a successful public objective award for a
Yin player who holds `yinbt`; scoring a secret does not produce it. Delivery goes through the
Game's strict staged-event resolver path.

The canonical random pool comes from source-scoped `DEFAULT` commander records and assigns equal
weight to faction families. Its 30 initial entries are the content record named
`<faction>commander` for each family. The legacy `redcreusscommander` record is not a second Ghost
commander. Obsidian is Firmament's alternate face, so `obsidiancommander` is not a separate pool
entry; seating either face uses the Firmament family. Keleres, Thunder's Edge factions, and Crimson
remain in the canonical pool when unused. The pool does not depend on handler coverage.

“Unused” is interpreted as a faction family not represented by a seated player and an ability not
already acquired by Yin through an Alliance grant or faceup Alliance. This interpretation is not
spelled out further in the card text. Prior direct grants are filtered through
`promissory::has_commander_ability`, so each successful trigger adds a new public right while any
candidates remain. A seeded `GameRng` stream samples uniformly over the sorted canonical pool.

## Fixtures

- Canonical pool fixture checks the 30-entry roster, the Ghost and Firmament/Obsidian canonical
  forms, Keleres and Crimson inclusion, and exclusion of seated faction families.
- Deterministic sampler fixture repeats a seeded pick and checks empty-pool behavior.
- Breakthrough acquisition fixture adds `yinbt` after Game construction and resolves the actual
  gain event through `Game::step`.
- Public scoring fixture calls `objectives::award`, verifies the staged payload path, flushes it via
  `Game::step`, and checks that two successful scores grant two distinct rights.
- Negative controls cover no breakthrough, a secret score, and a non-Yin player carrying the
  breakthrough id. None grants or stages a public-score reaction.

## Limitation and verification

The grant is generic and durable. Several canonical commander effects still have no native handler
or only partial coverage. Those factions remain in the random pool as printed; the implementation
does not fabricate their effects or narrow eligibility to implemented handlers. Yin Ascendant
therefore remains unclaimed until root reviews this scope and reconciles the unsupported granted
effect limitation.

No Cargo command was run by this scoped worker. Root owns compilation, focused acceptance tests,
and review. Writable paths for this work were limited to `crates/ti4-engine/src/factions/yin.rs`
and this evidence file; no shared source outside those paths was edited.


## Coordinated October 5 complete engine gate

Root command `cargo test -p ti4-engine -q -j4 -- --test-threads=16` passed:2100 library,0 failures,1 ignored; integration groups1/1,4/4,5/5; doctests passed. Full log target/bf-oct5-engine-final.log. Compilation4 preserves memory headroom; independent tests16 use authorized cores. This includes the actual fixtures described here and all current shared WIP, not an isolated committed tree. Independent acceptance/atomic scoped commits and milestone closure remain separate gates. Historical pending statements above describe earlier checkpoints.
