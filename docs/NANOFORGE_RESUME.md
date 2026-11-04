# Nano-Forge port (lane core-nanoforge, cut from core-integration-20 b8e43ef8d)

Card text used (crates/ti4-content/content/relics.json, alias `nanoforge`): "ACTION: Attach this card to a non-legendary, non-home planet you control, its resource and influence values are increased by 2 and it is a legendary planet. This action cannot be performed once attached."
The attachment record `nanoforge` (+2/+2) already existed (attachments.json, `Attachment::Nanoforge`), and the core already summed attachment stats.

## Done
- `choice/relic.rs`: `Ask::NanoForge`, `Opt::Attach(Planet, Shown)`, `Subtype::NanoForgeChooseTarget` (appended at the end).
- `query/relic.rs`: `has_action` includes NanoForge; `nano_targets` (controlled, not a printed or seated home, not legendary printed or by the attachment, not destroyed; Mecatol Rex qualifies, the card does not exclude it); `action_relics` offers it only with a target. `UseArm` is unchanged on purpose (it mirrors the string's `use_relic` match; the string reads it as Unresolved).
- `query/board.rs`: `is_legendary` (printed or Nano-Forge attached), read by `stellar_targets` and the two objective families that count legendary planets.
- `rules/relic_windows.rs`: window `NanoForge` (asks the planet, no decline, `explore::attach` puts `Attachment::Nanoforge`); `driver/turn/relic.rs` routes it; the generic purge-after-use takes the card out of the hand (it is attached, not in hand: it cannot be used twice).
- Render texts in both copies (`ti4-engine-next/src/render/relic.rs`, `ti4-engine/src/render/relic.rs`): prompt "Nano-Forge: attach it to which planet", subtype `nano_forge_choose_target`, option label "attach the Nano-Forge to <planet> (+2 resources, +2 influence, legendary)", parse by planet id.
- State: no new field; the attachment list and ATTACH_ENTRY flag are the existing ones, so serde and fingerprints are unchanged (quick native_play 1165 steps fingerprint 74d5675ac754ac81).
- Tests: `ti4-engine-core-tests/tests/nanoforge.rs` (5), three in `action_dispatch.rs`, `query_cards.rs` list updated (NanoForge now has an action).

## Remaining / join notes
- The string engine does not offer or play it. A shadow game that holds Nano-Forge with a legal target would differ at the menu (core offers `relic|nanoforge`); the Z-gate (3,4,5,6 players x 8 seeds x all factions, shadow) was green with zero differences, so no shadow game reached it and no KNOWN_NANOFORGE recognizer was added.
- The string test `the_use_dispatch_and_the_action_list_run_in_lockstep_with_the_core` (ti4-engine/src/relics.rs) does not hold Nano-Forge in its hand; not run here (legacy test binary is heavy).
- `driver/turn/relic.rs::routed` is unused by callers (pre-existing); the NanoForge entry in it has no test.
