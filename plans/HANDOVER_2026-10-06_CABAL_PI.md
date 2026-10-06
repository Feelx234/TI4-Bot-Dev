# Handover: Vuil'raith Cabal for Pi (Qwen 3.8 Flash), 2026-10-06

Faction: **Vuil'raith Cabal** (`cabal`), the next faction in the post-base queue. This file
is self-contained. Pi runs **one package per `/task`**, in the order below. The coordinator
reviews, commits and merges. Pi never commits.

## 0. Operator setup (before the first task)

1. **Start state.** Nomad may still be uncommitted and in progress; operator approved on
   2026-10-06. Uncommitted Nomad work is expected, so do not stop because of it. Until a Nomad
   commit appears in `git log --oneline -5`:
   - **Never edit** these files, which the Nomad agent owns:
     - `factions/nomad.rs`, `factions/nomad_agents.rs`;
     - `combat.rs`, `factions/hooks_combat.rs`;
     - `invasion.rs`, `factions/hooks_ground.rs`;
     - `leaders.rs`, `promissory.rs`, `transactions.rs`;
     - `movement.rs`, `transit.rs`, `factions/hooks_movement.rs`.
   - **In `factions/mod.rs`, touch only your own Cabal lines.** The Nomad lines
     (`pub mod nomad;`, `nomad_agents`, `&nomad::MODULE`, `leader_count`) stay as they are.
     `MODULES` is then 15 entries long.
   - **Run only these packages** while Nomad is open: CB-00, CB-02, CB-03, CB-04, CB-06, CB-08,
     CB-09, CB-11. Each must stay within `factions/cabal.rs` plus its other non-Nomad writable
     files. If one needs a Nomad-owned file, stop and report.
   - **Wait for the Nomad commit** before CB-01, CB-05, CB-07 and CB-10. They touch the
     combat, invasion, movement or promissory code.
   - **Expect build failures caused by Nomad.** The Nomad agent may be mid-edit. A build failure
     in a file you don't own is not yours to fix: wait two minutes and retry once. If it still
     fails, report it and stop.
2. **Launch the controller on the new backend:**
   `python tools/pi_rpc_bridge.py --model <qwen-3.8-flash model/provider id>` (other flags as
   usual). Check `GET /health`. Only one Pi writer may own the session.
3. **Per-task limits.** These packages compile Rust, and an engine test build takes several
   minutes at `-j1`, so the 600 s default is too short. Use `timeoutSeconds = 1800`,
   `noEditSeconds = 180`, `maxToolErrors = 2` for CB-01 and later; CB-00 can keep the defaults.
   Record the raised limits in each package's evidence.
4. **Prompt per task.** Send exactly: "Read plans/HANDOVER_2026-10-06_CABAL_PI.md sections 1–3,
   then do package CB-NN only, then stop with the section 3 report." Substitute the package ID.
5. **Review after each task.** Run `git status --short` and the package diff, and read
   `plans/evidence/BF-cabal.md`. Run the commands under "Commands" yourself before accepting.
   Commit only the package's paths (section 1.4).

## 1. Rules for every package

### 1.1 Hard limits

- Work only in `D:\Projects\ti4-engine-rs`, branch `wp/base-factions`. Do not switch branches.
  Do not run `git add/commit/stash/reset/checkout/clean`, and do not create folders or worktrees.
  Do not delete files or run `cargo clean`.
- The checkout is **shared and dirty on purpose**. Never edit, format or revert:
  - `crates/ti4-engine/src/game.rs` hunks that mention `round_income` or
    `STATUS_INCOME_TRADE_GOODS`;
  - anything under `crates/ti4-mlp`, `ti4-policy`, `ti4-training`, `ti4-replayer`,
    `ti4-review`, or `scripts/`;
  - `plans/INDEX.md`.
- Edit only the files the package lists under "Writable". If you need a change elsewhere, stop
  and report it with the file, function, and exact change.
- Run rustfmt only on the files you changed: `rustfmt --edition 2024 <file>`. Never run
  `cargo fmt` on the workspace. If a writable file already fails rustfmt before your edit, format
  your own lines by hand instead.
- Shell: `C:/Windows/System32/WindowsPowerShell/v1.0/powershell.exe`. For search, use
  `rg --threads 1`.
- Make your first edit promptly. Do not survey the whole corpus. Read the files named in the
  package; widen only if one of them points you elsewhere.

### 1.2 Engine rules

- Legal options are generated up front. Never offer an option and refuse it later.
- Every transition is atomic. On any error path, restore state; ask every question before the
  first mutation, or clone and restore.
- Determinism: use `BTreeMap`/`BTreeSet` only, and the game RNG only.
- Gate everything on actual Cabal ownership or rights:
  - an ability or unit: `state.player(p).faction == "cabal"`;
  - the commander: `crate::promissory::has_commander_ability`.
  
  A game without a Cabal seat must behave exactly as before. Every package adds a
  "no-Cabal neutrality" test.
- Card text in `crates/ti4-content/content/*.json` is authoritative (latest errata/Codex). Quote it
  in the evidence.
- Claims: add an asset id to the matching list in `factions/cabal.rs` `MODULE` only when every
  clause is implemented and tested through a real route (a `Game` step, combat/invasion/production
  window, or resolver event), not just a helper call.

### 1.3 Commands

Build is memory-tight: always use `-j1` and set these once per shell.

```powershell
$env:CARGO_PROFILE_TEST_DEBUG='0'; $env:CARGO_PROFILE_TEST_INCREMENTAL='false'; $env:CARGO_PROFILE_TEST_CODEGEN_UNITS='16'
Remove-Item Env:LIBTORCH -ErrorAction SilentlyContinue
cargo test -p ti4-engine --lib -j1 factions::cabal -- --test-threads=8
cargo test -p ti4-engine -j1 -- --test-threads=8
cargo test -p ti4-engine --lib -j1 print_faction_ledger -- --ignored --nocapture
```

- If a build dies with `out of memory`, `memory allocation failed` or `invalid metadata`, rerun
  once. If it dies again, stop and report.
- Do not rerun a failing test until it passes. Diagnose it.
- Decision registry: `tests/decision_delivery_inventory.rs` fails when you add a new
  `Choice::new` or `ask`/`ask_seeing` call site. It names the function. Register it the way
  neighbouring entries are registered: one `Producer` entry, plus an `OBSERVED_ASKS` line when the
  site is asked through a `TimingContext`.

### 1.4 Evidence and report

- Append one section per package to `plans/evidence/BF-cabal.md` (create it in CB-00). Keep each
  section under 40 lines. Include:
  - card text;
  - the route (file:function);
  - tests added;
  - rule decisions;
  - exact command results, with pass/fail counts;
  - task limits used;
  - anything blocked.
- Final report, under 15 lines:
  - claimed ids;
  - files changed;
  - new decision sites;
  - exact test results;
  - blocked items with the exact change needed.
  
  Then stop. Do not start the next package.

## 2. Context the packages rely on

- **Module seam.** `crates/ti4-engine/src/factions/mod.rs` holds `FactionModule` (the claim lists
  plus `Hooks`), `MODULES`, the ledger, and the roster tests (`every_planned_faction_*`).
  - Hook families: `hooks_combat.rs`, `hooks_ground.rs`, `hooks_economy.rs`,
    `hooks_movement.rs`, `hooks_cards.rs`, `hooks_strategy.rs`.
  - Every family has `NONE`. End struct literals with `..XHooks::NONE`.
  - Timing abilities: `Hooks::timing_abilities` returns `Vec<crate::timing::Ability>`. See
    `factions/titans.rs` and `factions/borrowed_commanders.rs` for `Ability::stateful(...)`
    `.with_optional(true).with_stateful_condition(...)`.
- **Patterns.** Copy from complete modules:
  - `factions/titans.rs` (newest, cleanest);
  - `factions/naaz.rs` (unit-form hooks, staged events);
  - `factions/saar.rs`;
  - `factions/nomad.rs`.
  - Read `plans/BF_AGENT_GUIDE.md` once.
- **Capture already exists.** In `supply.rs`:
  - `capture_from_board`, `capture_from_reinforcements`, `return_captured`, and their
    `*_announced` variants;
  - `captured_by`, `captured_of`.
  
  Captured units sit on the captor's sheet (`Player::captured_units`) as `(owner, unit type)`.
- **Destruction events.** Combat destruction is announced as typed events with a cause and a
  `during_space_combat` flag. See `combat.rs` (`SHIP_DESTROYED`, the `pending_destructions`
  tuple `(system, owner, type, cause, during_space_combat)`, `announce_staged_destructions`) and
  `invasion.rs` (`GROUND_FORCE_DESTROYED`).
- **Cabal work that is already done.**
  - The commander effect `cabalcommander` (up to 2 fighters/infantry do not count against
    PRODUCTION) is in `production.rs` (~lines 1922 and 1975, gated by `has_commander_ability`).
    Only its unlock remains.
  - Gravity rifts are in `movement.rs`.
  - The Fracture is in `fracture.rs`.
- **Thunder's Edge Mecatol:** use `crate::seating::is_mecatol` / `is_mecatol_planet`, never `"18"`.
- **Card texts** (from content):

| Asset | Window | Text |
|---|---|---|
| `devour` (ability) | permanent | Capture your opponent's non-structure units that are destroyed during combat. |
| `amalgamation` (ability) | When you produce a unit | You may return 1 captured unit of that type to produce that unit without spending resources. |
| `riftmeld` (ability) | When you research a unit upgrade technology | You may return 1 captured unit of that type to ignore all of the technology's prerequisites. |
| `cabal_flagship` The Terror Between | — | Capture all other non-structure units that are destroyed in this system, including your own. |
| `cabal_mech` Reanimator | — | When your infantry on this planet are destroyed, place them on your faction sheet; those units are captured. |
| `cabal_spacedock` Dimensional Tear I | — | This system is a gravity rift; your ships do not roll for this gravity rift. Place a dimensional tear token beneath this unit as a reminder. Up to 6 fighters in this system do not count against your ships' capacity. |
| `cabal_spacedock2` / tech `dt2` Dimensional Tear II | — | Same, with up to 12 fighters (and PRODUCTION 7). |
| `vtx` Vortex (tech) | ACTION | Exhaust this card to choose another player's non-structure unit in a system that is adjacent to 1 or more of your space docks. Capture 1 unit of that type from that player's reinforcements. |
| `cabalagent` The Stillness of Stars | After another player replenishes commodities | You may exhaust this card to convert their commodities to trade goods and capture 1 unit from their reinforcements that has a cost equal to or lower than their commodity value. |
| `cabalcommander` That Which Molds Flesh | When you produce fighter or infantry units | Up to 2 of those units do not count against your PRODUCTION limit. Unlock: Have units in 3 gravity rifts. |
| `cabalhero` It Feeds on Carrion | ACTION | Each other player rolls a die for each of his non-fighter ships that are in or adjacent to a system that contains a dimensional tear; on a 1-3, capture that unit. If this causes a player's ground forces or fighters to be removed, also capture those units. Then, purge this card. |
| `crucible` (promissory) | After you activate a system | Your ships do not roll for gravity rifts during this movement; apply an additional +1 to the move values of your ships that would move out of or through a gravity rift instead. Then, return this card to the Vuil'raith player. |
| `cabalbt` Al'Raith Ix Ianovar | — | This breakthrough causes The Fracture to enter play without a roll, if it is not already in play. After this card enters play, move up to 2 ingress tokens into systems that contain gravity rifts. Apply +1 to the MOVE value of each of your ships that start their movement in The Fracture. |

Re-read each text from the JSON before implementing; the table is a convenience copy.

## 3. Packages (run in order)

Each package is one `/task`. Each has a tier-C review by the coordinator, except CB-00
(tier A).

### CB-00 — module skeleton and registration

- **Objective:** register an empty Cabal module so the ledger and roster tests see it.
- **Writable:**
  - `crates/ti4-engine/src/factions/cabal.rs` (new);
  - (the `MODULES` length is 15 once Nomad is registered: count the entries, don't assume);
  - `crates/ti4-engine/src/factions/mod.rs` (only the `pub mod cabal;` line, the `MODULES` array
    length and entry, and roster-test expectations if a test assumes the old count);
  - `plans/evidence/BF-cabal.md` (new).
- **Do:** model `cabal.rs` on the skeleton style of `factions/titans.rs`:
  - a `FACTION` const;
  - `MODULE` with empty claim lists;
  - `hooks: Hooks { timing_abilities: Some(timing_abilities), ..Hooks::NONE }`;
  - `timing_abilities` returning an empty `Vec`.
  
  Register it in `mod.rs`. Check that `fixtures::seated_game(&[("a","cabal"),("b","sol")], DEFAULT)`
  seats Cabal: the roster tests `every_planned_faction_*` do that.
- **Tests:** none new. The existing roster tests must pass with Cabal included.
- **Done when:** `cargo test -p ti4-engine -j1` is green, and the ledger prints a `cabal 0/N` line.

### CB-01 — Devour and The Terror Between

- **Objective:** capture destroyed units per Devour and the flagship.
- **Writable:** `factions/cabal.rs`, `plans/evidence/BF-cabal.md`, and if no existing event
  carries what you need, `combat.rs` / `invasion.rs` (only an added payload field or a call to a
  hook). Read first: `supply.rs` capture functions, and `combat.rs` / `invasion.rs` destruction
  announcements.
- **Do:**
  - **Devour:** after a combat destruction of an opponent's non-structure unit, in a combat the
    Cabal player is in, capture it with `capture_from_board_announced` or the equivalent (the unit
    is already gone, so it goes onto the sheet).
  - **Flagship:** every other non-structure unit destroyed in the flagship's system is captured,
    the Cabal's own units included, combat or not.
  - Do not double-capture when both apply.
  - Use the typed destruction events through timing abilities.
- **Tests:** add 5–8.
  - Real space combat: Devour captures a destroyed enemy cruiser.
  - Ground combat: Devour captures infantry.
  - A structure (PDS) is not captured.
  - The flagship captures the Cabal's own destroyed ship and an enemy ship, with no double capture.
  - A destruction outside combat is captured by the flagship but not by Devour.
  - Neutrality.
- **Claim:** `devour`, `cabal_flagship`.
- **Traps:**
  - Fighters and infantry destroyed in combat are non-structure, so they are captured.
  - A captured unit must leave the owner's reinforcements count (`supply::held` already counts
    captured units).

### CB-02 — Reanimator (mech)

- **Writable:** `factions/cabal.rs`, evidence.
- **Do:** when the Cabal player's infantry on a planet holding a Cabal mech are destroyed, capture
  them onto the Cabal's own sheet (owner = Cabal).
- **Tests:** 3–5.
  - Ground combat on a mech planet.
  - No mech on the planet, so no capture.
  - Another planet is unaffected.
  - Neutrality.
- **Claim:** `cabal_mech`.

### CB-03 — Amalgamation

- **Writable:** `factions/cabal.rs`, evidence; `production.rs` only through an existing economy
  hook if one fits (read `hooks_economy.rs` first). If a new hook is needed, add it to
  `hooks_economy.rs` with dispatch, plus a call site in `production.rs`, and keep it minimal.
- **Do:** when producing a unit, the Cabal player may return 1 captured unit of the same type to
  its owner's reinforcements (`return_captured`) to produce that unit without spending resources.
  - Offer this per produced unit type in the production window, as an option, not as a late
    rejection.
  - It still uses production capacity, unless the card says otherwise. It does not.
- **Tests:** 4–6.
  - A real `ProductionWindow` produces a cruiser free by returning a captured cruiser.
  - No captured unit of that type means no offer.
  - The returned unit goes back to its owner's reinforcements.
  - Capacity is still spent.
  - Neutrality.
- **Claim:** `amalgamation`.

### CB-04 — Riftmeld

- **Writable:** `factions/cabal.rs`, evidence; the research prerequisite seam in `technology.rs`
  through an existing hook if one fits (read how the Yin commander and the faction waivers
  bypass prerequisites first).
- **Do:** when researching a unit upgrade technology, the Cabal player may return 1 captured unit
  of that unit's type to ignore all of the technology's prerequisites.
- **Tests:** 3–5.
  - Real research of Cruiser II without prerequisites by returning a captured cruiser.
  - Not offered for non-unit-upgrade techs.
  - Not offered without a matching captured unit.
  - Neutrality.
- **Claim:** `riftmeld`.

### CB-05 — Dimensional Tear I/II

- **Writable:** `factions/cabal.rs`, evidence, `movement.rs` / `fleet.rs` through hooks
  (`hooks_movement.rs`); add a minimal hook if none fits.
- **Do:** a system with the Cabal's `cabal_spacedock` or `cabal_spacedock2` is a gravity rift.
  - The Cabal's own ships do not roll for it; other players' ships do.
  - Up to 6 fighters (12 with II) in that system do not count against the Cabal's capacity.
  - PRODUCTION values come from the unit data; verify them.
- **Tests:** 5–7.
  - Another player moving through the tear system rolls for the rift.
  - The Cabal player moving through does not.
  - The capacity exemption, 6 vs 12.
  - Research `dt2` upgrades the dock.
  - Neutrality.
- **Claim:** `cabal_spacedock`, `cabal_spacedock2`, `dt2`.
- **Trap:** the dimensional tear is the unit itself, so destroying the dock removes the rift.

### CB-06 — Vortex

- **Writable:** `factions/cabal.rs`, evidence.
- **Do:** an ACTION component action, through the module's `component_actions` /
  `perform_component` hooks.
  - Exhaust Vortex to choose another player's non-structure unit type that is in a system adjacent
    to 1 or more Cabal space docks.
  - Capture 1 unit of that type from that player's **reinforcements**
    (`capture_from_reinforcements`).
  - Offer only types that have a unit left in reinforcements.
  - Offered only with a map (adjacency needs the galaxy).
- **Tests:** 4–6.
  - A real component action captures a reinforcement unit.
  - An exhausted Vortex is not offered.
  - No adjacent enemy unit, no offer.
  - Empty reinforcements, no offer.
  - Neutrality.
- **Claim:** `vtx`.

### CB-07 — The Stillness of Stars (agent)

- **Writable:** `factions/cabal.rs`, evidence; the commodity replenish site only if no typed event
  exists. Search `commodities = ` / `replenish` in `strategy_cards.rs`, then use a typed or
  staged event, following how `supply::note_trade_goods_gained` stages events.
- **Do:** after another player replenishes commodities, the Cabal player may exhaust the agent to:
  1. convert that player's commodities to trade goods;
  2. capture 1 unit from that player's reinforcements whose cost is at most that player's
     commodity value, with the Cabal player choosing the type.
  
  Both happen; the capture is skipped only if no unit qualifies.
- **Tests:** 4–6.
  - The Trade secondary replenish triggers the offer.
  - Accepting converts and captures.
  - Declining changes nothing.
  - Own replenish does not trigger it.
  - Neutrality.
- **Claim:** `cabalagent`.

### CB-08 — commander unlock

- **Writable:** `factions/cabal.rs`, evidence.
- **Do:** add `commander_unlocked` in the module hooks. It returns `Some(true)` when the player has
  units in 3 systems that are gravity rifts (Dimensional Tears count; reuse the rift predicate from
  CB-05).
- **Tests:** 3.
  - Locked with 2 rifts.
  - Unlocked with 3.
  - The existing production exemption (`production.rs` tests named `cabal_*`) still passes.
- **Claim:** `cabalcommander`.

### CB-09 — It Feeds on Carrion (hero)

- **Writable:** `factions/cabal.rs`, evidence.
- **Do:** an ACTION delivered through the module's `leader_action` / `use_leader` hooks. Copy the
  hero pattern in `factions/winnu.rs` or `factions/naaz.rs`.
  - Each other player rolls 1 die (game RNG) per non-fighter ship in or adjacent to a system
    containing a dimensional tear. On a 1–3, capture that ship.
  - If capturing removes ground forces or fighters that ship was carrying (capacity), capture
    those too.
  - Then purge the hero.
- **Tests:** 4–6.
  - With a seeded RNG, a known roll captures and another does not.
  - Carried fighters and infantry are captured along with their transport.
  - Fighters themselves are not rolled for.
  - The hero is purged.
  - Neutrality.
- **Claim:** `cabalhero`.

### CB-10 — Crucible (promissory)

- **Writable:** `factions/cabal.rs`, evidence; `promissory.rs` / `movement.rs` through existing
  seams if needed.
- **Do:** the holder (not the Cabal player) may play it after activating a system.
  - During that movement the holder's ships do not roll for gravity rifts.
  - They get an additional +1 move when moving out of or through a rift.
  - Then the card returns to the Cabal player.
  - Follow how other "after you activate a system" notes are offered and returned in
    `promissory.rs`.
- **Tests:** 3–5.
  - The holder's ship passes through a rift with no roll and with +1.
  - The card is returned.
  - The Cabal player cannot play its own note.
  - Neutrality.
- **Claim:** `crucible`.

### CB-11 — Al'Raith Ix Ianovar (breakthrough)

- **Writable:** `factions/cabal.rs`, evidence; `fracture.rs` through its public functions.
- **Do:** on `BREAKTHROUGH_GAINED` (the game announces it) for the Cabal's `cabalbt`:
  1. the Fracture enters play without a roll if it is not already in play (find the
     enter-play function in `fracture.rs`);
  2. move up to 2 ingress tokens into systems containing gravity rifts, with the player choosing;
  3. +1 MOVE to each Cabal ship that starts its movement in a Fracture system, through a movement
     hook.
- **Tests:** 4–6.
  - Gaining the breakthrough puts the Fracture into play and moves tokens.
  - Already in play: no second entry.
  - The +1 move applies to a ship starting in the Fracture.
  - Neutrality.
- **Claim:** `cabalbt`.

### CB-12 — close-out

- **Writable:** evidence; `crates/ti4-sim/examples/base_faction_soak.rs` (add `"cabal"` to
  `FACTIONS`, keeping the array sorted).
- **Do:**
  - Run the full engine suite and the ledger. The expected line is `cabal N/N implemented`.
  - Run the routine soak:
    `cargo run --release -p ti4-sim --example base_faction_soak -q -j1 -- cabal 0 25 10`.
    It must report 25/25 games and 0 failures.
  - Write a summary section: every asset, the test that proves it, and the open questions.
- **Done when:** every row is claimed or explicitly blocked with a reason, all tests pass, and the
  soak is clean.

## 4. Coordinator checklist (not for Pi)

After each package, do these checks before committing:
- Diff review against the card text.
- A neutrality check.
- The decision registry is green.
- Only the package's paths are staged; filter any `game.rs` income hunks.

After CB-12, also:
- One Sonnet review of the whole Cabal diff.
- Fix the findings.
- Merge to main on the operator's OK.

Record in this file's evidence:
- per-package Pi wall time;
- tool errors;
- retries;
- watchdog aborts.

That record is the point of the backend test.
