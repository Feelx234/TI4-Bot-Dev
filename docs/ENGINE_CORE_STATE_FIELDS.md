# GameState field map: every field and its home in `ti4-engine-core::state` (package F1)

Code: `crates/ti4-engine-core/src/{containers,state}/`. Source: `ti4-model/src/state.rs` at rules version 35.
Conventions: `Seat` = owner table index; `Code(u16)` = adapter-interned text for an open string set (X1 replaces
it by an enum when the set closes); "text order" notes say where the typed order is not the id text order
(the string side re-sorts on store, the core must not rely on iteration order across such keys).
Deviations from `ENGINE_CORE_STATE.md`: the packed board unit is `Piece` (the name `Unit` is the generated
unit-type enum); the eight `Vec<u32>` round lists of `Player` share `Player.round_marks`; `Rng` streams sit
behind one `Cow`; `LeaderSlot[4]` became a `SortedMap<Leader, LeaderStatus, 5>` (the string map may hold any leader).

## GameState

| Field | Home | Note |
|---|---|---|
| players | `players: Players` (`Cow<Player>` x 8) | index = position in the vec |
| seating_order | `seats: Seats.seating_order` | |
| speaker | `meta.speaker` | |
| phase | `meta.phase` (core `Phase`) | |
| round | `meta.round` | |
| active | `meta.active` | |
| diplomacy | `trade.diplomacy: Cow<Diplomacy>` | typed port of `DiplomacyState`; texts `rules_version`, `legacy_promise` are `Code` |
| unclaimed_strategy_cards | `strategy.unclaimed` | |
| strategy_card_goods | `strategy.goods` | `SortedMap` keeps key presence |
| card_initiative | `strategy.initiative` | |
| strategy_cards_per_player | `meta.strategy_cards_per_player` | u8 |
| board (`SystemState` units, command_tokens) | `board.sys[System]`: `units`, `cmd` | `flags.TOUCHED` = entry exists |
| SystemState.planet_control | `board.planet[Planet].control` | |
| SystemState.planet_units | `PlanetCell.units` | `flags.UNITS_ENTRY` = entry exists (even empty) |
| SystemState.coexisting | `PlanetCell.coexist` | `flags.COEXIST_ENTRY` |
| SystemState.purged_planets | `PlanetCell.flags.PURGED` | |
| active_system | `meta.active_system` | |
| pending (`Option<String>`, only `"move"`) | `pending.tag: PendingTag` | the window stack is new (`pending.stack`) |
| exhausted_planets | `PlanetCell.flags.EXHAUSTED` | |
| revealed_objectives | `objectives.revealed` | |
| objective_deck | `decks.objective` | |
| scored_objectives | `objectives.scored[seat]` (`ObjectiveSet`) + `scored_present` | public/secret classes merge by text outside the core |
| finished | `meta.finished` | |
| exploration_decks | `decks.explore[ExploreType]` + `explore_present` | |
| planet_attachments | `PlanetCell.attach` | `flags.ATTACH_ENTRY` |
| exploration_log | `explore.log: Log<ExploreRec>` | `outcome`, `result`, gain keys, `placed` are `Code` |
| legacy_device_ids | `meta.flags.LEGACY_DEVICE_IDS` | |
| starting_tech_picks | `transient.misc.starting_tech_picks` | |
| agenda_log | `agenda.log` | `AgendaRec`; `name` is derived text (not stored); `discard_reason` is `Code` |
| agenda_casts | `agenda.casts` | |
| relic_log | `ledger.relics` | |
| relic_deck | `decks.relic` | |
| agenda_deck | `decks.agenda` | |
| action_card_deck | `decks.action` | |
| discarded_action_cards | `cards.discarded_action` | |
| salvage_yard | `cards.salvage_yard` | |
| secret_deck | `decks.secret` | |
| laws | `agenda.laws: SortedMap<Agenda, Outcome>` | |
| custodians_removed | `meta.flags.CUSTODIANS_REMOVED` | |
| frontier_tokens / ingress_tokens / breach_tokens | `explore.frontier / ingress / breach: Set<System,4>` | |
| expedition_slices | `explore.expedition_slices` (`Expedition`) | |
| fracture_in_play | `meta.flags.FRACTURE_IN_PLAY` | |
| wormhole_tokens | `explore.wormhole_tokens` | keys `ALPHA`/`BETA` |
| ion_storm | `meta.ion_storm` | face = `Wormhole` |
| nexus_unlocked | `meta.flags.NEXUS_UNLOCKED` | |
| styx_holder | `meta.styx_holder` | the holder must be in the owner table |
| rng_seed | `rng.seed` | |
| deck_reserve (serde skip) | `cards.reserve: Option<Cow<DeckReserve>>` | the shared atomic cursor becomes a plain value the adapter syncs |
| placed_planets | `PlanetCell.placed_in` | |
| influence_pays_for_units | `meta.influence_pays_for_units` | |
| thunders_edge_system | `meta.thunders_edge_system` | |
| galactic_event | `meta.galactic_event` | |
| purged_systems | `SysCell.flags.PURGED` | |
| combat_round_seq, production_seq, activation_seq, agenda_seq, turn_seq, action_turn, feat_occurrence_seq | `meta.*` | |
| transactions_this_round | `trade.this_round` | |
| negotiations_this_action / _round | `trade.negotiations_this_action / _round` | |
| rules_version | `meta.rules_version` | |
| thunders_edge | `meta.flags.THUNDERS_EDGE` | |
| scored_feat_occurrences | `objectives.scored_feats` (sorted `(Seat, u64)`) | |
| vp_ledger | `ledger.vp: Log<VpRec>` | `LedgerReason` typed forms + `Other(Code)` |
| combat_draw_round | `meta.combat_draw_round` | |
| reroll_staging | `transient.combat.reroll_staging` | `kind`, `unit`, `unit_types` keys are `Code` |
| last_reroll_player, last_committed_unit, last_sustain, last_ship_destroyed, last_control_gained, last_combat_sides, last_action_discarded, pending_reflective_hits | `transient.combat.*` | |
| pending_destructions | `transient.combat.pending_destructions` | reason = `Code` |
| action_card_plays, declared_card_plays | `transient.misc.*` | |
| active_space_combat | `transient.combat.active_space_combat` | |
| active_invasion | `transient.invasion.active` | `phase`, step `kind`, die `group` are `Code` |
| combat_round_hits / combat_round_dice / combat_presentation | `transient.combat.*` | `phase` is `Code` |
| last_strategy_choice | `transient.misc.last_strategy_choice` | |
| agenda_veto_replacement, covert_agenda, agenda_elected_override, agenda_votes, agenda_choices, agenda_in_play, agenda_predictions | `agenda.*` | |
| transient_flags | `transient.misc.flags` | same bits |
| fighters_produced_this_use .. gravleash_move_values | `transient.production` | |
| transactions_this_turn, traded_goods, traded_goods_for_promissory, promises, support_holders | `trade.*` | promise text is `Code` |
| promissory_notes / promissory_faceup | `cards.promissory / promissory_faceup` (`NoteKey = (note, faction)`) | typed order is not the text order of `alias:faction` |
| faction_marks | `marks.tail: SortedMap<MarkKey, MarkVal>` | families/values pinned by X1; per-faction records follow with the C shards |
| initiative_overrides | `strategy.initiative_overrides` | |

## Player

| Field | Home |
|---|---|
| id | `seat` |
| faction, home_system, home_planets | same names |
| strategy_cards, exhausted_strategy_cards | `strat`, `strat_exhausted` |
| tactic/fleet/strategic_tokens, trade_goods, commodities, victory_points, passed, public_objectives_forbidden, stability, duress_by | `econ.*` (i16) |
| technologies, exhausted_technologies, assimilated_technologies | `tech.owned / exhausted / assimilated` |
| action_cards, reaction_card_modes, secret_objectives, relics, exhausted_relics, exhausted_legendary, exploration_cards, plots, plot_objectives, relic_fragments, breakthrough | `cards.*` (`FragmentCounts` keeps key presence; `Plot` typed) |
| leaders | `leaders.status` |
| conduit_activation, move_bonus_activation, gravity_drive_used_activation, anomalies_ignored_activation, silence_activation, silence_system, spatial_conduit, hack_votes_last_agenda, extra_votes_agenda | `timers.*` |
| combat_bonus_round, rout_round, waylay_barrage_round, retreat_barred_round, extra_die_round, extra_die_unit, munitions_round, ground_roll_suppressed_round, sustained_damage_round, sustained_damage_types, cancel_hits_round | `combat_timers.*` |
| fleet_supply_unlimited_until, free_production_use | `production_timers.*` |
| agency_supply_used_turn, fleet_logistics_used_turn | `card_timers.*` |
| fighter_bonus_round, bunker_invasion, blitz_invasion, disable_invasion, war_machine_use, solar_flare, lost_star, dominus_orb | `round_marks` (`RoundKind`) |
| infantry_returning, spec_ops_returning, spec_ops_destroyed, spec_ops_card, technology_units_returning, captured_units, event_feats | `misc.*` |

## Not in core (1.7)

Content store and all text; `Table`; `Game.events: Vec<String>` text and `Resolver.log` (typed `ledger.events` is the new home, the text is rendered by the adapter);
`DecisionLog`; `GameRng.observation`/`sync`; `aftermath_observer`, `round_income_*` knobs; caches (`SeatReach`, adjacency);
`Game.galaxy` is the `map` slice (`MapSlice.galaxy`, `None` until loaded); `Game.dice` history is `ledger.dice`; `Game.strategy_cards` (the card list of the table) is content;
`Game.event_sequence` is `EventRec.id` (next id = events.len()+1 is not assumed: the adapter carries the counter until A5).

## Known gaps handed to later packages

* `Code` uses (X1/B1/B2/B4 to close): `Outcome::Other`, `LedgerReason::Other`, `RollPurpose::Other`, `ExploreRec.{outcome,result,gains,placed}`, `AgendaRec.discard_reason`, combat `RerollKind/RerollEntry.unit/unit_types/DestroyReason/CombatStage`, invasion `phase/kind/group`, `Promise`, `DiplomacyRules`.
* `EventRec` carries `EventKind(u16)` + 3 args until A5 fixes the event layout; `Window`/`PendingAsk` are placeholders for D1/F4.
* Typed iteration order differs from text order for `Outcome`, `NoteKey`, `ObjectiveSet` (public vs secret), `RngDomain`; code that iterates these in an observable way needs the generator's `NAME_RANK` (X3).
* Values narrowed on purpose: econ counters to `i16`, hits/counters to `u32`; the codec must reject (not wrap) out-of-range input.
