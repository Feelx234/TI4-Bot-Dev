# Wave B2 + first factions (BF-00e-movement, BF-00h-cards, BF-00d-strategy, Sardakk, Yin)

Implementers: five Sonnet subagents in parallel (disjoint files). Coordinator registered the faction
decision sites (`sardakk.rs::exotrireme` 2, `supremacy` 1, `yin.rs::ask_one` 1). Per-package detail
in `BF-00e-movement.md`, `BF-00h-cards.md`, `BF-00d-strategy.md`, `BF-sardakk.md`, `BF-yin.md`.

## Checks on the committed tree

| Command | Result |
|---|---|
| `cargo test -p ti4-engine -q --no-fail-fast` | lib 1577 passed, 1 ignored; all integration binaries ok |
| `cargo test -p ti4-model -p ti4-content -q` | 81 + 134 + 1 passed |
| `cargo check --workspace --all-targets` | ok |
| `LIBTORCH=out/libtorch-2.9.1-cpu cargo test -p ti4-sim -q` | 51 passed — behaviour unchanged |

Ledger: sardakk 10/12 (commander partial, hero blocked), yin 8/11 (commander effect, hero, bt blocked).

## Opus review (tiers C/D): no blockers

Neutrality clean; card rules for Sardakk and Yin match the latest printing. Open (fix package BF-01):

| # | Finding |
|---|---|
| S1 | Redaction helpers (`choice.rs` `redact_others`/`redacted_full_state`, `ti4-model/src/view.rs` `view_for`, `ti4-policy/src/view.rs` `view_for`) copy `faction_marks` whole: reveal rows and staged card ids would leak to other seats once used. Must be fixed before Yssaril. |
| S2 | `ACTION_CARD_TAKEN` payload names the card publicly. |
| S3 | Waiver path offers "yes" to a tokenless follower, then refuses it (late rejection). |
| S4 | `transit::relocate_ships` does not check the destination may be entered (supernova, asteroid field). |
| S5 | Creuss home: Rearmament (`agenda_effects.rs:1024`) reads the record's 17; `ti4-training` rollout and `ti4-sim` seat from `home_system()` without `place_creuss_home`. |
| N | `yin_flagship` reacts only to announced `SHIP_DESTROYED` (gravity-rift loss?); T'ro misses the strategy-card free tactical action; captured fighters/infantry counted as held; partial-replay error leaves galaxy partly edited. |

## game.rs changes required (coordinator, BF-01)

1. `begin_one_move`: `path_from_ship` + `effective_move_value_for_ship`; move_bonus in Gravleash `own_move`.
2. `ACTION_COMPLETED`: `clear_reveals(Action)`; after component actions, leaders and timing effects: `announce_staged_card_events`.
3. `laws::apply_to_galaxy` sites: `apply_extra_wormholes`, `replay_map_edits`.
4. `note_arrival`: `system` (and `origin`) in `SHIP_MOVED`.
5. End of tactical production: `end_value_swap`.
6. Typed `STRATEGY_PHASE_ENDED`.
7. Typed `SYSTEM_ACTIVATED` for the strategy-card free tactical action.
