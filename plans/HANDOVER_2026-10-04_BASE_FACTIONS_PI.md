# Pi handover: finish the base-factions plan

Date: 2026-10-04 (Europe/Vienna). This handover supersedes the October 3 handover's status and work queue. It does not replace normative card/rules text or acceptance gates.

## Copy this prompt into Pi

Read `plans/HANDOVER_2026-10-04_BASE_FACTIONS_PI.md` and follow its reading order. Take over the remaining base-factions plan on `wp/base-factions`, preserving the shared dirty checkout. Begin with P01, Yin destroyed-ship combat provenance. Execute one bounded package at a time, persist evidence and EXECUTION_STATE at checkpoints, and use your supported auto-compaction between packages. Continue through the dependency-ready queue; do not stop after merely proposing a plan. Do not restart completed packages or infer completion from passing aggregate tests. Use at most eight Cargo build/test workers, one Cargo coordinator. Do not create folders, worktrees, delete files, reset history, push, or stage unrelated changes. No Crimson or external-faction interaction expansion. No subagent above Terra; Luna implementations are preferred if available. Obtain separate tier-C reviews for timing/legality/schema changes; self-review is not acceptance. If that reviewer is unavailable, leave acceptance pending and work on another independent package. Report genuine scope/authority conflicts precisely rather than inventing approval. This prompt authorizes Pi implementation; it does not authorize external publication or bypassing review gates.

## Reading order and current state

1. AGENTS.md, plans/SCOPED_PERMISSIONS.md, plans/EXECUTION_STATE.md (newest BF entry first), plans/MASTER_PLAN.md, plans/PI_WORK_PACKAGE_STANDARD.md.
2. plans/BASE_FACTIONS_PLAN_2026-10-02.md, plans/evidence/BF-INTEGRATION-READINESS-OCT4.md, plans/evidence/BF-SSRUU-SEATED-AGENT-CENSUS.md.
3. The evidence named by the next package below; inspect its actual source and diff before editing. Older evidence often records a pending state later resolved in an appended section.
4. Git status/log, then faction MODULE declarations and the asset ledger tests. Trust current source and measured results over stale historical prose.

Actual checkout used by Codex: `D:\Projects\ti4-engine-rs`. The app displays `C:\Users\Niko\Documents\ChatGPT\ti4-engine-rs`; verify which checkout resolves to the shared repository before writing. Do not create a second checkout to repair a path mismatch.

Branch: `wp/base-factions`; source HEAD at handoff: **ac296cbc**. The index was empty after the focused commits. The working tree remains extensively dirty. A captured status inventory appears at the end; recapture before edits because other sessions may move it.

Recent independently reviewed scoped commits:

| Commit | Scope |
|---|---|
| 409af6f1 | Winnu copied agent production discount |
| 66c723c6 | Ghost copied agent activation wormhole link |
| 8196c7b7 | Resolver checkpoint/restore, including owned decision log |
| d0ed0b4a | Sardakk copied T'ro at precise, retryable tactical end |
| 96ae2aad | Pi Muaat Umbat eligibility plus later atomic error repairs |
| ac296cbc | Mentak native/copied Pillage timing and copied draws |

Earlier cadbbe2d (Naaz immunity) and 920c852c (combat-origin wave) remain in history; read their evidence and BF-CODEX-INHERITED-REVIEW-2026-10-04.md rather than assuming every inherited acceptance issue is resolved.

Last measured FULL SHARED TREE check:

- `cargo test -p ti4-engine -q -j8 -- --test-threads=8`: **2043 library passed, zero failed, one ignored**; integrations 1/1, decision delivery inventory 4/4, other 5/5; doctests passed.
- `cargo clippy -p ti4-engine --lib -q -j8`: exit 0 with existing/shared-WIP warnings. Not warning-clean.
- Tests include uncommitted shared changes. No isolated committed-tree build or complete BF milestone is claimed.
- No Pi, Cargo or Codex worker should be assumed active from old process IDs or agent names. Coordinate with actual active writers before running builds.

## Scope, authority and completion standard

Twelve added factions: Arborec, Argent, Ghost/Creuss, Mentak, Muaat, Naalu, Naaz-Rokha, Saar, Sardakk, Winnu, Yin, Yssaril. Existing six: Sol, Hacan, Letnev, Xxcha, Jol-Nar, L1Z1X. Target seating roster is **6 + 12 = 18**. One sentence in the readiness audit incorrectly says twelve plus ten; do not propagate that arithmetic error. Nekro deferred. Crimson and interactions with Crimson are explicitly excluded.

The user authorized finishing the whole BF plan. They specifically approved both existing-play fixes from the October 3 handover (movement events for ships without cargo; activation events for strategy-card free tactical actions) and preparing a simulator rebaseline. Do not re-ask that authorization. Check the current implementations and actual call sites before accepting them. This is not blanket permission for unrelated original-six behavior changes, checkpoint/schema reinterpretation, or arbitrary golden updates.

Four asset rows remain unclaimed: `naaz_voltron`, `naazbt`, `yinbt`, `yssarilagent`. The reported 143/147 is a working-tree claim count, not reviewed milestone completion. Recompute from the ledger; do not reuse October 3's 92/145. A MODULE entry means implemented end to end with required acceptance, not merely representable in the engine.

For each package: write/update exact spec and P1 permission scope first, specify named source/rule versions, add real route/error tests, implement minimally, format owned files, run focused tests then affected crate/inventory/lints, obtain independent review, fix findings, inspect exact staged diff, commit scoped hunks, update evidence/execution state. Split packages that exceed roughly 1–5 files or 200–500 source lines or cross behavior/schema boundaries. Update this queue as work closes; never silently shrink criteria.

## Dependency-ordered remaining packages

### P01 — Yin copied Milor: reconcile combat provenance, then accept existing work

Read BF-SSRUU-YIN-TIMING.md, yin.rs native/copied ship and ground listeners, combat.rs destruction emitters/queue, invasion.rs ground causes, Muaat Nova Seed and other destroy producers. Existing copied listeners and fixtures now pass. The ship comment assumes SHIP_DESTROYED only means combat, but generic destroy producers now also use it. Model pending_destructions currently stores system/player/unit without an explicit combat cause.

Determine and record every SHIP_DESTROYED producer. Printed Brother Milor requires destruction **during combat**. Add an explicit provenance boundary if necessary; do not label Nova Seed or an ordinary component destruction as combat. Preserve actual combat/Direct Hit/Harrow distinctions and existing reaction consumers. Avoid changing native behavior incidentally; separately specify any necessary native Yin legality correction and its compatibility effect. Ground copy already filters ground_combat/harrow.

Tests: actual combat ship casualty and Direct Hit allowed; non-combat destruction refused; source Readied/Exhausted; locked source/readied change after resolver arming; actor-owned placement and supply cap; only Ssruu exhausts; native/copy simultaneous windows; invalid choice cannot partially spend or corrupt replay. The test-only semantic decider is intentional: resolver declines are pass-local and can be re-offered, while exhausted Scripted falls back to FirstOption. Never fix these fixtures by guessing extra positional declines. Tier C. Start writable scope yin.rs + evidence; separately specify a small shared producer package before broader edits.

### P02 — Jol-Nar copied research agent

Source: strategy_cards.rs::doctor_sucaban/paid_research, leaders.rs modifier registry, printed jolnaragent. Native route currently finds a readied native source and offers its owner the agent choice. Add Ssruu's independent optional right when the exact source is Readied OR Exhausted; caller/borrower, source owner, researcher and infantry owner are distinct. Keep native stable prompt/choice IDs. Use current borrowable_agents contract and notify borrowed_agent_used only on successful borrowed use; create the needed timing/services seam explicitly rather than fabricating RNG/context.

Tests: real paid Technology secondary and other live paid research producers; chosen researcher pays/removes their own infantry; source unaffected; only Ssruu exhausts; native and copied ordering; no infantry/unavailable cards; partial removal/payment error plus same-table retry. Include diplomacy promise accounting where relevant. Scope strategy_cards.rs/owned inline tests + narrowly required shared hook + evidence. Tier C.

### P03 — L1Z1X copied activation conversion

Source: printed l1z1xagent, leaders.rs dispatch, SYSTEM_ACTIVATED registration and production/placement helpers. Implement the copied printed activation window, not a generic ACTION option masquerading as activation. Offer legal infantry/mech replacement targets, count reinforcement availability, use exact event actor/location, recheck dynamic rights, spend only borrower Ssruu. Preserve native original-six behavior unless a separately specified correction is necessary.

Tests: actual Game activation; Readied/Exhausted source; correct removed unit and replacement owner/location; missing infantry or mech supply; decline; stale/illegal selection; state/log/RNG rollback and retry; source unchanged. Separate shared registration from effect if needed. Tier C.

### P04a/P04b — Sol and Letnev copied combat-round agents

Read printed solagent/letnevagent, leaders.rs::dispatch_leader, combat.rs round opening, invasion.rs rounds, existing extra_die_round/unit consumers. Current shared dispatch hardcodes infantry/cruiser and applies a type marker to the caller; printed text permits choosing any ground force/ship, possibly another player's. Native start-round offers are absent. COMBAT_ROUND_STARTED is emitted for space combat; ground needs a precise start-round producer.

First specify the selected-unit representation and exact round timing. Do not reuse a type-wide marker that boosts multiple units or bind beneficiary to borrower. Split space/ground producers and copied consumers into small children. Preserve original-six compatibility; additional native play changes need their own documented authority resolution, not an inference from the two already approved fixes.

Tests: actual pre-dice round window, one selected unit gets exactly one die, other same-type units unaffected, opponent target, upgrade/faction units, round expiry, source Exhausted, only Ssruu exhausts, legal no-unit/refusal, failure/retry. Tier C; architecture review before committing a new shared representation.

### P05 — action-agent and recursive Ssruu closure

Audit all 18 seated source routes against printed text and BF-SSRUU-SEATED-AGENT-CENSUS.md. Arborec/Hacan/Xxcha action routes exist but need real dispatch recipient/payment/source-exhaustion compatibility fixtures. Muaat committed; do not redo it. Define Ssruu-copying-Ssruu handling without infinite recursive menus or invented text. Confirm copied trigger registrations survive readiness changes; borrowed right is exact source/card, not a reason to restrict an unrelated event actor.

Accept existing uncommitted Argent/Naalu/Naaz/Saar copied routes using their BF-SSRUU-*-TIMING evidence and actual Game/production/token/end-turn paths. Ghost/Winnu/Sardakk/Mentak already committed. Resolve hidden hand views, nested errors and source/native coexistence. Only claim yssarilagent after the full inventory is covered, reviewed, committed and no printed route is represented by a disabled test-only hook. Tier C.

### P06 — Naaz Maximum and Absolute Synergy acceptance

Read BF-NAAZ-EIDOLON, BF-COMBAT-ORIGIN, BF-NAAZ-COMBAT-COMPLETION, BF-NAAZ-ABILITY-PRODUCTION(-REVIEW), BF-NAAZ-INVASION-ROUTES, BF-NAAZ-SPACE-CANNON-ROUTE, BF-NAAZ-HERO-WARFARE, and current naaz.rs/combat.rs/invasion.rs/fleet/supply/production hooks. Most behavior already exists in WIP: planetary Maximum participating as a ship, unit-ability hit immunity, repair/retreat, mech placement/production ban while standing, capture-return form, breakthrough gain/action triggers. Do not rewrite these based on stale gap descriptions.

Reconcile every printed route and add missing actual window regressions. Review sustain/assignability semantics and producer origin labels explicitly; no aggregate game statistic can decide a rules ambiguity. Validate planet context rather than defaulting all units to space. Check ordinary/ability production ban, effect placement, captured returns, space cannon/bombardment/Harrow, actual planetary space-combat participation, priority assignment intersection, and round repair/retreat. Supplemental Luna review is not a tier-C acceptance. Claim naaz_voltron/naazbt only after independent acceptance and focused scoped commits. Tier C.

### P07 — Yin breakthrough and Alliance dependency

Read printed yinbt, BF-YIN-ALLIANCE-CENSUS, BF-ALLIANCE-RIGHTS and routes A/B/C/WINNU. Public Alliance-right foundation exists; Yin sampler and gain/public-objective-score callbacks still do not. Implement deterministic canonical unused-faction selection and printed grants/triggers using an explicit bounded package. Test seeded selection/replay, no duplicate grants where prohibited, grant visibility, gain versus public-score triggers, and actual supported commander consumers.

Scope conflict: canonical unused-faction grants may select factions whose commander handlers do not exist, while the user excludes Crimson/out-of-scope interactions. Do not filter the pool to implemented handlers, add Crimson, or claim mere rights representation is a complete effect. Record exact candidate/dependency proof and obtain a narrow human scope resolution if required. Continue independent work while that resolution is pending. `yinbt` cannot be honestly closed without resolving this dependency or an explicit accepted limitation; an exception must remain visible in the final report.

### P08 — reconcile and commit remaining inherited BF WIP

Before integration, inventory every BF-owned unstaged hunk and map it to a scoped package/evidence/reviewer. Includes Alliance rights/consumer routes, Winnu hero actual primaries/free tactical continuation, Yin hero eventful combat, Naaz hero Warfare secondary, Naalu token-placement consumers, strict staged events, production entry errors, Saar/Ghost/Argent/Arborec hooks, and faction claims. Source files still dirty after a narrow commit can legitimately hold another package; never stage them wholesale.

Tests must cover real callers, failure atomicity, stable IDs, hidden/public projections, native six-faction compatibility and the two already approved behavior fixes. Audit registered decision sites against actual observed asks, not a passing inventory alone. Inspect inherited waiver-index fixture for a real independent invariant. Preserve unrelated Nomad/Cabal/Titans acquired-commander WIP as suspended scope, not required accepted output. Read old A–O queue for traceability, but reconcile against today's live code rather than reimplementing old blockers.

### BF-20 through BF-25 — integration, after faction acceptance

| Package | Scope and required result |
|---|---|
| BF-20 | Widen engine IN_SCOPE_FACTIONS to exactly 18 unique content-backed aliases. Audit active rollout consumers; do not rewrite historical dataset rosters indiscriminately. Coordinate parity dependencies with BF-22 before leaving a broken shared build. |
| BF-21 | Deterministic seeded faction selection/draft, six-seat uniqueness, defined behavior for oversized tables, RNG stream separated from map generation; same-seed identity and seed variation fixtures. |
| BF-22a/b | Widen policy battle roster and training arena profiles, version faction/schema/vocabulary explicitly. Define old checkpoint compatibility/rejection/migration; no silent reinterpretation. Verify all six faction-decomposition feature families for all 18. Split schema from profile behavior across crates. |
| BF-23 | Prepare the authorized simulator rebaseline with saved old/new workload and all metrics, per-seed completion/replay identity, reviewed semantic explanation. Preserve original-six comparison separately from intended 18-faction rollout effects. Never auto-adjust bounds just to pass. |
| BF-24a/b | Inventory every new decision kind/source/context; readable owner/target/cost/consequence presentation. Render assertions and manual park/answer/resume coverage using existing review/replayer structures. Preserve unrelated UI work. |
| BF-25a/b | Full workspace suite, bounded deterministic 18-faction all-pairs seated soak and replay checks, zero twelve-faction ledger gaps/blocked rows (or explicit authorized exceptions), independent frontier exit review and milestone report. Existing one-new-faction replacement soak is insufficient. |

Use BF-INTEGRATION-READINESS-OCT4.md for exact consumers and harness locations. Verify current version numbers before editing; they may change in the shared checkout. Gates involving reviewer/human decisions remain gates; Pi may prepare all concrete artifacts and implementation before requesting them.

## Build, review and filesystem discipline

Operator follow-up: AGENTS.md now makes reasonable bounded parallelism mandatory for CPU-bound
commands estimated to take several minutes. Use the layer that actually performs the work:
`-j8` for builds, `--test-threads=8` for independent tests, and bounded deterministic seed/case
workers for a single long campaign. Preserve complete coverage and real exit status; record any
necessary serial exception. A filtered `grep | head` pipeline is not an acceptance log.

A suitable combined Bash test invocation (after the current owned run finishes) is:

```bash
set -o pipefail
cargo test -p ti4-policy -p ti4-bridge -j8 -- --test-threads=8 2>&1 | tee target/pi-policy-bridge-tests.log
```

This improves observability and bounds independent work; it does not make an internal sequential
campaign parallel. Inspect/split that bottleneck separately if it remains multi-minute.


Known working shell is Windows PowerShell v1.0 executable, with login disabled by Codex. Pi can use its normal shell. Prefer rg --threads 1 if resource/thread errors recur.

```powershell
$env:CARGO_PROFILE_TEST_DEBUG='0'
$env:CARGO_PROFILE_TEST_INCREMENTAL='false'
Remove-Item Env:LIBTORCH -ErrorAction SilentlyContinue
cargo test -p ti4-engine --lib factions::yin -q -j8 -- --test-threads=8
cargo test -p ti4-engine -q -j8 -- --test-threads=8
cargo clippy -p ti4-engine --lib -q -j8
```

These flags worked for engine-only checks. ML/workspace libtorch builds need the actual configured absolute LIBTORCH path from repository notes; do not guess a path or mistake a concurrent link failure for a rules defect. Only one coordinator runs Cargo. A filter matching zero tests is failure; Cargo accepts one filter per invocation. Eight workers are authorized, not a verified speedup claim. Do not benchmark Pi versus Luna now.

Use UTF-8 with preserved line endings for edits. Rustfmt only owned source files; never broadly format game.rs in this dirty tree. Generate scratch/build output only in existing ignored target areas; no new folders or deletion. The old handover's /tmp/new_section.rs scratch is not a request to override the current no-deletion rule.

Rollback includes state, relevant dice/RNG/sequence, resolver frequency/stack/application journal and decision log. Resolver::checkpoint restores its OWN table.log; emit_with_context callers must separately restore their external table.log. Do not rewind decider input. Valid decline differs from invalid answer/error. Retry through the same services/table, not fresh state, when claiming atomicity.

Shared timing pitfalls: optional declines can be re-offered after another seat resolves; stable-ID semantic deciders are more reliable than positional scripts. Preserve original optional choice identity even when adding a wrapper. Actor, beneficiary, source owner and borrower are separate. A native effect cannot erase a copied effect's target; precise event payloads should carry it.

Independent C review is required by AGENTS/PI_WORK_PACKAGE_STANDARD. User's Terra ceiling overrides old Opus/frontier delegation instructions: no reviewer above Terra. If no qualifying independent reviewer is available, label that gate pending; do not rename self-review or a supplemental review as acceptance. This handover launches no model/process and sends no message to another session.

Git: preserve branch/history. Inspect staged diff and dependencies; use exact hunk staging for dirty mixed files. game.rs contains another session's status-round income experiment; do not commit it with BF changes. Existing ignored target scripts demonstrate isolated staging but verify them before reuse. Test results so far describe the shared working tree, not isolated commit contents.

## Definition of finish and compact handover

Finish means all twelve printed asset scopes execute end to end, required original-six compatibility or authorized deviations have evidence, BF20–25 exit gates pass, independent findings are resolved, focused changes are committed, and execution/scope/test/artifact ledgers agree. A passing engine suite alone is insufficient.

After each bounded package, persist objective, source versions, package/dependencies, branch/HEAD, exact changed paths, checks/results, compatibility decisions, review status and next exact command. Use Pi auto-compaction; after compaction read durable state before resuming. Keep at most one overlapping shared package active. Do not assume earlier agent/process names still mean active writers.

Immediate next action: P01 read-only producer census and copied Milor source review, then a written small provenance spec and an actual red non-combat regression before implementation. Do not begin by widening seating or claiming the four remaining assets.

## Captured dirty checkout inventory

This inventory is a preservation map, not authority to edit every listed path. BF engine/evidence changes need hunk ownership reconciliation; policy/training/UI/scripts and income experiments are other-session work until an exact BF integration package establishes ownership.

```text
## wp/base-factions
 M crates/ti4-engine/src/action_cards.rs
 M crates/ti4-engine/src/agenda_effects.rs
 M crates/ti4-engine/src/choice.rs
 M crates/ti4-engine/src/combat.rs
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
 M crates/ti4-engine/src/production.rs
 M crates/ti4-engine/src/promissory.rs
 M crates/ti4-engine/src/strategy_cards.rs
 M crates/ti4-engine/src/supply.rs
 M crates/ti4-engine/src/tokens.rs
 M crates/ti4-engine/tests/decision_delivery_inventory.rs
 M crates/ti4-mlp/examples/capture_offline_pilot.rs
 M crates/ti4-mlp/examples/clearance_eval.rs
 M crates/ti4-mlp/examples/offline_bc.rs
 M crates/ti4-mlp/examples/ppo_update.rs
 M crates/ti4-mlp/src/bot.rs
 M crates/ti4-mlp/src/lib.rs
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
 M crates/ti4-training/src/lib.rs
 M crates/ti4-training/src/reward.rs
 M crates/ti4-training/src/rollout.rs
 M plans/BASE_FACTIONS_PLAN_2026-10-02.md
 M plans/EXECUTION_STATE.md
 M plans/INDEX.md
 M plans/evidence/BF-COMBAT-ORIGIN.md
 M plans/evidence/BF-NAAZ-EIDOLON.md
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
?? plans/evidence/BF-ALLIANCE-RIGHTS.md
?? plans/evidence/BF-ALLIANCE-ROUTES-A.md
?? plans/evidence/BF-ALLIANCE-ROUTES-B.md
?? plans/evidence/BF-ALLIANCE-ROUTES-C.md
?? plans/evidence/BF-ALLIANCE-ROUTES-WINNU.md
?? plans/evidence/BF-BORROWED-CABAL-PRODUCTION.md
?? plans/evidence/BF-BORROWED-NOMAD-PRODUCTION.md
?? plans/evidence/BF-BORROWED-TITANS-PRODUCTION.md
?? plans/evidence/BF-CODEX-INHERITED-REVIEW-2026-10-04.md
?? plans/evidence/BF-INTEGRATION-READINESS-OCT4.md
?? plans/evidence/BF-LOCAL-MODEL-EVALUATION-2026-10-04.md
?? plans/evidence/BF-NAALU-TOKEN-PLACEMENTS.md
?? plans/evidence/BF-NAAZ-ABILITY-PRODUCTION-REVIEW.md
?? plans/evidence/BF-NAAZ-ABILITY-PRODUCTION.md
?? plans/evidence/BF-NAAZ-COMBAT-COMPLETION.md
?? plans/evidence/BF-NAAZ-HERO-WARFARE.md
?? plans/evidence/BF-NAAZ-INVASION-ROUTES.md
?? plans/evidence/BF-NAAZ-ROUTES-INDEPENDENT-REVIEW.md
?? plans/evidence/BF-NAAZ-SPACE-CANNON-ROUTE.md
?? plans/evidence/BF-PRODUCTION-ENTRY-ERRORS.md
?? plans/evidence/BF-SSRUU-ARGENT-TIMING.md
?? plans/evidence/BF-SSRUU-NAALU-TIMING.md
?? plans/evidence/BF-SSRUU-NAAZ-TIMING.md
?? plans/evidence/BF-SSRUU-SAAR-TIMING.md
?? plans/evidence/BF-SSRUU-SEATED-AGENT-CENSUS.md
?? plans/evidence/BF-SSRUU-YIN-TIMING.md
?? plans/evidence/BF-STAGED-EVENT-ERRORS.md
?? plans/evidence/BF-WINNU-HERO-TIMING.md
?? plans/evidence/BF-YIN-ALLIANCE-CENSUS.md
?? plans/evidence/BF-YIN-HERO-TIMING.md
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
