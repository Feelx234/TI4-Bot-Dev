# Request for review — Astra, 2026-09-30

Four questions, in the order I think they matter. The first is the only one where I think something
might actually be wrong; the rest are judgement calls I do not think I should make alone.

Everything below was measured on this machine in the last few hours. Where I am inferring rather than
measuring, I say so.

## Where the code is

- `D:/Projects/ti4-engine-rs`, branch `wp/online-multiplayer`, HEAD `3f92016d` (pushed).
- **The working tree is dirty and shared with other sessions. Do not reset, clean, checkout, or
  prune anything.** `out/` is gitignored and holds every trained checkpoint; a recursive delete in
  this repo destroyed it once already (2026-09-08) and the loss is permanent.
- Local `main` is at `b1143a5b`, a merge commit I created but did **not** push. Rollback:
  `git update-ref refs/heads/main 22266e1e`.
- Build env: `LIBTORCH=D:/Projects/ti4-engine-rs/out/libtorch-2.9.1-cu128`,
  `LIBTORCH_BYPASS_VERSION_CHECK=1`, that `lib/` on `PATH`.
- `cargo test --workspace` **must** be run with `-j 4` or lower. At the default `-j 32` on this
  32-core machine, ~100 test binaries link libtorch statically in parallel, `link.exe` dies with
  `LNK1102` (out of memory), and the resulting missing rlibs surface as nonsense errors such as
  `cannot find macro println` in files untouched for two weeks. I wasted two runs on that.

Current state: `cargo test --workspace -j 4 --no-fail-fast` → **2558 passed, 4 failed**.

## 1. `ti4-sim` behaviour bounds: stale band, or a real invasion regression?

This is the question I most want a second opinion on, because I talked myself out of my own suspicion
and I am not sure I was right to.

```
behavior::tests::the_suite_reproduces_and_stays_within_the_recorded_bounds
metric share_INVASION_RESOLVED = 0.015906 is outside the recorded bounds
[0.018557, 0.019962] — diagnose before re-baselining (see module docs)
```

Invasions resolve about 15% less often than the bottom of the recorded band.

What I established:

- The bounds were last set in `c2566b99` ("Eight more recorded intervals to re-derive, and the note
  that said one").
- **40 engine commits** have landed since, six of which change combat or ground-combat lethality:
  `48e9ef39` Assault Cannon destroys an opposing non-fighter ship at the start of space combat;
  `b0693082` Graviton Laser System binds space cannon hits to non-fighter ships;
  `f75dd74f` Duranium Armor repairs a ship each space combat round;
  `f952b278` X-89 Bacterial Weapon doubles bombardment and ground combat hits;
  `0fc085e1` Hyper Metabolism, Light/Wave Deflector and Dacxive Animators;
  `db3829f9` L1Z1X agent, Plasma Scoring.

My reasoning was: Assault Cannon and Graviton Laser kill ships *before* an invasion can happen and
Duranium Armor keeps defenders alive, so fewer invasions reaching resolution is the expected
consequence, and the band is simply stale.

**Why I do not trust that reasoning.** It is a plausible story, not a measurement. Every one of those
six commits is a reason invasions might resolve *less* often, which makes the story unfalsifiable as
I have stated it — any drift in that direction would look explained. I did not attribute the change
to any specific commit, and I did not check whether the *other* recorded metrics moved in ways those
same commits predict. A genuine invasion regression (say, an invasion that now aborts early instead
of resolving) would look exactly like this.

**What I would like:** whether "re-derive the band and record the six commits as the cause" is
adequate, or whether this needs attribution first — and if so, the cheapest attribution that would
satisfy you, given I cannot create a worktree or switch this tree's branch to bisect. My own standing
instruction is that these bounds are re-baselined only with the operator's approval, so nothing has
been changed.

## 2. A replayer defect I fixed — is the fix in the right layer?

This one is fixed and tested; I want the design checked, because I chose the cheaper of two layers.

**Symptom.** With a seat on Manual, the replayer window would show "No seat is waiting" while the
engine sat parked inside `Table::ask` — a frozen game with no panel and nothing explaining it. The
operator has reported symptoms of this shape repeatedly ("branching does not work at all", "setting
other seats to manual mid game does not work").

**Cause.** `ChoiceFingerprint` binds actor, prompt, ordered `(id, kind)` pairs and typed context, and
deliberately **excludes** frame and ask ordinal — that exclusion is what lets a rebuild match a
recorded answer to the offer it was made against. But the engine legitimately raises asks that are
identical in every bound field. Measured, frame 107 of the example table:

```
ask 0  seat0  "end your turn"                       [end_turn, component|trade|hacan]
ask 1  seat0  "remove a unit: over capacity in 14"  [remove|0]   ctx None
ask 2..6      the same, byte-identical
```

A seat six units over capacity is asked six times, each with one option and no context: seven asks,
two distinct fingerprints. `ReplayApp::pending()` suppressed the panel whenever the pending
fingerprint equalled the last answered one, so asks 2–6 were treated as already answered and never
shown.

**Fix.** The app now identifies an answered offer by *instance* — `(fingerprint, frame, ask)` — not
by shape. `submit()` still refuses a genuine double-click (including when the gate has already
cleared `pending`, which is the normal case for a second click), but a new ask that merely looks
identical reaches the panel. Pinned by
`app.rs::an_identical_looking_ask_at_the_next_ordinal_is_a_new_question`, which asserts the fixture
actually reproduces the fingerprint collision so the test cannot silently stop testing anything.

**The alternative I rejected:** putting `(frame, ask)` into `ChoiceFingerprint` itself. That would
make the collision impossible everywhere rather than in one caller, but the fingerprint is also the
key a rebuild matches recorded answers by, and `control.rs` documents the frame-insensitivity as
deliberate. I judged the risk to replay matching too high for the benefit.

**What I would like:** whether instance identity belongs in the app or in the primitive. If it
belongs in the primitive, the replay-matching consequence is the part I could not convince myself
about. Related: the gate's own `ManualControl::submit` still validates by fingerprint alone, so a
click made for ask 1 arriving during ask 2 would be accepted by the engine. In this case both asks
offer the same single option so the outcome is identical, but I do not think that is a guarantee —
it may be a second, deeper instance of the same bug that I have left in place.

**I also changed a test assertion, which I want checked.** `live.rs`'s
`nested_asks_inside_one_engine_step_each_get_their_own_panel` asserted that asks in one step have
distinct fingerprints. That is false of the engine, as above. I replaced it with distinctness of the
ask *ordinal*, which is what catches the failure mode the assertion was protecting against (the gate
republishing one offer instead of parking again, which would repeat an ordinal). If you think that
weakened the test, say so — this is exactly the "do not edit a test to make a failure disappear" case
and I may have talked myself into it.

## 3. Three stale expectations: re-baseline, or fix properly?

Each of these fails, each looks like obsolescence, and in each case the honest fix is to change a
recorded expectation. I have changed none of them.

**`ti4-review` semantic golden** (`the_reviewers_default_path_matches_the_frozen_pre_r02_golden`).
The engine now offers a 4th option at frame 8 that the frozen golden does not have —
`component|trade|hacan` / `open_transaction` / "open a transaction with hacan" — plus float drift in
the 6th decimal of scores (`8.450350` → `8.450354`). The golden's purpose was to prove the R02 panel
extraction did not change R01 behaviour; it passed at the time. It now encodes an engine that no
longer exists. Re-baseline with the diff recorded, or is the frozen-pre-R02 guarantee still worth
something?

**`ti4-mlp` `smoke_refusals::a_pool_without_an_allowed_manifest_role_is_refused`.** Two layers of
data obsolescence, neither fixable by a test edit:

1. `out/vocabulary/current.json` names a generation whose provenance is a post-disaster
   reconstruction carrying literal sentinels — `"checkpoint_sha256":
   "UNRECOVERED-2026-09-08-out-directory-loss"` and two more — so `accepted_generation()` rejects
   the pointer and `mlp_smoke` refuses at `--slots` before it can reach the pool check the test is
   about. That provenance file argues, correctly in my view, that an invented plausible value would
   be worse than an obviously absent one.
2. Passing the committed bundle explicitly does not help either:
   `REFUSED: slots.json does not load: slots.json declares OOV registry version 10, but this build
   supports 11`. The committed `examples/reviewer/checkpoint-473312/slots.json` **is** the accepted
   generation (its SHA-256 is the generation id), and it is a registry version behind the build.

So there is no vocabulary artifact on this machine that `mlp_smoke` accepts. Options: regenerate at
v11 (a publish task touching gitignored data), restructure the test not to need a vocabulary, or
leave it red and documented.

**`ti4-training` `stage1::faction_training_starts_sparse_and_grows_named_weights`.** Asserts some
trained weight has a name not starting with `h`. `f7b2716c` ("perf: key feature vectors by hash
instead of by name (1.48x)", 2026-08-17, in HEAD) made `learned::bucket()` name every weight
`h{index:04}`, so the assertion cannot hold; the failure itself proves every name is hashed. I would
replace it with "one generation moved at least one weight off zero", which is true and still means
something — but tell me if there is a property worth keeping that I am about to lose.

## 4. Is this merge fit for `main`, and by what standard?

`main` locally is `b1143a5b`, unpushed. The merge itself is unusually safe:
`git merge-tree` reports no conflicts and the merged tree is **bit-identical** to `3f92016d`, because
`origin/main`'s diplomacy PR (`c15f4c04`) contained nothing this branch lacks. It is 127 commits of
branch work plus that one merge parent, and it changes no file. Done entirely with
`commit-tree`/`update-ref`, so no checkout and nothing on disk was touched.

`AGENTS.md` asks for the full workspace suite and a frontier review before a merge this size. The
suite is 2558/4. The operator has authorised the push; I have held it because the four failures were
not known when they authorised it, and because `main` is the trunk.

**What I would like:** whether 4 known-stale expectations is an acceptable trunk state if each is
documented, or whether they should be resolved first. And whether "the merged tree is bit-identical
to a commit that has been built and trained on all day" is adequate substitute verification for the
fact that this merge commit will never be built as a checkout — I cannot create a worktree or switch
this tree's branch, so it cannot be.

## What would help most

Question 1 first: if the invasion share is a regression rather than a stale band, everything else
here is noise by comparison, and it is the one place I chose the comfortable interpretation.
