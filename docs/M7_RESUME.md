# D2 lane M7 (`core-m7-native`): the native playthrough, resume note (2026-10-31)

Branch `core-m7-native`, cut from `core-integration-10` (`5563444d`) + `core-d2-f2-hooks` (`88aba4fa`, merged, no conflicts). Never pushed; `nightly-fixes-2026-10-26` untouched.
Not in the base (still being worked, NOT merged): `core-d2-c1-agenda`, `core-d2-c1-strategic`, `core-m1-trade`, `core-m6c-tactical`, `core-d2-c1-status`,
`core-m6b-tactical`, `core-w-l16`. Read `docs/INTEGRATION10.md` section 8 and `docs/D2_LF2_RESUME.md` first.

## What this lane is

The gate-keeper of the zero-bail rule: **one can play an ENTIRE game in the new engine without any bail on the agreed content.** Until this lane no game played natively
(`playthrough --native` was only printed): the arms were proven one at a time against the string `Game`. Now a bin plays whole lobby games on the core alone, counts every
step it cannot run, and says where. The heavy Z-gate (`docs/todo/D2-islands-machines.md` 5.4 b, `D2-islands-composition.md` section 9 G-zero) starts when this bin exits 0.

## How to run it

```text
export CARGO_TARGET_DIR=/root/deploytmp/<lane> CARGO_BUILD_JOBS=2
tools/engine_core/native_play.sh                                       # builds the gate profile, runs --quick: sol, 4 players, seed 1
tools/engine_core/native_play.sh --factions all --players 3,4,5,6 --seeds 1        # the Z-gate entry table (24 factions x 4 sizes)
tools/engine_core/native_play.sh --quick --shadow                      # + the legacy game in parallel, every step compared
tools/engine_core/native_play.sh --quick --twice                       # determinism: the second run must give the same fingerprint
tools/engine_core/native_play.sh --factions sol,hacan --players 4,6 --survey --maxsteps 6000   # volumes of every gap over WHOLE legacy games
```

Exit 0 only when every game finished natively with zero gaps (no defect, no step or round limit, no difference, same fingerprint twice). A game that finishes prints
`final scores: faction VP, ...` and a 16-digit fingerprint (sha256 of the materialized position and the chain of answers). The Z-gate entry condition is **8
consecutive green seeds over all lobby factions** (`--factions all --players 3,4,5,6 --seeds 8`, plus `--shadow`: the silent differences are visible only there).

## What it is (`ti4-engine-core-bridge/src/native_play.rs`, bin `native_play`)

* **Creation.** `lobby_spec(faction, players, seed)`: `card_set te`, `starting_tech true`, `thunders_edge false`, speaker from the seed, seat a = the faction under test,
  the others the next lobby factions in a seed-fixed rotation (never twice), maps by seat count (`4pOuterRim`/`4pHyperlanes`, `6pStandard`/`6pOuterRim`, `random` for 3 and 5).
  The string setup builds the opening position ONCE (the only use of the legacy `Game` in the native mode: bridge only), `Codec::load` turns it into a `CoreState`, the
  `Game` is dropped.
* **The loop** (`NativeGame::step`, no string `Game` steps the game): `live_reconcile` (the per-step hooks) -> `Facts::of_core` + the core's own `asks()`
  (`legal_options().is_some()` from the position: an open job, the draft, the starting technology, the menu with a seat in turn) -> `step_arm` -> `live_step(arm)`
  with the rendered question put to the bot -> the hand-overs between arms (`after_arm`: a closed tactical window opens `turn::open_finish`).
  A failed step restores the position (also after a panic: caught, a defect). The `Render` and the code table live for the whole game.
* **The bot.** The typed rule bot (`ti4-rule-bot-typed`) cannot answer yet (its only entry is the D3-0 self-check that calls the string bot with a string observation), so the
  bot is the seeded random chooser under the rule bot (`ti4_rule_bot::choose_explained(.., seen = None, seed, seat place, decision index)`, transaction opens weigh 0.1), the last
  strategy card is taken without a question (`LoneCard`). Deterministic by construction. A "prefer progress" policy is NOT built: it needs the action menu to exist (without
  it a random table may never end a game; the round/step budgets end it, never a loop).
* **Gaps** (`Gap { arm, kind, cause }`, counted, the game stops, exit 1): `ArmNotNative` (no live function), `AskSource` (the `Ask` arm's source is not native: only an open job, the draft,
  the starting technology and the closing menu are), `Bail` (a live window raised `LiveError::Unknown`: the cause is its `Debug`), `LentClose` (`lent_close`: the part of an arm's close the
  string still does; today only `Tokens`, steps 81.6-81.8, KS5). Anything else a live call returns (`Illegal`, `NotOpen`, a panic, ...) is a **defect**, printed apart (it is a bug, not a missing feature).
* **The matrix** (`Matrix::render`): per island native steps and gaps by cause, `green, not reached` for an island no step touched (trap T-Z1: a green row with zero steps proves nothing), then native steps by arm.
  The sort of a cause into an island is `islands::island_of` (extended with the live path's words: `turngap`, `unported(`, the `Unknown` card words, `commanderunlock`).
* **`--shadow`** (`shadow_play`): the legacy game is built first, loaded into the core, and then stepped in parallel with the answers the native bot gave; the arms the legacy engine runs together with the next one
  (turn prelude and announcement continue into the question of the same legacy step) are grouped. After every group: the questions (player, prompt, option ids), the plain events, and (groups ending in
  `Ask`/`Phase`, whose close is the core's) the whole position (`materialize` over the legacy state, leaf by leaf). The first difference stops the game and fails the run.
* **`--survey`** (`survey`): the legacy game LEADS a whole game (the shipped rule bot answers); every legacy step is classified by the arm and the source of questions the native loop would need
  (`turn_prelude>ask.action_menu` is one legacy step: two native steps and the source of the question). Gives the VOLUME of every gap, which a native game cannot (it stops at the first).

## The first report (gate profile, this base = int10 + F2)

Native, `--factions all --players 3,4,5,6 --seeds 1` (96 games, 0.5 s in total): **all 96 reach the same first gap**, in the first action turn of round 1:

| island | native steps | gaps | cause |
|---|---|---|---|
| B turn flow | 900 | 0 | green: 600 draft questions, 12 starting-technology picks, 96 strategy announcements, 96 phase flips, 96 turn preludes |
| F menu and leaders | 0 | **96** | `ask.action_menu`: the question source of the action menu is not native (lane F, KM: `driver::menu_sources` lists the strategic action, the leaders and Sling Relay; the tactical action, transactions, faction/mapped/relic/exploration/action-card sources and the dispatch are not) |
| every other island | 0 | 0 | not reached |

`--quick` (sol, 4 players, seed 1): 11 native steps (8 draft, announce, phase, prelude), gap `ask.action_menu`, fingerprint `20222ce7d67eaa15` twice. `--shadow` over the 96 games: 9 groups compared per game, 0 differences.
Before the F2 merge 40 of the 96 stopped earlier, at `turn_prelude [TurnGap(CommanderUnlock)]` (the 12 factions whose commander hook was missing): closed by the merge and the hand-over below.

**Survey of whole games** (12 legacy games: sol, hacan, letnev x 3,4,5,6 players, seed 1, all finished in 4-7 rounds; 17478 legacy steps): 5735 steps (33 %) need an arm or a source the core does not run:

| steps | arm / source the native loop needs | island (owner) |
|---|---|---|
| 1920 | `turn_prelude>ask.action_menu` (the menu asked in the step of the prelude) + 85 `ask.action_menu` | F menu |
| 1454 | `voting` | D agenda (`core-d2-c1-agenda`) |
| 1209 | `secondary` (follower windows of the strategic action) | E strategic (`core-d2-c1-strategic`) |
| 675 | `trade` | M1 (`core-m1-trade`) |
| 272 | `scoring` | C status (`core-d2-c1-status`) |
| 73 / 47 | `status` / `agenda` | C / D |

The arms that HAVE a live function and were run by the legacy-led shadow (`oracle --native`, `docs/D2_LF2_RESUME.md`) are tactical 4761, aftermath 3287, ask.closing 1846, tokens 612, event_scoring 509, ask.draft 458, phase 258
steps: those are reached by a native game only after the action menu is native; their internal gaps are the dual bail table of `playthrough --quick` (M6 tactical, M5 aftermath, ... `docs/D2_LF2_RESUME.md`).

## What was fixed (each with a test that fails on the pre-fix code)

1. `islands::is_cause` did not know `[gap:` (the matrix showed every gap as green): `islands::tests::a_gap_of_the_native_playthrough_is_a_cause_like_a_bail` (and the matrix test).
2. **The hand-over from lane F2** (`docs/F2_RESUME.md`): `COMMANDER_HOOK_MISSING` is deleted, the locked commanders of all in-scope factions are judged at the start of a turn
   (`driver/turn/start.rs`); `turn_jobs::a_start_of_turn_...locked_commander_is_judged` (fails before: `Err(TurnGap::CommanderUnlock)`). The closing menu lists the free leaders through `menu_sources::{leaders, leader_is_free}`
   (`closing::options`; `closing::has_free_leader` deleted) and a chosen option other than "end your turn" is the counted gap `TurnGap::MenuDispatch` (the dispatch of a menu option is the string's, KM2):
   `turn_jobs2::a_free_leader_of_the_closing_menu_is_asked_and_its_dispatch_is_a_counted_gap`, `a_leader_printed_during_the_action_phase_is_a_free_option_of_the_closing_menu` (fail before: `Err(MenuSources)`).
3. `NativeSubject` (legacy-led shadow, `oracle --native`): primes the loaded core with what the legacy game keeps outside `GameState` (the driver flags, the prepared turn, the closing seat, the actions of the turn) so the core's own
   `Facts`/`ask_ask` agree with the legacy step (`Game::actions_this_turn`, feature `native-subject`); counts a step with string `#staged:event` marks as lent (the core announces from `Pending::staged`, which a loaded position does not fill);
   `Ask` is compared on everything (its step is whole); new opt-in flags `oracle --gaps-lent` (a live window's `Unknown` is a lent step with its cause, not a difference), `--closing` (shadow the closing menu too), `--keep-going` (collect the
   differences by kind instead of stopping at the first: the follower is a legacy game, a difference never diverges it).
4. `the_turn_flow_arms_agree_with_the_legacy_game_for_every_lobby_faction` now runs with `--keep-going` semantics and pins the ONE known silent difference (Arborec Mitosis) instead of the CommanderUnlock allowance; it fails when a new one appears and when a listed one disappears.

## SILENT differences found (the native path differs from the string WITHOUT a counted bail; the zero-bail rule cannot see them, `--shadow` can) - the new part of INTEGRATION10 section 8 item 1

Found by the legacy-led shadow over 6 golden games (`oracle tools/rulebot/golden.json 6 --native ask,turn_prelude,phase,strategy_announce --gaps-lent --keep-going [--closing]`, 7640 steps, 1428+ lent) and the lobby-faction test:

| # | where | string | core | owner |
|---|---|---|---|---|
| S1 | `Phase` arm, `STRATEGY_PHASE_ENDED` | asks `promissory:empyrean:gift:STRATEGY_PHASE_ENDED:after` (Gift of Prescience) | asks nothing | W lanes (ability registry, INT10 item 1) |
| S2 | `Phase` arm, `STATUS_PHASE_BEGAN` | Arborec Mitosis: asks where the infantry goes, the unit lands on the board | asks nothing, no unit (`arborec/offer.rs`: "points left") | Arborec module (W) |
| S3 | `Ask` closing menu, End job, `TURN_PASSED` | asks `leader:naaz:naazagent:TURN_PASSED:after` (10 times in 6 games; its answer also moves the exploration decks: `exploration_decks.CULTURAL/INDUSTRIAL` differ) | asks nothing | W lanes (Naaz agent timing) |
| S4 | `Ask` closing menu, options | offers `component|trade|hacan` (a trade partner of a Hacan table; 11 times) | omits it: `rules::trade::KernelEnv::wide` reads `hooks.general.ignores_neighbours`, which no module registers (Guild Ships is ignored), so `closing::has_trade_partner` is false | M1 (`core-m1-trade` carries `kernel::trade_env`; check it closes this when merged) |

S3 and S4 appear only with `--closing` (the closing menu is NOT shadowed by default: it is native for the question but the free options of the trade source are not assembled, so the shadow would fail every Hacan table).
Of the 645 closing menus of those 6 games 33 were comparable: the rest hit `TurnGap::MenuSources` (a trade partner exists, or a second action with Fleet Logistics) and are counted lent.

## Where it stands against the work lists (what the native game needs next, in the order the first gap moves)

1. **Lane F (KM): the action menu as a question source** (`ask.action_menu`, 96/96 games): assemble `driver::menu` over the sources (tactical `Opt::Tactical` is native in the window, the strategic action and leaders exist as `menu_sources`, Sling Relay), the
   remaining sources (faction/mapped, relics, exploration, action cards, transactions) and the dispatch of the chosen option (`dispatch.rs`: `Pass`, `Tactical`, `Strategic`, `EndTurn` are the core's, the rest `Lend`). Then the closing menu's free options (transactions: M1).
2. `Arm::Tactical` needs the activation question from the menu; the end of the action is `turn::open_finish` (wired in `after_arm` for a closed tactical window; untested in a full game until 1 exists).
3. Status (`core-d2-c1-status`: `Arm::Status`, the scoring window, the `Tokens` close), agenda (`core-d2-c1-agenda`: `Voting`, `AgendaTalks`), strategic action (`core-d2-c1-strategic`: `Secondary`, `LeaderFollowers`), trade (`core-m1-trade`):
   the survey says how many steps of a real game each one owns (above). They are separate branches; merging them moves the survey's native share, not the native game's first gap, until 1 lands.
4. The silent differences S1-S4 (and the audit of INTEGRATION10 item 1: a string-registered ability without a core twin should be a counted bail of the live path).
5. The bot: a "prefer progress" policy once a game gets past round 1 (the rule bot on the typed `Seen` view when D3 delivers it).

## Only the latest RULES_VERSION (user decision of the coordinator)

`NativeGame::from_game` refuses a game whose `rules_version` is not `RULES_VERSION` (35) with an error (`a_game_below_the_latest_rules_version_is_refused_not_played`); `lobby_spec` leaves `rules_version` unset (= newest), so
no old-version gap is counted or chased. **Old-version branches the core still carries** (a `rules_at_least(K)` / `rules_version >= K` with K below 35 is always true at the latest version, so its else-branch is dead;
a `rules_version < K` test is always false: its body is dead). `RULES_BASE_DIPLOMACY` = 35 is the latest itself (live). Found with
`grep -rn "rules_at_least\|rules_version *[<>=]" crates/ti4-engine-core/src crates/ti4-engine-core-factions/src` (74 lines outside tests; the constants: ACTION_CARD_TARGETS 10, AGENDA_GRANT_REVEAL 14, ACTION_CARD_COSTS 23,
CLASSIFIED_LEAKS 32, COMMAND_TOKEN_POOL 12, COVERT_LEGISLATION 33, CRISIS_PACT 31, DMZ_EXPLORATION 34, FACTION_BUGFIXES 17, FACTION_EFFECTS 16, IMPERIAL_SECRET_CAP 18, INFILTRATE_FIX 1, LEAKED_EVENT_SECRET 24,
LETNEV_FLEET_POOL 4, MENDOSA_SHIP 2, MIRAGE_FRONTIER 27, MUNITIONS_AFTER_ROLL 8, NEXUS_ACTIVATION 21, NO_EMPTY_CONVERT 19, OFFICIAL_ACTION_DECK 30, PLAGIARIZE_OWNED 28, RETREAT_HAZARDS 11, SABOTAGE_TIMING 3, SELF_ASSEMBLY, SOL, LETNEV,
TECH_EFFECTS, TECH_PAYMENT_CHOICE, WINNU_AGENT_PRODUCTION, `rules_at_least(1)`):

* `ti4-engine-core/src/driver/turn/{end.rs:52, facts.rs:37}`, `driver/windows/combat/env.rs:94-95`, `driver/windows/invasion/fight.rs:111,152,565`, `driver/windows/production/mod.rs:86,160`, `driver/windows/scoring.rs:114`, `driver/windows/tactical/facts.rs:47`
* `kernel/children.rs:92`, `kernel/fx_perform.rs:110,163,175,181`
* `query/{laws.rs:144, promissory.rs:39, tech.rs:171,368,433, trade_deals.rs:378, transit.rs:35}`
* `rules/action_cards/{effects.rs:598,981,2143,2414,2580,2603, mod.rs:213,219, reaction_slot.rs:89,207}`, `rules/agenda/{close.rs:302, mod.rs:142,153,294, reveal.rs:134,164,300, window.rs:226}`
* `rules/combat/{begin_round.rs:39, machine.rs:1267,1501, mod.rs:323,758,924,960, sustain_offer.rs:287}`, `rules/explore_windows.rs:907,958,1037,1053,1120,1261`
* `rules/invasion/{begin.rs:51, control.rs:38, mod.rs:546}`, `rules/leaders.rs:183`, `rules/production/{home.rs:247, strategy_cards.rs:1032}`, `rules/reactions/{infantry_ii.rs:25, saar.rs:26}`, `rules/strategy.rs:175`
* `ti4-engine-core-factions/src/`: `cc_ground/borrowed/{query.rs:86,91, timing.rs:40,44}`, `cd_registry/naalu/query.rs:49`, `ce_movement/saar/movement.rs:38`, `magen_defense.rs:39`, `self_assembly.rs:41`
* the definition `state/meta.rs:112` (`Meta::rules_at_least`) and the constants themselves go last. The string engine carries many more (not listed: not the core).

## Not done / by decision

* `EventScoring`, `Tactical`, `Aftermath` still have no `NativeSubject::open_arm` line (INTEGRATION10 section 5: the observed sub-windows of D1 would have to be typed); the native-led `--shadow` covers the arms a native game reaches.
* `playthrough --native` (the dual bin) is unchanged: it still only selects arms for the dual run. The native playthrough is `native_play`.
* No heavy gate was run (golden/wide dual sweeps, benches, soak, mutation ratchet, census, workspace clippy, `TI4_CORE_STRICT=1`), as ordered: the harness now exists; the Z-gate starts when `native_play --factions all --players 3,4,5,6 --seeds 8 [--shadow]` exits 0.
