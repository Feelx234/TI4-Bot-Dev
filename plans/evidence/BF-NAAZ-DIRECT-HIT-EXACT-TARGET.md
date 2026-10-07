# BF Naaz Direct Hit exact target

The sustain choice carries a selected location (`space:<index>` or `planet:<id>`). After the
unit is marked damaged, both combat sustain paths call `combat::remember_sustain_target`, which
stores the system, selected planet when applicable, and the post-sustain `Unit` under the
BF-only `combat:sustain_target` mark. Direct Hit consumes this handoff, verifies the damaged unit
and ownership/type, removes it from that exact location, and clears the mark.

The `action_cards.rs` actual-effect regressions cover two consequences of exact placement:

- Courageous can select a participating Naaz Maximum on a planet; the actual effect removes it
  through the shared combat ship remover and stages one `SHIP_DESTROYED` record with the printed
  card source and combat flag.
- Direct Hit removes the just-sustained galvanized Maximum at the selected planet even when an
  earlier, already-damaged same-type galvanized Maximum is also present. The test invokes the
  actual Direct Hit effect with the serialized post-sustain handoff shape. It does not invoke the
  private sustain resolver itself; both production sustain call sites were reviewed and write that
  same mark immediately after changing the selected unit to its damaged state.

The mark is overwritten by the next sustain and consumed when Direct Hit resolves. The legacy
non-staging path remains separate and does not use this BF-only handoff.

## Coordinated October 5 complete engine gate

Root command `cargo test -p ti4-engine -q -j4 -- --test-threads=16` passed:2100 library,0 failures,1 ignored; integration groups1/1,4/4,5/5; doctests passed. Full log target/bf-oct5-engine-final.log. Compilation4 preserves memory headroom; independent tests16 use authorized cores. This includes the actual fixtures described here and all current shared WIP, not an isolated committed tree. Independent acceptance/atomic scoped commits and milestone closure remain separate gates. Historical pending statements above describe earlier checkpoints.


Independent Luna rereview found both planetary mutation paths re-found an already-damaged first Maximum even though only fresh units were offered. Root now filters both mutation sites to fresh Maximum and adds an actual planet-stacked sustain regression. A separate existing ambiguity remains for distinct fresh galvanized/plain Maximum variants because current menus deduplicate by type; this is not claimed closed by the fresh-state filter. Reviewer also found SUSTAIN_DAMAGE_USED errors swallowed despite a Result API; BF callers now propagate that error so outer Game rollback can restore the transition, while original-six-only legacy behavior remains unchanged. Follow-up validation pending. Inherited non-Naaz Refit/Rearmament identity/supply recommendations were rejected as explicitly outside this Naaz correction's original-six compatibility scope, not accepted new regression findings.


Complete current engine gate2103passed/1ignored plus integration1/1,4/4,5/5 and doctests passes; policy273/sim51 also pass, wholecommandexit0 (target/bf-oct5-closure-green.log). All-target clippyexit0 with inherited/shared warnings. No isolated stagedtree or wholeBF acceptance claimed.
