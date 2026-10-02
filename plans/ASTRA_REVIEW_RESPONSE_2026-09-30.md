# Astra response — merge review, 2026-09-30

**Hold the push. These are not four demonstrated stale expectations.** The invasion explanation is
incomplete, the manual-choice boundary accepts stale answers, and the proposed explanation of the
Stage-1 failure is false. The local app repair and its changed assertion are reasonable but do not
close the identity defect throughout the system.

This is an independent Codex review of Claude's four questions, against branch
`wp/online-multiplayer`, HEAD `3f92016dabdd737212fc974d66187bbc11597aec`, and its existing dirty
working tree. It is **not** an exhaustive review of all 127 commits or a completed multiplayer
security review. No existing source, tests, fixtures, bounds, checkpoint, Git index, or ref was
changed. Added only diagnostic examples and review documentation. Commands, results, scope, and
raw-output hashes are recorded in [the evidence](evidence/ASTRA-MERGE-REVIEW-2026-09-30.md).

## 1. Invasion share: attribution is required, but start with the measurement

`behavior::per_seed` divides the event count by **all events in that game**, then `batch_metrics`
averages those per-game shares. This is neither invasions per game nor the probability that a
started invasion resolves. The reported 14.3% shortfall below the lower bound cannot establish
that invasions resolve 14.3% less often. Counts or completion rates need their own denominators.

I ran the unchanged 30 seeds through `run::play_with`, serially to bound worker use, using the same
six seats, FULL scope, Scored policies, and default horizon. It reproduces the reported invasion
share. The gate stops at its first failed assertion; the full result is:

| Metric | Current mean | Recorded interval | Result |
|---|---:|---:|---|
| completion | 1.000000 | 1.000000–1.000000 | inside |
| faction differentiation | 0.793667 | 0.548201–1.101080 | inside |
| score spread | 2.001657 | 1.746929–2.222619 | inside |
| invasion share | 0.015906 | 0.018557–0.019962 | below |
| production share | 0.028446 | 0.034243–0.035447 | below |
| ship-movement share | 0.040317 | 0.044410–0.047578 | below |
| space-combat share | 0.003462 | 0.004248–0.004945 | below |
| activation share | 0.056298 | 0.067735–0.070061 | below |
| tactical-action share | 0.027852 | 0.033491–0.034637 | below |
| VP pace | 0.479630 | 0.387654–0.449383 | above |

That is **seven breaches**, not an isolated invasion change. The six shares moving down together
make changes to the event denominator and action economy important candidates. The higher VP pace
also needs explanation; a combat-lethality story alone is not enough.

The census provides two useful checks:

- All 30 games end cleanly: 27 through objective exhaustion and 3 through victory points. Each game has equal
  `INVASION_BEGAN` and `INVASION_RESOLVED` counts: **2,120 of each** in total. This does not show the
  suggested loss of a completion event after an invasion starts in these games. It does not prove
  that invasions which should have started were offered, or that casualties/control were correct.
- The stream contains **133,040 events**, including **6,237 `TURN_CLOSING` events**. Git attributes
  that label to `75f1d94a`, outside the proposed six-combat-commit explanation. Removing only that
  label from each denominator, while leaving all trajectories and numerators fixed, changes the
  mean invasion share from **0.015906088192 to 0.016688768853**. This measures a bookkeeping
  contribution, but still does not reach the old floor. It is not a causal ablation of that commit:
  the same commit also changes turn behavior.

The cheapest acceptable next attribution does **not** require blindly bisecting all 40 commits:

1. Preserve this per-seed census and obtain corresponding authenticated baseline counts/traces, if
   an existing artifact contains them. The old interval endpoints alone cannot recover numerators,
   denominators, or invasion frequency. Do not substitute an old executable of unknown provenance.
2. Compare absolute invasion, activation, production, combat, and event-family counts per seed.
   Separate added event logging from changed decisions. For play differences, locate the first
   changed offer/answer or tactical transition and name its implementing commit; prioritize the
   turn/trade changes as well as combat. A witnessed decision difference plus a focused acceptance
   fixture is better evidence than six plausible directional stories.
3. Verify the changed boundary: eligible invasion offered, no surviving invader correctly skips it,
   a started invasion completes once, and its resulting control/casualties agree with the accepted
   rules. A bounded paired counterfactual of the implicated change can establish contribution if
   needed; it must be isolated from this shared checkout and separately scoped.

If baseline data and an authorized isolated comparison are unavailable, leave the attribution
unresolved. Do not replace that missing evidence with a wider band. Once the change is understood,
use the existing versioned rebaseline process, review **all ten recomputed intervals**, and obtain
the required approval. The test's later protocol-integrity assertion requires exact rederivation
even for metrics whose current means remain inside the old intervals. No bounds are approved by
this review.

## 2. Preserve the fingerprint; enforce instance identity at the boundary

Keep `ChoiceFingerprint` as the stable offer-shape hash. `ReplayRecord::matches` deliberately uses
that shape for replay. Put live identity in a separate typed token and carry it from the rendered
offer all the way to acceptance. This is not a choice between fixing the app and changing the hash.

Claude's `(fingerprint, frame, ask)` app cache repairs suppression of a legitimate new panel. Its
branch-local use is consistent with the app resetting `answered` when attaching/changing branches.
The test change from fingerprint uniqueness to ordinal uniqueness is **approved**: the old assertion
was not an engine invariant. The new test retains ordering, ordinal uniqueness, common frame, and
advancement; the app test explicitly verifies the fingerprint collision. I ran the app and live
suites: **21 + 9 passed**.

Two actionable findings remain:

- **P1 — stale submissions consume a later ask.** `ManualSubmission` carries only the fingerprint
  and option ID; `PendingManualChoice::validate` and `ManualControl::submit` therefore cannot tell
  which occurrence a click targets. I published a two-option ask at frame 7 / ordinal 1, accepted
  option `a`, published the same shape at ordinal 2, and resubmitted the *old* object. It returned
  `Accepted { option_id: "a" }` and removed the second pending choice. The probe intentionally exits
  101 on that violated invariant. This also refutes the app's unconditional double-click claim:
  after the next identical ask appears, its current snapshot does not identify the click's origin.
- **P1 — the host suppresses successive identical pending updates.** `net/host.rs:704–708` compares
  only the fingerprint with `client.pending_sent`. The pump runs every 50 ms. If a completed ask is
  replaced before the pump observes `None`, the next same-shaped offer is not transmitted. The
  client can retain the old panel and old ordinal. This is a source-level finding, not a reproduced
  network campaign. `Delegate` also carries only a fingerprint and needs the same correction.

Use an instance identity including branch/frame/ask and the live table generation (or an equivalent
non-reused token), plus the existing shape hash. A branch identifier alone is insufficient if a new
table reuses its numbering. Capture the identity **when displaying the offer**, not when submitting
the click. Validate it atomically under the gate lock for both submit and delegation. Key app/host
deduplication on the same identity. Keep replay matching separate and unchanged.

Required regressions: identical shapes at successive ordinals remain answerable; replaying the first
submission against the second is refused without consumption; a fresh submission for the second
works; an old submission after a table/branch replacement is refused; and two pending instances
inside one host-pump interval are distinguished without requiring an intermediate `None` update.
Keep the existing app collision and live ordinal tests.

## 3. The three expectations need different treatments

**Reviewer golden.** A versioned refresh is reasonable after the semantic diff is explained, but
not because a golden is old. Preserve the historical R02 evidence in Git and keep the live
identity-hook equivalence check. I ran that check: **1 passed**. Commit `52066efa` concretely changes
legacy transaction offers from one-way `partners(...)` to the two-way `may_transact(...)` predicate;
that is a relevant candidate for the additional distant-Hacan offer. The option string predates
this commit, so searching for its original introduction is not sufficient attribution.

The request's frame-8 diff has not been independently regenerated here. Before updating, confirm
that frame's eligibility and inspect the complete 240-step projection, including later decisions
and outcome. Explain the score-digit drift against the old input/code versions too; small magnitude
alone is not proof that it is harmless. Use `TI4_REVIEW_GOLDEN_UPDATE=1` only after that audit and
rerun without it. Do not weaken equality or discard fields. A current golden may be renamed and
versioned so it no longer claims to be the original pre-R02 artifact.

**Smoke refusals.** The two artifact problems are real. I checked the pointer and provenance
sentinels, and hashed the committed slots file: it is the pinned
`fa3d6f945988cc9f210fffafff115422c9bf883c077ae8aac8aaf483d1ec41fc` generation, declaring OOV v10,
while the current registry is v11. Do not invent the lost provenance or relax acceptance checks.
Also, `mlp_smoke` still hardcodes that slots digest: publishing a different v11 generation alone
will fail its digest gate. Its resolver discards `accepted_generation`'s error with `.ok()`, hiding
the detailed provenance failure. Those are part of the repair scope.

Prefer making the pool-role regression hermetic at the input-validation boundary, retaining checks
that disallowed roles and unverified bytes cannot reach setup. If the CLI-level test remains, give
it a supported, versioned fixture and an explicitly built matching executable. Its current helper
chooses the first existing `target/debug` or `target/release` executable, regardless of the profile
under test; stale binaries are another source of false attribution. Keep artifact acceptance
qualification separate and explicit. A proper vocabulary migration is a separate artifact task;
documenting a red test does not repair its coverage.

**Stage-1 named weights. Do not make the suggested assertion replacement.** The cited
`f7b2716c` changes internal keys to FNV-based `FeatureKey`s and explicitly preserves stored names.
`bucket()` is the legacy hashed path. This test trains explicit schema 4, for which
`Profile::is_explicit()` is true; gradient application stores `name_of(key)`.

Measured with the exact test configuration (one generation, one seed): **399 decisions, zero
errors, zero weights in all three profiles, zero return spread, and zero update norm**. The failed
existential predicate does not prove all names are hashes; it is false because there are no names.
The proposed nonzero-weight predicate also fails.

As a planned diagnostic, I expanded to the existing reference plan's default **16 seeds**, still one
generation, with Rayon capped at four workers. Result: **6,752 decisions, zero errors**, maximum
return standard deviation **1.98761598**, sum of head update norms **0.5740347513**. Hacan/Jol-Nar/
Letnev produced **11,001 / 11,542 / 9,786** weights respectively; all counted as named and nonzero.
This is evidence for an uninformative one-seed learning fixture, not a removed named-feature
contract. It is not a retry-until-green campaign: the second sample size is the pre-existing plan
default, and both results are retained.

Repair the fixture with a specified positive learning signal or this bounded multi-seed sample.
Preserve the sparse explicit-schema/named-weight contract, assert informative returns and actual
weight movement, and retain name-based scoring/serialization coverage. No training mathematics
change is justified by the observed one-seed failure.

## 4. Merge fitness and verification without checking out main

Verified independently:

```text
HEAD = 3f92016dabdd737212fc974d66187bbc11597aec
main = b1143a5b8e047dca236d07f3f9981c623b6b89d1
both tree hashes = 54dcd09370e33d2506f4d56255f1e34147009a11
git diff --quiet HEAD main -> exit 0
```

This is strong proof that the merge introduced no tracked content difference. It can transfer
source-level verification **when that verification was actually performed on those inputs**. A
merge does not inherently need its own checkout just to repeat identical semantic tests.

It does not turn this dirty-tree test run into clean-commit verification:

- The 16 restored bridge fixtures are staged additions and **absent from main/HEAD**. The app fix
  and its test correction are also absent from both commits. The four-failure count in the request
  therefore describes a different input set from the merge tree. Commit the scoped repairs and
  update the candidate by an ordinary forward integration before claiming their results for main.
- Other sessions own dirty capture/training-example changes. The rollout diff is formatting-only;
  the example diffs are not. Record a tested input manifest and preserve those owners' work.
- `ti4-review/build.rs` obtains the actual `git rev-parse HEAD` and dirty state. Host and client use
  its embedded commit, and `net/host.rs:420` refuses mismatched commits. A binary built on `3f92016d`
  is not a build of `b1143a5b` merely because their source trees match. Existing binaries may be
  retained with honest build provenance and a verified matching host/client pair; do not relabel
  them. If the release requires the merge commit embedded, use a properly supported reproducible
  build-identity mechanism or an authorized build environment. An environment variable cannot
  override the current build script's Git call by assumption.

Finish the attributed behavior review, the live-instance boundary repair, the golden audit, and the
two test/fixture repairs; commit only their scoped paths; then run
`cargo test --workspace -j 4 --no-fail-fast` on the exact candidate inputs and close its review
findings. Preserve source-tree, artifact, toolchain, feature, and build-identity evidence. Do not
treat "trained all day" as the workspace semantic gate. No push or rollback is authorized by this
review, and no ref was moved.
