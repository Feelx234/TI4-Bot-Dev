# Engine decision — one negotiation system: transactions are diplomacy

Date: 2026-09-19. Authority: operator, on the record for the bugs they named ("for the concrete ones I
named you are allowed to edit what's necessary", then "transactions should be subsumed under diplomacy").
Status: **accepted decision, not yet implemented.** Nothing in this file is claimed as done.

## The decision

A TI4 transaction — two players swapping trade goods, commodities, relic fragments, promissory notes, and
where some card allows action cards and unscored secret objectives — is **one negotiation, implemented
once, in the diplomacy system**. `crates/ti4-engine/src/transactions.rs` keeps its rules and its arithmetic
and stops being a second state machine: its `Terms` become the payload a diplomatic deal carries, and its
parallel `TradeWindow` / `open_transaction` / `offer` / `transaction` decision kinds are retired in favour
of the diplomacy offer/response/counter/accept path.

## Why, stated as a defect and not a preference

Today the engine runs **two negotiation machines side by side**:

| | `transactions.rs` | `diplomacy/window.rs` |
|---|---|---|
| decision kinds | `open_transaction`, `offer`, answer kind `transaction` | `diplomacy_offer`, `diplomacy_response`, `diplomacy_counter` |
| state machine | `TradeWindow` | `DiplomacyWindow` |
| payload | `Terms` (goods, commodities, fragments, note, action card, secret) | deal terms of its own |
| legality | LRR 60 / 21.5: neighbour presence, `can_pay`, `why_illegal`, once-per-player-per-turn | its own gates, `consume_initiation`, promises, relations |
| journal | none | `state.diplomacy.journal` |

Two machines that negotiate the same thing is exactly the class of defect the operator reported as
*"transactions with hacan not possible, generally diplomacy buggy"*: a seat has to be asked by the right
one of two machines, a viewer has to render two vocabularies, a policy has to be taught two shapes, and
the parts that live on one side — the diplomacy journal, promises, relations, the per-pair initiation
budget — cannot see the other side at all. The transaction rules themselves are not the suspect:
`Terms::describe`, `can_pay`, `why_illegal` and `partners` (which already handles Hacan's
"not your neighbour" case, with a test that Guild Ships reaches the whole table and Sol reaches nobody) are
specific and tested.

## What must survive the merge, exactly

These are the invariants; a change that breaks one of them is wrong even if the new shape is nicer.

1. **Rules legality, unchanged and exact.** LRR 60 neighbour presence with Hacan's and the note's
   exceptions (`partners`, `ignores_neighbours`, `reaches_anyone`); `can_pay`/`why_illegal` verdicts; the
   once-per-turn and once-per-player-per-turn limits (`neighbours_who_transacted`); 94.3's action-card ban
   with Hacan's Arbiters as the only exception (`trades_action_cards`); unscored secrets only where the
   card allows; a promissory note costing differently to give than to receive, because each one says
   "then, return this card".
2. **Determinism.** No iteration-order, clock or locale dependence. `Terms` fields are already ordered
   collections (`Vec`, `BTreeMap`, `BTreeSet`) — the merged prompt and the merged state must keep stable,
   sorted renderings so the same deal always hashes the same.
3. **The answer log stays replayable.** R02 rebuilds a branch prefix from recorded decisions
   (`plans/R02_REPLAYER.md`, evidence R02-004/R02-007d). Changing decision kinds therefore changes what a
   recorded answer *means*: the migration must say what happens to existing logs containing
   `open_transaction`/`offer`/`transaction` — refuse them, or translate them — and must not silently
   reinterpret them. Existing operator recordings are already unforkable for an unrelated engine-commit
   reason (evidence R02-007b), which is the only reason a kind change is affordable now rather than after a
   compatibility layer.
4. **Legal actions are generated, not rejected late.** The merged offer list contains the deals that are
   actually available; it does not offer a deal and refuse it. Where a refusal is the honest answer, it
   carries a reason — today `use_leader` and parts of both machines answer `false`/empty with no
   explanation, which is why "it doesn't work" is the whole user-visible output.
5. **Hidden information stays typed.** A transaction can name an unscored secret objective. Whatever
   view a negotiating seat is given must not leak which secrets the other seat holds; `why_illegal`
   wording must not reveal hidden state either.
6. **`crates/ti4-engine/**` behaviour changes only through this package's tests**, and the shared
   presentation layer keeps R01's wording wherever R01 already described a transaction (its HTML export and
   player panel both do).

## Shape of the merge

- Keep `Terms` as the *thing exchanged*. It is the good part of the current design: it already covers
  goods, commodities, relic fragments by trait, one promissory note, one action card, one unscored secret,
  `is_empty`, and a `describe()` whose wording R01 and the policy both depend on.
- Make the diplomacy deal carry `Terms` on each side, and route a Hacan transaction into it as
  "a deal whose payload is exactly what each side hands over, with the transaction legality rules attached".
- Fold the limits: the per-pair diplomacy initiation budget and the transaction once-per-turn rules need
  one explicitly written rule about how they interact. They are different rules and the package must state
  the interaction rather than let it fall out of whichever gate runs first.
- Record transactions in the diplomacy journal, so the "Diplomacy" panel in both windows shows the deals
  the operator actually struck. Today they happen off-book.
- Retire `TradeWindow` and its kinds only once the diplomacy path passes the transaction tests that
  currently live beside it — move the tests first, watch them fail against the merged path, then delete the
  machine they were written against.

## Package split (each atomic, each test-first)

- **DIPLO-001 — a deal carries `Terms`.** Diplomacy deal payloads become `Terms`; the existing diplomacy
  tests still pass; new tests for each payload part and for the exceptions (94.3, Arbiters, Black Market,
  note-as-loan). No behaviour visible to a game yet.
- **DIPLO-002 — the transaction rules move.** `can_pay`, `why_illegal`, `partners`, `neighbours_who_transacted`
  and the once-per-turn limits become the legality layer of a diplomatic deal; the transaction tests move
  with them and must fail before they pass. Includes the initiation-budget interaction, stated in writing.
- **DIPLO-003 — one window, one vocabulary.** `TradeWindow` retired; `open_transaction`/`offer`/`transaction`
  removed or translated; project/session schema version bumped where a kind appears; replay of a log that
  contains the retired kinds produces a typed error, not a reinterpretation.
- **DIPLO-004 — what the operator sees.** Both windows negotiate through one panel; the journal lists
  transactions; refusals carry reasons; R01's wording preserved where R01 already had wording.

## What this settles from the bug reports

*"transactions with hacan not possible"* stops being a search for a missing offer in a machine nobody can
see working, and becomes: a transaction is a diplomatic deal, so if a seat can be asked for a diplomacy
offer it can be asked for a transaction. That is the honest framing, and it is still a hypothesis until
DIPLO-002 runs against the operator's saved game (`crates/ti4-engine/src/transactions.rs:646`
`available_actions` is where a Hacan seat's transaction action is produced today; if the operator's seat
was asked nothing there, the failing condition is now in one place to find).

Unchanged and independent: the L1Z1X agent defect (evidence R02-007e — an agent whose printed window is
"After a player activates a system:" is never offered, because leaders are filtered on
`is_action_window`). It is not part of this decision and should not wait for it.
