# Card audit P1 fixes

Branch `card-p1-fixes`, based on `consolidated-2026-10-25` (rules version 29). Fixes the five P1
issues of `docs/CARD_USAGE_AUDIT.md` (branch `card-usage-audit`) and one clear rule bug from its
P2 list (I09). Each fix is its own rules version, so a game keeps the behaviour it was started
with; the golden benches pin older versions and are unchanged (see "Benches"). Details per
version are in `docs/RULES_VERSION.md` (30 to 35).

| issue | card | rules version | constant |
|---|---|---|---|
| I01 | action deck (Veto copies) | 30 | `RULES_OFFICIAL_ACTION_DECK` |
| I02 | Galactic Crisis Pact | 31 | `RULES_CRISIS_PACT` |
| I03 | Classified Document Leaks | 32 | `RULES_CLASSIFIED_LEAKS` |
| I04 | Covert Legislation | 33 | `RULES_COVERT_LEGISLATION` |
| I05 | Demilitarized Zone (exploration) | 34 | `RULES_DMZ_EXPLORATION` |
| I09 | Diplomacy, base-game printing (`base2`) | 35 | `RULES_BASE_DIPLOMACY` |

## What was wrong, and the fix

**I01, action deck: 142 cards.** Verified: `action_cards.json` has 142 records (base 82, codex1
20, pok 20, thunders_edge 20), three of them Veto (`veto`, `veto3`, `veto4`, all source `base`),
and `deck::build_starting_decks` deals every record in scope. A Thunder's Edge game had 142
cards, a PoK game 122; `decks.json` `action_cards_te` lists 140 and `action_cards_pok` 120, each
with one Veto. Fix: `deck::drop_unofficial_action_cards` removes `veto3`/`veto4` from the deck
before every draw (`action_cards::draw`, the status-phase draw, the Politics and Keleres Rider
fallbacks) from version 30; the other cards keep their shuffled order, and the deck has its own
RNG domain, so no other deck or die moves. It is not done in setup on purpose: hosts set the
rules version after setup (the offline host rebuilds a resumed game's setup at the current version
and only then gives it its stored version; selfplay sets the configured version after setup), so
setup cannot know which deck the game plays, while a draw always sees the game's real version. A
resumed old game therefore keeps its 142-card deck. The host's action-card facts count the deck
without the two cards (`deck::action_deck_len`), so a new game shows 140 before its first draw.
Not changed: the components catalogue (`ti4_host::view::components`, the rules reference list of
"every copy as printed") still lists three Vetos; it is one catalogue for every game, old ones
included.

**I02, Galactic Crisis Pact: always discarded.** Verified: target "Elect Strategy Card",
`vote::outcomes` returned no candidate ("Neither is modelled"), so `Game::reveal_agenda` discarded
it and revealed the next card; the effect arm was an empty availability-only arm. Fix: the
candidates are the game's strategy cards (`state.card_initiative`) in initiative order, and on
resolution each player, speaker first then clockwise, is asked the elected card's ordinary
secondary question without a token cost (`strategy::crisis_pact_choice`); a yes runs
`strategy_cards::secondary`. A seat the secondary could do nothing for (the follower window's
`secondary_can_do_something`, or Leadership when it cannot afford a token) is not asked.

**I03, Classified Document Leaks: always discarded.** Verified: target "Elect Scored Secret
Objective (...)", no candidate in any state, so the effect (`laws::classified_leak` in
`Game::resolve_agenda_outcome`, the `classified` arm) never ran; the existing test covered only the
no-secret case. Fix: the candidates are the secret objectives any player has scored that are not
already public, sorted. With none, the card is still discarded and replaced (rules version 14).

**I04, Covert Legislation: a plain For/Against vote with no effect.** Verified: its target is the
printed rule, which `vote::outcomes` read as For/Against; the effect arm was empty and no other
code named `covert`. Fix: on reveal the top agenda is drawn face down into
`GameState::covert_agenda` (not serialised while empty), Covert's outcomes are that card's, the
vote is held on Covert Legislation, and closing it resolves the hidden agenda with the winning
outcome (law, effect, elected feat), logs `COVERT_AGENDA_RESOLVED:<agenda>:<outcome>` and lists it
in `agenda_log` after Covert. Only the speaker's vote decisions carry the hidden card
(`details.covert_agenda`). A Veto / Quash / Political Favor on Covert discards the face-down card
with it.

**I05, Demilitarized Zone exploration: attach only.** Verified: `dmz` (cultural, Attach,
attachment `dmz`) stored an attachment with no modifiers, and no production code named `dmz`;
`laws::planet_is_demilitarized` covered only the base agenda. Fix: `exploration::demilitarize`
returns the planet's structures to reinforcements and moves its other ground forces to the space
area, and `planet_is_demilitarized` is true for a planet carrying the `dmz` attachment, which
applies the checks invasion, production and placement already make for the agenda.

**I09, base Diplomacy (`base2`).** Verified: `base2` prints "Then, ready each exhausted planet you
control in that system", but the primary was dispatched by name to the PoK "ready up to 2
exhausted planets" (two forced questions, anywhere). Fix: `base2` readies every exhausted planet
the player controls in the chosen system, without a question. The `base_game` set is still not
offered by the lobby (`ti4_host::setup::card_set::OFFERED`, whose note gave this bug as the reason).

## Other audit items checked

Only I09 was a clear rule bug with a one-line answer and a small fix. The rest are not rule bugs
in the games that are played, or not small:

- I11: technologies come from `techs_pok_c4` regardless of scope, so a base-only game would offer
  PoK technologies. No host offers a base-only scope; not fixed.
- I06, I07, I08, I10, I12, I13, I14, I16 to I19, I22: text-parsing, duplication and id hygiene;
  every printed window, target and threshold is bound correctly today (the audit's own
  measurements). No rule changes.
- I15: a dangling id in `faction_abilities::blocked`; no rule effect.
- I20, I21: Galactic Events / Wild Wild Galaxy printings and never-dealt records: scope, not bugs.
- I23 (coverage ledgers counting these cards as implemented): becomes true for I02 to I05 with
  these fixes; the ledger code is unchanged.

## Left for a decision (ambiguous)

The repo carries no Living Rules Reference text (`rules.json` holds only Thunder's Edge
overviews); the card texts below are quoted from the corpus. Each item names the reading that was
implemented.

1. Galactic Crisis Pact, order: "Each player may perform the secondary ability of the elected
   strategy card without spending a command token" names no order. Implemented: speaker first,
   then clockwise.
2. Galactic Crisis Pact, "command tokens placed by the ability are placed from a player's
   reinforcements instead": no secondary the engine resolves places a command token (its
   Construction secondary places only the structure), so the clause has nothing to apply to.
3. Covert Legislation, who votes: "the speaker reads the eligible outcomes aloud ... The other
   Players vote for these outcomes". Implemented: every player votes, the speaker included, as on
   any agenda. Reading it as "the speaker does not vote" would need a change to the vote order.
4. Covert Legislation, a face-down card with no eligible outcome (Classified Document Leaks with
   no scored secret, an Elect Law agenda with no law in play): the card says nothing.
   Implemented: it is discarded and the next card drawn; an empty deck discards Covert.
5. Covert Legislation replaced by Veto / Quash / Political Favor after the face-down draw:
   implemented as discarding the face-down card with Covert (not putting it back on the deck).
6. Covert Legislation, "the next card in the agenda deck": the engine queues both agendas of the
   phase before the first is revealed, so the face-down card is the one after them, not the
   phase's second agenda. Equivalent for an unknown shuffled deck.
7. Covert Legislation, effects keyed to the hidden agenda's kind: Committee Formation ("an agenda
   that requires a player to be elected") is not offered for a hidden Elect Player agenda, and
   "when an agenda is revealed" effects do not fire for the hidden card (it is not revealed).
   Riders predict the hidden agenda's outcomes.
8. Demilitarized Zone (exploration): "return all ground forces on this planet to the space area"
   with no capacity in the system: no capacity check is made here. The agenda-phase clause ("this
   planet's planet card can be traded as part of a transaction") is not implemented: transactions
   carry no planet cards, and it would need a new transaction item and its UI.

## UI and decision changes (new rules versions only)

- Vote, tie-break and rider-prediction options of Galactic Crisis Pact and Classified Document
  Leaks are labelled with the card / objective name (ids unchanged). The existing ballot shows
  labels, so this needs no client change.
- New decision: Galactic Crisis Pact's per-player question. It reuses the strategy secondary
  question (`details.kind` `strategy_secondary`, same option ids) with `costs_token` false, no
  `played_by`/`tokens_left`, `details.agenda` = Galactic Crisis Pact, and a new prompt "Galactic
  Crisis Pact: perform the <card> secondary without spending a command token" (Leadership keeps
  its purchase question). The web's `describeStrategySecondary` should render it with the
  existing panel (card, secondary text, no token pips; the yes button falls back to the option
  label because the prompt no longer starts with "spend a strategy token"). Not verified in a
  browser: a screenshot folder (`web/e2e/screenshots/<next letter>`) is needed before relying on it.
- Covert Legislation: the speaker's vote decisions carry `details.covert_agenda` (the hidden card's
  agenda details). The web does not render it yet, so the speaker cannot see the hidden card in
  the client. Showing it is UI work and needs the screenshot-folder process.
- The rule bot is unchanged; the new questions reach its generic handling (Crisis Pact's prompts
  are not among the ones it reads, so it answers them as an unknown yes/no).

## Tests

Per fix, new-version behaviour and old-version behaviour:

- I01 `deck::tests`: `a_new_game_draws_from_the_official_action_deck` (140 / 120, equal to the
  `decks.json` lists, order kept), `an_old_game_keeps_the_corpus_action_deck` (142),
  `a_draw_skips_the_extra_vetos_only_from_the_new_version`. Five existing tests that compared the
  raw deck length before and after one draw now take the official length first.
- I02 `agenda_effects::crisis_pact_tests` (candidates, label, speaker-first order, no token spent,
  tokenless seat still asked, the question's shape, nothing below 31) and
  `game::tests::galactic_crisis_pact_is_voted_on_from_rules_version_31`.
- I03 `game::tests::classified_document_leaks_elects_a_scored_secret_from_rules_version_32`.
- I04 `game::tests::covert_legislation_votes_on_the_face_down_agenda_and_resolves_it`,
  `covert_legislation_skips_a_face_down_card_with_nothing_to_elect`,
  `only_the_speaker_is_shown_the_face_down_agenda`,
  `covert_legislation_is_a_plain_vote_below_rules_version_33`.
- I05 `exploration::dmz_tests` (structures home, ground forces to space, planet demilitarized; at
  33 attach only).
- I09 `strategy_cards::tests::base_diplomacy_readies_every_exhausted_planet_in_the_chosen_system`.

Also adjusted for I01 (the deck count a fresh game shows is now the official one):
`ti4-host` `view::action_cards` tests and `ti4-server` `projection_redaction`.

Runs on this branch (private target dir): `cargo test -p ti4-engine --lib` 3936 passed;
`ti4-host`, `ti4-rule-bot`, `ti4-selfplay`, `ti4-model`, `ti4-content`, `ti4-policy` all pass;
`ti4-server` and `ti4-wasm` (with `ts`) pass after the fixture regeneration. `ti4-sim`'s
`profile::tests::fixture_capture_is_deterministic` fails because its map-pool file is not in this
checkout (unrelated). Crates that need libtorch (`ti4-replayer`, `ti4-review`, `ti4-advisor`,
...) were not built. Web unit tests were not run (no `node_modules` in the worktree).

Fixtures: `fixtures/protocol/*` regenerated with `TI4_WRITE_WIRE=1` per test
(`ti4-server` `fixtures_verification`, `ti4-wasm --features ts` `protocol_fixtures`). The only
changes are `rules_version` 29 -> 35 in the three snapshots and `action_cards.deck` in the seven
update fixtures (122 -> 120, 95 -> 93, 49 -> 48: the extra Vetos are no longer counted).

## Benches

All six golden benches are unchanged (they pin rules versions 23, 18 and 11):

| bench | result |
|---|---|
| `bench` | 200 identical, 0 different |
| `bench-v5` | 200 identical, 0 different |
| `bench-v4` | 200 identical, 0 different |
| `bench-v1` | 200 identical, 0 different |
| `bench-v2` | 200 identical, 0 different |
| `bench-v3` | 200 identical, 0 different |

A smoke at the new version (`selfplay run --players 4,6 --games 60 --seed-start 5000`, rules
version 35): 120 games, all finished, no stuck game, engine error or panic. The `ti4-sim`
behaviour gate is retired (`#[ignore]`) and was not re-derived.
