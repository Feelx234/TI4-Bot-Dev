# D2-8 lane F resume: the plan view and the secondary preview off the legacy string helpers

Branch `core-d28f-movement`, cut from `core-d28d-wrapper` (`3904d80cb`). Pure refactor: `RULES_VERSION` 35, serde, fixtures, `host_create.golden`, fingerprints and wire TS unchanged. Not pushed.

## Native now (thin crate `ti4-engine`, `crates/ti4-engine-next`)

| legacy | native | where | differential test |
|---|---|---|---|
| `movement::{Board, MovementRules}` + `action_cards::apply_movement_effects` + `tactical::ship_routes` + `transit::{CargoWindow, rifts_exited}` (as the host plan view used them) | `movement::MoveView` (`path_from_ship`, `rifts_exited`, `may_depart`, `nebula_caps`, `rolls`, `routes`, `pools`) over `query::reach`, `rules::tactical::routes`, `query::transit::Hold` | `src/queries/movement.rs` | `tests/movement_facts.rs`: played games, 15 hyperlane template maps, boards with gravity rifts, every lobby faction armed, 9 constructed variants (laws, Nav Suite, Dominus Orb, Light/Wave Deflector, Circlet, Silence, gamma token, crowds, command tokens); about 3 000 ships x 4 boosts compared on path, rifts, routes, pickups per route |
| `reactions::window_table` | `reactions::{Window, window_table}` from the core's `kernel::reaction::window_of` | `src/queries/reactions.rs` | `tests/reactions_table.rs` |
| `secondary_preview::preview_secondary` (JSON bridge to the string flow) | a throwaway native `Game` + `prepare_hypothetical_secondary` + a scripted table; blocker from `follower_facts`; payment plan from `ObjView::first_resource_plan` | `src/queries/secondary_preview.rs`, `queries/objectives.rs` | `tests/secondary_preview.rs` (540 whole-`SecondaryPreview` comparisons: every PoK card, followers, answer chains, bogus answers, blockers, payment plans) and the server contract `secondary_preview_differential` |

Host `view/tactical.rs` now builds one `MoveView` per `movement()` call. `surface_paths.py --check`: 0 legacy paths (the script had a bug: it cut `ash` off names ending in `Hash`, and `game::*` was a glob; both fixed; the 39 "pending" are the wire types, native since the swap). The facade glob (`pub use ti4_engine_legacy::*`) is untouched, so the legacy items still resolve for tests; only production callers were re-pointed. No production code calls `Board`, `MovementRules`, `ship_routes`, `CargoWindow`, `rifts_exited`, `apply_movement_effects`, `window_table`, or the string `preview_secondary`.

## Differences found

* Movement facts, window table: none (every compared value equal).
* Secondary preview: the string stamps the question's `context.phase` with the phase of the position it is given; the core always opens the window in the action phase. Identical for every real request (a secondary is only offered in the action phase); pinned by `a_position_outside_the_action_phase_is_previewed_as_the_action_phase_it_is`. Not a rule bug, nothing for STRING_ENGINE_BUGS.md.
* Thin `Window` of the window table has no `guard` (nothing outside the string engine read it).
* A position the core cannot hold: `MoveView::new` is `None`, so `movement()` gives no facts (the string built rules from any state). Not seen in any compared position.

## Remaining / join notes

* The facade glob and the legacy `Pool`/`MovementRules` types stay for the bridge, tests and dev tools (D2-9 removes the glob).
* `ObjView` gained `first_resource_plan` (+ `ResourcePlan`); `lib.rs` gained shells `movement`, `reactions` entries and explicit `game` consts: join with lanes that edit `lib.rs` by keeping both sides.
* Not run (heavy gates excluded by the brief): wide dual, golden, soak, census.

## Checks run
`cargo check --workspace --all-targets` (minus torch crates) green; host lib 299 ok; server `it secondary_preview` 10 ok (includes `secondary_preview_differential`); thin crate `movement_facts` (6), `reactions_table`, `secondary_preview` (3) ok. Mutation checks (single file, new code): rift-roll exemption, `nebula_caps` flip, `unused_answers` count, window relation: all killed (the exemption needed the Cabal tear test, added). Not run: `D4_COVERS_FILE` union with core lockstep tests, wide gates.
