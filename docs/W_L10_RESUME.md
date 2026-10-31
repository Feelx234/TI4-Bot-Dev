# W lane 10 resume note, branch `core-w-l10` (from `core-integration-8`; `core-integration-8` re-merged at `0271ca64`, `core-eod1-a` is part of it)

Task: close the remaining counted gates that are not heroes (lane 9 owns Empyrean/Yin/Mahact/Warfare-Imperial). Pure refactor: `RULES_VERSION`, serde and fixtures are unchanged.
Lines for `docs/ENGINE_KNOWN_ISSUES.md` are in the section "W lane 10" at its end (rows W10-design-01 to W10-design-11).

## Ported (core half + Ask/Opt/Subtype + byte-exact renderer + `dual_window!`/`dual::write` + lockstep test, each failing on the pre-W10 code)

| gate | where |
|---|---|
| hits cancelled by a card earlier in the round (machine and flow) | `rules/combat/flow.rs`, `machine.rs`, `dual_window::FLOW_STEP` (writes Players), `combat::flow_dual` |
| a roll that ends the fight inside the roll step | `rules/combat/roll.rs` (`ended`), `machine.rs` (`finish_roll`), `combat::roll_round` |
| destructions staged by a retreat (stranded ships) announced by the machine | `machine.rs` (`announce_staged`, `Env::retreat_event`) |
| Jol-Nar commander reroll in the anti-fighter barrage | `rules/combat/barrage.rs` (new stages, `RerollSource::Commander`), `Env::{jolnar, shown_codes, barrage_extra_shown}`, `SettleArgs::jolnar` |
| Waylay played in the barrage | `barrage.rs` (`Absorb` child, `Absorb::in_space_combat`), `dual_window::waylay_check` |
| Munitions Reserves paid at the opening below rules version 8 | `rules/combat/begin_round.rs` (`Ask::MunitionsStart { have }`, `Facts::munitions_start`), render `details` |
| galvanized ground force that falls | `kernel/fx_ground.rs`, `rules/agenda/table_effects.rs`, `rules/hero_windows.rs` (the `\|g` field = `Word::Galvanized`, codec test already in `core_sync::keys::tests`) |
| command tokens in Mahact fleet pools (Warfare, Leadership, TE Warfare, Fabrication, Leadership Rider) | `kernel/fx.rs` (`tokens_held_by_mahact`), footprints with the Marks slice |
| Hacan breakthrough / Prophecy of Ixth / swapped planet value after a production by an ability | `leaders_core::announce_uses` |
| X-89 Bacterial Weapon (ground roll, its tail, the bombardment's planet exhaust) | `rules/invasion/{roll,tail,bombard}.rs`, `dual_window` |
| Nekro Malleon used by a seat not holding it readied (Ssruu copy): an empty window | `leaders_core::nekro_agent` |
| Riftwalker Meian on a map that lags the recorded edits | `leaders_core::ghost_hero` (the guard is gone) |

## Remaining gates, with the reason (not ported by this lane)

* Pinned openings: the Heart of Ixth in the barrage and the roll (Thunder's Edge relic, undecided); Assault Cannon against a neutral side (neutral casualty order, string-only by design); diplomacy opening (`gs.diplomacy.enabled`: structured diplomacy is dropped by decision, `unsupported::Disabled::Diplomacy`, B12-bail-03); a Last Bastion retreat (dropped); goods a Keleres agent could turn into spendable ones for Munitions / the Hacan flagship (`round_opening_why`, `roll_why`: Keleres is dropped by decision).
* Nested card plays inside combat windows: Courageous To The End (destruction staged by the card play, announced after it), Reflective Shielding (hits staged for the sustain step), Scramble Frequency (a reroll of the barrage dice as a registered `CardPlay`: needs the staged set mirrored at `UNIT_ABILITY_ROLLED` and the redraws in the machine's dice history): `reactions_core::table()` has no registration for them; they are the "nested (unported ability ... (card courageous|reflective|scramble))" census rows.
* Temporal Command Suite / Mercer (Nomad): the core never emits `AGENT_EXHAUSTED`; every site that exhausts an agent would have to, and TCS is a reaction on it (`Unknown::NomadAgentExhaustion` in the Tellurian, L1Z1X, Yin, Ssruu, Argent, Nomad abilities and the Keleres agent).
* Valefar Assimilator carried texts (Nekro C4R PDS, Production Biomes, Vortex, Chaos Mapping, the war sun technology of Fires of the Gashlai, Supercharge): the core would need the carrier (a token on a technology) as the owner of "exhaust this card"; Nekro seat for The Inferno (Z token flagship text).
* Umbat copied through Ssruu (an atomic use: a failed copy restores the decision log; the core window has no failure path).
* Titans' Hecatoncheires as an alternative to a PDS in Construction (`StructureFacts` has one unit per structure).
* Bombard, ground tail and roll of a neutral side.
* Thunder's Edge pieces (Saar Deorbit Barrage, Arborec Psychospore, Heart of Ixth, the Yin breakthrough event, TE Construction production nested in the card): documented bails, undecided.
* Dropped by decision, not ported: Last Bastion, Crimson Rebellion, Keleres, Deepwrought, Firmament, Obsidian, Ral Nel.

## Checks (cheap only; private target dir `/root/deploytmp/core-w-l10`, `CARGO_BUILD_JOBS=2`)

See the report; run chains are `cargo test -p ti4-engine --lib` (normal and with `TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1`), the touched `ti4-engine-core-tests` files, `tools/engine_core/check_layers.py core`, `tools/engine_core/lint_core.sh`. Mutation checks: scratch worktree `core-w-l10-mut`, `/root/deploytmp/core-w-l10/mut/{runmut.py,spec1.py}` (`MUT_ROOT=... MUT_TARGET=... python3 runmut.py spec1.py`).
Not run: golden / wide dual sweeps, benches, soak, ratchet, workspace clippy, census (the end-of-D1 heavy run is in progress elsewhere).
