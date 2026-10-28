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

## C. Documented bails per package

A bail = the core declines and hands the decision back to the string engine. Seed list; counts where
given are from the package reports.

- **Combat (B1)**: readied The Thundarian (core ability table empty); Keleres agent paying for Munitions/Wrath; neutral sides; `begin_round` Naaz/Letnev/diplomacy (278 of 550 calls); retreat window (Eidolon/Voltron, Floating Factory, Last Bastion seat, neutrals); composed `CombatWindow` (in progress, B1g).
- **Invasion (B2)**: nested reaction windows in `GroundRoundHits` (82/361); Begin/Tail/Advance bails; Harrow and `commit_deploy`/Keleres toll; zero-coverage Advance bails.
- **Production (B3)**: Bastion and Crimson declined for typed reads; Argent/Yssaril/Nomad cross-seat guard; Naaz mech staging; empty tier-W offers (Keleres/Winnu agent, Hacan hero, payment-window agent, Argent agent redirect); `sling_relay`/`integrated_economy`; supply mutations; `ProductionCounters`/timer split.
- **Agenda (B9)**: ~28 effects with no core window (event staging, questions, secret deck draw, Regulations Mahact token count, Crisis version-gated); reveal/resolution windows; `laws::offer_discard`.
- **Action cards (B6)**: 12 cards' legal-target checks bail; rest of B6 not ported.
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

## E. Process / ops notes

- E1: nightly cron disabled (`#SKIPPED-2026-10-27`); restore with `crontab /root/crontab.backup-2026-10-27`.
- E2: cron `NIGHTLY_START=23:30` disagrees with CLAUDE.md (20:30 Berlin); reconcile when restoring.
- E3: alpha-server feedback recovery after the disk-full incident is unconfirmed.
- E4: agents must delete their private target dirs when stopping.
- E5: machine contention with 4 agents running; stagger builds.
- E6: six `torch-sys` crates cannot build here.
- E7: do not touch the live branch `nightly-fixes-2026-10-26` from port work.
- E8: the live branch has 8 commits not in `core-state-main`; conflicts expected only in `ti4-host/src/view/scenario/mod.rs`, `ti4-server/src/session/{mod,registry,worker}.rs`, `ti4-server/tests/it/main.rs`.
