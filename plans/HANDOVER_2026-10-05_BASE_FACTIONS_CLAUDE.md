# Claude handover: base factions, October 5, 2026

## Resume contract

Finish the entire twelve-base-faction package, including runtime/simulator/training integration. This plan is **not complete**. Runtime default seating still supports the original six. Four asset rows remain unclaimed: `naaz_voltron`, `naazbt`, `yinbt`, `yssarilagent`. Implemented shared-tree code, passing tests, ledger acceptance, independently reviewed focused commits, and integration completion are distinct states.

Actual repository: `D:\Projects\ti4-engine-rs`, branch `wp/base-factions`. The Codex app's C: workspace is not the active checkout. Always specify D: explicitly. HEAD at handoff: d159edd82d28ba3b2ce73dfc7b17f37775948eb9. Index empty. No Cargo, rustc, soak, or diagnostic process was found running at the handoff process check. All three latest Luna tasks have finished source writes; no pending agent-owned edits remain.

Latest user limits: **subagents only up to Luna**. Do not spawn Terra/frontier implementers or reviewers under this session's authority. Critical independent review must be performed by the coordinating capable model against another implementer's patch; do not call Luna review frontier acceptance. User wants autonomous completion and permits all 16 physical cores. User now specifically requested this written handover.

User approved both original-six behavior changes: emitting movement events for ships without cargo and activation events for strategy-card free tactical actions, plus preparing the simulator rebaseline for review. Do not ask again for those two corrections. The separate Yin/integration scope-exception question remains unanswered: do not invent approval. Crimson and implementations of excluded external faction commanders are out of scope. Yin's canonical unused-commander pool must not be narrowed to existing handlers merely to obtain a green ledger. Preserve unsupported acquired effects as an explicit limitation; reconcile acceptance with the operator before claiming completion based on that exception.

No new folders/worktrees, deletion, destructive reset/checkout, history rewrite, publication, or unrelated process termination. Preserve all unrelated shared income, policy, training, review/UI experiments. Earlier `/tmp/new_section.rs` deletion mentioned in historical handover has not been performed by this continuation; do not infer a general deletion authorization.

## Read first

Follow AGENTS.md's full required reading order: AGENTS.md; plans/SCOPED_PERMISSIONS.md; plans/EXECUTION_STATE.md; plans/MASTER_PLAN.md; plans/PI_WORK_PACKAGE_STANDARD.md; plans/INDEX.md and BASE_FACTIONS_PLAN_2026-10-02.md; current package and latest completed evidence; status/log. This handover supersedes stale active-process notes lower in EXECUTION_STATE; verify repository state first.

Especially read:

- `plans/evidence/BF-CASUALTY-STAGING.md`: corrected seven-file atomic migration map.
- `plans/evidence/BF-INTEGRATION-READINESS-OCT4.md`: roster/schema/historical-artifact risks.
- `plans/evidence/BF-ORIGINAL-SIX-DIAGNOSTIC-2026-10-05.md`: actual old/new metrics and limits.
- `plans/evidence/BF-SOAK-PARALLEL.md`: new runner, not yet compiled or executed.
- `plans/evidence/BF21a-SEEDED-ASSIGNMENT.md`: standalone preparation, not runtime widening.
- `plans/evidence/BF-MANUAL-CHOICE-ACCEPTANCE.md`, BF-CHOICE-PRESENTATION.md.
- Naaz placement/combat/direct-hit/survivor/sustain variants, Ssruu round/nested errors, Yin BT/provenance evidence; faction evidence ledgers.

Normative rules and pinned content are those named in each package. No historical Python inspection or new parity claim occurred in this continuation. Treat legacy written claims as evidence for their historical checkpoint only.

## Latest focused commits

- `c42c8638` BF-POLICY-CAMPAIGN-PARALLEL: policy bot test-only campaign retains every 51 pair / 102 game case, bounded workers, canonical result coverage and assertions.
- `d159edd8` BF-SIM-DIAGNOSTIC-PARALLEL: bounded seed workers, visible panics, complete diagnostic accounting. Later summary replay changes to its example/evidence are still uncommitted.
- Earlier focused foundation/timing commits: `8196c7b7` Resolver checkpoint; `d0ed0b4a` Sardakk copied tactical end; `96ae2aad` Muaat copied Umbat; `ac296cbc` Mentak copied Pillage.

Do not repeat a blanket behavior-neutral claim for the current dirty tree or treat shared-tree tests as an isolated committed-tree build. Historical `cadbbe2d` and `920c852c` review followups are in BF-CODEX-INHERITED-REVIEW-2026-10-04.md and associated evidence.

## Verified checks at this checkpoint

All logs below are ignored local files in `target/`; do not commit them. No full-workspace exit gate has completed at this checkpoint.

| Check | Exact result | Log |
|---|---|---|
| Full engine/policy/sim shared-tree suite | engine 2103 passed, 1 ignored; policy 273 passed (56.22s); sim 51 passed, 1 ignored; integrations 1/1, 4/4, 5/5 and doctests pass; exit 0 | bf-oct5-closure-green.log |
| Earlier affected model/policy/sim/bridge/content/legacy suite | model83; policy273; sim51+1ignored; bridge62+6+12+3; content134; legacy25; integrations/doctests pass; exit0 | bf-oct5-affected.log |
| Engine/policy/sim all-target clippy | exit0 with inherited/shared warnings, not warning-clean | bf-oct5-closure-clippy.log |
| Final sim test and focused sim/policy clippy | sim51+1ignored; lint exit0 after fixing new panic-doc/mapflatten findings | bf-oct5-sim-final.log; bf-oct5-focused-clippy.log |
| Focused parallel policy campaign | 1passed,272filtered;53.34s | bf-oct5-policy-parallel.log |
| Two real Ssruu manual callback tests | 2passed,9filtered | bf-oct5-manual-callback.log |
| Full replayer live integration test target | 11passed,0failed;2.60s;exit0 | bf-oct5-live-all.log |
| BF21a seating helper suite | 41passed,2067filtered;0.03s;exit0 **before subsequent root oracle edit** | bf-oct5-seating-helper.log |
| Original-six diagnostic with per-seed repeat | all30 seeds completed,error-free and reproduced every deterministic GameResult summary field;exit0 | bf-oct5-original-six-replay-diagnostic.log |

Full engine count above predates BF21a's four added tests. Expected next engine total2108 (2107passed+1ignored), but this has **not been measured**. The BF21a ordering-test oracle was changed after the 41-pass run; rerun it. New soak source has only rustfmt/whitespace evidence, **no build/smoke/full soak**.

No controlled speedup claim: old serial policy tail470.21s versus new53.34s is an observation across different runs, not a matched benchmark protocol.

Original-six diagnostic: all30 fixed seeds812001..812030 completed. Five event-share metrics are outside recorded intervals: invasion, production, ship movement, system activation, tactical-action begin. Bounds untouched. Repeat checks compare every GameResult summary field except wall seconds; this is not full cross-version decision/state trace equivalence. Approved no-cargo movement affects counts/denominators but does not establish full causal attribution, particularly with unrelated income WIP present. The diagnostic report contains the ten-metric table and raw-log SHA256.

## Source-ready work and ownership

All latest workers were Luna, ran no Cargo, created no commits, and have finished.

**BF-SOAK-PARALLEL** (`base_faction_soak.rs`, own evidence): dynamic mutex job cursor, max16 scoped workers, independent faction/seed state, paired replay sequential within each case. Preserves full serialized state/events/DecisionRecords equality, then drops large replay objects. Compact counters/failure messages only. Catches per-case panics, exposes worker panics, verifies each faction and total completion, sorts final failures by faction/seed, reports progress. Root flagged literal PowerShell newline text, mutex shadowing, late omission errors, per-faction accounting and monotonic progress; agent reports fixes. Independently inspect latest source, particularly progress locking, before building.

Important: default 12x200 means2400 substituted-seat cases,4800 actual game runs. Each pair repeats the **same substituted assignment**. This is replay determinism, not native-six-vs-substituted differential and not BF25 eighteen-faction all-pairs coverage. CLI: `base_faction_soak naaz 0 4` for smoke; no args for full twelve-by200. No smoke or full run yet. Run a small slice, estimate bounded time/memory, then run the authorized full campaign autonomously; do not rename it all-pairs acceptance.

**BF21a seeded assignment** (`seating.rs`, evidence): public standalone helper taking explicit ordered candidate aliases, ordered player IDs, seed; validates empty/duplicate candidates, duplicate players, insufficient candidates. Dedicated local GameRng domain `seating:faction-assignment:v1`; unique shuffle assignment, no modulo reuse. Eighteen corpus aliases tested, current IN_SCOPE_FACTIONS and seat_in_scope unchanged. No caller wiring. Root replaced an implementation-mirroring shuffle oracle with actual player-order and two-candidate roster reversal properties; rerun tests/lint before committing. This file was clean before the helper, so scoped whole-file staging is reasonable after review/gates. BF21b runtime wiring still gated by BF20 acceptance.

**Manual callback acceptance** (appended `replayer/tests/live.rs`, evidence): public Gate/ControlledDecider/Table/resolver threads genuinely park/submit/resume. Copied Letnev selected exact second cruiser, grants only it one die, exhausts only Ssruu; copied L1Z selects the second of two content planets and replaces the beneficiary infantry with sol_mech. Failure RAII shutdown releases parked workers. Both and full live11 pass. Does not exercise full LiveBranch/Game tactical driver or establish ground manual coverage. Root removed new unused import. Evidence honestly records first test preceded spec. **Do not whole-file stage live.rs:** preexisting `lineup: None` hunk belongs to another session. Tests depend on uncommitted borrowed modules/shared activation; don't commit ahead of dependencies.

## Important uncommitted implementation clusters

- `borrowed_round_agents.rs`, module/reaction wiring: copied Sol/Letnev exact unit, owner/type/location/roster ordinal/damage/galvanize/round markers. Real space/ground round producers and dice readers; actual Game retry tests. No-copy original-six event path retained. Ground strict errors propagate before dice.
- Game aftermath wrapper/inner and endturn transaction: state/dice/RNG/resolver sequence/log/windows/tactical rollback on BF failure, consumed decider input retained. Registry points to actual ask in `step_aftermath_inner`. Table failure counter exposes errors swallowed by legacy Option-based nested exploration. Same-Table actual nested exploration/retry tests pass. All Game hunks must be separated from unrelated income experiment.
- Naaz combat/placement: Maximum planet participation, unit-ability hit immunity, context/repair/retreat/captured return normalization; exact survivor/sustain variants and selected Direct Hit marker. Immediate/windowed fresh plain/galvanized Max variants have distinct stable IDs, identical-value dedup, human labels, exact mutation. Three new fixtures initially used POK for TE Max, corrected to DEFAULT while preserving assertions. Shared full gate green, ledger closure still pending.
- Naaz placement hook covers exploration/Hope's End/Refit/native and copied L1Z/TE Xxcha hero/Rearmament/production paths. Refit/Rearmament actual Naaz cap/identity corrected; unrelated generic non-Naaz identity/cap behavior intentionally preserved for original-six compatibility. Do not expand that fix casually.
- Destruction cause + independent during-space-combat flag; legacy decoding conservative; strict BF staged drain retry, exact Direct Hit/Courageous planet target. See seven-file staging map below.
- Yin own breakthrough gain/public-objective trigger: canonical unused-commander sampling, seeded stream, acquired/seated exclusion, real gain and score tests. Canonical pool keeps unsupported external factions; excludes duplicate faction representations appropriately. Acquisition exists; unsupported commander effects prevent an unqualified end-to-end claim. No Crimson handler added.
- BF choice presentation: copied Naaz/L1Z content planet names; BF ground damaged/undamaged casualty labels and visible sustain location. Stable IDs/order/payload retained; no-copy original-six ground text preserved. Manual tests validate a subset only.
- Older Alliance/borrowed commander and production work remains uncommitted, including Nomad/Cabal/Titans. Preserve it; scope correction does not authorize deletion or claim it as necessary accepted BF work.

## Scoped casualty migration commit map

BF-CASUALTY-STAGING records a necessary **seven-file** atomic-size exception, not the entire inherited wave:

1. model/state.rs exact tuple fields/legacy decoders/current+old fixtures; exclude initiative formatting.
2. combat.rs destruction API/cause/timing payloads/emitter/drain and sustain tuple adaptation; narrow provenance fixtures. Exact Max target/menu and copied round dice separately.
3. action_cards.rs tuple readers/writers/Direct Hit/Courageous provenance; keep Refit/Rider/Max placement and exact marker/remover separate.
4. reactions.rs exact pending payload/drain; copied factory registration separately.
5. muaat.rs Nova Seed cause adapter and arity fixture.
6. sardakk.rs **both** ExotriremeII cause/context adapters; omitted from the first map, now corrected.
7. yin.rs Van Hauge inherited timing and Brother Milor boolean-filter/provenance fixtures; BT/Alliance/copy separately.

New pending tuple5: system,owner,type,cause,during_space_combat. Old3 unknown/false; transitional4 preserves cause, true only old space_combat sentinel. last_sustain old4 ->5 adds conservative timingfalse. Source cause is distinct from when destruction occurred.

Game strict Result drain is an optional separate eighth-file error-atomicity package; invasion has no direct tuple reference. HEAD already has supply::staging_enabled. New payload/API helpers are migration dependencies; exact Max target marker/removal helpers are separate. Select precise hunks and inspect staged blobs/dependencies; don't stage35 engine files as one wave. No migration commit or isolated staged-tree build yet.

## Remaining acceptance and integration queue

1. Review/build/smoke new soak runner, run full12x200 and preserve failures. Rerun modified BF21a tests/lint; focused preparation commit. Review/commit example replay diagnostic plus report without changing bounds.
2. Reconcile Naaz voltron/BT full route acceptance and Ssruu all-seated-agent/action/round/nested/error/manual census. Review stale claimed rows where new routes changed. Close ledger only from actual end-to-end evidence.
3. Resolve Yin unsupported-acquired-effect scope limitation with explicit operator authority; do not silently treat it approved or implement excluded handlers.
4. Independently review and commit compile-coherent shared packages in dependency order (schema/provenance, strict delivery/transactions, borrowed round wiring, exact Max target/placement, Yin acquisition, presentation/manual fixtures). Preserve all unrelated hunks. Reconcile original-six differences separately from shared-tree aggregates.
5. BF20 widen default supported roster to18 only after prerequisite acceptance/approved exceptions. BF21b wire explicit deterministic selection into sim/training games, preserve historic fixed-six authored corpus/checkpoints.
6. BF22 policy roster compatibility/versioning: battle FACTIONS is alias-to-numeric shift rather than simply six one-hot columns. Existing v1..7 predictors consulting a widened global roster risk semantic relabeling with identical weight widths. Explicit support/version gates or checked migration required. Training transient arena six-entry catalogue separate. Profile schemas2..5, archive schema1, vocabularyv10/fingerprints are separate contracts; do not global-search-replace faction constants.
7. BF23 eighteen-faction versioned simulator baseline, original-six comparison retained; no unreviewed bounds adjustment. Existing approved original-six diagnostic is preparation, not this exit gate.
8. BF24 presentations and real manual path checks per new decision kind. Callback11 proves two windows only; ground/full driver and remaining new decisions still require census acceptance.
9. BF25 full workspace tests, actual eighteen-faction all-pairs seated soak, zero twelve-faction registry gaps or explicit approved exceptions, independent exit review, reconciled ledgers and milestone report. Do not substitute the twelve one-seat soak for this gate.

## Exact next commands and resource discipline

Only one coordinator runs Cargo against the shared target. Compile4 for memory headroom; tests16 and seed/case workers up to16. CPU-bound campaigns expected to take >=2minutes must be reasonably parallelized at the actual independent-work layer. Preserve full logs and native exit status; no grep/head filters. Do not terminate strata or any unrelated process. User freed memory and 4-job builds succeeded; apparent free RAM does not prove Windows commit headroom.

PowerShell, cwd D:\Projects\ti4-engine-rs:

```powershell
$env:CARGO_PROFILE_TEST_DEBUG='0'
$env:CARGO_PROFILE_TEST_INCREMENTAL='false'
$env:CARGO_PROFILE_TEST_CODEGEN_UNITS='4'
Remove-Item Env:LIBTORCH -ErrorAction SilentlyContinue
cargo test -p ti4-engine --lib seating::tests -q -j4 -- --test-threads=16 2>&1 | Tee-Object -FilePath target/bf-claude-seating-recheck.log
$bfExit=$LASTEXITCODE
if ($bfExit -ne 0) { exit $bfExit }
$env:CARGO_PROFILE_DEV_DEBUG='0'
$env:CARGO_PROFILE_DEV_INCREMENTAL='false'
$env:CARGO_PROFILE_DEV_CODEGEN_UNITS='4'
cargo run -p ti4-sim --example base_faction_soak -q -j4 -- naaz 0 4 2>&1 | Tee-Object -FilePath target/bf-claude-soak-smoke.log
$bfExit=$LASTEXITCODE
exit $bfExit
```

Review soak source first, including progress synchronization and omission detection; command is not an acceptance claim until executed. Expand sample if runtime/memory estimates need it. Full unchanged campaign command: `cargo run -p ti4-sim --example base_faction_soak -q -j4` (no args), max16 caseworkers, bounded monitoring/cancellation and full log.

Replayer/review torch builds require absolute pin:

```powershell
$env:LIBTORCH='D:\Projects\ti4-engine-rs\out\libtorch-2.9.1-cpu'
$env:PATH='D:\Projects\ti4-engine-rs\out\libtorch-2.9.1-cpu\lib;'+$env:PATH
cargo test -p ti4-replayer --test live -q -j4 -- --test-threads=16
```

Use `rg --threads 1` first for source search. Python C:\Python314\python.exe, explicit UTF8 reads/writes and LF; default Windows encoding previously caused mojibake. Avoid injecting literal PowerShell backtick newline tokens into Rust. Preserve ongoing native session IDs and inspect actual completion; don't lose them by printing only output. Treat historical semantic_golden failure as recorded noise only where proven identical; don't ignore new unexplained failures or rewrite goldens to pass.

## Compact AGENTS-format checkpoint

Objective: Finish twelve BF factions and eighteen-faction integration.
Normative source versions: Current pinned rules/content and each package evidence; no Python comparison this continuation.
Active package: BF-SOAK-PARALLEL source-ready; BF21a prep rerun pending; shared casualty/asset closure outstanding.
Status: Two bounded-runner commits accepted; full shared engine/policy/sim green before helper; live11 and original-six30 summary repeats pass; plan incomplete.
Current branch and HEAD: wp/base-factions / d159edd82d28ba3b2ce73dfc7b17f37775948eb9.
Working-tree state: Intentionally dirty shared work; index empty; full snapshot below.
Tests: Table above, including post-test edit caveat.
Compatibility evidence: Original-six default remains six, approved event corrections diagnostic prepared; no aggregate parity claim.
Decisions: Luna ceiling; one Cargo coordinator; bounded16 independent workers/compile4; external commander scope excluded; canonical Yin pool retained.
Open findings/blockers: Four asset acceptances; Yin exception unanswered; scoped commits; manual census; BF20..25 gates; soak not run.
Next exact action: Independently inspect soak, rerun seating helper, compile/smoke soak with commands above.
Files to read first: Required reading order plus listed active evidence.

## Full dirty-path snapshot

This snapshot lists every dirty path at handoff. Source ownership must be established from exact diffs, not filename alone. Engine/model faction and shared timing paths contain BF work mixed with earlier experiments; Game specifically contains unrelated round income. MLP/policy feature/critic/progress/power files, training/scripts, review/replayer UI/projects/store and their earlier tests, target-cuda-repack, and pi_rpc_bridge are shared unrelated work unless a named BF package explicitly owns a hunk. Preserve all. The new manual tests occupy only appended live.rs portions; helper and soak ownership is stated above. Plans/evidence include both earlier handovers and current BF documentation; AGENTS changes include operator parallelism rule. Do not use this snapshot as blanket staging authority.

```text
 M AGENTS.md
 M crates/ti4-engine/src/action_cards.rs
 M crates/ti4-engine/src/agenda_effects.rs
 M crates/ti4-engine/src/choice.rs
 M crates/ti4-engine/src/combat.rs
 M crates/ti4-engine/src/exploration.rs
 M crates/ti4-engine/src/factions/arborec.rs
 M crates/ti4-engine/src/factions/argent.rs
 M crates/ti4-engine/src/factions/ghost.rs
 M crates/ti4-engine/src/factions/hooks_cards.rs
 M crates/ti4-engine/src/factions/hooks_economy.rs
 M crates/ti4-engine/src/factions/hooks_ground.rs
 M crates/ti4-engine/src/factions/mentak.rs
 M crates/ti4-engine/src/factions/mod.rs
 M crates/ti4-engine/src/factions/muaat.rs
 M crates/ti4-engine/src/factions/naalu.rs
 M crates/ti4-engine/src/factions/naaz.rs
 M crates/ti4-engine/src/factions/saar.rs
 M crates/ti4-engine/src/factions/sardakk.rs
 M crates/ti4-engine/src/factions/winnu.rs
 M crates/ti4-engine/src/factions/yin.rs
 M crates/ti4-engine/src/factions/yssaril.rs
 M crates/ti4-engine/src/game.rs
 M crates/ti4-engine/src/invasion.rs
 M crates/ti4-engine/src/leaders.rs
 M crates/ti4-engine/src/legendary.rs
 M crates/ti4-engine/src/objectives.rs
 M crates/ti4-engine/src/production.rs
 M crates/ti4-engine/src/promissory.rs
 M crates/ti4-engine/src/reactions.rs
 M crates/ti4-engine/src/seating.rs
 M crates/ti4-engine/src/strategy_cards.rs
 M crates/ti4-engine/src/supply.rs
 M crates/ti4-engine/src/tokens.rs
 M crates/ti4-engine/tests/content_ids_resolve.rs
 M crates/ti4-engine/tests/decision_delivery_inventory.rs
 M crates/ti4-mlp/examples/capture_offline_pilot.rs
 M crates/ti4-mlp/examples/clearance_eval.rs
 M crates/ti4-mlp/examples/offline_bc.rs
 M crates/ti4-mlp/examples/ppo_update.rs
 M crates/ti4-mlp/src/bot.rs
 M crates/ti4-mlp/src/lib.rs
 M crates/ti4-model/src/state.rs
 M crates/ti4-policy/src/critic.rs
 M crates/ti4-policy/src/features.rs
 M crates/ti4-policy/src/lib.rs
 M crates/ti4-policy/src/progress.rs
 M crates/ti4-replayer/src/gui.rs
 M crates/ti4-replayer/src/project.rs
 M crates/ti4-replayer/src/store.rs
 M crates/ti4-replayer/tests/app.rs
 M crates/ti4-replayer/tests/feed.rs
 M crates/ti4-replayer/tests/live.rs
 M crates/ti4-replayer/tests/online.rs
 M crates/ti4-replayer/tests/project.rs
 M crates/ti4-replayer/tests/rebuild.rs
 M crates/ti4-replayer/tests/shell.rs
 M crates/ti4-replayer/tests/table.rs
 M crates/ti4-review/src/gui.rs
 M crates/ti4-review/src/lib.rs
 M crates/ti4-review/src/main.rs
 M crates/ti4-review/tests/board_layout.rs
 M crates/ti4-review/tests/board_view.rs
 M crates/ti4-review/tests/decision_view.rs
 M crates/ti4-review/tests/low_temperature.rs
 M crates/ti4-review/tests/panel_paint.rs
 M crates/ti4-review/tests/presentation.rs
 M crates/ti4-review/tests/semantic_golden.rs
 M crates/ti4-sim/examples/base_faction_soak.rs
 M crates/ti4-sim/examples/rebaseline_behavior.rs
 M crates/ti4-training/src/lib.rs
 M crates/ti4-training/src/reward.rs
 M crates/ti4-training/src/rollout.rs
 M plans/BASE_FACTIONS_PLAN_2026-10-02.md
 M plans/EXECUTION_STATE.md
 M plans/INDEX.md
 M plans/evidence/BF-COMBAT-ORIGIN.md
 M plans/evidence/BF-NAAZ-EIDOLON.md
 M plans/evidence/BF-SIM-DIAGNOSTIC-PARALLEL.md
 M plans/evidence/BF-argent.md
 M plans/evidence/BF-ghost.md
 M plans/evidence/BF-naalu.md
 M plans/evidence/BF-saar.md
 M plans/evidence/BF-winnu.md
 M plans/evidence/BF-yin.md
 M plans/evidence/BF-yssaril.md
 M scripts/ppo_train.ps1
 M scripts/publish_and_train_stopped_corpus.ps1
 M tools/pi_rpc_bridge.py
?? crates/ti4-engine/src/factions/borrowed_commanders.rs
?? crates/ti4-engine/src/factions/borrowed_round_agents.rs
?? crates/ti4-mlp/examples/economy_audit.rs
?? crates/ti4-mlp/examples/economy_probe.rs
?? crates/ti4-mlp/examples/faction_trial.rs
?? crates/ti4-mlp/examples/power_audit.rs
?? crates/ti4-mlp/examples/spend_audit.rs
?? crates/ti4-policy/src/power_facts.rs
?? crates/ti4-policy/src/power_map.rs
?? crates/ti4-review/examples/
?? crates/ti4-review/src/power.rs
?? plans/BF_LOCAL_MODEL_CRIMSON_COMMANDER_WORK_PACKAGE.md
?? plans/BF_LOCAL_MODEL_SSRUU_MUAAT_WORK_PACKAGE.md
?? plans/HANDOVER_2026-10-04_BASE_FACTIONS_PI.md
?? plans/evidence/BF-ALLIANCE-RIGHTS.md
?? plans/evidence/BF-ALLIANCE-ROUTES-A.md
?? plans/evidence/BF-ALLIANCE-ROUTES-B.md
?? plans/evidence/BF-ALLIANCE-ROUTES-C.md
?? plans/evidence/BF-ALLIANCE-ROUTES-WINNU.md
?? plans/evidence/BF-BORROWED-CABAL-PRODUCTION.md
?? plans/evidence/BF-BORROWED-NOMAD-PRODUCTION.md
?? plans/evidence/BF-BORROWED-TITANS-PRODUCTION.md
?? plans/evidence/BF-CASUALTY-STAGING.md
?? plans/evidence/BF-CHOICE-PRESENTATION.md
?? plans/evidence/BF-CODEX-INHERITED-REVIEW-2026-10-04.md
?? plans/evidence/BF-INTEGRATION-READINESS-OCT4.md
?? plans/evidence/BF-LOCAL-MODEL-EVALUATION-2026-10-04.md
?? plans/evidence/BF-MANUAL-CHOICE-ACCEPTANCE.md
?? plans/evidence/BF-NAALU-TOKEN-PLACEMENTS.md
?? plans/evidence/BF-NAAZ-ABILITY-PRODUCTION-REVIEW.md
?? plans/evidence/BF-NAAZ-ABILITY-PRODUCTION.md
?? plans/evidence/BF-NAAZ-COMBAT-COMPLETION.md
?? plans/evidence/BF-NAAZ-DIRECT-HIT-EXACT-TARGET.md
?? plans/evidence/BF-NAAZ-HERO-WARFARE.md
?? plans/evidence/BF-NAAZ-INVASION-ROUTES.md
?? plans/evidence/BF-NAAZ-PLACEMENT-CLOSURE.md
?? plans/evidence/BF-NAAZ-ROUTES-INDEPENDENT-REVIEW.md
?? plans/evidence/BF-NAAZ-SPACE-CANNON-ROUTE.md
?? plans/evidence/BF-NAAZ-SURVIVOR-CHOICE.md
?? plans/evidence/BF-NAAZ-SUSTAIN-VARIANTS.md
?? plans/evidence/BF-ORIGINAL-SIX-DIAGNOSTIC-2026-10-05.md
?? plans/evidence/BF-PI-PROGRESS-2026-10-04.md
?? plans/evidence/BF-PRODUCTION-ENTRY-ERRORS.md
?? plans/evidence/BF-SOAK-PARALLEL.md
?? plans/evidence/BF-SSRUU-ACTION-CENSUS.md
?? plans/evidence/BF-SSRUU-ARGENT-TIMING.md
?? plans/evidence/BF-SSRUU-JOLNAR-RESEARCH.md
?? plans/evidence/BF-SSRUU-L1Z1X-TIMING.md
?? plans/evidence/BF-SSRUU-NAALU-TIMING.md
?? plans/evidence/BF-SSRUU-NAAZ-NESTED-ERRORS.md
?? plans/evidence/BF-SSRUU-NAAZ-TIMING.md
?? plans/evidence/BF-SSRUU-ROUND-AGENTS.md
?? plans/evidence/BF-SSRUU-ROUND-ATOMICITY.md
?? plans/evidence/BF-SSRUU-SAAR-TIMING.md
?? plans/evidence/BF-SSRUU-SEATED-AGENT-CENSUS.md
?? plans/evidence/BF-SSRUU-YIN-TIMING.md
?? plans/evidence/BF-STAGED-EVENT-ERRORS.md
?? plans/evidence/BF-WINNU-HERO-TIMING.md
?? plans/evidence/BF-YIN-ALLIANCE-CENSUS.md
?? plans/evidence/BF-YIN-BT-GRANTS.md
?? plans/evidence/BF-YIN-DESTROYED-PROVENANCE.md
?? plans/evidence/BF-YIN-HERO-TIMING.md
?? plans/evidence/BF21a-SEEDED-ASSIGNMENT.md
?? scripts/long500_control.psd1
?? scripts/long500_projection002_critic.psd1
?? scripts/pilot2_control.psd1
?? scripts/pilot2_projection002_critic.psd1
?? scripts/pilot_control_49232.psd1
?? scripts/pilot_income20status_49232.psd1
?? scripts/pilot_powerfacts_A_fleet.psd1
?? scripts/pilot_powerfacts_B_proj.psd1
?? scripts/pilot_projection002_49232.psd1
?? scripts/pilot_projection002_critic_49232.psd1
?? scripts/resume_tech01_fleet015_114592.psd1
?? scripts/resume_tech01_fleet015_33436.psd1
?? scripts/resume_tech01_fleet015_40132.psd1
?? scripts/resume_tech01_fleet015_49232.psd1
?? scripts/resume_tech01_fleet015_vp2_49232.psd1
?? target-cuda-repack/
```
