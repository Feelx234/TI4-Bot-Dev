# D4 test disposition (2026-10-30)

Written by `tools/engine_core/d4/test_inventory.py` (static scan, no build) from `core-integration-4` @ `42e69e3c`; the per-test table is `tools/engine_core/d4/test_disposition.tsv` (columns: crate, file, test, group, area, generic, dual, game). `test_inventory.py --check` fails when a row count differs from `grep -c '#\[test\]'` of its crate or when the TSV is stale. Method and groups: `docs/todo/D4-core-tests.md` section 7.

## Counts

| crate | `#[test]` | rows |
|---|---:|---:|
| ti4-engine | 4544 | 4544 |
| ti4-engine-core | 336 | 336 |
| ti4-engine-core-bridge | 13 | 13 |
| ti4-engine-core-factions | 87 | 87 |

## Groups of `ti4-engine` (4 544 tests)

| group | tests | disposition |
|---|---:|---|
| G | 657 | Game-driven depth tests: KEEP, run unchanged against the flipped wrapper (D2-8 exit criterion) |
| U | 3353 | module-internal unit tests: MOVE with the module to `ti4-engine-legacy` (D2-7), neither ported nor deleted |
| L | 358 | lockstep tests (`compared`, `dual::serial`): STAY as oracle tests, version-bounded, `cfg(feature = "oracle")` |
| C | 38 | codec / kernel_sync tests: STAY in the wrapper (they feed P7) |
| S | 138 | string-only machinery (timing, choice, dual, fingerprint, registry, render, decision_*): with legacy / the bridge |

The typed core crates (`ti4-engine-core` 336, `ti4-engine-core-factions` 87, `ti4-engine-core-bridge` 13) are group K: not part of the legacy move.

The classification reads the body of each test: **G is an upper bound** (any `Game::`/`Table` mention), **U a lower bound**. The todo estimated ~870 G and ~120-170 L by a cruder look-ahead; this scan counts 657 G and 358 L (every test that holds a `serial()` guard or reads `compared` is L). D4-8 settles the split by compile triage after the move (a G file that does not compile against the facade becomes U).

## Fixtures

* tests whose body mentions `fixtures::game(` or the placeholder faction `generic`: **418** (G 84, U 308, L 12, C 0, S 14); their core half bails silently (`B6-design-02`, `XF-6`): re-seat through `seated_dealt` (D2-1) when touched, lockstep ones assert `compared >= 1`.
* tests that mention `dual::` or `compared`: **364**; tests that call `Game::`: **229**.

## The 25 largest areas of `ti4-engine`

| area | tests | G | U | L | C | S |
|---|---:|---:|---:|---:|---:|---:|
| factions | 1111 | 77 | 1031 | 3 | 0 | 0 |
| faction_effects_tests | 559 | 68 | 490 | 1 | 0 | 0 |
| game | 194 | 145 | 46 | 3 | 0 | 0 |
| technology_effects_tests | 177 | 23 | 154 | 0 | 0 | 0 |
| action_cards | 173 | 36 | 129 | 8 | 0 | 0 |
| combat | 158 | 48 | 85 | 25 | 0 | 0 |
| invasion | 121 | 34 | 57 | 30 | 0 | 0 |
| production | 96 | 37 | 51 | 8 | 0 | 0 |
| exploration | 77 | 13 | 52 | 12 | 0 | 0 |
| agenda_effects | 73 | 5 | 63 | 5 | 0 | 0 |
| objectives | 71 | 0 | 71 | 0 | 0 | 0 |
| leaders | 65 | 6 | 48 | 11 | 0 | 0 |
| action_cards_b6d_tests | 64 | 0 | 1 | 63 | 0 | 0 |
| strategy_cards | 64 | 58 | 6 | 0 | 0 | 0 |
| choice | 62 | 0 | 0 | 0 | 0 | 62 |
| seating | 57 | 0 | 57 | 0 | 0 | 0 |
| transactions | 51 | 0 | 51 | 0 | 0 | 0 |
| secrets | 48 | 5 | 43 | 0 | 0 | 0 |
| technology | 48 | 16 | 32 | 0 | 0 | 0 |
| relics | 43 | 14 | 19 | 10 | 0 | 0 |
| tactical | 42 | 0 | 42 | 0 | 0 | 0 |
| movement | 39 | 0 | 39 | 0 | 0 | 0 |
| reactions | 37 | 2 | 35 | 0 | 0 | 0 |
| transit | 36 | 1 | 35 | 0 | 0 | 0 |
| vote | 36 | 0 | 33 | 3 | 0 | 0 |
