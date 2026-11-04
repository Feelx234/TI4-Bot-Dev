# YIN_NEXUS_RESUME (branch `core-yin-nexus`, cut from `core-integration-21` 4a6cc5200)

User ruling 2026-11-04: Dannel of the Tenth may NOT target the locked Mallice (`82a|lockedmallice`); it MAY target the unlocked Mallice (`mallice`, Nexus open).

## Change
* `crates/ti4-engine-core/src/driver/turn/hero.rs::yin_spots`: walks the systems in hex order (`Galaxy::placed_in_hex_order`, as the string's `system_ids`), never offers
  `LockedMallice` (or `Mallice` from the grid walk), and appends `(MalliceLockedNexus, Mallice)` last when `NEXUS_UNLOCKED` is set and the tile is in play.
* Found on the way: once S4 stopped hiding it, yin 4p seed 8 step 927 differed in ORDER (core walked the system table, string the hex placement: `18|mr` vs `19|wellon`). Fixed
  in the same function.
* The string never offers the open Mallice either (no hex): S4 is rewritten in `docs/STRING_ENGINE_BUGS.md` as "leaves out the OPEN Nexus' planet". Core is right by the ruling.
  The harness recogniser `s4_nexus_in_error` stays (it covers that open case; no Z-gate game reaches it).

## Tests
* `ti4-engine-core-tests/tests/action_dispatch.rs`: `dannel_of_the_tenth_never_offers_the_locked_mallice`, `..._offers_the_unlocked_mallice`, `..._lists_the_planets_in_the_order_of_the_hexes`
  (all fail on the pre-fix code; the first two checked).
* `ti4-engine-core-bridge` `native_play::tests::yin_four_players_seed_eight_plays_clean_through_the_hero`; the S4 game was removed from
  `the_string_bugs_of_integration_19_are_recognised_and_nothing_else_is`, whose recogniser cases now use `82a|mallice`.

## Resume
Results of the Z-gate and the mutation check are in the final report of the lane. Nothing pushed.
