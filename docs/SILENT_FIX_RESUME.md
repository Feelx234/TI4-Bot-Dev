# Silent-gap fixes (lane core-silent-fix-18), cut from core-silent-gaps-18

## Done (each with a native/core test that fails on the pre-fix code)
1. N-01 / X-89: `query::tech::holds_x89` reads the deck printing (`X89BacterialWeaponOmegaOmega`); used by ops.rs, invasion/fight.rs, turn/hero.rs. Also found: the driver never set `Bombardment::x89`, so bombarded planets were never exhausted; fixed in `bomb_window`. Tests: invasion_composer `x89_*` (3), action_dispatch `dannel_..._x89`.
2. B-X01: windows/scoring.rs unlocks after every score. Test: status_phase `..._secret_scored_for_the_first_time` (event window, so no later bookkeeping masks it).
3. Sh'val: `cd_registry/sardakk/offer.rs` provides `general.skip_to_commit`; new `Ask/Opt/Subtype::SardakkSkip` in choice/leaders.rs; render literals in BOTH render/leaders.rs (ti4-engine and ti4-engine-next): `sardakk_hero_skip`, id `skip`, label `skip to commit`, same prompt as the string. Test: tactical_composer `shvals_skip_...` (offer, accept writes the mark and the aftermath opens at the commitment, decline, locked/purged not offered).
4. Census (tests/silent_census.rs): empty hook slots (59, each with class and read-kind), unrendered decision subtypes (corpus + `DecisionContext::new` literals, 38 accounted), technologies outside the deck named by the core, deck technologies never named. New `HookTable::empty_slots`.
   Real gaps the census led to (both fixed, tests in seam_only_providers.rs): Nomad Quantum Manipulator / The Cavalry (`seam.grants_sustain` was never read; `sustains_in_space` now ORs it), Mentak Moll Terminus (`seam.may_sustain` was not read by `can_sustain_here`).
   Stale headers fixed (naalu/yssaril offer.rs, cabal observe.rs, magen_defense.rs, invasion_live.rs comment). The bail label strings in production_window_dual.rs are census keys and were left.

## Checks
native_play quick game unchanged: 1165 steps, 74d5675ac754ac81. Core test binaries run: invasion_*, action_dispatch, status_phase, tactical_composer, hero_windows, combat_*, kernel_hooks, seams, query_tech, leader_heroes, new ones. Single-file mutations (x89 printing, scoring condition, bombard x89, fight x89, sardakk register, seam OR in combat, seam in can_sustain_here) are all killed. Legacy lockstep tests were NOT run (heavy); `cargo check --tests` of legacy/thin/bridge is clean.

## Remaining
- F-relic-nanoforge (awaits ruling). Lane D wrapper rows.
- Observation: core reads `ERRORERROR` (nekroc4r) in several windows; the card is in no deck, in the string either (parity, accounted in the census).
- Seam predicates with no reader (`production_discount`, `commit_blocked`, `deploy_planets`) have no faction provider outside dropped ones.

## Join notes
Touches choice/leaders.rs (3 enum variants appended), both render/leaders.rs, kernel/hooks/mod.rs (empty_slots), rules/combat/mod.rs, rules/invasion/mod.rs, scoring.rs, tech.rs. RULES_VERSION, serde, wire TS untouched.
