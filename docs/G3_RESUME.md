# D2 lane G3 (`core-g3-abilities`): the loud gap rows of lane G turned into native rules, part 2 -- resume note (2026-10-31)

Branch `core-g3-abilities`, cut from `core-g-silent` (`1ef52a35`). Never pushed; the live branch is untouched. Read `docs/G_SILENT_RESUME.md` (the 61 loud rows, the audit, the recipe) first.
Slots `42_000..` are this lane's rules (`ti4-engine-core-factions/src/silent/g3/`, one file per ability family); the questions are `choice::silent3::{Ask, Opt, Subtype}`
(variants `T(..)` of `silent::{Ask, Opt, Subtype}`, renderer `ti4-engine/src/render/silent3.rs`); `SilentCodes::g3` holds the interned texts the rules name.

## Native now (rule + renderer + lockstep against the string + core tests + gap row deleted)

| ability | file | string | evidence beyond the core tests |
|---|---|---|---|
| Scavenger Zeta (Saar mech, deploy after control) | `g3/zeta.rs` | `saar::scavenger_zeta` | lockstep: `invasion_live_g3_tests` (conquest by a Saar, with / without a mech in the box) |
| Ragh's Call | `g3/ragh.rs`, `Ask::Ragh` | `saar::ragh_call` | lockstep: `invasion_live_g3_tests` (the Saar defends; use, decline, 1-2 destinations) |
| Tekklar Legion (play, end, turn) | `g3/tekklar.rs` | `sardakk::{tekklar_legion, _cleanup_ended, _cleanup_turn}` | lockstep: `invasion_live_g3_tests` (N'orr defends / a third-seat Sardakk; use, decline) |
| T'ro's activation-mark clean-ups (turn, strategic action) and Sh'val's skip clean-up (turn) | `g3/tro_marks.rs` | `sardakk::{tactical_unmark, hero_unmark_turn}` | lockstep: `silent3_live_tests` (turn start); the strategic action has no native emitter here; the hero's skip: string tests under dual |
| The Alastor (Nekro flagship) | `g3/alastor.rs`, `Ask::Alastor` | `nekro_units::alastor` | lockstep: `combat_live_g3_tests` (four tables) |
| Exotrireme II | `g3/exo2.rs`, `Ask::{ExoSelf, ExoVictim}` | `sardakk::exotrireme` | lockstep: `combat_live_g3_tests` (40 seeds, 1-2 Exotrireme II, damaged, 4-6 enemy ships; the scan demands the windows and both questions were seen) |
| Z'eu (Naalu Thunder's Edge agent) | `g3/zeu.rs` | `naalu::agent` | lockstep: `combat_live_g3_tests` (the Naalu activates: this base's tactical driver offers windows to the active seat alone) |
| Acidos (Nekro commander) | `g3/nekro_commander.rs` | `borrowed_commanders::nekro_commander` | lockstep: `silent3_live_tests` (the turn prelude announces the technology) |
| Suffi An (Mentak agent) and Ssruu's copy | `g3/suffi_an.rs`, `Ask::SuffiAn`; Pillage ends in `after_pillage` | `mentak::{suffi_an, native_suffi_an_window, borrowed_suffi_an}` | the string's Mentak / Yssaril tests under `TI4_AREAS=all=dual`; core tests (a lockstep does not reach: the native combat announces Farran's staged trade goods later than the string, so no Pillage window lines up) |
| S'ula Mentarion (Mentak commander) | `g3/mentak_commander.rs`, `Ask::{SulaOpponent, SulaGive}` | `mentak::commander` | lockstep: `combat_live_g3_tests` (8 seeds, the hand varies) |
| Bioplasmosis | `g3/bio.rs`, `Ask::Bio` | `arborec::bioplasmosis` | the string's Arborec tests under dual (no native emitter of `STATUS_PHASE_ENDED` in this base) |
| The Oracle (Naalu hero) | `g3/oracle.rs`, `Ask::OracleGive` | `naalu::hero` | the string's Naalu tests under dual (same) |
| Scepter of Dominion | `g3/scepter.rs`, `Ask::Scepter` | `mahact::scepter` | lockstep: `silent3_live_tests` (the status phase into the next round; 0, 1, 2 systems) |
| Null Reference (`nekroc4y`) | -- | `nekro_units::null_reference` | **Dropped** in the audit: no faction claims the technology (operator ruling 2026-10-07 in the string module), no seat can gain it |

19 rows of `silent::gaps::GAPS` are gone (61 -> 42). `SUFFI_AN` / `SUFFI_AN_COPY` are gone from `gaps.rs`; Pillage's take ends in `g3::suffi_an::after_pillage`.

`g3::draw::draw` is the core's own `DrawCards` window stepped inside an effect (Scheming's bonus card and discard, the hand limit, all asked on the tape), a loud `AbilityGap` of the rule's slot where the adapter gave no printed-name groups.

Findings fixed on the way (each guarded by a test that fails before the fix):

* `rules/combat/flow.rs::round_ended`: the fight was decided "over" BEFORE the window of `SPACE_COMBAT_ROUND_ENDED`; an ability of that window (Exotrireme II) that destroys the last ships of a side ended the fight one opening later
  (presentation phase `pre_roll` instead of the string's `complete`). The flow now reads the end of the fight again after the announcement. Guard: `combat_live::exotrireme_ii_destroys_itself_and_up_to_two_ships_after_a_round_like_the_string` (the unit tests of the flow keep their `over` input: it is read again only when the window changed the board).
* `silent::gaps` Foresight row fired for every Naalu seat at every `MOVEMENT_FINISHED` (even its own move, even without a strategy token): now only where Foresight could. Guard: `zeu_removes_the_activation_token_like_the_string` (a Naalu active seat bailed before).
* `silent_abilities::scavenge_pays_...` (lane G's test) now declines the Zeta window that follows Scavenge's good.

## How it is wired

* Slots `42_000..42_017` (`g3/mod.rs` registers them; `NAMED` in `reactions_core.rs` names them; `PAIRS` has `SUFFI_AN_COPY`).
* Registry order: `reactions_core_g3_tests.rs` (`the_registry_of_the_g3_rules_equals_the_string_one`, an `include!` of `reactions_core::tests`) compares the native registry with the string's for each of the 24 lobby factions as first and as second seat, and the Mentak / Yssaril pair. Three insertion points: `g3::register_first` (before `silent`'s own: Bioplasmosis, the Arborec module comes first), `g3::register_after_ca_combat` (called from `reactions_core::table()` after `ca_combat::register_abilities`: Tekklar's play, the Alastor, Exotrireme II: the Sardakk and Nekro modules sit before the Winnu's), `g3::register_abilities` (the end of `silent`'s). A new rule goes where the test accepts it.
* New question types: `choice::silent3::{Ask, Opt, Subtype}` (variants `T(..)` of `silent::{Ask, Opt, Subtype}`: one line each in `silent.rs`, `render/silent.rs`), renderer `render/silent3.rs` (unit tests: parse round trips and the words of the questions no lockstep reaches). `carries_trigger` is false for all of them (the lockstep decided: the adapter puts the combat and the invasion into every nested context).
* `SilentCodes::g3` (places named in `g3::code`, texts in `reactions_core::G3_CODE_TEXTS`: the core holds no text): the interned texts a rule needs (`nekro_flagship`, `technology:exo2`).
* Lockstep files, each an `include!` into the module that owns the harness (no change of the harness' own tests): `invasion_live_g3_tests.rs` (and `check_ignoring` in `invasion_live.rs`: the native run lists top-level emissions only), `combat_live_g3_tests.rs`, `silent3_live_tests.rs` (a child of `game`), `reactions_core_g3_tests.rs`, `ability_audit_g3_tests.rs` (the events the lane's rules listen to, each with its native emitter or the honest "none": part c of the bail map for these rules).
* Core tests: `ti4-engine-core-tests/tests/g3_abilities.rs` (38 tests).
* Lanes that merge: the three engine harnesses (`invasion_live`, `combat_live`, `reactions_core`) gain one `include!` line each; the audit lists shift (`Gap(i)` renumber: `tools` script is `delgap.py` in the run's scratchpad, not committed: delete the row of `GAPS`, the line of `GAP_IDS`, the `ACCOUNTED` entry, renumber later `Gap(i)`).
* This base's tactical and invasion live drivers do not set the initiative order of the resolver (lane M6b's `sync_clock` does): a window of a seat other than the active one is not offered natively here. The lockstep tests that need another seat's window put that seat in the active seat's place.

## Remaining gap rows of this share, with reasons (nothing of them is silent: the live path stops with `Unknown::AbilityGap(slot)`)

| row | what is missing |
|---|---|
| Foresight (Naalu) | the relocation of the fleet (`transit::relocate_ships`: 95.5 checks, fleet overflow, the carried ground forces and fighters, the new token as a nested `COMMAND_TOKEN_PLACED`) has no core twin; the destinations need it (the string filters them on a scratch copy) |
| Spy Net | the look at a hand and the take (`reveal_hand` marks, `take_from_revealed_hand`, the hand limit after the take) is a window of its own; the core has only the hero's (`KyverHero`) |
| Creuss IFF | the destinations of a Creuss wormhole token (`tokens::wormhole_generator_destinations`: map order by hex, home systems, other players' ships, the tokens' places) are read by the adapter for the Wormhole Generator; an ability cannot call the adapter. The question and the placement (`fx_explore::place_wormhole_token`) are small |
| Stymie | the universe of systems (map systems plus board entries, no home system), `can_place_command_token`, the staged `COMMAND_TOKEN_PLACED`; and a holder other than the active seat (no lockstep here) |
| Magen Defense Grid hit | a ground hit assigned to a chosen unit: the victim's sustain offer (`ground_effect_sustain` with its card captions) and the events with the cause `md` are not in the core (the core's hits step assigns by the rules, `absorb` asks the owner only in a combat) |
| Yin Spinner, Naalu Iconoclast (TE mech) | `action_cards::place_units_choosing` (spot questions with offer captions, `PlacementLimits`) has no core twin |
| Ambuscade | shares the staged extra die of a unit-ability roll with Argent's commander and Strike Wing Alpha II (lane G2's Argent) |
| Propagation, `asn_clear`, Titans commander, the Winnu / Naaz / Muaat / Ghost / Empyrean rows | other lanes (G2, M6c, M6d) or no owner named in the task |

## Hook slots (part b of `docs/BAIL_MAP.md`) and listeners without an emitter (part c): audit, not done

* Cabal `research_waiver_offer` / `research_waiver_paid` (Riftmeld): needs the unit-upgrade table and the research window (`cb_economy/cabal/offer.rs` says BLOCKED); not started.
* `secondary_waived`: Naalu's is Mindsieve, a Thunder's Edge breakthrough (`naalubt`): out of scope by decision; Mahact's (Jae Mir Kan) needs the kernel's leader exhaust with the Nomad's Suite announcement (`fx_strategy::exhaust_agent` exists now: the offer and the token take are the work left); not started.
* Naalu `may_view_hand` (M'aban's look at the promissory hands of neighbours): an observation hook (`choice.rs` builds the seat view with it); the core has `naalu_commander_has_peek`; the view belongs to the bridge's seat observation, not to a rule. Not started.
* Part c, the 7 listeners without a native emitter (`BAIL_MAP` 5c): `PILLAGE_USED` is announced by Pillage now (where a copy could answer); `STATUS_PHASE_ENDED` (Bioplasmosis, the Oracle, Gift of Prescience's return) and `STRATEGIC_ACTION_BEGAN` (T'ro's clean-up) have none in this base; `TECHNOLOGY_GAINED` comes from the turn prelude's reconcile only. `ability_audit_g3_tests.rs` pins this for the lane's rules.

## Checks

Private `CARGO_TARGET_DIR=/root/deploytmp/core-g3` (about 1.1 GB, incremental dirs removed after builds), `CARGO_BUILD_JOBS=2`.

* `cargo test -p ti4-engine-core-tests --test g3_abilities --test silent_abilities`; the combat and flow binaries (`combat_flow combat_machine combat_end combat_small`) after the `flow.rs` change.
* `cargo test -p ti4-engine --lib`: see the end of this file; `TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1` on the Mentak / Yssaril / Arborec / Naalu / Sardakk string modules and on the new lockstep names.
* Mutation (`tools/engine_core/mutants.sh`, `D4_COVERS_FILE=/dev/null`, a scratch copy of the tree): `silent/g3/` (the 14 files of the lane) 275 points: the core tests alone kill 270 (98 %), the remaining five are `optional` / `space_battle` fields of question contexts that the engine lockstep kills (the union leaves none outside the marked equivalents: `// mutants:skip` with the reason on each); the engine lockstep alone kills 160 of 215. `render/silent3.rs`: 6 points (the operators of the file), the two survivors (`parse`) got unit tests.
* Not run: golden / wide dual sweeps, benches, soak, the mutation ratchet, census, workspace clippy, the full D4 crate, the shadow.
