# BF-LOCAL-MODEL-EVALUATION-2026-10-04

Operator requested a bounded local implementation package and comparison with Luna. P2: operator-provided loopback model API 8080, one isolated Pi worker/controller 41874, bounded ignored target artifacts. No unrelated service was stopped, no folders/deletions/commits/pushes. Controller PID 91780 and Pi PID 97852 were verified by their task-specific session command line and stopped after both tasks settled.

## Model and method

Endpoint identifies qwen3.8-flash-next-iq3_xxs, context 131072. Context size is distinct from response budget. Existing Pi provider qwenSTRATA_ISTA02 was explicitly selected. Server timings are reported measurements, not independently benchmarked throughput or an M00 performance claim. One code-review input was provided unchanged to gpt-6-luna and to local chat-completions. Luna used a file-read harness; local used direct HTTP. Luna's generation latency/token count was not instrumented, so no numerical speed ratio is justified. Request queuing and shared host load limit interpretation.

## Bounded implementation attempts

Full Crimson package: 98.36 seconds, 30 tool calls, one tool error, no edit/final report/evidence. Terminal assistant error: engine stopped unexpectedly, exit code 3221226505; next request restarts it. Smaller retry, one Titans recipient-isolation test: 25.67 seconds, zero tool calls, zero edits, same engine exit. Neither attempt implemented anything or met acceptance. Do not attribute a host/backend crash to semantic model quality.

Controller incorrectly labeled these settled errors completed. tools/pi_rpc_bridge.py now preserves assistant errorMessage/stopReason as failureReason and settles as failed; an abort takes precedence. A synthetic lifecycle check tested normal completion, engine error and abort-plus-error: 3/3 passed. Existing controller gained explicit --model and --no-extensions flags to isolate the authorized model/session. These changes are uncommitted and review pending.

## Same-code review sample

Input: target/bf-model-review-comparison-prompt.md. It includes printed Nomad commander text, recipient-specific flagship selection, both helper call sites and selected-option validation. No demonstrated cross-faction flagship defect exists in the supplied path.

| Attempt | Wall time | Output tokens | Server generation rate | Final answer/outcome |
|---|---:|---:|---:|---|
| Local default reasoning, 1000 cap | 13.23 s | 1000 | 85.1 tokens/s | Length stop, reasoning only, no review |
| Local thinking disabled, 1000 cap | 50.77 s | 805 | 72.9 tokens/s | Final review, three unsupported findings |
| Local default reasoning, 8192 cap | 104.82 s | 8192 | 79.3 tokens/s | Length stop, reasoning only, no review |
| Fresh Luna, same source input | Not instrumented | Not instrumented | Not comparable | Correct no-demonstrated-bug answer and legal-choice/recipient boundary |

The no-thinking run wrongly assumed acquired Nomad rights should grant the Nomad flagship instead of the recipient's flagship, despite the supplied text and assertion. It then hypothesized an unsupported faction without a flagship and claimed an availability/pricing disconnect even though the supplied preview calls the price helper. Its last hypothetical hook-input concern had no demonstrated reachable defect, contrary to the prompt. These are not actionable findings.

The larger reasoning run spent all 8192 tokens exploring speculative possibilities and still delivered no final answer. This removes the initial 1000-token cap as the sole explanation, but does not prove that an unlimited or differently configured run could not answer correctly. A previous unrelated Luna review also produced a rejected foreign-flagship false positive; do not generalize this one favorable Luna sample into universal accuracy.

Provisional judgment: local raw generation is fast in these short requests, but useful work was weaker in this sample: zero implemented patches, two infrastructure crashes, two reasoning-cap failures, and an inaccurate no-thinking review. Luna gave the useful review here. More implementation-quality samples require stable virtual-memory headroom and a backend that delivers final answers. Neither model's answer substitutes for required independent tier-C acceptance.

## Resource and artifact limits

During failures Windows reported about 20 GiB free physical RAM but roughly 46 MiB free virtual memory; allocation failures occurred in rustc and modern PowerShell too. This is a plausible environmental contributor, not a proven backend crash diagnosis. After owned workers stopped, free virtual memory rose to roughly 947 MiB. Do not change paging/server configuration or terminate unrelated workloads without operator scope.

Ignored raw response artifacts: target/bf-local-review-response.json, target/bf-local-review-response-no-thinking.json, target/bf-local-review-response-8192.json. Session/events stay in target; controller token must never be displayed or committed. Last completed engine suite: 2000 lib pass, zero fail, one ignored, integrations 1/1, inventory 4/4, other integration 5/5 and doctests. Subsequent production-entry green compile was interrupted by memory allocation failure and remains unverified.

## Operator-requested Pi compaction retry

October 4: operator asks to run through Pi's automatic compaction instead of direct API. A fresh isolated session target/bf-local-review-pi-session.jsonl was launched through the same owned loopback controller, PID 18624, Pi PID 87740. set_auto_compaction enabled:true returned success; get_state confirmed autoCompactionEnabled:true. Model/provider are unchanged. Pi reports contextWindow:128000, maxTokens:128000, reasoning:false, thinkingLevel:off; server previously advertised n_ctx:131072. This is a materially higher native response budget than either direct-API cap. No settings/configuration file was edited.

Task BF-LOCAL-PI-MATCHED-REVIEW uses the same supplied source and a read-only/no-tools preface. Limits: 240 seconds absolute and no-edit (the latter deliberately matches the timeout because reviews must not edit), one allowed tool error. The controller wrapper says make an edit, but the explicit review instruction forbids all edits. Run started 1791110039.5036263. At 64.7 seconds it was running without a tool call, answer or failure. Final result and actual compaction events still pending. A successful fresh-session review would not itself prove compaction handled a long conversation; distinguish enabled configuration from observed compaction.

### Native Pi retry final result (supersedes the earlier provisional judgment)

BF-LOCAL-PI-MATCHED-REVIEW completed normally in **118.92 seconds** (finishedAt 1791110158.424216 minus startedAt 1791110039.5036263). Zero tools, errors, edits and crashes. Native usage: 6319 input, 9972 output, 16291 total tokens, stopReason stop. Pi provider allowed 128000 output tokens. Automatic compaction was enabled, but **zero compaction events occurred** in this fresh short session: do not credit compaction for the result.

The local model delivered the same main correct conclusion as Luna: no demonstrated defect, and recipient legal choices/validated selection guard the free-flagship path. It preserved general discounts/credit and production capacity in its explanation. It added three explicitly unproven/defensive caveats rather than adhering to the requested concise no-bug answer. These are not accepted findings or reasons to alter the engine. Cost-cap semantics remain printed-cost constraints unless the exact source rule says otherwise; unknown catalog IDs cannot pass generated-choice validation; suffix matching had no source-catalog counterexample.

Revised judgment: with Pi's larger native output allowance, the local model produced a useful review and agreed with Luna on the main result. The earlier capped direct requests underestimated it. Luna was more concise and followed the requested no-demonstrated-bug format better. No numerical Luna speed ratio is available, and one source-review sample cannot establish implementation accuracy or relative throughput. The two earlier implementation attempts remain infrastructure failures; no local implementation patch has yet passed acceptance. This retry needs no code changes and satisfies neither an implementation gate nor required frontier review.

Verified owned Pi PID 87740/controller PID 18624 were stopped after the final result was recorded; the model server and unrelated workloads were preserved. No active local worker or Cargo command remains. Raw final message/usage stays in target/bf-local-review-pi-session.jsonl; no bearer token is recorded in this evidence.
