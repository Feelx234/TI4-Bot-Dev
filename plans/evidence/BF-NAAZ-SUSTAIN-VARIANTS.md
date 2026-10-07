# P1: Naaz planetary sustain choices preserve physical variants

## Scope

With Prophecy of Kings enabled, a Naaz player may use a planet-standing Maximum as a combat ship when planetary participation is legal. Every fresh eligible Maximum is a distinct physical sustain choice when its serialized unit state differs (including galvanized versus plain). The choice must damage the selected copy and record that exact post-sustain unit and planet for later Direct Hit handling. Identical copies at the same planet may share one choice because sustaining either produces the same state and effect.

Both the immediate hit-absorption choice and the windowed `CombatWindow` sustain choice follow this rule. Existing `sustain|planet:<planet>` ids remain the id for the first canonical variant; additional distinguishable copies use stable indexed ids. Space options and the original-six behavior remain unchanged. Legacy planet ids remain resolvable.

## Acceptance checks

- Actual immediate sustain production offers a fresh plain and a fresh galvanized Maximum separately at one planet, and choosing the latter sustains that exact copy.
- Actual windowed sustain production has equivalent distinct options and resolves the selected copy.
- The sustain target marker retains planet and exact post-sustain unit state.
- Identical eligible copies do not create duplicate choices.
- Existing damaged-copy exclusion, first canonical option id, non-Naaz streams, original-six streams, and staged error behavior remain intact.

## Source interpretation

The printed Absolute Synergy breakthrough permits returning three of four mechs in the same system, but the combat implementation only allows Maximum to participate as a ship while a normal ship shares the space area. This change concerns only the sustain choice among eligible planetary Maximums; it does not change eligibility or the general unit-removal semantics.


First root coordinated build failed before tests: indexed parser needed Some(usize) inside optionalindex, test FirstOption needed qualification and a SystemState temporary needed a binding. Root corrected those three compile issues and replaced raw planet IDs in new variant labels with content-backed names. No passing result from the failed build is claimed. Independent root source review confirms exact indexed mutation and identical-value dedup preserve target identity; final tests pending.


Next lib gate:2100 passed,3 new fixture failures,1ignored. All three constructed the Thunder's Edge Maximum under PoK-only sources, so its sustain unit record was unavailable. Root corrected only these fixtures to DEFAULT; production eligibility assertions were retained. No successful whole gate is inferred from that failed run.


Focused `cargo test -p ti4-engine --lib planetary_sustain -q -j4 -- --test-threads=16`:3 passed,0 failed,2101 filtered (target/bf-oct5-sustain-variants.log). Covers actual immediate/windowed producers, fresh versus damaged and distinct galvanized variants. Complete gate/lint still pending.


Complete current engine gate2103passed/1ignored plus integration1/1,4/4,5/5 and doctests passes; policy273/sim51 also pass, wholecommandexit0 (target/bf-oct5-closure-green.log). All-target clippyexit0 with inherited/shared warnings. No isolated stagedtree or wholeBF acceptance claimed.
