# D4 lane A: mutation scores with the D4 tests added

Measured at commit `a9076c85` with lane B's `tools/engine_core/mutation_ratchet.sh` (`MODE=baseline`, `EXTRA_TEST_CRATES=ti4-engine-core-tests`, 2 jobs),
the modules of `tools/engine_core/d4/mutation_modules.tsv`. "Core tests alone" is the run against the unit tests of `ti4-engine-core` (the same
mutation points, taken from the first half of each module's run); "with D4 tests" is a mutant killed by either run. `mutation_floor.json` was not
written (lane B owns it). `core/leaf` was not run. The survivors of every module are in `docs/d4a_mutation_survivors/`.

| module | points | core tests alone: killed/(killed+survived) | with D4 tests: killed/survived | score before -> after |
| core/choice | 35 | 12/35 | 22/13 | 34% -> 62% |
| core/containers | 288 | 217/288 | 262/26 | 75% -> 90% |
| core/driver | 117 | 106/117 | 116/1 | 91% -> 99% |
| core/kernel | 904 | 387/904 | 640/264 | 43% -> 70% |
| core/keys | 34 | 15/34 | 24/10 | 44% -> 70% |
| core/query | 1621 | 148/1621 | 882/739 | 9% -> 54% |
| core/rules-top | 1460 | 222/1460 | 830/630 | 15% -> 56% |
| core/rules/action_cards | 1164 | 354/1164 | 406/758 | 30% -> 34% |
| core/rules/agenda | 761 | 152/761 | 335/426 | 20% -> 44% |
| core/rules/production | 861 | 118/861 | 425/436 | 14% -> 49% |
| core/rules/tactical | 126 | 0/126 | 71/55 | 0% -> 56% |
| core/state | 142 | 60/142 | 96/46 | 42% -> 67% |


Reading: the D4 tests lift every module they were written for (containers 75 to 90, driver 91 to 99, tactical 0 to 56, kernel 43 to 70); the weak
spots are the card effects (`rules/action_cards` 34: the 86 card plays are smoke-tested under four policies, the per-card text is tested for
five cards), `rules/agenda` effects, `rules/production` (the window details) and `query` (`tech`, `trade_deals`, `objectives`). The P1-P6 properties
alone kill 5 of 19 mutants of `driver/mod.rs` (the rest are accessors the properties never call; `reactions.rs` now tests them) and the
`kernel/resolver.rs` mutants that remain are the setters of the registry.
