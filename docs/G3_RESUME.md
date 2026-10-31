# D2 lane G3 (`core-g3-abilities`): the loud gap rows of lane G turned into native rules, part 2 -- resume note (2026-10-31)

Branch `core-g3-abilities`, cut from `core-g-silent` (`1ef52a35`). Never pushed; the live branch is untouched. Read `docs/G_SILENT_RESUME.md` (the 61 loud rows, the audit, the recipe) first.
Slots `42_000..` are this lane's rules (`ti4-engine-core-factions/src/silent/g3/`, one file per ability family); the questions are `choice::silent3::{Ask, Opt, Subtype}`
(variants `T(..)` of `silent::{Ask, Opt, Subtype}`, renderer `ti4-engine/src/render/silent3.rs`); `SilentCodes::g3` / `g3::CODE_TEXTS` hold the interned texts the rules name.

## Native now (rule + renderer + lockstep against the string + core tests + gap row deleted)

| ability | file | string | lockstep |
|---|---|---|---|
| Scavenger Zeta (Saar mech, deploy after control) | `g3/zeta.rs` | `saar::scavenger_zeta` | `invasion_live_g3_tests` (conquest by a Saar, with / without box and good) |
| Ragh's Call | `g3/ragh.rs`, `Ask::Ragh` | `saar::ragh_call` | `invasion_live_g3_tests` (the Saar defends; use, decline, 1-2 destinations) |
| Tekklar Legion (play, end, turn) | `g3/tekklar.rs` | `sardakk::{tekklar_legion, _cleanup_ended, _cleanup_turn}` | `invasion_live_g3_tests` (N'orr defends / third-seat Sardakk; use, decline) |
| T'ro's activation-mark clean-ups (turn, strategic action) | `g3/tro_marks.rs` | `sardakk::tactical_unmark` | `silent3_live_tests` (turn start; the strategic action has no native emitter here: core test) |
| The Alastor (Nekro flagship) | `g3/alastor.rs`, `Ask::Alastor` | `nekro_units::alastor` | `combat_live_g3_tests` |
| Exotrireme II | `g3/exo2.rs`, `Ask::{ExoSelf, ExoVictim}` | `sardakk::exotrireme` | `combat_live_g3_tests` (40 seeds, 1-2 Exotrireme II, damaged, 4-6 enemy ships) |
| Z'eu (Naalu TE agent) | `g3/zeu.rs` | `naalu::agent` | `combat_live_g3_tests` (the Naalu activates: this base's tactical driver offers windows to the active seat alone, lane M6b's `sync_clock` fixes it) |
| Null Reference (`nekroc4y`) | -- | `nekro_units::null_reference` | Dropped in the audit: no faction claims the technology (operator ruling 2026-10-07 in the string module), no seat can gain it |

Findings fixed on the way (each guarded by a test that fails before the fix):

* `rules/combat/flow.rs::round_ended`: the fight was decided "over" BEFORE the window of `SPACE_COMBAT_ROUND_ENDED`; an ability of that window (Exotrireme II) that destroys the last ships of a side ended the fight one opening later
  (presentation phase `pre_roll` instead of the string's `complete`). The flow now reads the end of the fight again after the announcement. Guard: `combat_live::exotrireme_ii_destroys_itself_and_up_to_two_ships_after_a_round_like_the_string`.
* `silent::gaps` Foresight row fired for every Naalu seat at every `MOVEMENT_FINISHED` (even its own move, even without a strategy token): now only where Foresight could. Guard: `zeu_removes_the_activation_token_like_the_string` (a Naalu active seat bailed before).

## Registry order

The registry test (`reactions_core::tests::the_registry_of_the_g3_rules_equals_the_string_one`, in `reactions_core_g3_tests.rs`) compares the native registry with the string's for each of the 24 lobby factions as first and second seat.
`g3::register_after_ca_combat` (called from `reactions_core::table()` after `ca_combat::register_abilities`) holds the rules the string registers between the Nekro and Winnu modules (Tekklar's play, the Alastor, Exotrireme II);
the rest are in `g3::register_abilities` (the end, after `silent`'s). A new rule goes to the point the test accepts.

## Remaining gap rows of this share (not ported) and why

See the list at the end of this file (updated at the end of the run).
