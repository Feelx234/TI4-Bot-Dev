# Operator report 2026-09-20 — seven more, triaged against code where I could

Same split as yesterday: **UI experience** (things the window makes hard) and **bugs** (rules or
legality). `[verified]` means I read the code this session; `[unverified]` means it is a hypothesis and
said so; `[known]` means it already has a package or tracking file.

Everything already done from the 2026-09-19 report is marked in that file: seat labels, attachment
naming, Guild Ships legality, and the technology prerequisite guard.

## The order I propose, and why

| # | Item | Type | Why here |
|---|---|---|---|
| 1 | Transaction windows: notes for notes, and paying what is owed | engine legality | both are "the option that should exist doesn't", in a file I just fixed, with 3.4 s tests |
| 2 | Strategy card shown unpicked after being picked | viewer truth | one field, wrong on screen right now |
| 3 | Warfare asks production before spending from strategy | engine flow/ordering | ordering inside one sequence; small, but it is the engine's turn structure |
| 4 | Planet rider: refreshed/exhausted resource + influence totals, trait and specialty breakdown | viewer | their most concrete display request, and it answers questions the map cannot |
| 5 | Diplomacy tooltips carrying the actual terms | viewer | needs the terms that already exist in the frame |
| 6 | Probabilities shown to the choosing player | viewer + a rule | doable, but see the honesty guard under item 6 |
| 7 | L1Z1X agent | `[known]` LEADER-FIX-002 | unchanged: the engine never opens the activation window |
| 8 | Exploration cards explicitly resolved | viewer | largest of the display items; see below |

---

## UI experience

### 1. Exploration cards have to be explicitly resolved

`[verified]` Neither viewer mentions exploration at all: no `grep` hit for `exploration` in
`crates/ti4-review/src` or `crates/ti4-replayer/src`. The engine's anomaly/ancient-ruins/lost-planet
cards are asked about and answered, and the frames simply do not show it.

This is the biggest item in the file. What "explicitly resolved" needs is a decision I should not make
alone: whether the replayer shows the *draws* (which card came up, whether it was resolved, what was
gained), an explicit per-card state (unexplored / revealed / resolved / refused), or a pending-actions
lane for anything the engine asked and the seat has not answered. The first is presentation of recorded
facts; the last is a new concept in the frame. I would start with the first and see whether the second
becomes necessary.

### 2. Seat labels — done, with a caveat

Now `seat2 (The Emirates of Hacan)` (`view::seat_name`), falling back to `seat3 (unknown_id)` rather than
inventing a name for a faction the content store does not know. `[verified]`

### 3. Attachments still do not say what they do — and I cannot make them say it

`[verified]` All 22 records in `crates/ti4-content/content/attachments.json` carry `id`, `name`, `source`,
`techSpeciality` and **no rules text at all**. The viewer now prints names instead of
`attachment_fragment_count`, which is the last thing the view layer can do honestly.

Two separate things remain, and they are not mine to invent:
- the corpus needs the text, sourced from the official rules, as a `text` field;
- and then an engine test that an attachment **does** something, which is the half of their question
  ("maybe attachments are broken") I cannot answer from a name. The engine reads attachment fragments in
  `recovery` (a named mechanic, and that is why the HUD counts them), but whether each specific
  attachment's effect is implemented is an open question this repository has not answered — see
  `plans/BUG_2026-08-30_STUB_AND_CONTENT_GAPS.md`.

### 4. Diplomacy tooltips need the terms

`[unverified]` I have not read the current diplomacy tooltip, but `Terms::describe` already produces a
spoken leg ("3 trade goods, Mercantism, a promissory note from the Arbiter"), and the engine already puts
`terms` into the action summary. So this is wiring, not design. Until it is done, the replayer shows a
deal without saying what was on the table — which makes every diplomacy question in this file
unanswerable after the fact.

### 5. Show the engine's probabilities to a player who is choosing

Worth doing, and the risk is specific rather than general: the policy model's number and a probability
are different quantities, and `plans/DIPLOMACY_HONESTY.md` exists precisely because a number that looks
like a chance will be trusted like one. The rule I would hold to, and put in the UI text itself:

- label **what the number is** ("policy preference out of 1000", "softmax share of the offered options",
  "rollouts this seat won out of N");
- never print one as "win chance" unless rollouts actually produced it;
- keep the number off the game state, so it cannot leak into a save or change what the engine chose.

`p-choose` already surfaces policy scores for diagnosis, so the plumbing exists.

---

## Bugs

### 6. Notes for notes with Hacan — found, and it is not Guild Ships

`[verified]` `transactions::available_actions` begins:

```rust
if state.diplomacy.enabled {
    return Vec::new();
}
```

and `crates/ti4-engine/src/diplomacy/candidates.rs:1201` and `:1252` are tests that **assert** it: with
structured diplomacy on the standalone transaction window offers nothing; with it off it offers the
partners. So the engine has already taken the architecture decision recorded in
`plans/ENGINE_DIPLOMACY_UNIFICATION.md` — *transactions are a leg of diplomacy* — and taken it
unilaterally, **without delivering the substitute**: when diplomacy is enabled, a seat transacts only
inside a diplomatic contact, and if no contact window arrives for that partner, transacting is not merely
hard, it is unreachable. The operator turned diplomacy **on** in the new setup form, which is the only
reason they are seeing this now.

That explains the whole shape of their report: "in spite of Hacan ability… or in general", and today's
"needs full transaction availability". Guild Ships is a separate, smaller bug and it is fixed (the offer
list asked `partners`, `why_illegal` asked `are_neighbours`, commit `09409d7`), so a Hacan seat now at
least stops being refused after it accepts.

What is left is a decision rather than a patch, and it is the first real argument for `DIPLO-001`:

- **the right fix** is delivery — a seat that could transact with a partner must be offered the contact
  that carries the trade; that is DIPLO-001 plus the LEADER-FIX-002 window problem, and it is the work I
  would schedule next;
- **the interim fix** is to let `available_actions` keep offering transactions while diplomacy is on,
  which restores capability immediately but deliberately breaks two passing tests and gives the table two
  negotiation windows again — the thing the unification was meant to stop. I am not doing that on my own
  authority; say the word and it is an hour.
- **this week's workaround**, which needs nothing from me: leave the diplomacy checkbox off and Hacan
  transacts normally, Guild Ships and all.

One more number for the same pile, still needing the rule text rather than code: the per-partner limit is
enforced here in generation (`state.transacted_with(player)` filtering `partners`), not in `why_illegal`,
and `transactions::neighbours_who_transacted` (transactions.rs:210) counts only *neighbours* — so a
Guild-Ships partner outside the neighbourhood may sit outside that count. Whether a limit worded about
neighbours should bind a non-neighbour is LRR 60's question, not mine.

### 7. Politics shown as unpicked after it was picked

`[unverified]` Likely a viewer truth bug: the strategy-card row must be reading "was this card chosen this
round" from the wrong place (round state vs the assignment event), or reading the state at the wrong
frame. First move is to reproduce it in a test against a frame where a card was picked, then look at which
field the row uses. If it turns out the frame never recorded the pick, that is a `ti4-game-shell`
projection gap instead, and it is worth fixing there so both viewers get it.

### 8. Refresh for 1 commodity was offered, paying the due was not

`[unverified]` This reads as the due/refresh sequence generating only one of its two branches. The rule is
that a player who cannot pay a due may refresh instead — so at the moment of the due there are at least two
legal answers, and the engine offered one. That is exactly the shape this project treats as a bug ("legal
actions are generated, not accepted by late rejection"), and it is also the operator losing an option
rather than gaining one. First move: reproduce the due with a seat that has too few trade goods and assert
both answers are present.

### 9. Warfare asks what to produce before asking whether to spend from strategy

`[unverified]` An ordering defect: spending from the strategy card has to be decided before the production
budget is known, so asking production first either wastes the spend or makes the produced set wrong.
Engine-flow, in the invasion/warfare sequence. This is a real behaviour change and I would not move an
existing ask without a test that pins the current order first.

### 10. Transactions during the action phase — noted as a constraint

Understood, and recorded rather than acted on: whatever happens with
`DIPLO-001..004` in `plans/ENGINE_DIPLOMACY_UNIFICATION.md` must not be sold as making action-phase
transactions easy. The related open question I will not guess at: the engine's per-round transaction record
filters through *neighbours* (`transactions::neighbours_who_transacted`, transactions.rs:210, one consumer
at action_cards.rs:4508), and there is no "already transacted with this partner" error variant at all.
Whether that is correct under LRR 60 needs the rule text, and the answer decides whether a Guild-Ships
partner counts toward a limit worded about neighbours.

---

## How to make the next report faster for both of us

Three of the ten items above would be settled by a saved file rather than an hour of reading code. The
replayer writes one when the window closes; with it I can say, for a specific seat and round, which
prerequisites it had, what its planets' specialties were, which laws were in play, what it was offered,
and what it answered — and write a failing test at that exact moment instead of guessing at a
reproduction. What I need is the `.ti4replay` (or the recording directory) plus one sentence per item:
which seat, which round, what it expected instead.
