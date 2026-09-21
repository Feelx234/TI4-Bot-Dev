# Trade rework — deals built item by item (2026-09-22)

Replaces OP-06 (Guild Ships trade too limited) and answers BUG-04 with option A: with diplomacy on,
the contact is the only negotiation window and it can express every legal transaction.

## Operator decisions (2026-09-22)

1. During the action phase every negotiation involves the active player.
2. Bots and humans use the same step-by-step builder. Shortcuts (like fleet-move packages) may be
   added on top later.
3. At most 2 counter-offers.
4. Rules as written: any number of trade goods / commodities / fragments, **one promissory note per
   player per transaction**.
5. One-sided deals ("gifts") are allowed: with future payment in the model, a deal that looks like a
   gift now is often paid for later.

## What changes

Today a contact offers a capped flat menu of pre-built bundles from fixed templates
(`diplomacy/candidates.rs`). That menu is why Hacan sees an arbitrary assortment and cannot build
the deal it wants. It goes away; the window builds the deal instead.

### The window (`diplomacy/window.rs`)

`DiplomacyStage` becomes:

| Stage | Who is asked | Options |
|---|---|---|
| `Offering` | the side building | `add\|<item>` for each item still addable · `done` · (first step only) signals, `make no offer` |
| `Quantity` | same | `amount\|1` … `amount\|n` for a counted item just picked |
| `Asking` | same | `ask\|<item>` for each item the partner could still give · `done` |
| `Review` | same | `propose` (needs ≥1 term on either side) · `restart` · `cancel` |
| `Responding` | the other side | `accept` · `decline` · `counter` (only while fewer than 2 counters) |

`counter` re-enters `Offering` with the roles swapped, pre-filled with the terms on the table; the
builder may add or remove items (`remove\|<item>` is offered for items already on the side).
Accept on a revision stores it exactly as today (`create_deal` / `add_counter` / `apply_acceptance`),
so promises, deadlines, breaches and the journal are untouched.

### Items

| Group | Items | Offered only when |
|---|---|---|
| Physical — a transaction (LRR 94) | trade goods, commodities, relic fragments, one promissory note per side, action cards (Hacan Arbiters) | the pair may transact: neighbours (shared/adjacent system, wormholes count), Guild Ships, Trade Convoys held by either side; or the agenda phase. And not already transacted this turn. Checked with the existing `transactions::why_illegal` on the whole deal before `propose` is offered |
| Promise — no transaction | don't attack until round N · don't activate system S · vote X on the current agenda · pay later (asset, round) · attack seat Z · replenish commodities for a seat · use an agent for a seat | always, subject to the existing duplicate-obligation check |

Counted items take a second `Quantity` choice. Deadlines take one choice between this round and next.
Candidate lists for promises reuse today's generators (attack targets, systems worth naming, agendas),
item by item rather than pre-combined.

### Who may negotiate

- Action phase: one party must be the active player (`available_contacts` filters; an incoming
  contact from a non-active seat to a non-active seat is never offered).
- Agenda and strategy phases: unchanged from today's gating.
- The per-turn initiation allowance and the once-per-partner transaction limit stay as they are.

## Consequences

- **Bots**: one negotiation is now 4–12 decisions instead of 1–3. The policy has no training on the
  builder heads; expect poor deals until retrained. Decision contexts get new subtypes
  (`diplomacy_offer_item`, `diplomacy_ask_item`, `diplomacy_quantity`, `diplomacy_review`) so features
  can tell them apart.
- **Baselines**: ti4-sim behaviour and the reviewer golden move again (diplomacy is off in the sim
  suite, so the sim may not; the replayer tests with diplomacy on will). Folds into the pending
  re-baseline.
- **Legacy transaction window**: stays for games with diplomacy off. With diplomacy on it remains
  unavailable, as today.
- **Replayer**: the choice panel shows the deal so far (both sides) above each builder question.

## Phases

1. Engine: builder stages in `DiplomacyWindow`, item catalogue, legality, tests (offer/ask/quantity,
   one note per side, neighbour gating incl. Guild Ships and Trade Convoys, active-player rule, two
   counters, gift allowed). Bundle generation removed from the contact.
2. Replayer: deal-so-far panel.
3. Policy features for the new subtypes; retraining is a separate run.
4. Shortcuts: pre-filled templates as `quick|<template>` options at the first `Offering` step.
