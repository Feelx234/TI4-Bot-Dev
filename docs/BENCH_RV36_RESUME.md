# Bench lane for rules version 36 (branch `core-bench-rv36`, from `core-integration-24` df7845b21, 2026-11-05)

Task: section 4 of `docs/HEAVY_RUN_PLAN.md` (re-record the six bench baselines for rules version 36 in two stages). Outcome: **stage 1 done, stage 2 stopped at the
classification: the differing games are not only Parley / Ixthian For. Nothing was re-recorded; `tools/rulebot/*.json` is unchanged in this branch. The four bench steps stay red.**

## What was run (all under a heavy slot via `/root/heavy2.sh`, `CARGO_INCREMENTAL=0`, 2 build jobs per build)

| step | how | result |
|---|---|---|
| recorder | worktree `/root/deploytmp/core-rec` at `core-integration-20` (b8e43ef8d, string engine), `cargo build --profile gate -p ti4-selfplay --bin selfplay`, target `/root/deploytmp/core-rec-target` | 2 min 23 s |
| engine under test | this branch (= int-24 tip), same command, target `/root/deploytmp/rv36-target` | 4 min |
| stage 1 | `FINAL_RV=36 tools/engine_profile/record_benches.sh <recorder> 6 35` | all six recorded, 10 min wall (54 CPU min); every game a victory; copies in `/root/deploytmp/rv36/base35/` |
| stage 2 verify | `VERIFY_OUT=... tools/engine_profile/verify_benches.sh <native> 6` | **every bench "0 identical, 0 different, 200 not in the baseline"** (see note 1), 15 min wall |
| compare | `tools/engine_profile/bench_rv36/compare_hashes.py`: baseline at 35 against the native output, per (setup, seed), ignoring the rules version | table below |

Disk stayed above 100 GB free. The configs `golden*.json` were set to 36 by the recorder and the 35 baselines were written, then everything was reverted (`git checkout -- tools/rulebot`).

## Result

| bench | games identical to the string at 35 | different | of which native `engine_error` |
|---|---:|---:|---:|
| bench (bot 4, rv config 23) | 0 | 200 | 10 |
| bench-v5 | 1 | 199 | 13 |
| bench-v4 | 5 | 195 | 8 |
| bench-v1 | 3 | 197 | 11 |
| bench-v2 | 1 | 199 | 12 |
| bench-v3 | 1 | 199 | 7 |
| total | 11 | 1 189 | 61 |

Expected by the plan: about half the games differ, all of them at a Parley or at the Ixthian Artifact (For). Found: 99 % differ.

### The 61 core stops (these are not S6/S7)

The native games end with `engine_error` where the string plays on to a victory. By cause (all six benches, 1200 games):

* 31 x `aftermath: Unported(HitsAddedToARoll)` (first: bench, `selfplay replay --seed 68 --players 4 --map 4pOuterRim --factions ghost,mahact,sardakk,jolnar --speaker random --starting-tech --rules-version 36`)
* 15 x `the position cannot be shown to the decider: Marks: mark without a text "NomadTcsNegotiating(Seat, Seat)"` (bench seed 17 4p 4pHyperlanes winnu,saar,nomad,nekro)
* 7 x `aftermath: PendingDestructions` (bench seed 70 4p 4pOuterRim yin,cabal,xxcha,muaat)
* 4 x `ask: UnportedAftermath(CombatUnported)` (bench seed 63 6p 6pStandard ghost,jolnar,yin,mahact,xxcha,l1z1x)
* 2 x `aftermath: MissingFact` (bench-v5 seed 47 4p winnu,sardakk,nekro,letnev, `--bot bot_version=5`)
* 2 x `the position cannot be shown: Board: planet entry in an untouched system "mallice"` (bench-v5 seed 70 4p)

The full list with a replay command per game is the `problems` array of `/root/deploytmp/rv36/ver36-out/<bench>.json`. These look like core gaps reached by the rule bot's play (or by the
games' divergent course); they are outside the "only Parley / Ixthian" rule, so the lane stopped. `NomadTcsNegotiating` has a `MarkKey` in the core (`keys/mark.rs`) and a family in `core_sync/keys/marks.rs`, but `mark_values`/`render_mark`
yields no text for it in these positions. Whoever takes this should start there and with `HitsAddedToARoll`.

### The other differences

First-difference scan (`first_difference.py`: replay the same game with the string at 35 and the native at 36 in `--text` mode; skip runs of reaction decisions that are the same set in a
different order) over the first 12 games of `bench`: the first real difference is never at a Parley or the Ixthian Artifact. Only 1 of the 12 games contains a Parley in its text at all. First differences seen:
`load_cargo` with 4 options in the string and 3 in the native (seed 1), `commit_ground_forces` commit against `done_committing` (seed 2), `load_cargo` choice (seed 3), a reaction offered to a seat (seed 4, 9), `predict_agenda_outcome` by another seat (seed 5, 11),
`return_over_secret_hand_limit` with different secret objectives in hand (seeds 6, 8: no earlier visible difference, so the hidden state differed before: a draw or an RNG stream), `fight_ground_combat_round` against `commit_ground_forces` (seed 7), `end_turn` against a commit (seed 10).
Visible earlier differences in the same games: reorders of reaction decisions (S1, user ruling 2026-11-01: active player first, then initiative order; the string is wrong) and, for the reactions at `STRATEGY_PHASE_BEGAN`,
a different round/phase label (`r2 strategy` in the string, `r1 status` in the native) for the same decision. **Hypothesis, not proved:** the S1 reorder shifts which seat's random-pick decision draws from the bot RNG, so after the first reorder
(there is one in nearly every game) the games diverge for reasons that have nothing to do with a card. If that is right, the stage-2 criterion "only Parley / Ixthian" cannot hold in any form while the string is the reference, and the
benches need a different reference (for example the native recording itself plus a determinism check, plus the golden dual for equivalence). To prove it: make the string recorder follow S1 (or the bot draw per seat) in a scratch tree and rerun the compare.

## Notes on the procedure (fix when it is run again)

1. `verify_benches.sh` finds games by the whole `spec`, and the spec contains `rules_version`. A recording made at 35 and a config at 36 therefore gives "200 not in the baseline" - the verify cannot be used for stage 2 as the plan writes it.
   Use `tools/engine_profile/bench_rv36/compare_hashes.py` (keyed by setup and seed). `record_benches.sh` with `FINAL_RV` only changes the configs, not the specs inside the recorded baselines.
   For the final native recording (`record_benches.sh <native> 6 36`) the specs say 36 and `verify_benches.sh` then works.
2. The string recorder accepts `--rules-version 36` and plays it as 35: bench-v1 at 36 gave the same 200 hashes as at 35. So S6/S7 are not gated by the version in the string.
3. Cost: native is about 3x the string (bench 1 741 CPU s, bench-v1 544 CPU s); string recording of all six is 54 CPU min.

## Resume

* Binaries (deleted at the end of this lane): rebuild with `CARGO_TARGET_DIR=... cargo build --profile gate -p ti4-selfplay --bin selfplay` in this branch and in a worktree of `core-integration-20`.
* Kept: `/root/deploytmp/rv36/` (17 MB): `base35/` (the six baselines and four configs at 35), `ver36-out/` (native outputs with `problems`), `texts2/` (text replays of the sample), `cmp.py`, `classify.py`.
* Next steps: (1) decide with the user whether S1 (and the rest of what the scan finds) is by design for the benches; (2) fix or port the 61 core stops (a game must not stop: zero-bail rule); (3) only then re-record natively at 36 in one commit
  (`record_benches.sh <native selfplay> 6 36`, then `verify_benches.sh` must give 200/200 x6 and the three `TI4_CHURN` bench steps likewise).
