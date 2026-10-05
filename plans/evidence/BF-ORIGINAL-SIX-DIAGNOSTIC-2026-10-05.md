# Original-six rebaseline diagnostic, October 5

Result: all 30 fixed seeds 812001–812030 completed with zero errors and reproduced every deterministic GameResult summary field across two runs. Only wall-clock seconds is excluded. Five event-share metrics lie outside the recorded intervals. No recorded bounds were changed; this is the reviewable diagnostic requested by the operator, not a completed BF23 eighteen-faction baseline.

Command: `cargo run -p ti4-sim --example rebaseline_behavior -q -j4`. Four compile jobs for memory headroom; seed workers capped at 16 with independent game/RNG state. Default six-faction roster and original 30-seed bootstrap protocol retained. Review: separate Luna source audit confirms complete seed/error/cutoff/result-vector accounting. Engine/policy/sim full shared gate passes 2,103/273/51 tests; no isolated committed-tree claim.

| Metric | Recorded interval | Recomputed interval | Current point | Outside recorded? |
|---|---|---|---|---|
| completion | 1.000000–1.000000 | 1.0–1.0 | 1.000000 | no |
| faction_differentiation | 0.502371–1.166296 | 0.45406259426005663–0.9425634864166583 | 0.597035 | no |
| score_spread | 1.756710–2.238151 | 1.7264127669572966–2.17381058170115 | 1.953851 | no |
| share_INVASION_RESOLVED | 0.015450–0.016371 | 0.013515054953094218–0.01443714799742519 | 0.013985 | yes |
| share_PRODUCTION_RESOLVED | 0.027790–0.029062 | 0.02523004028528873–0.026174630104298456 | 0.025706 | yes |
| share_SHIP_MOVED | 0.038854–0.041847 | 0.044361701461125135–0.04815118397677712 | 0.046214 | yes |
| share_SPACE_COMBAT_RESOLVED | 0.003181–0.003756 | 0.002924993643272542–0.0034752275717082636 | 0.003196 | no |
| share_SYSTEM_ACTIVATED | 0.054994–0.057501 | 0.05046008057057746–0.05234926020859691 | 0.051412 | yes |
| share_TACTICAL_ACTION_BEGAN | 0.027213–0.028447 | 0.024638108910827225–0.025585108726943282 | 0.025124 | yes |
| vp_pace | 0.443827–0.517901 | 0.43888888888888894–0.5092592592592593 | 0.473457 | no |

The approved no-cargo SHIP_MOVED event correction changes event counts and share denominators. That is a plausible contribution to the observed movement increase and other share decreases; this table cannot establish complete causal attribution or original-six decision-boundary compatibility. The working tree includes other shared experiments, including income changes. Keep the earlier behavior-neutral claim constrained to its historical checkpoint; do not repeat it for this current tree.

Raw log: target/bf-oct5-original-six-replay-diagnostic.log. SHA256: `a8772063bb4b28dd2a5a4086b360d44a3761b71834b8eaf7106172336c88b47a`.

Remaining acceptance: attribute observed changes using preserved source/case evidence, retain original-six comparison separately, resolve faction scope/asset gates, then execute eighteen-faction BF23 under its own versioned protocol. Per-seed summary reproduction is not full cross-version choice/state replay equivalence. This file does not authorize changing bounds to make them pass.
