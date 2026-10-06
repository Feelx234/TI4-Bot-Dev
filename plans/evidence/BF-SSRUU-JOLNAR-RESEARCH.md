# BF-P02 — Ssruu can copy Doctor Sucaban: a borrowed research discount that spends the copy

Package: P02 from `plans/HANDOVER_2026-10-04_BASE_FACTIONS_PI.md` ("Jol-Nar copied research agent").
Branch: `wp/base-factions`. HEAD unchanged (**ac296cbc**); no commit made — see "Commit state".

## Normative sources

- `crates/ti4-content/content/leaders.json`, `yssarilagent` (Ssruu):
  `"This card has the text ability of each other player's agent, even if that agent is exhausted."`
  The "even if exhausted" clause is the whole point of this package: the copy does not require the
  source agent to be ready.
- `crates/ti4-content/content/leaders.json`, `jolnaragent` (Doctor Sucaban): the infantry-for-research
  text ability, already implemented natively.
- Existing project contract: `crates/ti4-engine/src/factions/hooks_cards.rs::borrowable_agents` —
  the single definition of "what Ssruu may copy" (a readied Ssruu, another seat, an `agent`-type
  leader in `Readied | Exhausted` status). Reusing it rather than re-deriving the rule keeps the two
  routes from disagreeing.
- No Python reference consulted; Python parity is not an acceptance criterion.

## What was missing

`strategy_cards::doctor_sucaban` found a seat with a **readied** `jolnaragent` and asked that seat to
exhaust it for the paying player. A Ssruu holder with infantry and no resources had no route at all:
the copy existed in the corpus text and in the shared `borrowable_agents` contract, but not in the
one place where this ability is actually applied.

## Design

### Tier-C correction scope (2026-10-04)

The first implementation is not acceptable as written.  This bounded correction owns only
`strategy_cards.rs`, the state-only borrowed-use notification service in `factions/hooks_cards.rs`,
and this evidence file.  It does not change the combat provenance package or claim a general rewrite
of native paid research.

The corrected contract is:

- Every Sucaban right retains the exact source owner returned by `borrowable_agents`.  When native
  and copied rights coexist, their holders are offered in the same active-player-rotated initiative
  order used by action-phase timing windows; stable native prompt, option, and decision ids remain
  unchanged.
- A copied acceptance is provisional until a technology is actually researched.  The checkpoint is
  taken immediately before the copied offer.  Any later refusal, unaffordability, missing payment
  plan, failed application, or `IllegalChoice` restores `GameState` and `Table::log` to that point,
  without rewinding the decider's input stream, so the same table can retry.  Native behavior is not
  silently generalized by this package.
- A successful copied research calls the state-only `borrowed_agent_used` service exactly once,
  after research succeeds.  Decline, rollback, native use, and error call it zero times.  The
  contextual service continues to run contextual hooks and also the state-only hooks, so timing and
  non-timing copied routes share one notification contract.

Focused regressions must exercise a partial infantry removal followed by an invalid answer, exact
state/log rollback, and success on the same `Table`; they must also observe one borrowed-use
notification only after that success.

Two seats can open the same window. Their independent rights are offered in the engine's
active-player-rotated initiative order:

1. **Native** — a seat holding a readied Doctor Sucaban. Prompt, option ids and decision id
   (`doctor_sucaban_exhaust`) are unchanged, so existing policies, fixtures and prompt text are
   untouched.
2. **Copied** — the paying seat, if they hold a readied Ssruu and another seat has the agent at all
   (readied *or* exhausted, per `borrowable_agents`). New prompt and new decision id
   (`doctor_sucaban_borrowed_exhaust`, source `yssarilagent`), because it is a different right
   belonging to a different card.

A declined first offer does not close the second: the two rights are independent. Whichever offer is
accepted, the same `trade_infantry_for_research` loop runs — the researcher removes **their own**
infantry, and only the accepted card is exhausted. A copied use remains provisional until research
succeeds; otherwise its state and replay log are restored.

Diplomacy: the native path still settles a `LeaderUsedFor` promise when the owner served another seat.
The copied path is a seat spending its own card for its own research, so there is no promise to
another seat to settle; that is stated in the code comment rather than left implicit.

The `leaders::modifiers()` registry entry for `jolnaragent` now names the copy route too, so the
registry still points at where the modifier is actually read.

## Changed paths

| Path | Change |
|---|---|
| `crates/ti4-engine/src/strategy_cards.rs` | `doctor_sucaban` enumerates both sources; new `sucaban_sources`; the infantry-removal loop moved into `trade_infantry_for_research` so both routes share it |
| `crates/ti4-engine/src/factions/hooks_cards.rs` | adds a state-only borrowed-agent notification hook/service; contextual copied routes invoke it too, while paid research can report success without fabricating a timing context |
| `crates/ti4-engine/src/leaders.rs` | `modifiers()` entry for `jolnaragent` names the Ssruu route |
| `crates/ti4-engine/tests/decision_delivery_inventory.rs` | reviewed registry updated: `doctor_sucaban` now has 1 choice site, `trade_infantry_for_research` 1 (both delivered through `strategy_cards.rs::ask`) |

## Tests and results

Red first: with the copy branch stubbed to `false` (the state before this package), the new tests
fail for the intended reason:

```text
ssruu_copies_sucaban_even_when_the_agent_is_exhausted
  → "the copy opens the same window the agent does": left 0, right 1
a_declined_agent_leaves_the_copy_available
  → FAILED
```

With the copy wired in:

```text
cargo test -p ti4-engine --lib strategy_cards::tests::   → 34 passed, 0 failed
cargo test -p ti4-engine                                → lib 2052/0 (1 ignored),
                                                          integrations 1/1, 4/4, 5/5
cargo test -p ti4-model                                 → 82 passed
cargo test -p ti4-sim                                   → 51 passed, 1 ignored
cargo test -p ti4-policy                                → 273 passed
cargo test -p ti4-bridge                                → 62 + 6 + 12 + 3 passed
cargo test -p ti4-content                               → 134 + 1 passed
cargo test -p ti4-legacy                                → 25 passed
cargo fmt -p ti4-engine                                 → no changes after formatting
cargo clippy -p ti4-engine --all-targets                → no warning points at the new code
```

New tests, in order:

1. `ssruu_copies_sucaban_even_when_the_agent_is_exhausted` — the copy works when the source agent is
   already exhausted; the researcher's own infantry is removed; **only** Ssruu changes status; the
   copied agent stays exactly as it was.
2. `the_active_borrowers_copy_precedes_native_sucaban` — with both available and the borrower
   active, the copied right is first despite native insertion order; only Ssruu is spent and the
   exact source card is unchanged. The following decline test makes the native owner active and
   asserts the pre-existing native prompt and decision subtype before the copy resolves.
3. `a_declined_agent_leaves_the_copy_available` — the owner declines; the same table then accepts the
   copy; the agent stays readied and the copy is spent.
4. `sucaban_is_not_offered_without_infantry` — no infantry means no prompt at all (captured choice
   list contains neither prompt), no research, and Ssruu stays readied.
5. `a_failed_borrowed_payment_rolls_back_and_the_same_table_retries` — after accepting the copy and
   removing one infantry, an invalid second answer returns an error with exact state and decision-log
   rollback. The same table then consumes its next scripted answer, succeeds, and the state-only
   borrowed-use hook records exactly one successful `jolnaragent` copy.
6. `the_technology_primary_also_offers_the_copy` — the other live producer of this window (Technology
   primary's paid half) uses the same seam.

## Deliberate limitations recorded

1. **Native failed-attempt behavior is not generalized.** The new rollback boundary applies to the
   copied right. Extending identical rollback to the original-six native route can be correct, but
   requires its own compatibility evidence rather than being silently bundled into this package.
2. **No live-combat-style "already used this round" restriction** beyond exhaustion itself, which is
   the printed cost of the ability.

## Correction validation status

The tier-C correction above was written source-first under the coordinator's explicit no-Cargo rule.
The earlier 34/34 and affected-crate results predate it and must not be cited as validation of the
corrected source. Root must format and run the focused strategy-card tests, decision inventory,
affected engine suite, and lint before acceptance.

## Review status

Tier C (timing/legality of a new ability window, plus a change to a reviewed decision registry) is
**required and not yet performed**. No independent reviewer has run on this package; acceptance is
pending.

## Commit state

No commit. `strategy_cards.rs` and `leaders.rs` carry hunks from other sessions in the shared dirty
checkout; a file-level commit would include unrelated work. Behaviour is implemented and green in the
shared tree.

## Next safe action

Tier-C review of the two-route ordering and the decision-registry update, then P03 or the
documented `borrowed_agent_used` seam if a hook ever needs it.

## October 5 root source review and corrected validation

Root independently reviewed Terra's corrected exact source-owner identity, native-first rotated order, copied provisional-use rollback, and successful-copy state-only notification seam. No production contextual borrowed_agent_used consumer exists; the state-only hook is a deliberate contract for this resolver-less payment path rather than fabricated timing services. Future contextual consumers require their own real-service integration. Corrected fixtures passed in the coordinated full engine lib2088/0/1 run; the broader gate later stopped on new unrelated event payload vocabulary. Final affected-engine/inventory/lint and scoped commit remain pending.
