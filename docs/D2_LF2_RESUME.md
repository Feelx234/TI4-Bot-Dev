# D2 lane A2 (`core-d2-f-live2`): the live loop, second round, resume note (2026-10-31)

Branch `core-d2-f-live2` = `core-d2-f-live` (`da047471`) + the commits below + the merge of `core-todo-d2c`/`core-todo-d2d` (docs only) + the merge of `core-w-l11`
(cards played inside another window). Read `docs/D2_LF_RESUME.md` first (sections 0-5 are still the contract). Never pushed; the live branch `nightly-fixes-2026-10-26`
is untouched; `disable-xf123` is NOT merged (its content, the refusals of the dropped features at the product edges, is KQ's, and `core-w-l11` already contains it).

## What was done (the six items of the task)

| # | Item | Where | Evidence |
|---|---|---|---|
| 1 | KF5 `NativeSubject`: the oracle's shadow run of the native arms behind a legacy `Game` (bridge only) | `ti4-engine-core-bridge/src/native_subject.rs`, `oracle.rs` (`Subject::take_extra`, `MaskSubject::{game, peek_records, peek_events}`), `bin/oracle.rs --native ARMS`; engine: feature `native-subject` (`Game::{step_facts, observed_windows, content_sources}`, `d1_pending::facts_of`) | `native_subject::tests` (3): the tokens arm behind a real game through round one agrees (0 differences) and ran, the other arms are counted lent; a perturbed native position is reported as a `State` difference at its path; with no native arm nothing is compared and every step is lent |
| 2 | `plays` facts of action cards played inside a live window, from the CORE | `ti4-engine/src/live/plays.rs` (`play_facts`, `provider`, `placed`, `mahact_held`, `reveal_set`, `reveal_payable`), `live/env.rs::env_with_plays`; core: `kernel::ability::{PlaysProvider, AdapterFacts::plays_provider}`, `driver::live::table_at` | `live::plays::tests` (3: a card is played inside a live window and without the facts the step stops with `Unknown(PlayFacts)`, the facts follow the position, the Mahact/Reveal reads), `driver::live::tests::the_play_facts_an_ability_finds_are_computed_from_the_position_of_the_call`, bridge `tests/play_facts.rs` (string facts vs core facts, field by field, over the template openings and variants: unit upgrades, Reveal Prototype, Mahact pool) |
| 3 | The two hook slots no core faction module registers (`commander_unlocked`, `leader_action`) | NOT done: it is not small (see "Open for lane F2") | `check_kernel_env` now reports the gaps per leader |
| 4 | `playthrough` usable by the island lanes | `bin/playthrough.rs` (`--quick`, `--top`, per-island table, `capped`, seconds per game), `islands.rs` (the sort of a cause into M1..M6/B..F, tests), `tools/engine_core/playthrough_quick.sh` | section "Running the playthrough" below |
| 5 | `dual::tests::detects_an_injected_difference_in_each_category` failed alone under `TI4_DUAL_FAIL_FAST=1` | `dual.rs`: `set_fail_fast`/`fail_fast`/`tally` read the environment before the explicit store | regression test `the_injecting_tests_pass_alone_under_the_fail_fast_environment` (runs the injecting test alone in a child with the variable set; both tests fail on the pre-fix code, checked) |
| 6 | Docs | this file, `docs/D2_LF_RESUME.md` (updated), `docs/todo/D2-islands-composition.md` + `D2-islands-machines.md` merged and given an agreement block each | |

## Running the playthrough (the cheap one-game check of an island lane)

```text
export CARGO_TARGET_DIR=/root/deploytmp/<your lane>   # private; delete when done
CARGO_BUILD_JOBS=2 tools/engine_core/playthrough_quick.sh                    # sol, 4 players, seed 1, 2500 steps
CARGO_BUILD_JOBS=2 tools/engine_core/playthrough_quick.sh --factions hacan --players 6 --first-seed 3
CARGO_BUILD_JOBS=2 tools/engine_core/playthrough_quick.sh --maxsteps 600 --top 5
```

The script builds the `gate` profile (optimised, no LTO, `docs/ENGINE_BUILD_SPEED.md`; incremental off) and runs `playthrough --quick`: ONE faction, ONE player count,
ONE seed, `card_set te`, `starting_tech`, all areas `Dual`, `TI4_BAIL_IS_FAILURE` on. A debug build needs more than 17 CPU minutes for 2500 steps under `all=dual`; the gate
build runs the same game in seconds-to-minutes (the hashes do not depend on the profile). Not a release build.

Read, in this order: the per-game line (rounds, steps, `capped (quick)` or `finished`, bails, seconds); **"bails by island"**: one row per lane (`M1 trade` ...
`F menu and leaders`, `other`), `green` or the biggest causes (`--top N`); "lent steps"; "coverage of the live path": a zero bail count with zero live steps of YOUR arm proves
nothing (the game must have reached your island: raise `--maxsteps`). A game that reaches the step cap without a bail is `NO BAIL SO FAR`, not a failure; the exit status is
1 on any bail, unmapped position, lent step, panic or difference. `--list` prints the games without playing. The sort of a cause into an island is `islands::island_of`
(first rule that matches, tested against the causes of the first run); a new cause shows as `other`: teach it there.

The heavy Z-gate (`docs/todo/D2-islands-machines.md` 5.4 (b)) is NOT this: it needs the quick check green on 8 consecutive seeds first, then every lobby faction at 4 and 6 players.

## The design points you need

* **Play facts are per call, from the core.** `AdapterFacts::plays_provider` is a function of `(CoreState, Kernel)`; `driver::live::table_at` asks it every time a driver is built
  (once per `advance`/`ask`), so the unit a seat places after a technology, the Mahact pools, Reveal Prototype's set and payability follow the position. Build the `Env` of a window that
  can see a card play with `live::env::env_with_plays(&render, &core, kernel, seeds, sources)`; `env_for` alone leaves them out and the step stops with `LiveError::Unknown(PlayFacts)`.
  Within one call the facts are as fresh as the replay path's (read when the window is armed). Departures from the string side, by decision: no Crimson card (on-card ships 0), no Ral
  Nel (`skimmer` false), and the string declines Reveal Prototype at a table with a seat whose research it does not port (a counted `gap`, the core answers through the live kernel).
* **Reveal Prototype's matching is shared.** `action_cards_view::reveal_set_of_lines` takes the base-type texts of the seat's units in the active system; the string reads them from
  the `GameState`, the live path from the core board. The matching (`baseUpgrade`, the unnamed second generation by name) is one function, quirks included (it does not list a
  faction upgrade whose `baseUpgrade` is no unit type, nor the War Sun).
* **`NativeSubject`.** The legacy game steps for real (it is the follower of the oracle), so the line never diverges; before each step the subject reads the arm of the legacy step loop
  (`step_arm(Game::step_facts())`), materializes a `CoreState` (`Codec::load`), opens the arm's window from the windows the legacy game holds (`Game::observed_windows`, the payloads
  `d1::pending` compares) with `native_subject::open_arm`, and after the legacy step runs `live_step` with the same answers. Wiring an arm = a live function (`driver/live.rs`) plus ONE
  line in `open_arm`; an arm without one is counted lent (`no opener`). `native_subject::{ran_counts, lent_counts}` and `oracle ... --native tokens` print what ran and what was left.
  A step that ENDS the window is compared on the prefix of questions and events only (the legacy step goes on with the close the core does not own: for the token gain the
  remainder moves the very pools the last token filled); the position is compared on the steps that leave the window open. When a lane ports the close it should drop the
  `finished` exception in `compare` for its arm.
* **Gap in the oracle I did not close:** the subject compares from the position the LEGACY game has before the step; a native arm that depends on state the legacy game keeps outside
  `GameState` and outside the observed windows (host side channels: `rng_force`, observers, `blocked`) is not seen. `observed_windows` fails (`lent: <why>`) for a window the tables cannot type.

## Open for lane F2 (item 3, with the exact list)

Not small: 29 string functions, several with helper queries that are not in the core (`hero_destinations`, `devourable_planets`, `can_pay_for_agent`, `elysium_available`, `ship_targets`, ...).
`cargo test -p ti4-engine-core-bridge --test kernel_env -- --nocapture` prints the gaps per leader (55 answers each in the three-position smoke):

* `commander_unlocked` (in scope): arborec, argent, cabal, empyrean, ghost, mahact, mentak, muaat, nekro, nomad, saar, winnu. (dropped: bastion, crimson, deepwrought, firmament, obsidian.)
* `leader_action` (in scope): arborecagent, arborechero, argenthero, cabalhero, empyreanhero, ghosthero, mahacthero, muaatagent, naazhero, nekroagent, nekrohero, nomadhero, saarhero, titanshero, winnuhero,
  yinhero, yssarilhero. (dropped: crimsonagent, deepwroughthero, firmamenthero, keleresheroharka, obsidianhero, ralnelagent.)

The commanders are 3-12 lines each in the pattern of `ti4-engine-core-factions/src/cd_registry/{naaz,titans,yin}/query.rs` (`systems_with_piece`, `board::planet_units`); the string
bodies are in `ti4-engine/src/factions/<faction>*.rs` (`fn commander_unlocked`, `fn leader_action`). The variants of `check_leaders` do not reach most conditions (six dreadnoughts...): add
variants that satisfy and miss each condition, or the comparison proves nothing.

## Checks run (cheap) and results

See the end of this file (filled in at the end of the lane).
