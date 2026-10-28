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
- **Others**: B4 offer/diplomacy remainder; B5 mutations; B7/B8/A4 mutation halves (B7 in progress); A4 scoring/reveal not ported; S8 partial (only reaction slots as data).

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
| B2-K5 | open design: the `FinalizingControl`/`finish_control_gain` hook chain, the rest of `finish_committing`, the setup/end hooks and the scoring pauses are still string | B5 exploration, technology and the ability port |
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

## E. Process / ops notes

- E1: nightly cron disabled (`#SKIPPED-2026-10-27`); restore with `crontab /root/crontab.backup-2026-10-27`.
- E2: cron `NIGHTLY_START=23:30` disagrees with CLAUDE.md (20:30 Berlin); reconcile when restoring.
- E3: alpha-server feedback recovery after the disk-full incident is unconfirmed.
- E4: agents must delete their private target dirs when stopping.
- E5: machine contention with 4 agents running; stagger builds.
- E6: six `torch-sys` crates cannot build here.
- E7: do not touch the live branch `nightly-fixes-2026-10-26` from port work.
- E8: the live branch has 8 commits not in `core-state-main`; conflicts expected only in `ti4-host/src/view/scenario/mod.rs`, `ti4-server/src/session/{mod,registry,worker}.rs`, `ti4-server/tests/it/main.rs`.
