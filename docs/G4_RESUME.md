# D2 lane G4 (`core-g4-abilities`): the last loud gap rows and hook slots -- resume note (2026-10-31)

Branch `core-g4-abilities`, cut from `core-g3-abilities` (`e5f33fa7`) with `core-g2-abilities` (`959089b0`) merged. Never pushed; the live branch is untouched. Read `docs/G_SILENT_RESUME.md`,
`docs/G2_RESUME.md`, `docs/G3_RESUME.md` first (the rows, the audit, the recipe). Slots `44_000..` are this lane's rules (`ti4-engine-core-factions/src/silent/g4/`, one file per ability);
the questions are `choice::silent4::{Ask, Opt, Subtype}` (variants `U(..)` of `silent::{Ask, Opt, Subtype}`, renderer `render/silent4.rs`); `SilentCodes::g4` holds the interned texts.

## The merge of G2 into G3

* `silent/gaps.rs`, `ability_gaps.rs`, `ability_audit.rs`: took G3's side and deleted G2's 19 ids with `tools/engine_core/delete_gap_rows.py` (the script now skips the `SUFFI_AN*` constants when G3's move into `silent/g3` left none behind). The rest (`silent/mod.rs`,
  `choice/silent.rs`, `render/silent.rs`, `combat_live.rs`, `reactions_core.rs`) kept both sides.
* **G2 and G3 both used slots `42_000..`** (`42_001..42_007` clashed: Winnu's against Ragh/Tekklar). G2's slots moved to `43_000..` (`silent/{winnu,muaat,ghost,empyrean,cabal,sol,naaz}.rs`); the audit test `ability_audit_g3_tests` filters `42_000..42_100`.
* Registry order (the string's module order decides it): `g4::register_first` (Arborec: Stymie) right after `g3::register_first`; Saar's Zeta (`g3::register_saar`) and the rest of G3 before G2's Winnu rules; `g4::register_abilities` before
  Winnu; `g4::register_last` (Yssaril: Spy Net) after the Winnu/Sol/Empyrean tail; Magen Defense's hit next to its activation half (`reactions_core::table`).
* `AskCarriesTrigger` of the seven G2 questions: false (the G2 note), added to `silent::Ask::carries_trigger`.

## Native now (rule + typed question + byte-exact renderer + lockstep against the string + core tests + gap row deleted)

| row / hook | file | string | evidence |
|---|---|---|---|
| Propagation (Nekro) | `g4/propagation.rs` | `nekro::propagation` | the turn prelude announces `PROPAGATION_RESEARCH` (it was a `TurnGap::Propagation`, variant removed); lockstep `silent4_live_tests::a_replaced_research_gives_three_command_tokens_*` (1, 2 pending researches, the pools) |
| asn_clear | -- | `keleres::asn_clear` | **Dropped** in the audit: the Agency Supply Network is a Keleres technology (a dropped faction) |
| Titans commander Tungstantus | `g4/titans_commander.rs` | `borrowed_commanders` | lockstep `combat_live::the_titans_commander_gains_a_trade_good_*` (carried by a Hacan seat: the Titans seat itself stops at Awaken, lane M6d) |
| Spy Net (Yssaril note) | `g4/spynet.rs` | `action_cards::spy_net`, `look_at_hand_and_take` | lockstep `silent4_live_tests` (several / one / twin cards, decline, the hand limit): the take is staged as a `CardsStaged` row for the host's next step (the string announces it inside its step; the test counts the rows against the announced events) |
| Stymie (Arborec note) | `g4/stymie.rs` | `arborec::stymie` | lockstep `combat_live::stymie_*` (system question, decline, no note); the universe is the map's systems plus the board's entries, less home systems and systems holding the mover's token; staged `COMMAND_TOKEN_PLACED` (pool `reinforcements`) |
| Magen Defense Grid's hit | `g4/magen_hit.rs` | `magen_defense::combat_start`, `invasion::assign_selected_ground_hit_in_timing` | lockstep `invasion_live::magen_defense_grid_*` (owner's assignment, the victim's sustain offer with its card, destroyed / sustained events with the cause `md`) |
| Yin Spinner Omega, Naalu Iconoclast (TE mech) | `g4/yin_spinner.rs`, `g4/iconoclast.rs`, `g4/place.rs` (the core twin of `place_units_choosing`) | `yin::yin_spinner`, `naalu::mech` | lockstep `combat_live::yin_spinner_*`, `silent4_live_tests::another_players_relic_*` (string under the Thunder's Edge sources, as the native kernel); the placement question's offer card is completed by `render::silent4::complete` (`live/details.rs`) |
| Foresight (Naalu) | `g4/foresight.rs` | `naalu::foresight`, `transit::relocate_ships` | lockstep `combat_live::foresight_*` (destinations, fighters aboard go along, the nested `COMMAND_TOKEN_PLACED`, decline, no token, a neighbour with the token); the staged `STRATEGY_TOKEN_SPENT` is left out of the combat harness' comparison like Farran's staged gain |
| `may_view_hand` (Naalu M'aban) | `cd_registry/naalu/query.rs` (hook provider) | `naalu::may_view_promissory_hand` | parity test `silent4_live_tests::the_core_hook_may_view_hand_*`. **Decision: a rule of the hook table (a standing permission, read from the position), not dropped; its consumer is the seat's view** (`SeatObservation::revealed_cards` in the string; the typed `Seen` has no revealed-card accessors at all, So Ata's and Spy Net's reveals included): D3's view layer, not an engine rule |
| `secondary_waived` (Mahact Jae Mir Kan) | `cc_ground/mahact/offer.rs` | `mahact::secondary_waived` | parity test `the_core_hook_secondary_waived_*` (ready / exhausted agent, no token, the Nomad watching: `exhaust_agent` stages `AGENT_EXHAUSTED`). The dual's `SecondaryStep::pay_waiver` still uses `exhaust_leader` and the adapter's `waiver_effect` gate for a watching Nomad stays (it runs the plain kernel) |
| Riftmeld providers (Cabal) | `cb_economy/cabal/offer.rs` | `cabal::{riftmeld_offer, riftmeld_paid}` | parity test `the_core_hooks_research_waiver_*`. **The research window does not ask the waiver and the payment yet** (see below) |

All the in-scope loud rows of lanes G/G2/G3/G4 are gone; the rows left in `silent::gaps::GAPS` (13) belong to other lanes:

* H2 (Strike Wing): `leader:*:argentcommander`, `promissory:*:ambuscade`, `technology:*:swa2`.
* M6d (tactical): `muaathero`, `muaathero_moved` (Nova Seed), `yssarilcommander_clear`, `awaken`, `titans_flagship`, `arboreccommander`, `arborec_flagship`, `yssarilcommander`, the Ssruu copy of I48S (`yssarilagent: l1z1xagent`), `nomadagentmercer`.

## The six G3 bail-as-failure acceptance tests after the merge of G2

Before: `combat_live::{a_space_fight_of_each_faction..., a_won_invasion_with_a_space_dock..., an_undefended_system_with_a_space_dock...}`, `production_step_tests::the_production_step_of_every_faction_runs_native_...`, `invasion_live::the_custodians_offer_runs_natively_declined_and_paid`,
`tactical_live::a_tactical_action_of_each_faction_runs_native_...` (G3 resume: 6 failures). After the merge: **closed**: Winnu agent / `reclamation_mark` / HTP / the mech (G2), Empyrean agent and commander (G2), the Mercer-free Custodians test, production-step test.
**Still open (3 tests)**: `combat_live::{a_space_fight_of_each_faction_runs_native_without_a_bail_and_as_the_string_does, an_undefended_system_with_a_space_dock_goes_on_to_the_production_step_in_lockstep}`, `tactical_live::a_tactical_action_of_each_faction_runs_native_without_a_bail_and_as_the_string_does`,
each stopping at `AbilityGap` of a lane M6d row only: the Arborec flagship (`41018` before G4's deletions), Titans flagship / Awaken, the Nomad's Mercer. They pass when M6d's rows merge.

## Checks (cheap only)

Private `CARGO_TARGET_DIR=/root/deploytmp/core-g4`, `CARGO_BUILD_JOBS=2`, no debug info, incremental removed after each build (deleted at the end).

* `cargo test -p ti4-engine --lib` (whole): 5069 passed, 3 failed, 4 ignored (337 s); the same with `TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1`: 5069 passed, the same 3 failed (636 s). The 3 are the M6d-owned acceptance tests above.
* `ti4-engine-core --lib` 474, `ti4-engine-core-factions --lib` 146, `ti4-engine-core-bridge --lib` 95 and `--test kernel_env` 3; `ti4-engine-core-tests --test g2_abilities` (25) `g3_abilities` (40) `silent_abilities` (10) `g4_abilities` (28) `turn_jobs` (18): all green.
* `tools/engine_core/check_layers.py core`: 527 files, 0 violations; `lint_core.sh`: 529 files, 0 hits.
* Mutation (`tools/engine_core/mutants.sh`, `D4_COVERS_FILE=/dev/null`, a scratch worktree at the committed snapshot, the target dirs deleted after): `silent/g4/` 209 points (207 run, 2 not compilable under the switch): the core tests (`g4_abilities`) alone kill 189; the
  engine lockstep (`silent4_live_tests`, the `combat_live` / `invasion_live` G4 tests, the registry and audit tests, `render::silent4`) alone kill 166; **the union leaves no survivor**. Found and fixed on the way: guards repeated by an effect (Propagation, the Magen hit, Iconoclast, the
  placement), the Foresight adjacency (links, blocked borders, extra wormholes, either end of a link; a token count of exactly one; an event without the count of ships), the hit event's damaged / galvanized flags and the Spec Ops II count, Stymie's staged event (module seated or not),
  the context flags of Spy Net's question. Named equivalent with `// mutants:skip (why)`: the `<=` of a border's normalisation, the no-galaxy answer, the cards-in-play flag of `can_enter`, the unused default of a missing cost / box / list, infantry having no plastic limit (the Spinner's box is never empty).
  `render/silent4.rs`: 10 points, 9 killed, 1 not compilable.
* Not run: golden / wide dual sweeps, benches, soak, the mutation ratchet, census, workspace clippy, the full D4 crate, the shadow, the other 140 `ti4-engine-core-tests` binaries.

## Findings

* The staged events of an effect (`STRATEGY_TOKEN_SPENT`, `COMMAND_TOKEN_PLACED`) are announced by the native arms with the next emission, the string's step announces them at the start of the next step: the combat harness' comparison leaves `STRATEGY_TOKEN_SPENT` out like Farran's
  staged gain (and drops the string's staged mark of it). A card moved by an ability (`CardsStaged` rows: Spy Net, So Ata, Kyver, Mageon) is announced by NOBODY natively: the host's next step (`announce_staged_card_events`) turns the rows into `ACTION_CARD_TAKEN` / `ACTION_CARD_DISCARDED`
  and moves the discard to the pile. If the host is to be the core alone, a native consumer of the rows is needed (the pile is not written by `fx::discard_action_card_staged` either).
* G2 and G3 numbered their slots from `42_000` alike; the G2 slots are `43_000..` now.
* `Unit::as_str` and the string unit ids agree for the factionless mech (`mech`) -- not for every faction unit: the question texts use `as_str` of the typed unit, as the G3 renderer does.

## Not done: the research window's waiver and payment questions (Riftmeld, and Yin's Brother Omar with it)

The hook providers exist (Cabal: `cb_economy/cabal/offer.rs`; Yin: `cd_registry/yin`), so `Research::open` lists the technology only a waiver opens, and a seat that picks it ends the window `failed` (`Report::Left`: a COUNTED bail of the live
path, `Unknown` -- never silent; the dual run declines it: `strategy_cards_core::research` "a research only a faction waiver opens", and `research_unported` / `strategy_cards::research_asks_more` keep the string's). What the window has to do, from
`strategy_cards::{resolve_research, offer_research_inner, paid_research}`:

1. `choice::strategy`: `Ask::{ResearchWaiver{tech}, ResearchWaiverPayment{tech, waiver}}`, `Opt::{ResearchWaiver(u8), ResearchPayment(Faction, u16)}`, the two subtypes `research_waiver`, `research_waiver_payment`; the renderer words (prompts
   `research {tech}: choose a prerequisite waiver` / `{waiver label}: choose the payment`, options `waiver|{index}` kind `research_waiver`, the payment ids `captured|{owner}|{unit}` (Cabal) and Yin's, a decline each, the two offer cards of `resolve_research`).
   The labels are module text (`riftmeld_offer`, `yin::research_waiver_offer_text`), so the renderer needs a table per module (the dual uses `Render::shape` for the secondary waivers, which the live path cannot).
2. `Research`: stages `Waive` and `WaivePay` after a technology whose prerequisites the colours/Inheritance do not cover and `research_waiver_offer` is `Some`; `research_waiver_paid` pays, then the research completes; a decline of either question puts
   the technology on a `declined` list the offer skips (`offer_research_inner`'s `declined_waivers`) and -- in the PAID research -- restores the position from before the payment (the string pays the bill first, then asks the waiver, then `*state = before`), so the
   window keeps a snapshot of the state taken when the technology was picked.
3. `first research_waiver_offer` keeps only the first module's waiver; the string offers one per module (a Cabal seat holding Yin's commander through an Alliance would see two): a lockstep for that corner needs the slot to `concat`.
4. A payment list longer than the `SmallList<Waiver, 4>` of `ResearchWaiver`: the Cabal provider lists a captured (owner, form) once (the string lists a model held twice twice, with the same id).
5. Lockstep: `strategy_research_tests::play` (Dual backend) with a Cabal seat, a captured cruiser and an unmet unit-upgrade technology: `counts == (1, 0)` once the window carries it.
