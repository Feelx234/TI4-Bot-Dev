//! The Ral Nel Consortium's cards: Nanomachines (technology actions), Kan Kip Rel (agent), the
//! Nano-Link Permit and Data Skimmer (breakthrough), and the status readiness of both. See
//! `ralnel.rs` for the abilities and `plans/evidence/BF-ralnel.md`.
//!
//! * Nanomachines: "ACTION: Exhaust this card to place 1 PDS on a planet you control. ACTION:
//!   Exhaust this card to repair all of your damaged units. ACTION: Exhaust this card and discard 1
//!   action card to draw 1 action card." One exhaustion covers whichever of the three is taken.
//! * Kan Kip Rel: "ACTION: Exhaust this card and draw 2 action cards; give 1 of those cards to
//!   another player."
//! * Data Skimmer: "During the action phase, if you have not passed, when other players would
//!   discard action cards, they are placed on this card instead. When you pass, take 1 action card
//!   from this card and discard the rest."

use std::sync::Arc;

use ti4_content::ContentStore;
use ti4_content::units::UnitType;
use ti4_model::content_types::SourceSet;
use ti4_model::id::{ActionCardId, LeaderId, PlanetId, PlayerId, SystemId, TechnologyId};
use ti4_model::state::GameState;
use ti4_model::units::Unit;

use super::ralnel::{AGENT, MECH, ask};
use crate::choice::ChoiceOption;
use crate::timing::{Ability, Relation, TimingContext, TimingError};

/// Nanomachines (technology).
pub const NANOMACHINES: &str = "nanomachines";
/// The Nano-Link Permit (promissory note alias).
pub const NANOLINK: &str = "nanolink";
/// Data Skimmer (breakthrough).
pub const BREAKTHROUGH: &str = "ralnelbt";

const PDS_ACTION: &str = "faction|ralnel|nano_pds";
const REPAIR_ACTION: &str = "faction|ralnel|nano_repair";
const DRAW_ACTION: &str = "faction|ralnel|nano_draw";

const EXHAUSTED_PREFIX: &str = "ralnel:nano:exhausted:";
const SKIM_PREFIX: &str = "ralnel:skim:";

fn exhausted_key(player: &PlayerId) -> String {
    format!("{EXHAUSTED_PREFIX}{player}")
}

fn skim_key(player: &PlayerId) -> String {
    format!("{SKIM_PREFIX}{player}")
}

fn owns_nanomachines(state: &GameState, player: &PlayerId) -> bool {
    state
        .player(player)
        .is_some_and(|seat| seat.technologies.contains(&TechnologyId::new(NANOMACHINES)))
}

fn nanomachines_ready(state: &GameState, player: &PlayerId) -> bool {
    owns_nanomachines(state, player) && !state.faction_marks.contains_key(&exhausted_key(player))
}

/// Planets `player` controls, as `(system, planet)`, in board order.
fn controlled(state: &GameState, player: &PlayerId) -> Vec<(SystemId, PlanetId)> {
    state
        .controlled_planets(player)
        .into_iter()
        .map(|(system, planet)| (system.clone(), planet.clone()))
        .collect()
}

fn damaged_units(state: &GameState, player: &PlayerId) -> bool {
    state.board.values().any(|board| {
        board
            .units
            .iter()
            .chain(board.planet_units.values().flatten())
            .any(|unit| &unit.owner == player && unit.sustained_damage)
    })
}

/// Nanomachines' PDS action: a PDS from reinforcements onto one of your planets.
fn place_pds(context: &mut TimingContext<'_>, player: &PlayerId) -> Result<bool, TimingError> {
    let planets = controlled(context.state, player);
    let Some((system, planet)) = (match planets.as_slice() {
        [] => None,
        [only] => Some(only.clone()),
        _ => {
            let options = planets
                .iter()
                .map(|(system, planet)| {
                    ChoiceOption::labelled(
                        planet.to_string(),
                        "planet",
                        format!("{planet} in {system}"),
                    )
                })
                .collect();
            let answer = ask(
                context,
                player,
                "Nanomachines: place a PDS on which planet?".to_owned(),
                NANOMACHINES,
                "pds",
                options,
            )?;
            planets
                .iter()
                .find(|(_, planet)| planet.as_str() == answer.id)
                .cloned()
        }
    }) else {
        return Ok(false);
    };
    let placed =
        crate::action_cards::place_units_counted(context, player, &system, Some(&planet), "pds", 1);
    Ok(placed > 0)
}

/// Nanomachines' repair action: every damaged unit of `player` is repaired.
fn repair_all(state: &mut GameState, player: &PlayerId) {
    for board in state.board.values_mut() {
        for unit in board
            .units
            .iter_mut()
            .chain(board.planet_units.values_mut().flatten())
        {
            if &unit.owner == player {
                unit.sustained_damage = false;
            }
        }
    }
}

/// Nanomachines' draw action: discard 1 action card (asked when there are several), then draw 1.
fn discard_and_draw(
    context: &mut TimingContext<'_>,
    player: &PlayerId,
) -> Result<bool, TimingError> {
    let hand: Vec<ActionCardId> = context
        .state
        .player(player)
        .map(|seat| seat.action_cards.clone())
        .unwrap_or_default();
    if hand.is_empty() {
        return Ok(false);
    }
    let index = if hand.len() == 1 {
        0
    } else {
        let options = hand
            .iter()
            .enumerate()
            .map(|(index, card)| {
                ChoiceOption::labelled(
                    index.to_string(),
                    "card",
                    crate::action_cards::name_of(context.content, card),
                )
            })
            .collect();
        let answer = ask(
            context,
            player,
            "Nanomachines: discard which action card?".to_owned(),
            NANOMACHINES,
            "discard",
            options,
        )?;
        answer.id.parse::<usize>().unwrap_or(usize::MAX)
    };
    let Some(card) = crate::action_cards::discard(context.state, player, index) else {
        return Ok(false);
    };
    crate::action_cards::discarded(context.state, player, &card, true);
    crate::action_cards::draw(context.state, context.content, context.table, player, 1)
        .map_err(TimingError::IllegalChoice)?;
    Ok(true)
}

/// Kan Kip Rel: draw 2, give one of them to another player (asked when there are several cards
/// and several other players).
fn courier(context: &mut TimingContext<'_>, player: &PlayerId) -> Result<bool, TimingError> {
    let drawn = crate::action_cards::draw(context.state, context.content, context.table, player, 2)
        .map_err(TimingError::IllegalChoice)?;
    let recipients: Vec<PlayerId> = context
        .state
        .seating_order
        .iter()
        .filter(|seat| *seat != player)
        .cloned()
        .collect();
    let Some(card) = drawn.last().cloned() else {
        return Ok(true);
    };
    if recipients.is_empty() {
        return Ok(true);
    }
    // The card to give: the one just drawn when there is only one; otherwise asked.
    let give = if drawn.len() == 1 {
        card
    } else {
        let options = drawn
            .iter()
            .enumerate()
            .map(|(index, card)| {
                ChoiceOption::labelled(
                    index.to_string(),
                    "card",
                    crate::action_cards::name_of(context.content, card),
                )
            })
            .collect();
        let answer = ask(
            context,
            player,
            "Kan Kip Rel: give which of the two cards?".to_owned(),
            AGENT,
            "give_card",
            options,
        )?;
        let index: usize = answer.id.parse().unwrap_or(0);
        drawn.get(index).cloned().unwrap_or(card)
    };
    let to = if recipients.len() == 1 {
        recipients[0].clone()
    } else {
        let options = recipients
            .iter()
            .map(|seat| ChoiceOption::labelled(seat.to_string(), "player", seat.to_string()))
            .collect();
        let answer = ask(
            context,
            player,
            "Kan Kip Rel: give the card to which player?".to_owned(),
            AGENT,
            "give_to",
            options,
        )?;
        recipients
            .iter()
            .find(|seat| seat.as_str() == answer.id)
            .cloned()
            .unwrap_or_else(|| recipients[0].clone())
    };
    if let Some(seat) = context.state.player_mut(player)
        && let Some(index) = seat.action_cards.iter().position(|held| *held == give)
    {
        seat.action_cards.remove(index);
        if let Some(recipient) = context.state.player_mut(&to) {
            recipient.action_cards.push(give);
        }
    }
    Ok(true)
}

// -- the Data Skimmer ------------------------------------------------------------------------------

/// The player holding Data Skimmer, if any.
fn skimmer_holder(state: &GameState) -> Option<PlayerId> {
    state
        .players
        .iter()
        .find(|seat| crate::breakthroughs::holds(state, &seat.id, BREAKTHROUGH))
        .map(|seat| seat.id.clone())
}

/// Whether Data Skimmer takes an action card that `discarder` would discard: it is placed on the
/// card (true) rather than the discard pile, when the holder is another player who has not passed
/// during the action phase. Returns false, changing nothing, otherwise.
pub(crate) fn skimmer_takes(
    state: &mut GameState,
    discarder: &PlayerId,
    card: &ActionCardId,
) -> bool {
    let Some(holder) = skimmer_holder(state) else {
        return false;
    };
    let holder_passed = state.player(&holder).is_some_and(|seat| seat.passed);
    if &holder == discarder || holder_passed || state.phase != ti4_model::state::Phase::Action {
        return false;
    }
    let key = skim_key(&holder);
    let mut held = state
        .faction_marks
        .get(&key)
        .map(|text| {
            text.split(',')
                .filter(|part| !part.is_empty())
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    held.push(card.as_str().to_owned());
    state.faction_marks.insert(key, held.join(","));
    true
}

/// When the holder passes: take one card from the Data Skimmer (asked when there are several) and
/// discard the rest.
fn skimmer_pass(owner_name: &str, seat: &PlayerId) -> Ability {
    let (owner, condition_owner) = (seat.clone(), seat.clone());
    Ability::stateful(
        format!("breakthrough:{owner_name}:{BREAKTHROUGH}:PLAYER_PASSED:after"),
        seat.clone(),
        "PLAYER_PASSED",
        Relation::After,
        Arc::new(move |_event, _resolver, context| {
            let key = skim_key(&owner);
            let held: Vec<String> = context
                .state
                .faction_marks
                .get(&key)
                .map(|text| {
                    text.split(',')
                        .filter(|part| !part.is_empty())
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default();
            context.state.faction_marks.remove(&key);
            let take = match held.as_slice() {
                [] => None,
                [only] => Some(only.clone()),
                _ => {
                    let options = held
                        .iter()
                        .enumerate()
                        .map(|(index, card)| {
                            ChoiceOption::labelled(
                                index.to_string(),
                                "card",
                                crate::action_cards::name_of(
                                    context.content,
                                    &ActionCardId::new(card.as_str()),
                                ),
                            )
                        })
                        .collect();
                    let answer = ask(
                        context,
                        &owner,
                        "Data Skimmer: take which action card?".to_owned(),
                        BREAKTHROUGH,
                        "take",
                        options,
                    )?;
                    answer
                        .id
                        .parse::<usize>()
                        .ok()
                        .and_then(|index| held.get(index).cloned())
                }
            };
            for card in held {
                let card = ActionCardId::new(card);
                if take.as_deref() == Some(card.as_str()) {
                    if let Some(seat) = context.state.player_mut(&owner) {
                        seat.action_cards.push(card);
                    }
                } else {
                    context.state.discarded_action_cards.push(card);
                }
            }
            Ok(())
        }),
    )
    .with_stateful_condition(Arc::new(move |event, _, context| {
        event.text("player") == Some(condition_owner.as_str())
            && crate::breakthroughs::holds(context.state, &condition_owner, BREAKTHROUGH)
            && context
                .state
                .faction_marks
                .get(&skim_key(&condition_owner))
                .is_some_and(|text| !text.is_empty())
    }))
}

// -- readiness ------------------------------------------------------------------------------------

/// The status phase readies Nanomachines (the one exhaustion covers all three actions).
fn nanomachines_readies(owner_name: &str, seat: &PlayerId) -> Ability {
    let (owner, condition_owner) = (seat.clone(), seat.clone());
    Ability::stateful(
        format!("technology:{owner_name}:{NANOMACHINES}:STATUS_PHASE_ENDED:after"),
        seat.clone(),
        "STATUS_PHASE_ENDED",
        Relation::After,
        Arc::new(move |_event, _resolver, context| {
            context.state.faction_marks.remove(&exhausted_key(&owner));
            Ok(())
        }),
    )
    .with_stateful_condition(Arc::new(move |_event, _, context| {
        context
            .state
            .faction_marks
            .contains_key(&exhausted_key(&condition_owner))
    }))
}

// -- Alarum -----------------------------------------------------------------------------------------

/// The Alarum moves at most this many ground forces.
const ALARUM_MOVES: usize = 2;

/// The ground forces `owner` could move to `planet` in `system`: theirs on another planet of that
/// system or of an adjacent one (the ruling counts the planet's own system as adjacent), as
/// `(origin system, origin planet, index on the planet, unit)`.
fn alarum_candidates(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    galaxy: &ti4_content::galaxy::Galaxy,
    owner: &PlayerId,
    system: &SystemId,
    planet: &PlanetId,
) -> Vec<(SystemId, PlanetId, usize, Unit)> {
    let types = ti4_content::units::catalogue(content, sources);
    let mut near: std::collections::BTreeSet<SystemId> =
        crate::movement::PlayerAdjacency::new(state, content, sources, galaxy, owner)
            .neighbours(system.as_str())
            .into_iter()
            .map(SystemId::new)
            .collect();
    near.insert(system.clone());
    let mut found = Vec::new();
    for origin in near {
        let board = state.system_state(&origin);
        for (origin_planet, units) in &board.planet_units {
            if origin_planet == planet {
                continue;
            }
            for (index, unit) in units.iter().enumerate() {
                if &unit.owner == owner
                    && types
                        .get(unit.type_id.as_str())
                        .is_some_and(UnitType::is_ground_force)
                {
                    found.push((origin.clone(), origin_planet.clone(), index, unit.clone()));
                }
            }
        }
    }
    found
}

/// Alarum: "At the end of a round of combat on this planet, you may move up to 2 ground forces to
/// this planet from planets in adjacent systems." Asked one ground force at a time.
fn alarum(owner_name: &str, seat: &PlayerId) -> Ability {
    let (owner, condition_owner) = (seat.clone(), seat.clone());
    Ability::stateful(
        format!("unit:{owner_name}:{MECH}:GROUND_COMBAT_ROUND_ENDED:after"),
        seat.clone(),
        "GROUND_COMBAT_ROUND_ENDED",
        Relation::After,
        Arc::new(move |event, _resolver, context| {
            let (Some(system), Some(planet)) = (
                event.text("system").map(SystemId::new),
                event.text("planet").map(PlanetId::new),
            ) else {
                return Ok(());
            };
            let Some(galaxy) = context.galaxy else {
                return Ok(());
            };
            for _ in 0..ALARUM_MOVES {
                let candidates = alarum_candidates(
                    context.state,
                    context.content,
                    context.sources,
                    galaxy,
                    &owner,
                    &system,
                    &planet,
                );
                if candidates.is_empty() {
                    break;
                }
                let mut options: Vec<ChoiceOption> = candidates
                    .iter()
                    .map(|(origin, from, index, unit)| {
                        ChoiceOption::labelled(
                            format!("{origin}|{from}|{index}"),
                            "ground_force",
                            format!("{} on {from} in {origin}", unit.type_id),
                        )
                    })
                    .collect();
                options.push(ChoiceOption::decline());
                let answer = ask(
                    context,
                    &owner,
                    format!("Alarum: move a ground force to {planet}?"),
                    MECH,
                    "move",
                    options,
                )?;
                let Some((origin, from, index, _)) =
                    candidates.iter().find(|(origin, from, index, _)| {
                        format!("{origin}|{from}|{index}") == answer.id
                    })
                else {
                    break;
                };
                let (origin, from, index) = (origin.clone(), from.clone(), *index);
                let unit = context
                    .state
                    .system_mut(&origin)
                    .planet_units
                    .get_mut(&from)
                    .map(|units| units.remove(index));
                if let Some(unit) = unit {
                    context
                        .state
                        .system_mut(&system)
                        .planet_units
                        .entry(planet.clone())
                        .or_default()
                        .push(unit);
                }
            }
            Ok(())
        }),
    )
    .with_optional(true)
    .with_stateful_condition(Arc::new(move |event, _, context| {
        let (Some(system), Some(planet), Some(galaxy)) = (
            event.text("system").map(SystemId::new),
            event.text("planet").map(PlanetId::new),
            context.galaxy,
        ) else {
            return false;
        };
        let mech_here = context
            .state
            .system_state(&system)
            .planet_units
            .get(&planet)
            .is_some_and(|units| {
                units
                    .iter()
                    .any(|unit| unit.type_id.as_str() == MECH && unit.owner == condition_owner)
            });
        mech_here
            && !alarum_candidates(
                context.state,
                context.content,
                context.sources,
                galaxy,
                &condition_owner,
                &system,
                &planet,
            )
            .is_empty()
    }))
}

pub(crate) fn timing_abilities(
    _state: &GameState,
    owner_name: &str,
    seat: &PlayerId,
) -> Vec<Ability> {
    vec![
        nanomachines_readies(owner_name, seat),
        skimmer_pass(owner_name, seat),
        alarum(owner_name, seat),
    ]
}

// -- component actions ----------------------------------------------------------------------------

pub(crate) fn component_actions(
    state: &GameState,
    _content: &ContentStore,
    player: &PlayerId,
) -> Vec<ChoiceOption> {
    let mut actions = Vec::new();
    if !nanomachines_ready(state, player) {
        return actions;
    }
    if !controlled(state, player).is_empty() {
        actions.push(ChoiceOption::labelled(
            PDS_ACTION,
            crate::faction_abilities::ACTION_KIND,
            "Nanomachines: exhaust to place 1 PDS on a planet you control",
        ));
    }
    if damaged_units(state, player) {
        actions.push(ChoiceOption::labelled(
            REPAIR_ACTION,
            crate::faction_abilities::ACTION_KIND,
            "Nanomachines: exhaust to repair all of your damaged units",
        ));
    }
    let holds_cards = state
        .player(player)
        .is_some_and(|seat| !seat.action_cards.is_empty());
    if holds_cards {
        actions.push(ChoiceOption::labelled(
            DRAW_ACTION,
            crate::faction_abilities::ACTION_KIND,
            "Nanomachines: exhaust and discard 1 action card to draw 1",
        ));
    }
    actions
}

pub(crate) fn perform_component(
    context: &mut TimingContext<'_>,
    player: &PlayerId,
    option: &ChoiceOption,
) -> bool {
    if !nanomachines_ready(context.state, player) {
        return false;
    }
    let performed = match option.id.as_str() {
        PDS_ACTION => place_pds(context, player),
        REPAIR_ACTION => {
            repair_all(context.state, player);
            Ok(true)
        }
        DRAW_ACTION => discard_and_draw(context, player),
        _ => return false,
    };
    match performed {
        Ok(true) => {
            context
                .state
                .faction_marks
                .insert(exhausted_key(player), "1".to_owned());
            true
        }
        _ => false,
    }
}

pub(crate) fn leader_action(
    state: &GameState,
    _content: &ContentStore,
    player: &PlayerId,
    leader: &LeaderId,
) -> Option<bool> {
    (leader.as_str() == AGENT).then(|| state.player(player).is_some())
}

pub(crate) fn use_leader(
    context: &mut TimingContext<'_>,
    player: &PlayerId,
    leader: &LeaderId,
) -> Option<bool> {
    (leader.as_str() == AGENT).then(|| courier(context, player).unwrap_or(false))
}
