# ASTRA-MERGE-REVIEW-2026-09-30

Independent review of the operator-provided request in
`plans/ASTRA_REVIEW_REQUEST_2026-09-30.md`. Reviewer: Codex, separate from Claude's implementation.
Full findings and suggested fixes: [review response](../ASTRA_REVIEW_RESPONSE_2026-09-30.md).
Status: review delivered; push recommendation HOLD. This does not close all 127 commits' required
reviews, multiplayer security qualification, or any milestone exit.

## Scope and permissions

- P0 reads of repository source, Git objects/diffs, named local artifacts; P1 diagnostic examples,
  tests and documentation. No production source, pre-existing test, golden, bound, artifact pointer,
  checkpoint, index, ref, branch or remote mutation.
- Writable paths: the three `astra_*_20260930.rs` diagnostic examples under `ti4-sim`, `ti4-training`,
  and `ti4-replayer`; the response, this evidence, appended execution checkpoint; ignored
  `out/astra-review-20260930/`; ordinary Cargo build outputs/cache.
- No external read-only dependency, historical Python access, network, server, port, GUI, or external
  state change. Initial read command could not launch because of the Windows sandbox helper. The
  operator subsequently enabled unrestricted tooling; repository scope restrictions remained in force.
- All Cargo commands use `--offline -j 4`. Simulation runs 30 seeds serially. Training uses one
  generation with one seed, then one generation with the plan's default 16 seeds; the latter uses
  `RAYON_NUM_THREADS=4`. All started processes completed. Diagnostic text artifacts are bounded to
  1 MiB, excluding normal Cargo products. No deletion, cleanup, checkout, reset, prune, or worktree.

## Source and contract versions

Branch `wp/online-multiplayer`; HEAD `3f92016dabdd737212fc974d66187bbc11597aec`.
Local main `b1143a5b8e047dca236d07f3f9981c623b6b89d1`; both trees
`54dcd09370e33d2506f4d56255f1e34147009a11`. Existing dirty app/live changes were reviewed and tested
as working-tree changes, not as part of either commit. Rollout changes were inspected as formatting.
Contracts: `R02_REPLAYER.md`, `control.rs` instance/shape and replay contracts, behavioral protocol v1
and v44 constants at `c2566b99`, sparse explicit schema 4, OOV registry v11, named request and existing
ONLINE-001/M08-021/operator evidence. No new rules ruling or performance claim.

## Commands and exact outcomes

Run from the repository root. For replayer/reviewer commands, environment was the request's
`LIBTORCH=out/libtorch-2.9.1-cu128` resolved absolutely, `LIBTORCH_BYPASS_VERSION_CHECK=1`, its `lib`
prepended to PATH. Training diagnostics used the same environment. No artifact acceptance bypass
was added; that variable is the existing torch version setting.

| Command after `cargo` | Outcome | Raw log under `out/astra-review-20260930/` |
|---|---|---|
| `run --offline -j 4 -p ti4-sim --example astra_behavior_diagnostic_20260930` | exit 0; 30/30 clean endings; 7/10 bounds breached; 2,120 invasion starts and resolutions, equal per seed | `behavior-current.log` |
| `run --offline -j 4 -p ti4-training --example astra_stage1_diagnostic_20260930` | exit 0; 399 decisions, 0 errors; all three schema-4 profiles empty; update norms/return spreads zero | `stage1-current.log` |
| `run --offline -j 4 -p ti4-training --example astra_stage1_diagnostic_20260930 -- 16` | exit 0; 6,752 decisions, 0 errors; 32,329 named nonzero weights across three profiles | `stage1-16seeds.log` |
| `run --offline -j 4 -p ti4-replayer --example astra_stale_submission_20260930` | Cargo exit 1 / probe exit 101, EXPECTED RED: stale ask-1 click accepted at ask 2, second pending consumed | `stale-submission.log` |
| `test --offline -j 4 -p ti4-replayer --test app --test live` | exit 0; app 21 passed, live 9 passed; 0 failures | `replayer-app-live.log` |
| `test --offline -j 4 -p ti4-review --test semantic_golden an_identity_hook_reproduces_the_default_path_exactly -- --exact` | exit 0; 1 passed, 0 failed, 2 filtered | `review-identity-hook.log` |

`rustfmt --edition 2024` ran only on the three added diagnostic files. The two larger builds briefly
contended on Cargo's artifact lock; no unrelated process was stopped. No full workspace rerun or
historical simulation build was performed. The reported 2558/4 workspace result is from the request,
not a result independently obtained here. No new determinism repeat campaign or speed measurement.

The PowerShell reduction in `invasion-denominators.csv` divides each seed's invasion count by its
event sum, then by that sum minus `TURN_CLOSING`; means are 0.0159060881921281 and 0.0166887688531635.
`git log -S TURN_CLOSING -- crates/ti4-engine/src/game.rs` attributes the label to `75f1d94a`.
This is arithmetic attribution only, not a counterfactual gameplay run.

`Get-FileHash` confirmed committed slots digest
`fa3d6f945988cc9f210fffafff115422c9bf883c077ae8aac8aaf483d1ec41fc`; its file declares OOV v10.
The current pointer/provenance were read, not edited. No search of unrelated artifact repositories.

## Findings, disposition, and next safe action

- P1: live submission identity missing; independent two-option probe confirms acceptance of a
  repeated old click against a new ask. Unresolved; implement typed instance validation at gate.
- P1: host pending deduplication compares only fingerprints; new identical ask can be omitted
  between pump ticks. Source finding; network reproduction and fix outstanding.
- Invasion drift remains partly unattributed: seven metric breaches, clean sampled starts/finishes,
  demonstrated partial denominator effect. No permission to rebaseline inferred or supplied here.
- Stage-1 proposed hashed-name rationale rejected with code and measured evidence. Preserve named
  sparse schema; repair the uninformative fixture with a documented learning-signal precondition.
- App panel fix and live test assertion approved for their limited claim; app/live tests pass.
- Golden refresh conditional on full semantic audit; identity-hook test passes. Smoke test requires
  hermetic input coverage and/or supported artifacts, not fabricated provenance or weaker checks.
- Merge tree equality verified; dirty input coverage and commit-based multiplayer build identity
  prevent treating it as fully qualified. Fixtures/app repair are absent from both committed trees.

Next: a bounded R02 live-instance repair, followed by the behavior-attribution/fixture work described
in the response; do not advance migration milestones or push main on this review. Existing staged
bridge restores and other sessions' dirty files remain untouched. No package commit was made.
`raw-sha256.json` contains raw-log and diagnostic-source hashes; `git-status-final.txt` and
`git-diff-before-final-docs.patch` record the preserved working-tree state/diffs for the handover.
