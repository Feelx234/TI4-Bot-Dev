# core-integration-9: merge record (2026-10-31)

Branch `core-integration-9`, cut from `core-integration-8` @ `4b148d65`. Three rounds, cheap checks only (the user rule: heavy work waits for a full
zero-bail playthrough). Not pushed. The live branch `nightly-fixes-2026-10-26` was not touched. Not merged, by order: `core-m3-invasion`,
`core-m6-tactical`, `core-d2-f-live`, `core-d2-f-live2`, `core-w-l15` (running lanes).

## 1. Merges

| Round | Branch | Tip merged | What it brings | Conflicts |
|---|---|---|---|---|
| 1 | `core-w-l14` | `06c7191f` | lane 14, and with it lanes 9, 10 and 13 (`core-w-l9` is an ancestor of l13, `core-w-l10` was merged into l13, l13 is an ancestor of l14): the last hero windows (Empyrean, Mahact, Yin, Naaz Warfare secondary, Winnu Imperial), `FightFacts`, TE Warfare and TE Construction primaries as children, Indoctrination and Greyfire Mutagen as rules of the table, `DrawCards` following Scheming / Crafty, Supercharge through a Valefar Assimilator, the lane 10 combat gates (Jol-Nar commander in the barrage, Waylay in the barrage, Munitions at the opening, X-89, galvanized forces, Mahact pool tokens) | 2 (below) |
| 2 | `core-w-l12` | `729c7184` | items 3 to 8 of lane 10's open list: legacy rules versions in the core (sustain offer below 16, research payment below 6), Umbat copied through Ssruu, The Inferno / Fires with carried texts, Titans' Hecatoncheires in Construction, `ForcedCombats` (Coalescence, Slumberstate), agent exhaustion announced by the core sites | 4 |
| 3 | `core-w-l11` | `9308abed` | lane 11: the neutral force fights through the core windows (reference card order, automatic answers, `Env::show_neutral`), Reflective Shielding inside a sustain, `SeatPlayMore` / `SharedPlay` handover for action cards played inside another area's window, Ghost Squad nested, Assault Cannon taking the last ship | 7 |

`core-w-l15` branched from `core-w-l11` and was not merged. The third merge brought only l11's own 8 commits (its base is l10, already in).

## 2. Conflicts and how they were resolved (keep both sides)

| File | Conflict | Resolution |
|---|---|---|
| `ti4-engine-core-tests/tests/combat_barrage.rs` (r1) | D4A4's Raid Formation test and l10's Jol-Nar / Waylay tests appended at the same place | both kept (the shared closing brace re-attached) |
| `docs/ENGINE_KNOWN_ISSUES.md` (r1) | D4A4 / lane 5 rows against l14's rows. **Row ids clashed**: l14's `W14-design-03` (Greyfire) against lane 5's `W14-design-03` (Military Support). **l14's edit of this file had also deleted the whole "W lane 10" and "W lane 13" sections** (it replaced the block that held them) | lane 5's rows kept; l14's rows renamed `W14b-design-03..06`; the lane 10 and lane 13 sections restored from `core-w-l13` with ids `W10b-design-NN` / `W13b-design-NN` (`W10-design-02..09` and `W13-design-01/02` are taken by earlier lanes; the clash is older: l10 and l13 already had it). `docs/W_L14_RESUME.md` names `W14b-design-03` now |
| `rules/invasion/mod.rs` (r2) | `pub mod fight;` (l9) and `pub mod forced;` (l12) | both |
| `rules/production/strategy_cards.rs` (r2) | `Construction` got fields from l13 (`uses`, `using`, `produced`, `unsupported`) and from l12 (`alt_at`, the Hecatoncheires stage) | l13's constructors with `alt_at: None` added; the `CStage::Alternative` code of l12 kept next to l13's `CStage::Producing` |
| `docs/ENGINE_KNOWN_ISSUES.md`, `docs/LENT_CENSUS.md` (r2, r3) | append at the same place; the base's lane 10 rows came back as stray duplicates | both kept, stray duplicates dropped; l12 and l11 rows renamed `W12b-design-NN`, `W11b-design-NN` (the ids `W12-design-01/02` and `W11-design-01..06` belong to the technology lane and lane 5 to 7; `W12-design-02` is still a pre-existing duplicate of those two lanes) |
| `ti4-engine-core-tests/tests/combat_machine.rs`, `ti4-engine/src/combat.rs` (r3) | D4A4's stage tests / the Reveal Prototype test against l11's neutral and Reflective Shielding tests | both kept |
| `rules/combat/sustain_offer.rs`, `cannon_dual.rs` (r3) | module docs: l12 says "below `RULES_FACTION_EFFECTS` is carried", l11 says "the neutral force is carried" | one doc that says both |
| `ti4-engine/src/dual_window.rs` (r3) | (a) `round_opening_why`: l14 dropped the Valefar Supercharge gate, l11 lets a neutral owner pass the "seated player" check; (b) `settle_run`: l9 had split it into `settle_run` + `build_settle_env`, l11 edited the old header (`tables_of_sides`, the neutral bail removed) | (a) no Valefar gate, neutral owner allowed; (b) l9's split kept, `tables_of_sides` in `settle_run`. **The first resolution kept the neutral bail at the top of `settle_run` (the deleted lines sat inside the conflict hunk, on l11's side)**: see section 3, B1 |

## 3. Bugs and breaks found and fixed

Each is found by a test that already existed in one of the lanes (nothing was loosened); where a new test was needed it is named.

| # | Where | What | Fix and guard |
|---|---|---|---|
| B1 | `dual_window::settle_run` | **My own merge slip, a real machine bug**: the neutral bail (`reason::NEUTRAL`) that l11 deleted stayed in the resolved `settle_run`, so every composed fight against the neutral force bailed (0 compared, 60 bailed) | removed. Guards: l11's `combat::tests::a_neutral_side_fights_in_the_core_in_lockstep` and `assault_cannon_against_a_neutral_side_runs_in_the_core_in_lockstep` (composed pass, bail-as-failure); both failed on the first r3 chain and pass now |
| B2 | `ti4-engine/tests/decision_delivery_inventory.rs` | rename-only registry edit: l9 split `mahact_units::use_leader_timed` into the dual wrapper and the producer `hero_string` (the Choice and the ask observation moved) | the `Producer` row and the tuple name `hero_string`; no count changed |
| B3 | `ti4-engine-core-tests/tests/promissory_windows.rs` `mageon()` | fixture seated a **Yssaril** holder with `limited: true` and expected the hand-limit question. Since l14 `DrawCards` follows the faction hook (Crafty = no limit), so the question vanished (`not asking (Some(Done))`). The code is right: a Yssaril holder has no limit; the test's seat was wrong | the holder is a Nekro seat when `limited` (a seat without a hook, as a Nekro copy of the card would be) and Yssaril when not; the assertions are untouched. New assertion in the same test: a Yssaril holder with `limited: true` is not asked either (the guard of the hook path) |
| B4 | `ti4-engine-core-tests` (`windows_misc.rs`, `hero_windows.rs` x2, `combat_fight.rs`) | struct literals without the new fields: `StructureFacts::alt` (l12), `Env::show_neutral` (l11) | `alt: None`, `show_neutral: false` |
| B5 | clippy gate (`tools/engine_core/clippy_core.sh`) | none of lanes 9 to 14 ran clippy: large enum variant on `Spawn` and `Began` (core), unused variables left by lifted gates in `dual_window.rs` (x6) and `cannon_dual.rs`, an unused `Unknown` import (l12, `cc_ground/borrowed/timing.rs`), two "very complex type" (`build_settle_env` return, `winnu_hero` value), an unused `LABEL` (l11), an ignored `#[must_use]` in a titans test | the large-enum allows carry a reason (repo convention), parameters are `_`-prefixed (the callers are unchanged), two type aliases (`BuiltEnv`, `WinnuValue`); the engine baseline lost one line (a fixed warning). Behaviour unchanged |

No assertion was edited to make it pass. The only test edit that changes what a test builds is B3 (fixture seat), argued above.

Checked and left alone: `dual_window::yin_unported` still bails "neutral" for a Yin hero planet with neutral infantry on it (l9's gate). The committed fight's ground-round windows are the ones l11 taught about the neutral force, but there is no lockstep test for the Yin hero against neutral infantry, so the gate stays (counted bail; listed in section 6).

## 4. Checks (cheap), `CARGO_BUILD_JOBS=2`, private target `/root/deploytmp/int9` (profile `lockstep`)

The last full chain (engine lib plain and dual, core libs, D4) ran at the code of the tip minus the two `_a` renames of `over_unported` / `roll_tail_unported` and the extra Yssaril assertion of the Mageon test; those were checked after by clippy_core (builds every target), the D4 crate (1394 passed), the inventory, `cargo check --workspace`, layers and lint. Per round:

| Check | r1 (l14) | r2 (+l12) | r3 (+l11, fixed) |
|---|---|---|---|
| `cargo check --workspace --all-targets` (without ti4-review, replayer, mlp, tensor, advisor) | ok | ok | ok |
| `cargo test -p ti4-engine --lib` | 4894 passed | 4900 passed | 4909 passed |
| same with `TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1` (whole lib) | 4894 passed | 4900 passed | 4909 passed |
| core, core-bridge, core-factions, rule-bot-typed `--lib` | 452 + 46 + 126 + 1 | same | 452 + 46 + 126 + 1 |
| `decision_delivery_inventory`, `content_ids_resolve` | 4 + 4 (after B2) | 4 + 4 | 4 + 4 |
| `ti4-engine-core-tests` with the D4 flags of `tools/engine_core/d4/run_core_tests.sh` | all green (after B3) | all green (after B4) | 1394 passed, 0 failed |
| `check_layers.py core`, `lint_core.sh` | 0 violations, 0 hits | same | 444 files, 0 violations; 446 files, 0 hits |
| `tools/engine_core/clippy_core.sh` | ok (after B5) | ok (after B5) | ok (after two more `_a` parameters; 41 engine warnings, baseline 41) |

Failures on the way, all fixed above: r1 registry (B2), Mageon (B3); r2 compile of core-tests (B4) and clippy (B5); r3 compile (B4), two composed neutral tests (B1), two unused parameters (B5).

Not run (user rule, plus nothing to run them against): `end_of_d1.sh` and its steps (golden / wide dual, `board_equivalence`, `core_sync_gate`, `alpha_roundtrip`, benches, soak, mutation ratchet, census, workspace clippy beyond `clippy_core.sh`), mutation checks, `TI4_CORE_STRICT=1` on the golden games, Playwright. Not pushed.

## 5. Findings worth knowing

* l14 deleted two sections of `ENGINE_KNOWN_ISSUES.md` by accident (section 2). Nothing in the code depended on them; the rows are back.
* The id scheme of the known-issues rows has collided since lanes 5 to 7 (three lanes use `W10-/W11-/W12-design-NN`). The rows added by lanes 10 to 14 now carry a `b` (`W10b`, `W11b`, `W12b`, `W13b`, `W14b`). The resume notes of those lanes still say the unsuffixed id.
* The alpha saves are all rules version 0 (`docs/W_L12_RESUME.md`): every old save runs below every version gate, so the legacy branches (sustain offer below 16, research payment below 6, Munitions at the opening below 8, the sustain in the machine below 17) are reached by all of them and are ported.
* `Spawn` / `Began` are now 688 bytes for their largest variant (the Thunder's Edge Construction uses). Boxed would be cheaper in memory only; left with the reasoned allow.

## 6. Remaining gates, consolidated from `W_L9` to `W_L14` (after this merge)

Dropped by decision (not open): Last Bastion, Crimson Rebellion, Keleres, Deepwrought, Firmament, Obsidian, Ral Nel (Alarum, Data Skimmer), Proxima Targeting VI, deck reserve and turn redo, structured diplomacy (`gs.diplomacy.enabled`; `unsupported::Disabled::Diplomacy` panics in the core by design). Thunder's Edge: Heart of Ixth, Saar Deorbit Barrage, Arborec Psychospore, the Yin breakthrough event and the Ascendant, the Deepgloom Executable, L1Z1X Fealty Uplink, the Fracture neutral force and the first claim of a Fracture planet, Coexistence (the chain of combats).

Open, product-reachable, each with the missing piece:

1. **Scoring window after a won ground combat in the Yin hero** (`yin_hero` declines a recording that holds a "score an objective" question; `CommittedFight::scoring` lists the occurrences). Needs an `EventScoring` window (a `ScoringBook` walk over core `Offers`, a `ScoreOne` child per player, the plain `OBJECTIVE_SCORED:{alias}` record and the `GAME_FINISHED` stop), `FStage::Scoring` in `CommittedFight`, a link in `driver::children`, and the adapter must compare the plain records the string pushes. The live driver's own `EventScoring` arm (`core-m3-invasion`) would order a hero's scoring after the whole hero. The Mahact hero has no such window to port.
2. **Van Hauge, the Spinner**: Van Hauge needs the combat machine to drain staged destructions at the string's four points (a retreat, produced hits, after `SPACE_COMBAT_ROUND_ENDED`, the end announcement) instead of bailing at a transition; the Spinner needs a placement question over planets and ship spaces (`PlaceUnits` has planets only). The "staged destructions pending at a transition" guard stays for positions handed over from a nested card play.
3. **Action cards played inside another area's window** (lane 11, all counted bails named by the card): Courageous To The End (two differences found in the dual adapter and ledger, not in the rules: its two dice are not recorded as `RollPurpose::Ability` ledger rows, and the opponent's casualty question has no display context on the core side), Scramble Frequency (a registered `scramble` play, hosts that stage and re-read their dice: barrage, space cannon offense, bombardment; Fire Team is done), Upgrade, Decoy Operation, Harness Energy, Infiltrate, Bribery, Lie in Wait, Crash Landing, Experimental Battlestation (hosts are windows of the tactical action, the vote and the transaction that are not ported or not lent the stream and the map; `reaction_can_matter` answers `HarnessOrInfiltrate` / `BriberyGoods` for three of them). Action-timed cards (Ghost Ship, the Contracts, Pirate Fleet, Jam, Refit Troops, Brilliance, Impersonation, Plagiarize, Focused Research, Divert Funding, Exploration Probe, Strategize / Overrule) are not bails: no host plays them. The inline Lost Crew of an action card's effect (`rules/action_cards/effects.rs`) needs a hook-free draw within the limit (`plain_draw_within_limit`); the agenda *effect* window (`agenda_effect`, plain kernel) keeps a faction-hooked draw with the string.
4. **Temporal Command Suite itself and Mercer (Nomad)**: the string answers `AGENT_EXHAUSTED` when staged events are flushed (`announce_staged_ground_events`, a flush that is no dual window); a core TCS (a `Spec` on `AgentExhausted`/`After`, exhaust, ready, an optional `TradeWindow` child) can only be run by a live loop that flushes staged events (`core-d2-f-live`'s contract). Mercer hangs on `TACTICAL_ACTION_ENDED`, which no core window emits. The family stays named by `bail_every_family_that_stays_in_the_string_is_named`. The core sites that exhaust an agent already stage the event (l12).
5. **The control-gain chain** (`invasion::finish_control_gain`: Assimilate, `PLANET_CONTROL_GAINED` and its windows, technology AFTER windows, legendary and Fracture, the exploration reward composition after a first claim, D1): a chain of windows of several areas on one planet gain; needs `rules::explore_windows` hosted inside the invasion's control step. The Minister of Exploration and the Crown of Emphidia on the control gain are done (l13).
6. **Yin hero, remaining gates** (`yin_unported`): a neutral side on a chosen planet (l9's gate, not lifted by l11: see section 3, no lockstep test), a coexisting planet (TE), L1Z1X Fealty Uplink (TE).
7. **Combat machine bails that stay** (`SettleArgs::gate_why`, `Machine::bailed`): a Letnev / Mentak round opening that the machine does not carry, goods only a Keleres agent could make spendable (dropped), a Last Bastion retreat (dropped), the Heart of Ixth (TE), the "staged destructions at a transition" guard, and **irreproducible dice** (test faces, a stream restore, a seed that is not the state's): the product assertion XF-7 (`product::reproducible`, strict under `TI4_CORE_STRICT`, the gates, nightly and D2); the counted bail is reached only by dev tools and unit tests.
8. **Nested reaction windows that are not rules of the core table**: the Spinner, Van Hauge and the Yin Ascendant (`Recording::unported` names them). There is no Ssruu copy of Indoctrination (Ssruu copies the agent only).
9. **Draw hooks that stay with the string** (`strategy_cards_core::draw_hook`): the Deepgloom Executable (TE), blank laws (Keleres' Law's Order) and a Ral Nel seat. Two seats of one faction (Empyrean replenish, Politics speaker, option ids keyed by faction) cannot be seated by the lobby (`DuplicateChoice`).
10. **Smaller**: the Imperial primary holds sixteen scoreable objectives (a seat cannot score more than ten at a time); a neutral PDS / space dock / mech (space cannon, bombardment against a neutral planet) is covered by the generic roster order but only Pirate Fleet / Contract ships and Mercenary Contract infantry have lockstep tests; a Valefar-carried C4R or Vortex is unreachable in the product (the adapters' guard can never fire; not ported).
11. **Mutation survivors** (equivalent or covered by lockstep tests only): lane 9 (settles counter, `at_start` sort, `prepared.find` vs `first`, the double wipe-out `Winner` key, `spots.contains`, `value_swapped_planet` clear, `answered > 0`), lane 11 (`front.hits > 0` in `Machine::neutral_due`), lane 13 (Dacxive's `remaining >= 1` needs an empty box; `x89` in the committed fight's roll tail facts), lane 14 (the `limit > 0` guard of `HomeProduction::start`, `requested > 0 &&` of the Scheming test, the answer check of the Scheming discard; `offer_supercharge` is not mutated, killed by the engine lockstep test only).

Heavy gates still to run once a zero-bail playthrough is possible (unchanged by this merge; commands in `docs/INTEGRATION7.md` 10.4 and `tools/engine_core/end_of_d1.sh`, with the six weaknesses of the script listed there): golden and wide dual sweeps (`dual-all`, `dual-drv`, `dual-wide`), `board_equivalence` (golden and wide), `core_sync_gate --strict`, `alpha_roundtrip --strict`, the benches (off / resolver / state / both), `census` (and the post-fix numbers of `docs/LENT_CENSUS.md`, which lanes 10 to 14 explicitly did not re-count), `soak`, the D4 gate steps, the mutation ratchet (`mutation_floor.json`), workspace clippy, `TI4_CORE_STRICT=1` on the golden games, and the mutation batches for the files named in the resume notes.

## 7. Tip

`git log -1 core-integration-9` (the last commit is this document's). The private target dir and the scratch worktree were deleted.
