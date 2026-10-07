# Handover to Codex — base-faction completion (BF), 2026-10-03

Written by the Claude Code coordinator at the end of its session. Everything below is reproducible
from the repository; where this file and the repository disagree, trust the repository and say so.

## 0. Read first, in this order

1. `AGENTS.md` (changed on this branch: Pi/Qwen is no longer the default implementer — see §9).
2. `plans/SCOPED_PERMISSIONS.md`, `plans/EXECUTION_STATE.md` (section "Base-faction completion (BF)").
3. `plans/BASE_FACTIONS_PLAN_2026-10-02.md` — scope, definition of done, waves.
4. `plans/BF_AGENT_GUIDE.md` — the contract every faction implementation follows. Read it even if
   you implement yourself; its rules are the acceptance criteria reviewers applied.
5. `crates/ti4-engine/src/factions/mod.rs` and every `factions/hooks_*.rs`.
6. Evidence, newest first: `plans/evidence/BF-wave-C.md`, `BF-wave-B2.md`, `BF-wave-B.md`,
   `BF-00l.md`, `BF-00a.md`, then the per-package and per-faction files listed in §6 and §7.

## 1. Objective and scope (decided with the operator, 2026-10-02)

Implement 12 factions to "every printed faction asset works in a real game": the ten base factions
not previously in scope — **Arborec, Ghosts of Creuss (`ghost`), Mentak, Muaat, Naalu, Saar, Sardakk
N'orr, Winnu, Yin, Yssaril** — plus PoK **Argent Flight** and **Naaz-Rokha Alliance**. **Nekro is
deferred** (it copies other factions' technologies; do it last, after all of these are complete).

Assets per faction: abilities, faction technologies (incl. unit upgrades), faction units (flagship,
mech, replaced base units and upgrades, Thunder's Edge extra upgrades), promissory note (for owner and
receiver), agent/commander/hero, Thunder's Edge breakthrough, TE reprints. Text: latest printing in
`crates/ti4-content/content/*.json` at `ti4_model::content_types::DEFAULT` (errata + Codex + TE).

The six previously in-scope factions (Sol, Hacan, Letnev, Xxcha, Jol-Nar, L1Z1X —
`seating::IN_SCOPE_FACTIONS`) are **not** reworked, and **their play must not change** unless the
operator approves a `ti4-sim` re-baseline (see §8).

**Not done yet, by design:** widening `IN_SCOPE_FACTIONS` / seating the new factions in sim and
training (plan rows BF-20..BF-25). The new factions exist in the engine but no rollout seats them.

## 2. Git state

- Branch **`wp/base-factions`**, created from `main` at `58986a6b`. Not merged, not pushed.
- Commits on the branch (oldest first):

| Commit | Content |
|---|---|
| `acd0f6b8` | Plan; AGENTS.md drops Pi/Qwen as default implementer |
| `29786800` | BF-00a: per-faction module seam, asset ledger, `leaders::for_faction` sheet fix |
| `85fa4155` | BF-00l: timing + per-unit combat hooks, area hook files, fixtures, agent guide |
| `60f7686a` | Wave B: space/ground/economy routes (events, hooks, effect APIs) |
| `f5bddcae` | Hook files movement/cards/strategy; `GameState::faction_marks` |
| `e18ec179` | Wave B2 routes (movement, hidden hands, strategy/capture/forms) + Sardakk + Yin |
| `aebf1ce9` | BF-01 coordinator fixes + 10 faction modules (Winnu … Naaz) |

- **Shared checkout.** Another session works in the same tree. At handover its uncommitted files
  were: `crates/ti4-engine/src/game.rs` (a trade-goods income *experiment*: `STATUS_INCOME_TRADE_GOODS`
  static, `round_income_trade_goods`/`with_round_income*`, and their use in `step_phase`), plus
  ti4-mlp, ti4-training, ti4-policy (`critic.rs`, `lib.rs`, `progress.rs`, new `power_facts.rs`,
  `power_map.rs`), ti4-replayer, ti4-review (`gui.rs`, `lib.rs`, `main.rs`, tests, `power.rs`,
  `examples/`), `scripts/`, `plans/INDEX.md`. **None of that is BF work. Never stage it.**
- Because `game.rs` carries their hunks, BF changes to `game.rs` were committed with a filtered patch:
  `git diff HEAD -- crates/ti4-engine/src/game.rs`, drop hunks mentioning `round_income` /
  `STATUS_INCOME` (split mixed hunks by hand), `git apply --cached --check --recount`, then
  `git apply --cached --recount`. Verify afterwards that `git diff HEAD -- game.rs` shows only their
  lines. Do the same for any future `game.rs` commit while their experiment is uncommitted.
- Untracked `target-cuda-repack/` and `scripts/resume_*.psd1` are not ours.

## 3. Architecture you are inheriting

### 3.1 The faction seam (`crates/ti4-engine/src/factions/`)

- `mod.rs`: `FactionModule { alias, abilities, technologies, units, promissory, leaders,
  breakthroughs, hooks }`. The id lists are **claims**: "printed text implemented, tested, and live in
  a real game path". `MODULES` is a fixed array (dispatch order = determinism).
- One file per faction: `arborec.rs argent.rs ghost.rs mentak.rs muaat.rs naalu.rs naaz.rs saar.rs
  sardakk.rs winnu.rs yin.rs yssaril.rs`. Each exports `pub const MODULE`.
- `Hooks` (in `mod.rs`): value hooks mirroring the old shared ones (`combat_modifier`,
  `fleet_supply`, `status_tokens`, `waived_prerequisites`, `substitutes_primary`, `secondary_is_free`,
  `trades_action_cards`, `ignores_neighbours`, `control_gained`, `component_actions` /
  `perform_component`, `strategy_resolved`, `ground_combat_round_ended`,
  `space_combat_round_started`, `commander_unlocked`, `leader_action` / `use_leader`, `vote_bonus`),
  plus `timing_abilities` (modules register `crate::timing::Ability` per seat inside
  `reactions::arm`), `unit_roll_modifier`, `unit_dice` (`CombatUnit`), and nested per-area tables:
  `combat` (`hooks_combat.rs`), `ground` (`hooks_ground.rs`), `economy` (`hooks_economy.rs`),
  `movement` (`hooks_movement.rs`), `cards` (`hooks_cards.rs`), `strategy` (`hooks_strategy.rs`).
  Each area file holds its fields, `NONE`, dispatch fns over `super::MODULES`, and a
  `#[cfg(test)] with_test_hooks(hooks, || ..)` (thread-local, restores on panic).
- Shared code calls the dispatchers; with all modules returning the neutral value nothing changes.
  Always end hook-struct literals with `..XHooks::NONE` (adding a field broke `naalu.rs` once).
- Contract (on `Hooks`): hooks are called for every player and module — check your own condition;
  refusal (`false`/`None`/`Some(false)`) leaves state untouched; module hooks run before shared code
  in `control_gained`, `perform_component` (claim first), `strategy_resolved`,
  `ground_combat_round_ended`, `space_combat_round_started`, after it in value hooks;
  `timing_abilities` is called once at construction (build the same abilities regardless of state);
  resolver `Frequency` bookkeeping is not saved — gate "once" on state; several emit sites swallow
  WHEN-cancels (`let _ = emit(..)`).

### 3.2 Ledger

`factions::assets / implemented / missing` compare claims (plus the old shared registries) against
the corpus. Report:

```bash
cargo test -p ti4-engine --lib print_faction_ledger -- --ignored --nocapture
```

Tests in `factions/mod.rs` enforce: every module is a real corpus faction, the sheet is complete,
claims name only the faction's own assets, leaders follow the faction sheet, every planned faction
seats with the fixture.

### 3.3 State and privacy

- `GameState::faction_marks: BTreeMap<String,String>` (ti4-model; skipped when empty, so old saves
  and hashes are unchanged). Key namespaces and visibility (`ti4_model::view::mark_visible_to`):
  `cards:reveal:<scope>|<source>|<viewer>|<owner>|<kind>` → viewer only; `cards:staged:*` → nobody;
  `private:<player>:*` → that player only; everything else public. `view_for` (ti4-model and
  ti4-policy) and `choice.rs` redaction drop invisible rows; `leaks()` checks them.
- Other typed fields added on the branch (all skip-when-empty): `initiative_overrides`,
  `production_value_swapped_planet`, captured units already existed.

### 3.4 Events added (typed, through the resolver)

Space: `SPACE_COMBAT_ROUND_ENDED`, `SPACE_COMBAT_ENDED`. Ground: `GROUND_FORCE_SUSTAINED`,
`GROUND_FORCE_DESTROYED` (invasion paths only), `GROUND_COMBAT_STARTED`, `GROUND_COMBAT_ENDED`,
`GROUND_COMMITMENT_FINISHED`; `UNITS_COMMITTED` gained `from_system/from_planet`. Economy:
`UNITS_PRODUCED`, `ACTION_CARDS_DRAWN`, `TRADE_GOODS_GAINED` (only via `supply::gain_trade_goods_*`,
which nothing in production calls yet). Cards: `ACTION_CARD_TAKEN` (no card id, by design).
Movement: `SHIPS_RELOCATED`; `SHIP_MOVED` payload gained `system`, `origin`, `unit`. Strategy:
`UNIT_CAPTURED`, `CAPTURED_UNIT_RETURNED`. Phases: `STRATEGY_PHASE_ENDED`, typed
`STATUS_PHASE_BEGAN` (in addition to the old log label), `STATUS_PHASE_ENDED`. All new names have no
reaction-card listeners, so in-scope play is unchanged; they do shift event ids/timing-log lines.

### 3.5 Effect APIs added (selection)

`combat::destroy_units` (+ staged announcement), `production::produce_by_ability` /
`ProductionWindow::for_ability`, `action_cards::{place_units_choosing, place_units_counted,
replace_unit, PlacementLimits}`, `tokens::{place_command_token_from_reinforcements,
place_creuss_token, remove_creuss_token}`, `transit::relocate_ships` (now refuses unenterable
destinations), `movement::{PlayerAdjacency, apply_map_edit, replay_map_edits}`,
`strategy::{set_initiative_override, exchange_strategy_cards}`, capture APIs in `supply.rs`,
`fleet::{other_form, flip_form}`, `production::{begin_value_swap, end_value_swap}`,
`hooks_cards::{reveal*, take_revealed_action_card}`, `action_cards::{choose_from_own_hand,
look_at_hand_and_take}`, `tactical::activation_options_with(content, sources, ..)`.

### 3.6 game.rs wiring done by the coordinator (BF-01)

`begin_one_move` uses `path_from_ship` + `effective_move_value_for_ship(.., Some(index))` and the
module move bonus in the Gravleash value; `announce_staged_cards()` after faction component actions,
leader actions and at the end of every `emit_typed`; `clear_reveals(Action)` at `ACTION_COMPLETED`;
`replay_map_edits` each step (`GameError::MapEdit` blocks the game); typed phase events above with
`sync_timing_context()` before `STATUS_PHASE_BEGAN`; activation through `activation_options_with`.
`laws::apply_to_galaxy` applies module extra wormholes.

### 3.7 Test fixtures (`crates/ti4-engine/src/fixtures.rs`)

`seated_game(&[("a","<alias>"),("b","sol")], DEFAULT)` (deploys home/units/techs/leaders — don't
overwrite `Player::faction` on `fixtures::game`), `with_context(..)`, `armed_resolver(&state)`
(arms through `reactions::arm`, so a test proves a module's timing hook is wired). Note: Naalu agent
found `seated_game` deals promissory notes before the faction swap; its tests call
`promissory::deal` again — worth fixing in the fixture.

### 3.8 Decision registry

`crates/ti4-engine/tests/decision_delivery_inventory.rs` scans every `Choice::new` / ask site. Every
new choice site must be registered (a `Producer` entry and an `OBSERVED_ASKS` entry). Faction sites
are registered under the comment "BF faction modules". The coordinator workflow: run the test, diff
"observed" vs "registered" from the panic message, register exactly what the scan found (helper
script logic: parse `Site { module, function, operation }: n` from left/right maps).

## 4. Status by faction (ledger at `aebf1ce9`: 92 / 145 claimed)

Claimed items are implemented, tested (positive + negative), Opus-reviewed, and reachable live.
Every faction also has a "a game without this faction is unchanged / offered nothing" test.

| Faction | Claimed | Not claimed (reason → see that faction's evidence "Hook requests") |
|---|---|---|
| Sardakk | 10/12 | `sardakkcommander` (commit from adjacent systems: `commit_candidates` needs `Option<&Galaxy>`), `sardakkhero` (invasion-as-effect `Aftermath` in game.rs) |
| Winnu | 10/11 | `winnuhero` (Warfare's free tactical action can't run inside a leader effect; hero is otherwise atomic and offers unclaimed cards) |
| Creuss (`ghost`) | 10/12 | `ghostcommander` (SHIP_MOVED lacks "moved through a wormhole"), `ghostbt` (extra-PRODUCTION + combined-cost hooks) |
| Yssaril | 9/12 | `scheming` (status-phase/agenda draws bypass `action_cards::draw`), `yssarilagent` (`leaders::use_leader_text` + borrowed ACTION windows), `yssarilbt` (transactions limit call site; consent shape for others using Stall Tactics/Scheming) |
| Arborec | 9/12 | `lw2` + `arborec_infantry2` (GROUND_FORCE_DESTROYED only on invasion paths), `arborechero` (leader production emits no UNITS_PRODUCED) |
| Mentak | 8/12 | `pillage` + `mentakagent` (TRADE_GOODS_GAINED not emitted at real gain sites), `mentak_cruiser3` + `mentakbt` (unit-form override hook; Corsair has no unlock route) |
| Yin | 8/11 | `yincommander` effect (green-prerequisite + research waiver hooks), `yinhero` (invasion-as-effect), `yinbt` (alliance grant + on-score event) |
| Muaat | 7/13 | `gashlai_physiology` (split supernova "through" vs "into"), `mr` (extra-PRODUCTION hook), `muaatagent` (per-unit max cost in `for_ability`), `muaatcommander` (strategy-token-spent event), `muaathero` (no typed SHIP_MOVED on no-cargo moves; needs movement-finished window; purge via shared destroy/purge), `muaatbt` |
| Argent | 6/13 | `zeal` (voting order hook), `ah` (passage block + extra PRODUCTION), `argent_flagship` (space-cannon bar in `combat.rs`), `argent_mech` (free cargo slot), `argentagent` (WHEN before ground-force placement), `argenthero` (relocation with cargo), `argentbt` |
| Naalu | 6/13 | `foresight` (no-cargo SHIP_MOVED + movement-finished window), `hcf2`/`naalu_fighter2` (half-ship fleet charge in `fleet.rs`), `naalu_flagship` (fighters as ground forces), `naalu_mech_te` (relic-gained events elsewhere), `naaluagent-te` (COMMAND_TOKEN_PLACED), `naalucommander` (live hidden-hand / agenda peek) |
| Naaz-Rokha | 6/13 | `distant_suns`, `pfa` (explore hooks in `exploration.rs`), `sc` (ground round start hook; space half is live but unclaimed), `naaz_mech`/`naaz_mech_space` (`flip_form` call sites), `naaz_voltron`, `naazbt` |
| Saar | 3/13 | `nomadic` (hook now exists: `scores_without_home` — implement in saar.rs), `cm` (hook now exists: `cannot_activate` with content — implement; production half gated to tests), `scavenge` + `saar_mech` (PLANET_CONTROL_GAINED only on landing), `ffac2`/`saar_spacedock*` (dock movement in `tactical.rs` + blockade destruction), `saaragent`, `saarcommander`, `saarbt` |

Partial-but-live items that run in real games while unclaimed (because they behave correctly on the
paths they reach): Scavenge, Scavenger Zeta, Naaz Supercharge (space only), Arborec Letani Warrior II
on invasion paths, Mentak Pillage on transactions. Items gated with `cfg!(test)` because they would
misbehave live: Naalu Foresight / Z'eu / Iconoclast, Muaat Nova Seed hero, Saar Chaos Mapping
production.

## 5. Verification state at `aebf1ce9`

| Command | Result |
|---|---|
| `cargo test -p ti4-engine -q --no-fail-fast` | lib 1782 passed, 1 ignored; all integration binaries pass (incl. `decision_delivery_inventory` 4/4) |
| `cargo test -q -p ti4-model -p ti4-content` | pass |
| `LIBTORCH=<abs>/out/libtorch-2.9.1-cpu cargo check --workspace --all-targets` | ok |
| `LIBTORCH=<abs>/out/libtorch-2.9.1-cpu cargo test -q -p ti4-sim` | 51 passed, 1 ignored — authored-bot behaviour unchanged |
| `cargo clippy -p ti4-engine --all-targets` | no new warnings (pre-existing ones in combat.rs, action_cards.rs, agenda_effects.rs, transactions.rs, game.rs:166/`step_secondary` unfulfilled expectation, invasion.rs, promissory.rs, diplomacy/*) |

Caveat: the committed `game.rs` = HEAD + BF hunks only; it was not compiled in isolation (the
working copy also has the other session's hunks). The BF hunks never reference the experiment, and
the full tree compiles, so the risk is low — but if you get a clean checkout, build it.

Known pre-existing red test, ignored by operator instruction: `ti4-review --test semantic_golden`
(red on `main` since `52066efa`; do not regenerate or report it).

`ti4-replayer` tests once failed to link (`link.exe` 1104 / missing rlib) — an environment/file-lock
issue from concurrent builds, not code. Rerun alone if seen.

## 6. Work queue (recommended order)

### 6.1 Needs operator approval first (changes in-scope play → `ti4-sim` re-baseline)

1. **Typed `SHIP_MOVED` for ships that sail with no cargo.** `game.rs` `begin_one_move` no-cargo
   branch (~"No capacity, or nothing to carry: sail immediately") calls only `self.emit("SHIP_MOVED")`;
   call `self.note_arrival(&window.player, origin, &ship, &outcome)` there too. Also add a
   "movement finished" typed event (e.g. `MOVEMENT_FINISHED` with system + the ships that arrived) for
   once-per-move effects. Unblocks Naalu Foresight, Muaat hero (needs the window, not per ship),
   Creuss commander (add `wormholes` crossed to the payload); fixes two existing reaction cards
   ("after a player moves ships into") that currently miss these moves.
2. **Typed `SYSTEM_ACTIVATED` for the strategy-card free tactical action** (`game.rs` ~1855 emits
   only labels). Wakes existing "after you activate a system" cards there.

Re-baseline process: memory/docs say derive new bounds with
`cargo run --release -p ti4-sim --example rebaseline_behavior` **in a clean checkout of the commit
that ships**, record old/new side by side in `plans/evidence/M08-021.md` with the semantic cause, and
get operator review. The operator forbids new folders/worktrees outside the repo; ask before creating
any folder.

### 6.2 Shared engine packages (no approval needed if neutral; prove with ti4-sim)

Each is a coordinator-style package touching shared files; afterwards resume the affected factions.

| # | Package | Unblocks |
|---|---|---|
| A | Emit `GROUND_FORCE_DESTROYED` from every ground-force destroy path (`action_cards::destroy_on_planet`, agendas, space cannon outside invasion, Saar hero) — new event name already exists; listeners only in modules | Arborec `lw2`/`arborec_infantry2`, Yin agent coverage |
| B | Emit `PLANET_CONTROL_GAINED` on every control gain (not only invasion landing) — careful: existing reaction windows listen to it → check `reactions::window_table`; if any, this needs approval | Saar Scavenge / Scavenger Zeta, Winnu Reclamation coverage |
| C | Route trade-good gains through `supply::gain_trade_goods_announced` at the main sites (Trade primary, status income, agenda/relic gains; 31 `+=` sites in 15 files) | Mentak Pillage + agent |
| D | Route status-phase draw (`status.rs`), Politics Rider and agenda draws through `action_cards::draw` (needs content/table threaded from `game.rs`); neutral when no hook | Yssaril Scheming |
| E | Leader effects get a resolver: let `use_leader` / leader production announce events (`UNITS_PRODUCED`, staged destructions) — pass a timing handle or stage + flush after `perform_leader_action` | Arborec hero, Saar hero destruction events |
| F | Invasion as an effect: `game.rs` `Aftermath` entry opening `InvasionWindow::at_commit_step` from a leader; commit from reinforcements; "no space cannon" flag | Sardakk hero, Yin hero |
| G | `GroundHooks::commit_candidates` gets `Option<&Galaxy>` | Sardakk commander |
| H | Explore hooks: `explore_extra_draw` in `exploration::explore_drawn`, `explored` at end of `explore_with` | Naaz Distant Suns, Pre-Fab Arcologies |
| I | `fleet::flip_form` call sites (start/end of space combat, landing) + ground round-start hook | Naaz mechs, Supercharge ground half |
| J | Supernova split: `may_pass_through_supernova` vs `may_end_in_supernova` in `MovementRules` | Muaat Gashlai Physiology |
| K | `EconomyHooks::extra_production` (+ producer semantics) and combined-cost reduction | Muaat `mr`, Creuss `ghostbt`, Argent `ah` |
| L | Mobile docks: `tactical.rs` use `UnitType::moves_as_ship()`, retreat, `production::destroy_blockaded_mobile_docks` after movement/combat | Saar `ffac2`, docks, `saarbt` |
| M | Hook signatures get `sources`/galaxy: `component_actions`, `leader_action`, `planet_spend_value` (modules currently assume DEFAULT) | Creuss Wormhole Generator offer check, correctness on non-DEFAULT games |
| N | `TURN_PASSED` for the final pass of the action phase (`game.rs` `advance_turn` returns None) | Naaz agent |
| O | Misc: unit-form override hook (Mentak Corsair), half-ship fleet charge (`fleet.rs`, Naalu `hcf2`), voting-order hook (Argent Zeal), space-cannon bar (Argent flagship), free cargo slot (Argent mech), `leaders::use_leader_text` (Yssaril agent), transactions limit call site (Yssaril bt), `TRANSACTION_RESOLVED` parties, strategy-token-spent event (Muaat commander), command-token-placed event (Naalu agent), green-prerequisite + research waiver hooks (Yin commander), alliance grant (Yin bt) |

Exact signatures proposed by the faction implementers are in each `plans/evidence/BF-<alias>.md`
under "Hook requests" — start there.

### 6.3 Directly implementable now (hooks exist, faction code missing)

- Saar `nomadic` → `StrategyHooks::scores_without_home` (called first in
  `objectives::controls_home_system`).
- Saar Chaos Mapping activation bar → `MovementHooks::cannot_activate(state, content, sources,
  activator, system)` (read in `tactical::activatable_with`; asteroid check via content).
  Also the agent's production half is `cfg!(test)`-gated pending a producer check (now required).

### 6.4 Then

Nekro (deferred faction), plan Phase 2: BF-20 widen `IN_SCOPE_FACTIONS` (operator approval; every
training/sim/review list with a set-equality test will fail by design: `ti4-review FACTIONS`,
`ti4-training teacher_corpus::FIXED_FACTIONS`, `ti4-policy battle::FACTIONS`, `coverage_report`),
BF-21 seat selection for 18 factions, BF-22 policy battle-arena width/schema, BF-23 ti4-sim
re-baseline, BF-24 replayer presentations, BF-25 exit (full suite, 18-faction soak, frontier exit
review). Also: the six existing factions show unclaimed techs/flagships/PNs in the ledger because
those kinds never had registries — audit before BF-25.

## 7. Evidence index

- Plan / guide: `plans/BASE_FACTIONS_PLAN_2026-10-02.md`, `plans/BF_AGENT_GUIDE.md`.
- Infrastructure: `plans/evidence/BF-00a.md`, `BF-00l.md`, `BF-00c-space.md`, `BF-00c-ground.md`,
  `BF-00b-economy.md`, `BF-wave-B.md`, `BF-00e-movement.md`, `BF-00h-cards.md`, `BF-00d-strategy.md`,
  `BF-wave-B2.md`, `BF-wave-C.md`.
- Factions: `plans/evidence/BF-<alias>.md` for arborec, argent, ghost, mentak, muaat, naalu, naaz,
  saar, sardakk, winnu, yin, yssaril — each with Items, Hook requests, Decision sites, Rules
  questions, Commands/results, "Review fixes".

Open rules questions are listed per faction (e.g. Stymie asked per ship, Ragh's Call destination
chooser, Foresight carrying ground forces, Kyver random discard now uniform, Fires of the Gashlai
withholding, Ospha cost reading). Resolve against the LRR / FAQ where possible; otherwise ask the
operator.

## 8. Invariants that reviews enforced (keep them)

1. Games seating only the six in-scope factions must play identically: same options, ids, order,
   dice, outcomes. Prove with `ti4-sim` after any shared change. New typed events: new names only;
   re-using a name some reaction card listens to changes play.
2. Faction timing abilities are registered for every seat; conditions must be false unless that
   seat really holds the card/ability/leader in a usable state (`ScriptDiverged` failures otherwise).
3. Claims = implemented + tested (positive and negative) + live. Unclaim honestly; gate
   misbehaving-live code with `cfg!(test)`.
4. Atomic: decide every choice and check every cost before the first mutation; snapshot/restore for
   multi-step effects (Winnu hero, Naalu Oracle, Kyver were fixed this way).
5. Legal options are generated, never rejected late (e.g. waiver path drops an unpayable "yes").
6. Hidden information only through typed views; private bookkeeping in `private:<player>:` marks.
7. Determinism: BTreeMap/BTreeSet only, game RNG only (`GameRng::die` for uniform picks), fixed
   module order.
8. Every new `Choice` site registered in `decision_delivery_inventory.rs`.

## 9. Process notes and pitfalls

- AGENTS.md now says: no Pi/Qwen default; Claude Code coordinator delegating to Claude subagents.
  If Codex takes over, the same rules apply: one implementer per file, independent frontier review
  before commit, evidence before commit, explicit-path commits.
- Parallel agents in one checkout share `target/`; a half-edited file breaks everyone's build. Agents
  were told: keep compiling at every save, write only their own file + evidence. The coordinator owns
  shared files, hook structs and the decision registry.
- Agent sessions hit API rate limits several times; resuming the same agent with its context worked.
  Check `git status` for partial edits after any interruption.
- `cargo test` filters: several filters go after `--` (`cargo test -p ti4-engine --lib -- a:: b::`).
- `LIBTORCH` must be an absolute path to `out/libtorch-2.9.1-cpu` for ti4-sim / workspace checks.
- `rustfmt --edition 2024 <file>` on your own files only; `cargo fmt --check` shows the other
  session's unformatted `game.rs` line (~749) — not ours.
- Operator rules (global): never create folders outside the repo, no new worktrees, ask before a new
  folder inside it; never permanently delete anything (no `rm -rf`, `clean`, `prune`, overwriting
  `out/`); be brief in reports. A stray scratch file `/tmp/new_section.rs` (git-bash tmp) was left by
  the Sardakk agent — the operator should remove it; do not delete it yourself.
- `plans/INDEX.md` has no BF row (the other session had it dirty); add one when it is safe.

## 10. Handover summary (AGENTS.md format)

```text
Objective: implement 12 factions (10 base + Argent + Naaz) to full PoK+TE assets; Nekro deferred.
Normative source versions: content corpus at DEFAULT (latest printings); LRR. Python reference not used.
Active milestone/package: BF plan; infrastructure waves A/B/B2 done; factions waves C/D first pass done.
Status: 92/145 assets claimed live; per-faction gaps in §4; shared packages queued in §6.
Current branch and HEAD: wp/base-factions @ aebf1ce9 (not pushed, not merged).
Working-tree state: dirty with another session's unrelated work (see §2); no uncommitted BF work.
Tests last run: ti4-engine lib 1782 passed + integration ok; ti4-sim 51 passed; workspace check ok.
Compatibility evidence: ti4-sim behaviour bounds unchanged on every BF commit.
Decisions made: no migration of the six old factions; per-area hook files; faction_marks with
  privacy namespaces; unclaim rather than overclaim; play-changing fixes deferred for approval.
Open review findings or blockers: two approval items (§6.1); shared packages (§6.2).
Next exact action: ask operator about §6.1; meanwhile implement §6.3 (Saar nomadic + Chaos Mapping bar),
  then package A (GROUND_FORCE_DESTROYED everywhere), verify with ti4-sim, resume Arborec.
Files to read first: AGENTS.md, plans/EXECUTION_STATE.md, this file, plans/BF_AGENT_GUIDE.md,
  crates/ti4-engine/src/factions/mod.rs, plans/evidence/BF-wave-C.md.
```
