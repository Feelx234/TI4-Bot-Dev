# BF-COMMANDERS-ALLIANCE-OCT5: acquired commanders and Alliance completion

Operator decisions, 2026-10-05: implement every commander in Yin's breakthrough pool (Crimson
still excluded by the 2026-10-04 correction); also finish Alliance promissory notes generally.
Coordinator: Claude (Opus). Implementers: two Sonnet agents (A: Obsidian/Nekro/Deepwrought/
Bastion, B: Empyrean/Firmament; source only, no Cargo while the soak owned the target) and the
coordinator for shared files and the remaining handlers. Rights predicate for every route:
`promissory::has_commander_ability` (own unlocked commander, faceup Alliance whose seated owner
has it unlocked, or a public Yin grant mark).

## Alliance promissory note

Card text (`promissory_notes.json`, `<color>_an`): "When you receive this card, if you are not
the <color> player, you must place it faceup in your play area. While this card is in your play
area, you can use the <color> player's commander ability, if it is unlocked. When you activate a
system that contains 1 or more of the <color> player's units, return this card to the <color>
player."

| Clause | Route |
|---|---|
| Faceup on receipt | existing `promissory::take` (play-area note) |
| Use the owner's unlocked commander | `has_commander_ability`, now read by every commander route below |
| Return on activating a system with the owner's units | **new** `promissory::alliances_returned_by_activation`, run inside `spend_support_on_activation` (every activation path that spends Support for the Throne) |

Test: `an_alliance_goes_home_when_its_holder_activates_a_system_with_the_owners_units`.

Original-six commanders made rights-aware and/or corrected to the printed text:

| Commander | Before | Now |
|---|---|---|
| `hacancommander` (Gila) | flat +3 votes, own status only | vote window stage `TradeGoods`: after the planets, a holder casting votes may spend N trade goods for 2N votes (`vote_spend_trade_goods`). Tests `gila_spends_trade_goods_for_two_votes_each_after_the_planets`, `gila_is_not_offered_without_the_ability_or_trade_goods`; `leaders::vote_bonus` test updated (0, not 3). |
| `solcommander` (Claire Gibson) | component-action arm that is never offered (commanders are not component actions) | `GROUND_COMBAT_STARTED` after-window in `borrowed_commanders.rs`, planet the holder controls, optional, only with an infantry in reinforcements. Test `claire_gibson_places_an_infantry_on_a_defended_planet_for_a_granted_seat`. The dead `dispatch_leader` arm is left in place. |
| `saarcommander` | own status + Saar faction | `has_commander_ability` |

## Acquired commanders (Yin pool)

| Commander | Route | Tests |
|---|---|---|
| `obsidiancommander` | `borrowed_commanders::unit_roll_modifier`, summed in `factions::unit_roll_modifier` | 2 (agent A) |
| `nekrocommander` | new typed `TECHNOLOGY_GAINED` (`Game::announce_gains` diffs `private:#seen:technologies:<p>`; first look records without announcing) → optional draw | `the_nekro_commander_draws_one_after_a_technology_is_gained` |
| `deepwroughtcommander` | `strategy_cards::deepwrought_commander` inside `paid_research` after Sucaban; researcher may reduce by 1 per other holder; holder gains a commodity or converts one; rolled back with the research | `the_deepwrought_commander_takes_one_off_another_seats_research_and_pays_its_holder` |
| `bastioncommander` | Sabotage window guard `reactions::another_players_card_is_not_sabotage` excludes a holder's plays; Nekro assimilator half vacuous (no Nekro module) | to add |
| `empyreancommander` | `borrowed_commanders_b`, `SHIP_MOVED` after-window | 4 (agent B) |
| `firmamentcommander` | `borrowed_commanders_b::firmament_planets` read by `secrets::Position::controlled`, `mecatol_ships_while_controlling_count`, `shared_planet_systems_count` (a Firmament planet counts as mine, not theirs) | 2 (agent B); secrets-level test to add |
| `kelerescommander` | `ACTION_COMPLETED` now carries `component` (true when the action logged `COMPONENT_ACTION_RESOLVED` just before finishing); after-window arms `ADDITIONAL_ACTION` (Master Plan's retained turn) | `the_keleres_commander_grants_an_additional_action_after_a_component_action_only` |
| `mahactcommander` | `tactical::activatable_with` / `activate` allow a held system; `Game` returns the token and ends the turn (`TURN_ENDED_BY_MAHACT_COMMANDER`) | `the_mahact_commander_lets_a_held_system_be_activated_again` |
| `ralnelcommander` | `RETREAT_DECLARED` when-window: destination adjacent without other players' ships, up to 2 ships one at a time, command token placed; atomic on a refused answer | `the_ral_nel_commander_retreats_two_ships_at_once_and_places_a_token` |
| `titanscommander`, `cabalcommander`, `nomadcommander` | earlier uncommitted packages (`BF-BORROWED-*-PRODUCTION.md`) | per those files |

Rules readings recorded: Deepwrought pays the card holder; Ral Nel moves ships without cargo;
Mahact's "both tokens" is one board token (the board holds one per seat per system) plus the
tactic token already spent; Firmament-treated planets are not "theirs" for shared-system secrets.

## Open

- `crimsoncommander` is in Yin's canonical pool but Crimson is out of scope (operator, Oct 4), so
  `yinbt` stays unclaimed unless the operator allows that one handler.
- Not yet compiled or run: everything above waits for the soak to release the build directory.
