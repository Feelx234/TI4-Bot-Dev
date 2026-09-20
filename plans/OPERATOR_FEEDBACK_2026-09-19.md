# Operator feedback 2026-09-19 — UI experience and four legality bugs

Recorded after the operator drove a hand-played table in the replayer. Nothing here is fixed. Each item
says what was **verified in the code today**, what "done" would mean, and what would be needed to pin it.
Order of the work is at the bottom, and it is deliberately not the order the items arrived.

## UI experience

### 1. Exploration cards have to be explicitly resolved and shown

Verified: `grep -rn "exploration" crates/ti4-review/src/view.rs crates/ti4-review/src/panels.rs
crates/ti4-replayer/src/gui.rs` returns **nothing**. The engine has `exploration.rs`, content has
`explores.json`, and neither viewer mentions an explore in any panel, tooltip or event row. When the game
explores, the operator sees no card, no choice, and no result — which for a hand-played table means
"something happened to my resources and I cannot tell what".

Done means: an explore that needs a decision reaches the player as a real decision (so a Manual seat is
asked, and R02 records it for replay), and an explore that resolves automatically appears as a new-event
row naming what was gained (relic / technology / unit / fatigue) and where it went. Both apps share the
sheet, so one row builder in `view.rs` serves both.

### 2. Seats should read `seat4 ("Clan of Hacan")`, not `seat4`

Verified: `view::seat_label` (view.rs:119) takes pre-made text, and the replayer's chips and player panel
use the raw `PlayerId` (`format!("{seat} · {mode}")` in `Replayer::seat_chips`). The faction is in the frame
(`state.players[].faction`), and `crates/ti4-content` resolves a faction id to its printed name — the
reviewer's board legend already does the colour half of this.

Done means one helper, `seat_name(frame, player) -> "seat4 \"Clan of Hacan\""` (raw id when no seating is
known yet, which is a real state before the first frame), used by chips, the seat row, the choice panel's
actor line and the diplomacy rows. Cheap and it removes a whole class of "which seat is mine" confusion.

### 3. Attachments show a count and nothing else — and may be broken

Verified: `view.rs:632` has `pub attachments: usize`, drawn at `view.rs:1624` as `"+N"`. The state carries
`planet_attachments` — so **which** attachment a planet has is known and is thrown away on the way to the
screen. Whether the attachment's *effect* applies is a separate question and is not claimed either way
here; the operator's "maybe attachments are broken" is a legitimate open question, and the current display
cannot distinguish "attached and working" from "attached and inert".

Done means: name each attachment on the planet it is on, with its rules text from `attachments.json` (the
content corpus already has it — see `out/six-faction-abilities.html` for the same extraction), plus an
engine test that at least one attachment's effect actually changes a rule outcome. Until that test exists
the honest label in the UI is "attached", not "working".

### 4. Diplomacy tooltips need to say what the deal actually is

Verified: `panels::diplomacy_sheet` prints journal rows and `decision_sheet` prints option titles, and my
own replayer tooltips are generic prose about how a seat works. Nothing in the tooltip states the concrete
payload — who offers what to whom, what accepting costs, what the initiation budget is, what happens on a
counter.

Done means the tooltip is generated from the deal: parties, terms on each side (via `Terms::describe`,
which already produces the right words), the limit that would be hit if it were refused, and the reason an
option is disabled. That is also the cheapest way to make bug reports like "transactions not possible"
actionable, because the operator reads the actual constraint instead of inferring it.

### 5. Show the engine's own probabilities to a choosing player

The policy already scores the offered options (R01's decision rows carry score and rank, and the semantic
golden pins them). Presenting that at the moment of choice — a probability or ranked share beside each
option, with the number of samples and the temperature the run used — is a view change, not an engine
change, and it needs one honesty guard: a temperature-shaded sample is not a probability, so whatever the
UI prints has to say which it is (`policy score`, `softmax share`, `win probability from rollouts`) rather
than silently relabelling a score as a chance.

## Legality bugs

These four are a mixed bag, and two of them look related. None was reproduced in this session; each line
names the code to look at first.

### B1. Hacan: cannot exchange a promissory note in a transaction

`Terms` has a `promissory: Option<String>` field and `why_illegal` (transactions.rs:310) reasons about it,
and `plans/BUG_2026-08-29_PROMISSORY_NOTE_TRANSACTION_OFFERS.md` is marked **FIXED 2026-08-31** — it made a
note the partner cannot afford get offered as a *gift* (`pn{note}:0`) rather than be withheld. So either the
fix does not cover the operator's case, or the case is the *diplomacy* path rather than the transaction
path — which is the second time in two days that two negotiation machines have produced "impossible" with
no single place to look. `plans/ENGINE_DIPLOMACY_UNIFICATION.md` is the fix for the confusion; a failing
test that a Hacan–X transaction can name a note is the fix for the bug, and it should be written against
whichever path the operator's saved game shows was offered.

### B2. Non-Euclidean shielding offered to Letnev without prerequisites

Letnev's abilities are `munitions` and `armada` (verified in the content corpus) — **no prerequisite
waiver**. Offering a technology whose prerequisites are not met is an illegal option being generated,
which the project's own rules treat as a correctness failure, not a display defect. Code to read:
`faction_abilities::waived_prerequisites` (faction_abilities.rs:163), its test
`a_waived_prerequisite_reaches_what_can_actually_be_researched` (line 1153), and whatever produces the
research option list — note that grepping for `research_options`/`available_technologies` finds nothing,
so the producer is named something else and is the first thing to locate. Confounders to rule out from the
saved game before writing code: a granted technology (`entropic_scars.rs` grants ignore prerequisites by
design), an agenda effect (`agenda_effects.rs` has Research-Teams-style waivers), or a `Warp`/proxy prereq.

### B3. Space Dock II offered to Jol-Nar without two yellow

Same class as B2 and probably the same line. The difference is that Jol-Nar legitimately waives **one**
prerequisite (`analytical`), and `plans/BUG-001_ANALYTICAL_RIN_UPGRADE_EXCLUSION.md` already exists about
that waiver's boundary with unit upgrades. If the operator's seat had one yellow, offering a
two-yellow technology is correct; with zero it is not. The test has to pin the arithmetic — waiver applied
once, to a non-upgrade, against the *actual* prerequisite count — rather than "no upgrades ever".

B2 and B3 should be taken as one package: **research offers generate only what is legal right now**, with
the waiver as an explicit, counted, per-faction input. Two factions and one rule.

### B4. Transactions during the action phase — recorded as a constraint, as asked

The operator's words: *"transactions can happen during actions, this is hard to implement and should
primarily been noted."* Recorded, and it deserves to be said in the open rather than discovered:

- By the rules, a transaction is not an action and does not consume the action; it can be initiated during
  the action phase subject to the once-per-turn and once-per-player-per-turn limits
  (`transactions::neighbours_who_transacted`).
- Implementing that honestly means a negotiation window nested inside an action — a seat can be asked to
  negotiate between the steps of its own activation, and the engine must pause, ask, settle, and continue
  without losing the activation it was in the middle of. R02 already nests asks inside one engine step
  (measured: 400 steps raise 506 asks), so the machinery exists, but the transaction path is a *separate*
  machine and its nesting is not the same code.
- Consequence for the replayer: every nested negotiation must be recorded as a decision with its frame
  stamp, or a forked branch will not replay — see the inclusive prefix cut in R02-007d, which was exactly
  about answers stamped at the frame they belong to.
- Until B1/`ENGINE_DIPLOMACY_UNIFICATION` land, a table where transactions are unavailable is
  indistinguishable, from the seat, from a table where the operator did something wrong. That is the
  thing to fix first, and it is the same thing item 4 asks for.

## Order of work, and why

1. **B2 + B3 as one package** — illegal options being offered is the only category here that can silently
   corrupt a game, and the project treats legality as non-negotiable. Failing tests first, in the engine,
   using the operator's saved game as the seed for the fixtures.
2. **B1** — with `plans/ENGINE_DIPLOMACY_UNIFICATION.md` as the shape and the transaction tests as the
   content. Decide the path (transaction vs diplomacy) from the recording, not from preference.
3. **Items 2 and 3** — seat names and attachment detail. Both are small, both are in the shared sheet, and
   both directly reduce "what am I looking at".
4. **Item 4, then item 1** — deal tooltips and then exploration surfacing; item 5 (probabilities) last,
   behind its honesty guard.

Blocked on one thing: **the saved game** the operator was playing. B1–B3 are all "the engine offered or
refused a specific thing at a specific moment", and the recording carries the moment, the phase, the offer
list and the refusal. Without it these are hypotheses with good provenance rather than reproductions.

---

# Fixed the same day (items 2 and 3, partly)

Two of the UI items needed no reproduction and no engine change, so they were done rather than filed.

**Seats read `seat2 "The Emirates of Hacan"`.** `view::seat_name(frame, player, content)` resolves the
seat's faction through the content store and keeps the raw id first, because the id is also a stable
address. Before seating exists it returns the bare id - a real state, not a failure - and a faction this
build does not know prints `seat3 (some_faction_id)` rather than a name invented in a view layer. Wired
into the replayer's seat chips, which was the surface the operator named; **R01's strings are untouched.**

**Attachments are named, not counted.** The row read `· 2 attachment(s)`; it now reads
`· 2 attachment(s): Biotic Research Facility, …` via `view::attachment_names`, with an unknown id shown as
that id.

That is as far as item 3 honestly goes today, and the reason is worth recording because it is bigger than
the row: **the content corpus has no rules text for attachments at all.** All 22 records in
`crates/ti4-content/content/attachments.json` carry `id`, `name`, `source` and `techSpeciality` — nothing
else. So "what the attachment provides" cannot be printed from this engine's data; the name is the ceiling
here, and the effect would have to be added to the corpus from the official rules, or found to be missing
in the engine as well. That also sharpens the operator's *"maybe attachments are broken"*: there is no
text in the data to display, and no test in the corpus that an attachment changes a rule outcome. The next
step for item 3 is a content-schema addition (a `text` field, sourced from the rules) plus one engine test
that an attached thing does something — not more string formatting.

```text
cargo test -p ti4-review   -> 85 passed (was 83; + a_seat_is_named_by_the_faction_it_is_playing,
                                 + attachments_are_named_not_counted)
cargo test -p ti4-replayer -> 63 across lib/app/shell/table, the fast targets; the 12 rebuild tests
                               were last green at db7a220 and nothing in this change reaches them
cargo clippy -p ti4-review -p ti4-replayer --all-targets --no-deps -- -D warnings -> 0
cargo fmt -p ti4-review -p ti4-replayer --check                                   -> clean
```

---

# B1 fixed: Guild Ships genuinely did not work — the operator was right

> the hacan one is straight up guild ships not working

It was. Two lines disagreed, and the seat experienced the disagreement as a dead button.

- The **offer** path asked `transactions::partners`, which consults `faction_abilities::ignores_neighbours`
  — Hacan's Guild Ships — and `promissory::reaches_anyone` (Trade Convoys). So a Hacan seat was correctly
  **offered** a partner on the far side of the board.
- The **legality** check, `why_illegal`, asked bare `are_neighbours` and returned
  `OfferError::NotNeighbours`. So the moment the offer was accepted, the engine refused it.

That is the pattern this project's rules call out by name — a legal action rejected late rather than
generated correctly — and it is invisible to either half read alone, which is how it survived a passing
`partners` test in `faction_abilities.rs`.

### The test, which failed first

`transactions::tests::guild_ships_let_a_hacan_seat_deal_with_a_stranger` — two fleets placed so they are
provably not neighbours (the test asserts that, so it cannot pass vacuously), seat `a` given the Hacan
faction, a two-trade-good offer:

```text
Guild Ships reaches across the board; got Err(NotNeighbours(PlayerId("a"), PlayerId("b")))
test result: FAILED. 0 passed; 1 failed
```

### The fix

`transactions::may_transact(state, content, galaxy, proposer, partner)` — neighbours, **or** either side's
`partners()` reaching the other — and `why_illegal` asks that instead of `are_neighbours`. Both directions
count because the ability belongs to whoever holds it and a negotiation has two parties; this cannot make
legality looser than generation, because generation already offers from `partners`.

### Verification

```text
cargo test -p ti4-engine --lib                      -> 1342 passed, 0 failed
cargo test -p ti4-engine                            -> 1352 passed, 0 failed (all targets)
cargo clippy -p ti4-engine --all-targets -D warnings -> 16 errors, same as the baseline with this
                                                       change stashed: this fix adds no finding, and the
                                                       engine is not currently gated clean at that level
cargo fmt -p ti4-engine --check                      -> clean
```

### Two things this does *not* settle, so nobody assumes it does

1. **The per-round transaction limit counts neighbours.** `transactions::neighbours_who_transacted`
   filters `transactions_this_round` through `neighbours()`, and its one consumer is an action card
   (`action_cards.rs:4508`). Whether a Guild-Ships partner should count toward a limit that is worded
   about neighbours is a rules question, and this fix deliberately leaves the behaviour alone rather than
   guess alongside the fix it was asked for.
2. **`why_illegal` has no "already transacted with this partner" check at all** — no such error variant
   exists in the engine. That may be exactly right under LRR 60, or it may be a second gap sitting next to
   the first. Settling it needs the rule text, not more code, and it should be settled before the
   transactions-under-diplomacy work touches this file.

A discipline note while it is fresh: `guild_ships_makes_the_whole_table_a_partner` begins
`let Some(hacan) = faction_with("guild_ships") else { return; }`. If that fixture lookup ever fails the
test **passes silently**. It was not the cause here — `partners` was right all along — but a test that can
opt out of being a test is how a bug like this stays alive next to a green tick.

---

# B2/B3: the clean case is correct in the engine, so their tables hold the answer

A property test now sits in `technology.rs`: **a seat with no technologies, no specialties and no laws is
offered only the prerequisite-free part of the corpus** — `researchable` filtering through `can_research`,
asserting that every offered alias has an empty `prerequisites` map. It **passes**, and it also asserts the
offer list is not empty, so it cannot pass by offering nothing.

```text
cargo test -p ti4-engine --lib -> 1343 passed, 0 failed
cargo clippy -p ti4-engine --all-targets -D warnings -> 16 findings, the unchanged baseline; this
                                                        package adds none
```

So the engine does not hand a blank seat a technology it cannot legally research. What *can* legitimately
open a hole — and what the operator's two reports most likely ran into — is one of these, each of which the
engine implements on purpose:

- **Planetary tech specialties** (90.8): a specialty stands in for one prerequisite of that colour, and
  Letnev's home worlds and several blue-specialty planets supply exactly one blue.
- **Research Team laws**, **Prophet's Tears**, and **Jol-Nar's Brilliant/Analytical** waivers — all spend a
  budget of "ignore 1 prerequisite", counted in `can_research` rather than per colour. For B3 the whole
  question is arithmetic: Space Dock II against one yellow plus one waiver is legal, against zero yellow it
  is not.
- A **granted** technology (`technology::grant`) never checks prerequisites, by design (90.5), and effects
  that offer "a technology of that colour" (Specialist Compounds) offer from the same `researchable` list.

What would settle it in one read is the recording: seat, round, the technologies it held at the moment, its
planet specialties, and the laws in play. Without it, B2 and B3 are unconfirmed — and now guarded against
regression in the case that *is* decidable.

This is also the strongest argument in this file for item 4: if the offer row said *why* a technology is
available — "prerequisites met by your specialty on Almaar", "one prerequisite ignored by Analytical" — the
operator would not have needed to file a bug to find out whether the engine was wrong.
