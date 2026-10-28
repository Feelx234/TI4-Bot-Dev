# Engine known issues (core "owns game state" port)

Living document. One place for open issues, string-side bugs the core copies on purpose, documented
bails, and decisions waiting on the user. Update it in the same commit that opens or closes an item.
Provenance: items marked (seed) came from the coordinator's hand-over and were not re-verified
against code in this pass; items marked (checked) were. Per-package bail counts come from the
package branches (`core-*`), not from this branch.

## A. Decisions needing the user

| id | item | where | why open | suggested resolution |
|----|------|-------|----------|----------------------|
| A1 | Heart of Ixth fix (see B1) needs a `RULES_VERSION` bump | `docs/RULES_VERSION.md`, `crates/ti4-engine/src/combat.rs` | fixing changes engine behaviour for saved games | user decides; recommended: bump after the port lands |
| A2 | Merge of live branch `nightly-fixes-2026-10-26` into `core-state-main` | branches | user decided: wait until packages finish | revisit when all packages are merged; see E8 for expected conflicts |
| A3 | Merge of the core port into a nightly/live branch | branches | not scheduled | decide after A2; mind the sweep cut-off (20:30 Berlin, CLAUDE.md) |
| A4 | P1 card rule questions: Crisis Pact order, Covert Legislation edge cases | `docs/CARD_P1_FIXES.md` | rules rulings needed | user rules; then implement |
| A5 | UI work for the Crisis Pact prompt and the hidden Covert Legislation card | web | depends on A4 | queue after A4 |

## B. String-engine bugs copied on purpose

| id | description | where | why open | resolution |
|----|-------------|-------|----------|------------|
| B1 | `heart_ixth` emits option ids `hfix\|{entry}:{die}:{sign}` (checked: `combat.rs` ~l.902-923) but parses by splitting on `\|` and expecting four parts (checked: ~l.968-975, only three come out). Every pick fails to parse, no die shifts, the card never exhausts. The seed named `combat/roll_window.rs`; in this tree it is `crates/ti4-engine/src/combat.rs`. | string: `crates/ti4-engine/src/combat.rs`; core copy: `crates/ti4-engine-core/src/rules/combat/roll.rs` ~l.654 on branch `core-b1-combat` (comment names the bug) | core must match the string engine for dual-run parity | fix both together under a `RULES_VERSION` bump (A1): parse three parts after `hfix\|` |
| B2-K7 | Copied string behaviour of Harrow (package B2): it bombards with the invader's space-area units only, with no Assail, Bunker or galvanize bonus, and Plasma Scoring applies to the first roller only. | core: `crates/ti4-engine-core/src/rules/invasion/harrow.rs`; string: `crates/ti4-engine/src/invasion.rs` | core must match the string engine for dual-run parity | rules review, then fix both together (probably under a `RULES_VERSION` bump) |

## C. Documented bails per package

A bail = the core declines and hands the decision back to the string engine. Seed list; counts where
given are from the package reports.

- **Combat (B1)**: readied The Thundarian (core ability table empty); Keleres agent paying for Munitions/Wrath; neutral sides; `begin_round` Naaz/Letnev/diplomacy (278 of 550 calls); retreat window (Eidolon/Voltron, Floating Factory, Last Bastion seat, neutrals); composed `CombatWindow` (in progress, B1g).
- **Invasion (B2)**: nested reaction windows in `GroundRoundHits` (82/361); Begin/Tail/Advance bails; Harrow and `commit_deploy`/Keleres toll; zero-coverage Advance bails.
  - B2-K1 (new bail): `commit_one`/`commit_deploy` dual bails on a landing the Keleres toll bars (`dual_window::land_one`); goes away when the toll is a core rule fact.
  - B2-K2 (new bail): toll owed at a table `production_view::unported_window` declines (Crimson/Bastion tables, Keleres/Deepwrought/Naaz/Mentak invader, Argent/Nomad/Yssaril present); resolved by B3 and the faction ports.
  - B2-K3 (new bail): Harrow with irreproducible dice or Proxima Targeting VI at the table (`round_hits_unported`).
  - B2-K6 (dead guard): the "seats not in id order" bail in `advance` cannot trigger (Owners are sorted).
- **Production (B3)**: Bastion and Crimson declined for typed reads; Argent/Yssaril/Nomad cross-seat guard; Naaz mech staging; empty tier-W offers (Keleres/Winnu agent, Hacan hero, payment-window agent, Argent agent redirect); `sling_relay`/`integrated_economy`; supply mutations; `ProductionCounters`/timer split.
- **Agenda (B9)**: ~28 effects with no core window (event staging, questions, secret deck draw, Regulations Mahact token count, Crisis version-gated); reveal/resolution windows; `laws::offer_discard`.
- **Action cards (B6)**: reads and ~38 effects ported (branch `core-b6-action-cards`); the rest (about 100 effects that ask questions or need other areas' rules, the play/announce flow, `available_actions`/`perform`, `card_timers` state) not ported.
  - B6-bail-01: `has_legal_target` answers `None` for Uprising, Seize Artifact, Rise of a Messiah, Frontline Deployment, War Effort and Unstable Planet (neighbours, controlled-spot rules, the string board keeps empty `planet_units` entries). Earlier list also had Reactor Meltdown, Cripple Defenses, Lucky Shot, Plague, Tactical Bombardment, Scuttle: ported since (slice 6). The core reads use `SourceSet::all()` where the string passes the game's sources.
  - B6-bail-02: `cost_is_payable` answers `None` for Focused Research, Impersonation, Plagiarize, Mercenary Contract, Pirate Fleet, Reveal Prototype (payment, research, neighbour rules).
  - B6-bail-03: `reaction_can_matter` answers `None` for Harness Energy and Infiltrate.
  - B6-bail-04: `own_hand_choice` answers `None` for an unknown (subtype, prompt) pair; the string question stands.
  - B6-bail-05: Industrial Initiative, Intercept, Parley, Direct Hit, Scramble and the other effects that stage events, need a combatants read, a dice roll or a unit-form flip are still string (event staging outside a window has no adapter path yet).
- **Others**: B4 offer/diplomacy remainder; B5 (mutation half partly ported, see the B5 section below); B8/A4 mutation halves (B7 see its own section); A4 scoring/reveal not ported; S8 partial (only reaction slots as data).

## D. Design / known issues

| id | issue | note |
|----|-------|------|
| D1 | relic-sustain answered twice: seam data row vs `query::relic` | one reader should win; `board_view::relic_grants_sustain` already removed (commit 15614c22) |
| D2 | Nomad `agent_readied` needs X1 | blocked on X1 |
| D3 | `BorrowedStats.unit` is a kind, not a piece | model gap |
| D4 | Bastion `note_retreat` needs unit-type text order | ordering dependency |
| D5 | `verify_package.sh --no-default-features` build fails: `kernel_sync` and `dual_window` not feature-gated | gate them |
| D6 | golden games rarely hit many windows; CE golden-positions dual incomplete | add targeted goldens |
| D7 | core mode slow until per-position views / D2: `BoardView::of` per call ~12x slow; A1 stays String default | do not default to core before D2 |
| D8 | `GameState` has no `PartialEq` or revision counter | needed for cheap change detection |
| D9 | stale-answer risk for `may_sustain` / `temporary_ground_force` in multi-batch windows | revalidate answers per batch |
| D10 | Harrow dice-ordering risk | verify against string order |
| D11 | `state_size.sh` needs a release binary | build first |
| D12 | `gate` cargo profile exists only on branch `build-speed`; `string-disentangle` uses env vars | unify when merging (`docs/ENGINE_BUILD_SPEED.md`) |
| B2-K4 | open design: 82 nested-reaction bails in `ground_round_hits` | needs the reaction/ability port |
| B2-K5 | open design (B5 split the exploration tail into `control_gain_fracture_relic`/`control_gain_exploration`, see B5-bail-05): the `FinalizingControl`/`finish_control_gain` hook chain, the rest of `finish_committing`, the setup/end hooks and the scoring pauses are still string | B5 exploration, technology and the ability port |
| B2-K8 | bench-v1 seed 25 (`selfplay replay --seed 25 --players 6 --map 6pStandard --factions mentak,yin,nekro,mahact,ghost,letnev --speaker random --rules-version 11 --bot bot_version=1`) differed under `invasion=core` (hash 285d9162e1f792f0 instead of f1347a5256c63ade). Cause: Nekro Mordred's +2 was counted twice, by the native Nekro row and by the combat kernel's string bridge (ground rolled on 2 instead of 4; space combat had the same double count). FIXED on `core-b2-invasion` in d3c9316e (`core_combat.rs` bridge no longer adds it), with a regression test. | `dual` had not flagged it |

## B6 copied behaviour and design lines

| id | note |
|----|------|
| B6-copy-01 | Bribery's goods supply comes from the string `supply::potential_goods`, Keleres agent included (core takes it as an argument) |
| B6-copy-02 | `may_play` takes the Political Secret and Transparasteel bars from the string (laws module) |
| B6-copy-03 | own-hand options are built by the string from printed names; the core renders what it is given |
| B6-copy-04 | the `Simple` effects take `mahact_held` (Rally's reinforcement cap) and the sources as arguments from the string side |
| B6-design-01 | `dual!`/`dual_typed!` panic on a difference only with fail-fast; `with_mask` in a unit test does not enable it, so tests must read `dual::take_report()` |
| B6-design-02 | the fixture faction "generic" (and fixture promissory notes keyed "generic") is refused by the Players/Cards codec, so the core half silently bails; tests must use real faction ids and assert the core half ran (probe `play_simple(...).is_some()`) |
| B6-design-03 | `board_equivalence` prints one action-cards summary line with no per-area switch |

## B7 leaders: bails and design lines

Branch `core-b7-leaders`. Ported: status writes, the decision-free effects (extra-die agents, Darktalon Treilla, Hacan hero, Jace X., I48S, Claire Gibson's placement, end of round) and the question-asking windows of Ggrocuto Rinn (Xxcha agent), Carth of Golden Sands (Hacan agent), The Helmsman (L1Z1X hero) and Rin (Jol-Nar hero), all behind dual on `leaders`.

| id | note |
|----|------|
| B7-bail-1 | a leader id the core tables do not know makes every status write fall back to string (`leaders_core.rs`) |
| B7-bail-2 | positions the Players slice cannot map (placeholder faction "generic", more than 16 owners) bail every leader write |
| B7-bail-3 | `extra_die_agent` bails when the string-resolved unit form has no core `Unit` variant (A3 `unit_form` table at D2); the same holds for `l1z1x_agent` and `sol_commander` |
| B7-bail-4 | the ability arms owned by faction modules (`factions::use_leader_timed`, `use_leader_strategy_primary`, Ssruu's `use_leader_text` wrapper, Nomad, Titans, Mahact, Naaz, Yssaril leaders) stay string; the generic arms Ssruu borrows (xxcha, hacan, l1z1x agent, ...) are covered through `dispatch_leader` |
| B7-bail-5 | the four question windows bail inside a space battle or an invasion (the string context carries the battle), with diplomacy enabled, or when a nested emission was recorded |
| B7-bail-6 | `hacanagent` bails at a table with a Cabal, a Firmament siphon watcher or a Trade Agreement in play (commodity events, replenish hooks stay string); `xxchaagent` and `l1z1xagent` bail with diplomacy enabled (promise kept on a favour) |
| B7-bail-7 | `l1z1xagent` bails for the Naaz mech (announced placement); `jolnarhero` bails when `apply_unit_upgrades` would change the position |
| B7-bail-8 | `xxchaagent` and `l1z1xhero` run without the Map slice when the string arm has no galaxy; the removal / the whole use is then never offered, as in the string |
| B7-bail-9 | `xxchahero-te` (Thunder's Edge hero, a multi-placement loop) is not ported; `solhero` has no question and is ported |
| B7-design-1 | Nomad `agent_readied` and the agent-exhausted staged event live in the string callers outside the core status write (move to core hooks at D2) |
| B7-design-2 | tests build `seated_game` and clear promissory notes by hand to avoid unmapped Cards |
| B7-design-3 | `RinHero` takes the corpus (file) order of the technologies and the alias-text order from the adapter: the generated enum is in neither order and the generator emits no table for them; replace by generated tables when the generator grows them |
| B7-design-4 | the planet option of `XxchaPlanet` reuses B9's `agenda::Shown` (equal-always system payload) so a parsed answer equals the offered option; no copy of the type |
| B7-design-5 | `GameState: PartialEq` ignores the board; code that must see a board change (e.g. the `jolnar_hero` guard) uses `GameState::identical` |
| B7-design-6 | the lockstep tests assert the core half ran (probes call the `leaders_core` function and require `Some`), since a bail is silent |

## B1g combat composition: bails, copied behaviour and design lines

Branch `core-b1-combat`. Ported: the composed space-combat `Machine` (`rules/combat/machine.rs`), compared as one window at `CombatWindow::settle` (`combat::settle`), and the core versions of the pending questions with their answers (`Machine::awaiting`, compared at `CombatWindow::resolve` as `combat::answer`), all behind `driver=dual`.

| id | note |
|----|------|
| B1g-1 | bail: the composed settle stays string for Naaz / Letnev / diplomacy round openings, start-of-combat hit modules (Assault Cannon, Mentak Ambush, Dimensional Splicer, Impulse Core), Thundarian / Keleres roll gates, retreats of flipping units, Voltron, Floating Factory, Last Bastion, Dark Energy Tap (`combat.rs` `settle_args`; port the faction hooks and the ability table) |
| B1g-2 | bail: the Machine bails on cancelled hits, Duranium Armor with damaged ships, Devotion round-end hits, staged destructions, a roll that ends the fight, a Ralnel flagship arriving, announce-end flips / blockaded docks / mark changes (`machine.rs` `Bail`); a single casualty candidate is no longer one (`Machine::lone_casualty`) |
| B1g-3 | bail: a barrage with reactions and every reaction window in a settle is string-only (core ability table empty; `Recording.nested`) |
| B1g-4 | copied string bug: the `heart_ixth` option-id split (`hfix|e:d:sign` is split on `|` and wants four parts, so no pick applies and the card is never exhausted); fixing it needs a RULES_VERSION decision |
| B1g-5 | design: the pending questions (announce a retreat, retreat to which system, sustain, assign) were string-only and are now the core machine awaiting its answer (`combat::answer`); the settle gates of `settle_args` are unchanged by it |
| B1g-6 | design: under the outer `combat::settle` comparison the inner windows run bare, so per-window counts show only for gated positions and for unit tests with the composition off (`dual_window::with_composed`, default off under `cfg(test)`) |
| B1g-7 | design: roll group texts are pre-interned as all subsets of the unit types present (at most 10 types, more bail) because the core cannot intern; replace by a core text table at D2 |
| B1g-8 | bail: the answer to a pending question stays string for a hit the producer assigns (Devotion), a retreat the core does not move (`retreat_unported`), a sustain below `RULES_FACTION_BUGFIXES`, a commander held another way than as an unlocked leader, neutral sides, reaction activity (`dual_window::answer_machine`, `Machine::pending_choice`) |
| B1g-9 | design: `Ask::CancelHit.window` marks the sustain question of the combat window itself (space sustainers named `sustain|3`, `ShipsInSystem` previews) against the absorb path's (`sustain|space:3`, no previews): two string spellings of one core question; unify when the string absorb path is ported |
| B1g-10 | design: option previews are `serde(skip)`, so `compare_choice` never sees them; the answering mode of `settle_run` compares them itself (`combat::answer.previews`). Other windows' previews are still uncompared (the core renders none for them) |
| B1g-11 | design: the assign question's option ids index the owner's whole ship list (`casualty_candidates`), not the filtered list the `AssignHits` window of `assign_remaining` uses; the two differ when a hit is bound to non-fighters |

## B5 bails, copied behaviour and design lines (explore/relics, branch `core-b5-explore-relics`)

Ported in the mutation half: `rules::explore` (draw, fragment gain, purge for a relic, relic gain/exhaust/purge, attachment, frontier and gamma tokens, ion storm token, Crown point; second slice: the log row open/close, commodities gained and spent, the Enigmatic Device play-area card, the Maw of Worlds exhaust, the Book of Latvinia, the Silver Flame after the roll, the status-phase relic and legendary readying, legendary exhaust, a unit removed from a planet), `rules::explore_windows` (deck choice, Distant Suns, Ion Storm, reward questions, Dominus Orb offer, Crown choice, the mech-or-infantry payment), `rules::relic_windows` (Neuraloop, Codex, Titan Prototype, Stellar Converter, technology of the Enigmatic Device and the Maw), `query::explore::holdings`. Adapter: `explore_write.rs`; render: `render/explore.rs`, `render/relic.rs`. Lockstep tests: `explore_write::tests`, `exploration::window_tests`, `relics::relic_window_tests`, `legendary::exhaust_tests`.

| id | note |
|----|------|
| B5-bail-01 | writes (`explore_write`) bail to the string write on: a deck reserve (turn redo draws through `deck_reserve`), more than 16 owners, a trait/deck/attachment/relic/planet/system id the tables do not know, a value the codecs refuse |
| B5-bail-02 | windows bail when the question is asked inside a space battle (the string context carries `space_battle` and a battle target), with diplomacy on, inside a nested emission, or when the string side got no answer yet (unwound). Inside an invasion they are compared since the second slice: the table adds the invasion's sequence number to every context without another target, and the windows carry it (`InInvasion`, test `a_question_inside_an_invasion_is_compared_with_its_sequence_number`). Golden dual: 20 games on dual, 5285020 calls, 0 differences, 11 window call sites compared (crown_of_emphidia_choose 1665, dominus_orb 1472, pay_with_mech_or_infantry 114, the five reward questions 39-52 each, distant_suns 16, neuraloop 5, titan_prototype 3; ask_technology 2 compared, 7 bailed: the agenda sources that are no relic) |
| B5-bail-03 | the reward questions of the instant cards (Abandoned Warehouses, Merchant Station, Functioning Base, Local Fabricators, Mercenary Outfit) are core windows, but their option list (it depends on holdings and on rules-version gates) and the effect behind the answer (commodities, trade goods, action card, mech/infantry placement, production) stay string; the Distant Suns hook (`explore_extra_draw`) that decides the window applies is string |
| B5-bail-04 | still string after the second slice: the text of the log row (outcome, result, gains, placed: `Holdings::describe_change`; the core stores the interned codes it is given, the core `Holdings` read is compared against the string struct), the effects `resolve_instant` runs through other areas (command tokens: the pool question is B8's `strategy_cards::gain_tokens`; `mirage`: `planets::place`; `frln*`: production; `dv*`: secret draw; `lc*`: action-card draw; `dw` goes through the core `relic_gain` but the card dispatch is string; `gw`/`gamma` token is core), `demilitarize`, the `explore_frontier` flow and `frontier_systems`, `place_on_planet` (Mercenary Outfit, Local Fabricators: unit placement through `supply` and faction hooks), `use_relic` itself (the dispatch and the purge after), the Crown's exploring after the choice (the draw, the deck window, the record and the rewards are core, the glue is string), Heart of Ixth exhaust (`combat.rs`: the string bug that must stay copied). Test `the_effects_that_stay_string_run_alone_under_dual` runs the string-only cards under `Dual` and asserts nothing differs |
| B5-bail-05 | invasion `finish_control_gain` chain (B2-K5): only split into named steps (`control_gain_fracture_relic`, `control_gain_exploration`); the hooks (Assimilate, `control_gained` emissions, technology AFTER windows, Infiltrate) are string. The exploration step ends in `choose_deck` (core window) and `explore_with` (string) |
| B5-copy-01 | `purge_for_relic` logs the relic with 0 victory points even if it is the Shard (only `relics::gain` scores it); core copies this |
| B5-copy-02 | `purge_for_relic` for the FRONTIER trait takes matching and shortfall from the same pile and can leave a negative count; core copies it with signed counts (`purgeable` never offers FRONTIER) |
| B5-copy-03 | the Shard / Crown point ledger row carries the applied amount (0 at the victory target) while `relic_log` records 1 point; core copies both |
| B5-copy-04 | the Crown of Emphidia offers every controlled planet with a printed type; every planet has one (`Mr`, `Fake`, `Faction`, `Spacestation` included), so Mecatol and station planets are offered; the core filter `!traits.is_empty()` is an equivalent mutant of the string filter |
| B5-design-01 | `dual::write` (dual.rs) is the shared `dual_write`; B7's local `leaders::dual_write` delegates to it |
| B5-design-02 | `choice::generic::Content::Step` is new: the "card" of a string exploration question that is no card (`exploration`, `ion_storm`, ...). Strings live in `render/explore.rs`; the foundation steward should confirm the variant |
| B5-design-03 | `core_sync::conv` is `pub(crate)` now (text<->`ExploreType` for the adapter and the renderer) |
| B5-design-04 | `exploration::choose_deck` takes `&GameState`; on the Dual backend the windowed ask runs on a clone of the state (no clone on the other backends) |
| B5-design-05 | `explore_write::lend` bails on `gs.deck_reserve` itself: the Decks slice codec does not look at it (the Cards codec does) |
| B5-bail-06 | the relic questions are windows up to the answer: Neuraloop (the pool check, the roll in its own rng domain and the purge stay string), Titan Prototype (the payment, the structure and the trade good instead), Stellar Converter (the target list comes from the string side: map adjacency and the unit catalogue; the choice and the destruction are core), the technology of the Enigmatic Device and the Maw (the option list is the string's: held, faction, researchable; the research or gain behind it is string; a source that is no relic, e.g. an agenda's research, bails: `typed::<Relic>` is `None`); Codex is core end to end. Silver Flame: the roll is string (dice outside the state), everything after it is core |
| B5-bail-07 | `exploration::pay_with_mech_or_infantry` bails for a reward text that is none of the three cards' (`gain 1 trade good`, `ready this planet`, `gain 1 command token`) and for a card or planet the tables do not know; the offer decoration is built by the one string function `pay_offer` for both sides |
| B5-bail-08 | `lend_texts` (log row writes) bails like `lend`: a deck reserve, more than 16 owners, an id the tables do not know (a custom exploration card, deck or planet) |
| B5-copy-05 | `Holdings` counts the seat's own mechs on the planet by the unit-type id containing `mech`; the Naaz-Rokha mech `naaz_voltron` has no such id, so a Voltron placed on the planet is not counted ("placed a mech" is missing from the record). Core copies it: `Unit::EidolonMaximum` is excluded in `query::explore::holdings` |
| B5-copy-06 | the `set_record_system` write of `explore_frontier` is redundant (the row is opened with the same system); the mutant that drops it survives on purpose |
| B5-design-07 | `containers::Log::get_mut` is new (path copy of the shared chunks): the exploration row is closed after it was opened and a card in between can open another row |
| B5-design-08 | the Codex option's preview carries the cards held in `Shown::votes` (`Shown` compares equal, so the parsed answer equals the offered option) |
| B5-design-09 | `relics::ask_technology` takes `&GameState`; on the Dual backend the windowed question runs on a clone of the state (no clone on the other backends, as `choose_deck`) |
| B5-design-10 | `status::resolve_after_token_gain` readies the technologies in the string loop and the relics and legendary cards through the core write; the technology readying is B8's |
| B5-design-11 | `explore_write::lend_texts` interns the texts of a log row before the lend (`Ctx.texts` is shared between the lend and the codec) |
| B5-design-06 | tests need real faction ids (see B6-design-02) and a `ti4-engine/tests/content_ids_resolve.rs`-clean spelling of unknown ids (build them, do not write them as literals) |

## E. Process / ops notes

- E1: nightly cron disabled (`#SKIPPED-2026-10-27`); restore with `crontab /root/crontab.backup-2026-10-27`.
- E2: cron `NIGHTLY_START=23:30` disagrees with CLAUDE.md (20:30 Berlin); reconcile when restoring.
- E3: alpha-server feedback recovery after the disk-full incident is unconfirmed.
- E4: agents must delete their private target dirs when stopping.
- E5: machine contention with 4 agents running; stagger builds.
- E6: six `torch-sys` crates cannot build here.
- E7: do not touch the live branch `nightly-fixes-2026-10-26` from port work.
- E8: the live branch has 8 commits not in `core-state-main`; conflicts expected only in `ti4-host/src/view/scenario/mod.rs`, `ti4-server/src/session/{mod,registry,worker}.rs`, `ti4-server/tests/it/main.rs`.
