# Silent gaps, recomputed against core-integration-18 (2026-11-03)

Branch `core-silent-gaps-18`, cut from `core-integration-18` @ `1c11e5662`. Read-only analysis (no engine code touched). Result table: `tools/engine_core/bail_map_silent_18.tsv`
(the 239 silent rows of `bail_map.tsv` on `core-bailmap`, columns of `bail_map_silent.tsv` plus `status`, `evidence`, `severity`, `suggested_lane`; the old severity is `severity_old`;
3 new rows `N-01..N-03` at the end). A silent gap = the core plays without any bail/panic/lend marker but does not implement the rule.

## 1. Counts

| status | rows | of which old severity high / medium / low |
|---|---:|---|
| CLOSED | 196 | 13 high / 97 medium / 86 low (old severity; the other 2 high rows are the lane D wrapper rows, open) |
| STILL OPEN | 9 (+3 new rows N-01..N-03) | see section 3 |
| DROPPED | 0 (the 239 hold no dropped-faction row; the 41 dropped rows of the old file are in the 208 that were never "gaps") | |
| THUNDER'S EDGE (exempt) | 34 (old map: 31, +3 reclassified: B-067 Mindsieve, D-tether_border, E-15 Vaults of the Heir) | |
| total | 239 (+3 new) | |

By part: a (abilities) 77 closed; b (hook slots) 21 closed, 1 open, 1 TE; c (events) 6 closed; d (subtypes) 33 closed, 1 open, 17 TE; e (menu sources) 6 closed, 4 TE;
f (catalog ids) 45 closed, 2 open, 12 TE; g (arms/callers) 8 closed, 5 open.

Of the 9 old rows still open, only 2 are rules the core lacks (Sh'val's skip, Nano-Forge); 1 is a narrow timing difference; 3 are the product wrapper (planned lane D); 3 are latent or housekeeping.
The 3 new rows are one real bug (X-89 printing) and two guard/cleanup items.

The `te` strategy card set is NOT exempt and was checked: `ConstructionThundersEdge` and `WarfareThundersEdge` are ported (`kernel/fx_perform.rs:157-168`, `driver/windows/strategic/facts.rs:215-216`; tests
`te_uses_e2.rs`, `strategic_arms.rs`, `hero_windows.rs`, `action_dispatch.rs:1741`, `tactical_composer.rs:419,674`) and `native_play` plays them (INTEGRATION18 7.1). No gap found.

## 2. The known suspects

| suspect | verdict | evidence |
|---|---|---|
| Infantry II return | CLOSED | `silent/returns.rs` INF2_RETURN (+ Letani Warrior II, Crimson Legionnaire II), tests `silent_abilities.rs::infantry_ii_units_on_the_card_land_on_the_first_planet_of_the_home_system_at_the_start_of_the_turn`, `::units_with_no_planet_to_land_on_stay_on_the_card` |
| hero unlock timing | CLOSED except one narrow case (B-X01) | unlock sites: turn start `driver/turn/start.rs:85`, pass `turn/action.rs:107`, scoring `windows/scoring.rs:226`, strategy card `strategic/perform.rs:113`, status `status_bookkeeping.rs:209`; the case: a not-yet-revealed secret scored for the first time skips the check (see section 3) |
| commander unlock | CLOSED | providers for all 17 in-scope commanders; `bridge/tests/kernel_env.rs::no_hook_slot_is_empty_but_for_the_dropped_factions` and `::the_hook_slots_agree_with_the_string_on_positions_that_reach_every_condition` (both outcomes required) |
| 75 subtypes without a renderer | 30 left, 29 are TE/dropped/non-game, 1 real | `tools/engine_core/silent_gaps_d.py` (re-pointed at `ti4-engine-next/src/render`): 427 string subtypes, 30 without a core renderer. Real: `sardakk_hero_skip`. Others: 18 TE (expedition x5, ingress move, fracture ingress, slumberstate x2, avernus, specialist compounds, thunders paradox, deepgloom x2, coexistence research team x2, entropic scar, void tether), 6 dropped (deepwrought x4, crimson payment, keleres asn), 2 structured diplomacy, `produce_units` (false positive), `mid_action_pause` (a bot-side pause text), `courageous_to_the_end_assign_casualty` (interned by code, rendered). |
| empty faction component slots | CLOSED (slots are dead, the work is elsewhere) | `HookTable.general.component_actions / mapped_component_actions / perform_component` have no provider on purpose: the faction actions are listed by `driver/menu_faction.rs` and performed by `driver/turn/faction.rs`; bridge `menu_sources.rs::the_faction_component_actions_listed_equal_the_strings` asserts zero gaps. Vaults of the Heir is TE. |

Aids used: `ability_audit.rs` registry diff (`cargo test -p ti4-engine-legacy --lib ability_audit` on this tip: 4 passed; `ACCOUNTED` has only `Dropped` rows, no `Gap`/`Lane`/`Carried`, so every string ability of the 24 lobby
factions plus a Yssaril third seat has a core rule); `INTEGRATION18.md` (native_play 96/96 and 288/288 whole games, zero gaps; shadow 78/96 clean); `R_RESUME.md` (class list of the remaining shadow differences: none is a silent gap).
`silent_gaps_def.py` (catalog ids with no `::Variant` mention) was re-run and every non-dropped hit checked by hand (unit upgrades and explore cards are data-driven false positives).

## 3. Still open, grouped into proposed lanes

| lane | items | what is missing | severity | size |
|---|---|---|---|---|
| L-small-fixes | **N-01 / F-technology-x89c4** X-89 Omega-Omega printing | the core reads `Technology::X89BacterialWeapon` (base printing, not in the deck) in `driver/windows/invasion/ops.rs:632`, `invasion/fight.rs:237`, `driver/turn/hero.rs:484`; the deck holds `X89BacterialWeaponOmegaOmega` (id `x89c4`, which the string reads, `invasion.rs:1725`). A native game never doubles bombardment/ground hits nor exhausts bombarded planets for the holder. The dual hides it (it passes the flag from the string state), the D4 tests inject the flags. | medium | XS (3 reads + a native-driver test) |
| L-small-fixes | **B-X01** first-scored secret objective | `windows/scoring.rs:223-227` runs the unlock check only for a public or already revealed objective; the string `award_tail` always does. A 3rd-objective hero (or a commander) unlocks at the next turn start instead of at once. | low | XS |
| L-sardakk-hero | **D-sardakk_hero_skip** Sh'val Harbinger | nothing asks "skip to Commit Ground Forces": `skip_to_commit` is read by `driver/windows/tactical/mod.rs:225` but has no provider (`cd_registry/sardakk/offer.rs` is empty); the finish ability and `MarkKey::SardakkHeroSkip` exist, nothing writes the mark; no render literal. | medium (only Sardakk with the hero unlocked, once per game) | S (provider + render literal + test; cleanup of an abandoned skip is `sardakkhero_turn`, already a rule) |
| L-relic-actions | **F-relic-nanoforge** | Nano-Forge's ACTION (attach to a planet) exists in neither engine; the core has only the attachment's stats. Needs a ruling: native may be ahead of the string. | low | S |
| L-D wrapper (planned: D2-8 lane D, F) | **G-live-step-no-caller**, **G-live-reconcile-one-caller**, **G-viewless-asks** (unverified) | `Game::step` does not route to the core arms; `live_reconcile` is called only by the bridge; native questions are not yet delivered with the asking seat's view (no caller of `ask_seeing` found in the thin crate). Not a rule gap: the arms play whole games in `native_play`. | high until the swap | L (already planned) |
| L-cleanup / guards | **G-relic-breakthrough-event-payload** (latent), **G-stale-bail-texts**, **N-02**, **N-03** | dead hook slots (about 90 slots with no provider; empty slot = indistinguishable from "not ported", which is how Sh'val hid), stale headers (`magen_defense.rs`, `invasion_live.rs:480`, `ability_audit.rs` allow reasons), stale bail texts; the two census tests the old map proposed were never written (`every_decision_subtype_the_string_asks_is_rendered_or_accounted_for`, a catalog census) and both would have found N-01/Sh'val. | low | S |

Suggested order: L-small-fixes (an hour or two, then L-sardakk-hero), L-cleanup/guards together with them (the census tests turn the static scripts of this note into guards), then L-relic-actions on the user's decision.

## 4. New gaps found (not in the old map)

* **N-01 X-89 printing** (above). Found by the `silent_gaps_def.py` hit `technology x89c4 ... no ::Variant mention`, then following the string's `x89_multiplier` to the core's three reads.
* **N-02 dead hook slots / stale comments** and **N-03 missing census tests** (above).
* Rows that were wrongly "agreed": 3 are Thunder's Edge (B-067 Mindsieve, D-tether_border, E-15 Vaults); `F-leader-redcreuss*` and `D-produce_units` are false positives (noncanonical leaders; no such subtype).

## 5. Method and limits

* CLOSED means a rule/renderer/menu entry/event emitter exists in the core, wired into the live table, and is guarded by a named test where one exists (cited per row). It does NOT re-prove behavioural equality with the string: that is the job of the heavy gates
  (golden/wide dual, `core_sync_gate`, `board_equivalence`, mutation batches) that were not run (standing rule). The registry diff test and `kernel_env` are the only tests re-run here.
* Part (a) rows with wildcard ids (`*:*:*`, borrowed `yssarilagent:*`) are closed by the anonymised-id diff; the Ssruu-copy ids are covered by the third Yssaril seat in the test.
* Static scans (`silent_gaps_d.py`, `silent_gaps_def.py`, restored from `core-bailmap` and committed under `tools/engine_core/`; `silent_gaps_abc.py` was not restored: it cannot see the lane G modules and reports 72 false gaps) find candidates only; every hit was read.
* Not checked: whether the 208 non-gap rows of the old `bail_map_silent.tsv` (covered / ported / dropped / false positives) still hold; only spot checks (diplomacy_response differs between the legacy and the thin render: structured diplomacy, dropped).
