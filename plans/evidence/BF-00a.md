# BF-00a — per-faction module seam and asset ledger

Plan: `plans/BASE_FACTIONS_PLAN_2026-10-02.md`, wave A. Branch `wp/base-factions`.
Implementer: Claude Code coordinator (Opus 5.5). Python reference not used.

## What changed

| Path | Change |
|---|---|
| `crates/ti4-engine/src/factions/mod.rs` (new) | `FactionModule` (claimed ids per asset kind + `Hooks` fn-pointer table), fixed `MODULES` dispatch order, dispatch fns, registry unions, per-faction asset ledger (`assets`/`implemented`/`missing`), tests |
| `crates/ti4-engine/src/factions/<alias>.rs` ×12 (new) | Empty `MODULE` stubs: arborec, argent, ghost, mentak, muaat, naalu, naaz, saar, sardakk, winnu, yin, yssaril |
| `faction_abilities.rs` | Every existing hook also dispatches to modules; `registered()`/`registered_mech_abilities()` union module claims; `blocked()` drops entries a module implements; Telepathic's blocked reason corrected (it is the Naalu 0 token, not agenda-deck inspection) |
| `leaders.rs` | `commander_unlocked`, `use_leader`, action-leader offer/resolvability and `vote_bonus` fall through to modules; `registered_abilities()` unions module leaders and now lists `xxchahero-te` (already delivered by `use_leader`, missing from the ledger) |
| `leaders.rs` `for_faction` | **Bug fix.** Deals only leaders the faction sheet lists, reprints of listed ones, or stand-ins for a listed leader `sources` cannot reach. Before: Creuss got six leaders (unofficial `redcreuss*` records filed under `pok`), Naalu got two agents (`naaluagent` + `naaluagent-te`). |
| `breakthroughs.rs` | `registered_aliases()` unions module claims |
| `ti4-review` `FACTIONS`, `ti4-training` `FIXED_FACTIONS`, `ti4-policy` battle `FACTIONS` | Kept (ordered for their own artifacts) and each given a test that it names the same set as `seating::IN_SCOPE_FACTIONS`, so BF-20 cannot widen seating silently |
| `examples/coverage_report.rs` | Reads `IN_SCOPE_FACTIONS` |

## Decision recorded

The plan said the six existing factions would migrate into `factions/`. Not done: it is
behaviour-identical churn, nothing in waves B–E depends on it, and the hook seam gives parallel
agents disjoint files without it. Faction agents' writable scope is `factions/<alias>.rs` only;
unit tests live inline in that file (integration tests could not reach crate-private state).

## Checks (2026-10-02, tree also carries another session's uncommitted changes to `game.rs`, `ti4-training`, `ti4-mlp`, `ti4-review/tests`)

| Command | Result |
|---|---|
| `cargo test -p ti4-engine -q` | lib 1399 passed, 1 ignored; integration binaries all ok |
| `cargo test -p ti4-engine --lib factions::` | 4 passed, 1 ignored (ledger report) |
| `cargo test -p ti4-review --lib factions_matches` | 1 passed |
| `cargo test -p ti4-policy --lib battle_factions_matches` | 1 passed |
| `cargo test -p ti4-training --lib fixed_factions_matches` | 1 passed |
| `LIBTORCH=out/libtorch-2.9.1-cpu cargo test -p ti4-sim -q` | 51 passed, 1 ignored — existing-six play unchanged |
| `cargo clippy -p ti4-engine --all-targets` | no warnings in touched files |
| `cargo fmt --check` | clean for touched files (`game.rs` diff belongs to the other session) |

`leaders_follow_the_sheet_and_leave_the_six_unchanged` proves `for_faction` is identical to the old
rule for all six in-scope factions at DEFAULT and POK.

## Ledger at this commit (`cargo test -p ti4-engine --lib print_faction_ledger -- --ignored --nocapture`)

| Faction | Claimed / assets |
|---|---|
| arborec | 0/12 |
| argent | 0/13 |
| ghost | 0/12 |
| mentak | 0/12 |
| muaat | 0/13 |
| naalu | 0/13 |
| naaz | 0/13 |
| saar | 0/13 |
| sardakk | 1/12 (`unrelenting`, pre-existing) |
| winnu | 0/11 |
| yin | 0/11 |
| yssaril | 0/12 |
| *existing:* sol 7/15, hacan 8/12, letnev 7/11, xxcha 6/11, jolnar 8/12, l1z1x 7/13 | see findings |

## Findings for later (not fixed here)

| Finding | Owner |
|---|---|
| Existing six show unclaimed faction techs, flagships, some upgrades and every PN: those kinds never had a claim registry, so the ledger cannot tell implemented from missing. Audit before BF-25. | coordinator, BF-25 |
| `xxcha` `quash` still blocked (agenda replacement window) | wave B or out of plan |
| `ti4_content::units::faction_unit` for Naalu must resolve `naalu_mech_te` among three mech printings — verify in the Naalu package | Naalu package |

## Independent review (Opus subagent, tier C/D, 2026-10-02)

No blockers. Verified: empty-module splices are no-ops; `for_faction` old vs new over all 34 corpus
factions at POK and FULL changes only ghost (drops `redcreuss*`), naalu FULL (drops `naaluagent`)
and bastion FULL (drops pok-filed `orlandohero`) — all correct; six in-scope identical. Keleres gets
no leaders before and after (filed under `keleres`) — pre-existing, out of plan.

| Finding | Disposition |
|---|---|
| Hook table too thin for wave C (PN, faction tech, unit ability, breakthrough, production, movement, turn start, AC draw, status, start-of-combat, agenda windows) | Accepted: plan row BF-00l adds the hook batch each wave-C faction needs before agents spawn |
| No atomicity contract | Fixed: documented on `Hooks` |
| `assets()` doc vs `printed` filter | Fixed: ledger now lists every sheet unit; stats-only units claimed after a stats test (totals above updated) |
| Plan rules table stale (test path, test filter) | Fixed in plan |
| Hook order undocumented | Fixed: documented on `Hooks` |
| `type` compared case-sensitively in `for_faction` | Fixed |
| clippy `map_clone` in two new tests | Fixed (`to_vec`) |
| `vote_bonus` hook lacks `&ContentStore` | Deferred to BF-00l (caller `vote.rs` has no store in scope) |
| `leaders::unimplemented` uses POK (reports `naaluagent`) | Deferred: Naalu package / BF-25 |
| `coverage_report` `[&str; 6]` stops compiling at BF-20 | Intended: forces the report to be revisited |
