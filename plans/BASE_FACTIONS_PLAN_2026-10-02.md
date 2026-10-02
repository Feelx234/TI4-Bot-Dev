# Base-faction completion plan (BF) — 2026-10-02

## Scope (decided with operator 2026-10-02)

| Decision | Value |
|---|---|
| Factions | 12: ten base factions not yet in `IN_SCOPE_FACTIONS` — Arborec, Creuss (`ghost`), Mentak, Muaat, Naalu, Saar, Sardakk N'orr, Winnu, Yin, Yssaril — plus PoK Argent Flight (`argent`) and Naaz-Rokha Alliance (`naaz`) |
| Assets | Full PoK sheet **and** full Thunder's Edge additions (see per-faction checklist) |
| Card text | Latest errata + Codex (whatever `ti4_model::content_types::DEFAULT` resolves; e.g. `yso` is already Yin Spinner Ω, Naalu uses `naaluagent-te` / `naalu_mech_te`) |
| Deferred | Nekro (operator 2026-10-02): its tech copying needs every other faction's techs done first; revisit after this plan. |
| Out of scope | Keleres, other PoK factions, TE factions, Firmament. Existing 6 are not reworked except where a shared subsystem touches them. |

## Definition of done (per faction)

A faction is **in scope** only when every row is green, with a test per row:

| # | Asset | Source of truth |
|---|---|---|
| 1 | Home system, starting fleet, starting techs, commodities deploy via `seating` | `factions.json` |
| 2 | Every faction ability | `abilities.json` |
| 3 | Both faction technologies (incl. unit-upgrade techs) | `technologies.json` |
| 4 | Faction units: flagship, mech, any replaced base unit and its upgrade | `units.json` |
| 5 | Promissory note (own use **and** use by the receiving player) | `promissory_notes.json` |
| 6 | Agent, commander (unlock condition + effect), hero | `leaders.json` |
| 7 | TE breakthrough (`<faction>bt`) incl. synergy unlock | `breakthroughs.json` |
| 8 | TE reprints that replace the PoK card (Naalu agent/mech, Mentak `mentak_cruiser3`) | `units.json`, `leaders.json` |
| 9 | Registry ledgers show 0 unimplemented / 0 `blocked()` for the faction | `registry.rs`, `faction_abilities::unimplemented`, `breakthroughs::registered_aliases`, mech/leader ledgers |
| 10 | Legal-action generation offers every new option; failed transitions atomic | AGENTS.md accuracy rules |
| 11 | 200-game deterministic soak with the faction seated, zero engine errors, replay-identical | `ti4-sim` |

## Asset inventory (from content, 2026-10-02)

| Faction | Abilities | Faction techs | Units | PN | Breakthrough | Hard subsystems |
|---|---|---|---|---|---|---|
| Arborec | mitosis | bio, lw2 | flagship, infantry(II), mech | stymie | Psychospore | placement-outside-production, status-phase transfers, infantry production |
| Creuss | quantum_entanglement, slipstream, creuss_gate | ds, wg | flagship, mech | iff | Particle Synthesis | off-map home + Creuss Gate tile, delta wormhole, wormhole tokens, flagship moving tokens |
| Mentak | ambush, pillage | mc, so | flagship, mech, cruiser3 (TE) | pop | The Table's Grace | pre-combat window, transaction-trigger reactions, **capture** (hero) |
| Muaat | star_forge, gashlai_physiology | pws2, mr | flagship, war sun (start), mech | fires | Stellar Genesis | supernova movement/production, **tile mutation** (hero) |
| Naalu | telepathic, foresight | ng, hcf2 | fighter(II), flagship, mech_te | gift | Mindsieve | initiative-0 token, out-of-turn move on enemy activation, fighters as ground forces |
| Saar | scavenge, nomadic | ffac2, cm | flagship, mech, space dock (mobile) | ragh | Deorbit Barrage | space dock on ships, chaos mapping start-of-turn production |
| Sardakk | unrelenting (done) | exo2, vpw | dreadnought(II), flagship, mech | tekklar | N'orr Supremacy | sacrifice-for-hits, ground-combat hit windows |
| Winnu | blood_ties, reclamation | lgf, htp | flagship, mech | acq | Imperator | Mecatol specifics, resource/influence swap, strategy-card swap PN, hero primaries |
| Yin | indoctrination, devotion | yso, ic | flagship, mech | greyfire | Yin Ascendant | pre-ground-combat replace, destroy-own-ship hits, placement |
| Yssaril | stall_tactics, scheming, crafty | tp, mi | flagship, mech | spynet | Deepgloom Executable | AC free-action window, hand inspection (hidden info), agent copying ACs |
| Argent | zeal, raid_formation | ah, swa2 | destroyer(II), flagship, mech | ambuscade | Wing Transfer | vote bonus, AFB excess hits onto sustain, structure-based production, AFB vs infantry |
| Naaz-Rokha | distant_suns, fabrication | pfa, sc | flagship, mech (space/ground forms) | bmf | Absolute Synergy | extra explore draw + choose, fragment purges, mech switching between space and ground |

Each faction also has agent/commander/hero (`<faction>agent|commander|hero`).

## Phase 0 — shared subsystems (blockers first)

Built once, before faction packages that need them. Each is its own package with frontier
review (tiers C/D: timing, legality, hidden information).

| ID | Subsystem | Unblocks |
|---|---|---|
| BF-00a | Integration seams: replace every hard-coded 6-faction list with one source (`seating::IN_SCOPE_FACTIONS` + derived), plus a per-faction asset ledger test that enumerates rows 2–8 from content | all |
| BF-00b | "Place units" step outside production (supply-checked, atomic) | Arborec, Muaat, Yin, Saar, Naalu, Sardakk |
| BF-00c | Combat hit windows: after-roll / before-assignment, pre-combat (Ambush), sacrifice-to-hit | Mentak, Yin, Sardakk, Naalu; also unblocks `your_ships_have_no_shields` |
| BF-00d | Unit capture (captured-unit pool, return on PN/transaction) | Mentak hero/flagship |
| BF-00e | Wormhole topology: delta wormhole, wormhole tokens, Creuss Gate tile + off-map home in map templates, token movement | Creuss, Winnu `lgf`, Saar `cm` |
| BF-00f | Supernova/tile mutation: movement into supernova, hero converting a tile | Muaat |
| BF-00k | Dual-form unit (mech that is a ship in space, ground force on a planet) | Naaz |
| BF-00h | Action-card free-action window, hidden-hand inspection through typed views | Yssaril |
| BF-00i | Initiative override token (Telepathic / Gift of Prescience) and out-of-turn movement on enemy activation | Naalu |
| BF-00j | Mobile space dock (dock as ship cargo, production from space) | Saar |

`faction_abilities::blocked()` currently lists `telepathic` as "agenda deck not inspectable";
Telepathic is the Naalu 0-token. BF-00a corrects that ledger entry before work starts.

## Phase 1 — faction packages

One milestone row per faction, split into atomic children per
`PI_WORK_PACKAGE_STANDARD.md` (suffix `a` abilities, `b` techs/units, `c` PN, `d` leaders,
`e` breakthrough + TE reprints, `f` soak + ledger close). Order by subsystem readiness and
size, simplest first so the template settles early:

| Order | Faction | Depends |
|---:|---|---|
| 1 | Sardakk N'orr | 00a, 00c |
| 2 | Winnu | 00a |
| 3 | Arborec | 00b |
| 4 | Yin | 00b, 00c |
| 5 | Mentak | 00c, 00d |
| 6 | Saar | 00b, 00e, 00j |
| 7 | Yssaril | 00h |
| 8 | Naalu | 00b, 00c, 00i |
| 9 | Muaat | 00b, 00f |
| 10 | Creuss | 00e |
| 11 | Argent | 00c |
| 12 | Naaz-Rokha | 00k (exploration and relics already exist) |

Each package's evidence cites the card text from content and the rules section; Python is
historical context only (no parity claims).

## Phase 2 — integration and exit

| ID | Work |
|---|---|
| BF-20 | Widen `IN_SCOPE_FACTIONS` to 18 (operator approval — changes every rollout) |
| BF-21 | Seat-assignment for 18 factions: draft/random selection, determinism by seed |
| BF-22 | `ti4-policy` battle arena `FACTIONS` width → schema version bump; faction-decomposition features verified for new factions; vocabulary census |
| BF-23 | `ti4-sim` integrity re-baseline (operator approval; authored-bot play changes) |
| BF-24 | Replayer/review: every new option renders with a presentation, manual-path check per new decision kind |
| BF-25 | Full workspace suite, 18-faction soak (all faction pairs seated), registry ledgers at zero for the 12, frontier exit review, milestone report |

## Parallel execution (operator request: Sonnet agents, ≤ 6 at once)

Constraints that shape this: no worktrees or folders outside the repo, one shared checkout and
one `target/` (cargo's lock serialises builds), and today every faction's code lives in shared
modules (`faction_abilities.rs`, `leaders.rs`, `promissory.rs`, `breakthroughs.rs`, …) where six
agents would collide.

**Enabler (BF-00a, done serially before any wave):** move faction-specific code to one file per
faction, `crates/ti4-engine/src/factions/<alias>.rs` (**new folder — needs operator approval**),
each exposing a fixed `FactionModule` registration (abilities, techs, units, PN, leaders,
breakthrough hooks). Shared modules dispatch through the registry. The six existing factions
migrate first, behaviour-identical, proven by the `ti4-sim` integrity check. After that a faction
agent's edit scope is its own file plus its own test file; nothing else.

**Orchestration.** The coordinator is the Claude Code (Opus) session that owns this plan. It
spawns each implementer as a Sonnet subagent (`Agent`, `model: sonnet`, background, no worktree
isolation — forbidden by the global folder rule), at most 6 live at once. Each subagent gets a
self-contained prompt: its package spec, writable paths, the asset checklist rows for its
faction, and the rules below. When one finishes, the coordinator reads its report, runs the
package checks itself, spawns a separate Opus reviewer subagent, applies or rejects findings,
commits, updates `EXECUTION_STATE.md`, then starts the next queued package in the freed slot.
Pi/Qwen is no longer the default implementer (AGENTS.md, operator decision 2026-10-02).
Folder `crates/ti4-engine/src/factions/` and branch `wp/base-factions` approved the same day.

**Rules for parallel agents**

| Rule | Why |
|---|---|
| Writable: `factions/<alias>.rs`, `tests/faction_<alias>.rs`, `plans/evidence/BF-<alias>*.md` only | Disjoint scopes = safe in one tree |
| Needs a shared-module change → stop and report; coordinator does it | Shared seams stay single-writer |
| Crate must compile at every save; unfinished hooks stay unregistered | One broken file blocks all six agents' builds |
| Tests run filtered (`cargo test -p ti4-engine faction_<alias>`) | Shorter lock holds on the shared `target/` |
| Agents never commit, switch branch, or touch `out/` | Coordinator commits each package by explicit path |
| Each package gets a frontier (Opus) review before commit | AGENTS.md tiers C/D; Sonnet is implementer, not sole reviewer |

**Waves**

| Wave | Parallel | Work | Implementer |
|---|---:|---|---|
| A | 1 | BF-00a: module split + registry + ledger test; migrate existing 6 | Opus (architecture) |
| B | ≤ 3 | Subsystems with disjoint files: 00e wormholes (movement/galaxy), 00h AC window (action_cards), 00i initiative token (strategy); then 00b placement, 00c combat windows, 00d capture, 00f supernova, 00i out-of-turn move, 00j mobile dock, 00k dual-form mech | Sonnet, Opus review (timing/legality/hidden info) |
| C | 6 | Sardakk, Winnu, Arborec, Yin, Mentak, Yssaril | Sonnet ×6 |
| D | 6 | Saar, Naalu, Muaat, Creuss, Argent, Naaz-Rokha | Sonnet ×6 |
| E | 1 | Phase 2 integration (BF-20..25) | Opus |

Wave B's later subsystems overlap core files (`combat.rs`, `production.rs`, `movement.rs`), so
they run at most 3 at a time and only when their file sets are disjoint. A wave-C faction whose
subsystem is late simply starts with the rows that don't need it.

## Risks / decisions needed later

| Item | Why it matters |
|---|---|
| Widening seats invalidates trained checkpoints' faction distribution | Operator decides when BF-20 lands relative to training runs |
| Battle-arena schema width change | Breaks existing arena checkpoints; needs migration or new version |
| Creuss off-map home | Map templates and adjacency are touched by every game, not just Creuss games |
| Nekro deferred | Its Valefar/Singularity copying is gated on every other faction's techs |
