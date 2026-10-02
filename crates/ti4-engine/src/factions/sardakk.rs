//! The Sardakk N'orr (`sardakk`). See `factions/mod.rs` for the contract and
//! `plans/BASE_FACTIONS_PLAN_2026-10-02.md` for scope; the per-item record is
//! `plans/evidence/BF-sardakk.md`.
//!
//! Card texts (latest printing, `crates/ti4-content/content/*.json`):
//!
//! * Unrelenting: "Apply +1 to the result of each of your unit's combat rolls." (shared code)
//! * Exotrireme II (`exo2`): "This unit cannot be destroyed by \"Direct Hit\" action cards. After a
//!   round of space combat, you may destroy this unit to destroy up to 2 ships in this system."
//! * Valkyrie Particle Weave (`vpw`): "After making combat rolls during a round of ground combat,
//!   if your opponent produced 1 or more hits, you produce 1 additional hit."
//! * C'morran N'orr (flagship): "Apply +1 to the result of each of your other ship's combat rolls
//!   in this system."
//! * Valkyrie Exoskeleton (mech): "After this unit uses it's SUSTAIN DAMAGE ability during ground
//!   combat, it produces 1 hit against your opponent's ground forces on this planet."
//! * Tekklar Legion (`tekklar`): "At the start of an invasion combat: Apply +1 to the result of
//!   each of your unit's combat rolls during this combat. If your opponent is the N'orr player,
//!   apply -1 to the result of each of their unit's combat rolls during this combat. Then, return
//!   this card to the N'orr player."
//! * T'ro (`sardakkagent`): "At the end of a player's tactical action: You may exhaust this card:
//!   if you do, that player may place 2 infantry from their reinforcements on a planet they
//!   control in the active system."
//! * G'hom Sek'kus (`sardakkcommander`): "You can commit (move) up to 1 ground force from each
//!   planet in the active system and each planet in adjacent systems that do not contain 1 of your
//!   command tokens." Unlock: "Control 5 planets in non-home systems."
//! * N'orr Supremacy (`sardakkbt`): "After you win a combat, either gain 1 command token or
//!   research a unit upgrade technology."

use std::sync::Arc;

use ti4_content::ContentStore;
use ti4_content::units::{UnitType, catalogue};
use ti4_model::content_types::SourceSet;
use ti4_model::id::{LeaderId, PlanetId, PlayerId, SystemId, TechnologyId};
use ti4_model::state::{GameState, LeaderStatus};
use ti4_model::units::Unit;

use super::hooks_ground::{CommitCandidate, CommitOrigin, GroundHooks};
use super::{CombatUnit, FactionModule, Hooks};
use crate::choice::{Choice, ChoiceOption};
use crate::decision_context::{DecisionContext, DecisionSource};
use crate::timing::{Ability, Relation, TimingError};

/// The faction alias; also the faction name in promissory note ids (`tekklar:sardakk`).
const FACTION: &str = "sardakk";
/// The Tekklar Legion note's id: the printed alias and the N'orr player's faction name.
const TEKKLAR_NOTE: &str = "tekklar:sardakk";
/// `GameState::faction_marks` key for the combat Tekklar Legion was played into, valued
/// `"<system>|<planet>|<holder>|<n'orr player or empty>"`.
const TEKKLAR_MARK: &str = "sardakk:tekklar:combat";

/// What this faction implements.
pub const MODULE: FactionModule = FactionModule {
    alias: FACTION,
    abilities: &["unrelenting"],
    technologies: &["exo2", "vpw"],
    units: &[
        "sardakk_dreadnought",
        "sardakk_dreadnought2",
        "sardakk_flagship",
        "sardakk_mech",
    ],
    promissory: &["tekklar"],
    // `sardakkagent` only: the commander is partial (no adjacent systems) and the hero is blocked.
    leaders: &["sardakkagent"],
    breakthroughs: &["sardakkbt"],
    hooks: Hooks {
        unit_roll_modifier: Some(unit_roll_modifier),
        timing_abilities: Some(timing_abilities),
        commander_unlocked: Some(commander_unlocked),
        ground: GroundHooks {
            ground_rolls_extra_hits: Some(ground_rolls_extra_hits),
            commit_candidates: Some(commit_candidates),
            ..GroundHooks::NONE
        },
        ..Hooks::NONE
    },
};

// -- small readers -------------------------------------------------------------------------------

fn has_technology(state: &GameState, player: &PlayerId, alias: &str) -> bool {
    state
        .player(player)
        .is_some_and(|seat| seat.technologies.contains(&TechnologyId::new(alias)))
}

fn leader_status(state: &GameState, player: &PlayerId, leader: &str) -> Option<LeaderStatus> {
    crate::leaders::status(state, player, &LeaderId::new(leader))
}

/// Every ship in a system's space area, with its owner, in board order.
fn ships_in(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
) -> Vec<(PlayerId, Unit)> {
    let types = catalogue(content, sources);
    state
        .system_state(system)
        .units
        .iter()
        .filter(|unit| {
            types
                .get(unit.type_id.as_str())
                .is_some_and(UnitType::is_ship)
        })
        .map(|unit| (unit.owner.clone(), unit.clone()))
        .collect()
}

/// The owners of ground forces on a planet, in player-id order.
fn ground_force_owners(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
    planet: &PlanetId,
) -> Vec<PlayerId> {
    let types = catalogue(content, sources);
    let owners: std::collections::BTreeSet<PlayerId> = state
        .system_state(system)
        .on_planet(planet)
        .iter()
        .filter(|unit| {
            types
                .get(unit.type_id.as_str())
                .is_some_and(UnitType::is_ground_force)
        })
        .map(|unit| unit.owner.clone())
        .collect();
    owners.into_iter().collect()
}

fn decision(
    context: &crate::timing::TimingContext<'_>,
    player: &PlayerId,
    card: &str,
    subtype: &str,
) -> DecisionContext {
    DecisionContext::new(
        player.clone(),
        DecisionSource::FactionAbility(card.to_owned()),
        subtype,
        context.state.phase,
        context.state.round,
    )
}

// -- per-unit roll modifiers: the flagship and Tekklar Legion ------------------------------------

/// The C'morran N'orr: other ships of its owner in its system roll +1. Tekklar Legion: the holder's
/// units +1 and the N'orr player's -1, in the one ground combat it was played into.
fn unit_roll_modifier(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    unit: &CombatUnit<'_>,
) -> i64 {
    match unit.context {
        "space" => {
            let Some(system) = unit.system else { return 0 };
            if unit.unit_type == "sardakk_flagship" {
                return 0; // "your other ship's combat rolls"
            }
            let flagship_here =
                ships_in(state, content, sources, system)
                    .iter()
                    .any(|(owner, ship)| {
                        owner == unit.player && ship.type_id.as_str() == "sardakk_flagship"
                    });
            i64::from(flagship_here)
        }
        "ground" => {
            let (Some(system), Some(planet)) = (unit.system, unit.planet) else {
                return 0;
            };
            let Some(mark) = state.faction_marks.get(TEKKLAR_MARK) else {
                return 0;
            };
            let mut parts = mark.split('|');
            let (Some(mark_system), Some(mark_planet), Some(holder), Some(norr)) =
                (parts.next(), parts.next(), parts.next(), parts.next())
            else {
                return 0;
            };
            if mark_system != system.as_str() || mark_planet != planet.as_str() {
                return 0;
            }
            let mut shift = 0;
            if unit.player.as_str() == holder {
                shift += 1;
            }
            if !norr.is_empty() && unit.player.as_str() == norr {
                shift -= 1;
            }
            shift
        }
        _ => 0,
    }
}

// -- Valkyrie Particle Weave ---------------------------------------------------------------------

/// `vpw`: one additional hit when the opponent produced any this round.
#[allow(
    clippy::too_many_arguments,
    reason = "the shape of `GroundHooks::ground_rolls_extra_hits`"
)]
fn ground_rolls_extra_hits(
    state: &GameState,
    _content: &ContentStore,
    _sources: SourceSet,
    roller: &PlayerId,
    _system: &SystemId,
    _planet: &PlanetId,
    _own_hits: usize,
    opponent_hits: usize,
) -> usize {
    usize::from(has_technology(state, roller, "vpw") && opponent_hits >= 1)
}

// -- G'hom Sek'kus -------------------------------------------------------------------------------

/// Control 5 planets in non-home systems.
fn commander_unlocked(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    _galaxy: Option<&ti4_content::galaxy::Galaxy>,
    player: &PlayerId,
    leader: &LeaderId,
) -> Option<bool> {
    if leader.as_str() != "sardakkcommander" {
        return None;
    }
    let outside_homes = state
        .controlled_planets(player)
        .into_iter()
        .filter(|(system, _)| {
            !ti4_content::galaxy::is_home_system(content, system.as_str(), sources)
        })
        .count();
    Some(outside_homes >= 5)
}

/// The commander's extra commit sources: one ground force from each planet in the active system.
///
/// PARTIAL: the adjacent-systems half ("each planet in adjacent systems that do not contain 1 of
/// your command tokens") needs the map, which this hook is not given. See the hook request in the
/// evidence file.
fn commit_candidates(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    invader: &PlayerId,
    system: &SystemId,
    already: &[CommitOrigin],
) -> Vec<CommitCandidate> {
    if leader_status(state, invader, "sardakkcommander") != Some(LeaderStatus::Unlocked) {
        return Vec::new();
    }
    let types = catalogue(content, sources);
    let board = state.system_state(system);
    let mut found = Vec::new();
    for planet in board.planet_units.keys() {
        if already.iter().any(|(_, from)| from == planet) {
            continue; // "up to 1 ground force from each planet"
        }
        let mut seen: Vec<&Unit> = Vec::new();
        for unit in board.on_planet_of(planet, invader) {
            let ground = types
                .get(unit.type_id.as_str())
                .is_some_and(UnitType::is_ground_force);
            if ground && !seen.contains(&unit) {
                seen.push(unit);
                found.push(CommitCandidate {
                    system: system.clone(),
                    planet: planet.clone(),
                    unit: unit.clone(),
                });
            }
        }
    }
    found
}

// -- timing abilities ----------------------------------------------------------------------------

fn timing_abilities(_state: &GameState, owner_name: &str, seat: &PlayerId) -> Vec<Ability> {
    vec![
        exotrireme(owner_name, seat),
        mech_hit(owner_name, seat),
        tekklar_legion(owner_name, seat),
        tekklar_cleanup_ended(owner_name, seat),
        tekklar_cleanup_turn(owner_name, seat),
        tactical_mark(owner_name, seat),
        tactical_unmark(owner_name, seat, "TURN_BEGAN", "turn"),
        tactical_unmark(owner_name, seat, "STRATEGIC_ACTION_BEGAN", "strategic"),
        agent(owner_name, seat),
        supremacy(owner_name, seat, "SPACE_COMBAT_ENDED"),
        supremacy(owner_name, seat, "GROUND_COMBAT_ENDED"),
    ]
}

/// Exotrireme II can act now: the owner holds `exo2`, fought in this combat, has an Exotrireme II
/// in the system, and there is another ship to destroy.
fn exotrireme_ready(
    context: &crate::timing::TimingContext<'_>,
    owner: &PlayerId,
    system: &SystemId,
    fought: bool,
) -> bool {
    if !fought || !has_technology(context.state, owner, "exo2") {
        return false;
    }
    let ships = ships_in(context.state, context.content, context.sources, system);
    let mine = ships
        .iter()
        .any(|(who, ship)| who == owner && ship.type_id.as_str() == "sardakk_dreadnought2");
    mine && ships.len() >= 2
}

fn fought_in(event: &crate::event::Event, player: &PlayerId) -> bool {
    event.text("attacker") == Some(player.as_str())
        || event.text("defender") == Some(player.as_str())
}

/// Distinct values of a unit list, in first-seen order.
fn distinct(units: &[(PlayerId, Unit)]) -> Vec<(PlayerId, Unit)> {
    let mut out: Vec<(PlayerId, Unit)> = Vec::new();
    for entry in units {
        if !out.contains(entry) {
            out.push(entry.clone());
        }
    }
    out
}

fn ship_label(owner: &PlayerId, unit: &Unit) -> String {
    format!(
        "destroy {owner}'s {}{}",
        unit.type_id,
        if unit.sustained_damage {
            " (damaged)"
        } else {
            ""
        }
    )
}

/// Exotrireme II: "After a round of space combat, you may destroy this unit to destroy up to 2
/// ships in this system."
///
/// Requires another ship to destroy (a use that destroys only itself is not offered). Every choice
/// is asked before the first ship is removed.
#[allow(
    clippy::too_many_lines,
    reason = "one ability: every choice is asked before the first ship is removed"
)]
fn exotrireme(owner_name: &str, seat: &PlayerId) -> Ability {
    let owner = seat.clone();
    let condition_owner = seat.clone();
    Ability::stateful(
        format!("technology:{owner_name}:exo2:SPACE_COMBAT_ROUND_ENDED:after"),
        seat.clone(),
        "SPACE_COMBAT_ROUND_ENDED",
        Relation::After,
        Arc::new(move |event, _resolver, context| {
            let Some(system) = event.text("system").map(SystemId::new) else {
                return Ok(());
            };
            // The window's own "use this ability" question is the "may"; nothing is asked twice.
            if !exotrireme_ready(context, &owner, &system, fought_in(event, &owner)) {
                return Ok(());
            }
            // Which Exotrireme II: a damaged one and an undamaged one are different ships.
            let ships = ships_in(context.state, context.content, context.sources, &system);
            let own: Vec<(PlayerId, Unit)> = ships
                .iter()
                .filter(|(who, ship)| {
                    who == &owner && ship.type_id.as_str() == "sardakk_dreadnought2"
                })
                .cloned()
                .collect();
            let own = distinct(&own);
            let (_, dreadnought) = if let [only] = own.as_slice() {
                only.clone()
            } else {
                let options: Vec<ChoiceOption> = own
                    .iter()
                    .enumerate()
                    .map(|(index, (who, ship))| {
                        ChoiceOption::labelled(
                            format!("self|{index}"),
                            "ship",
                            ship_label(who, ship),
                        )
                    })
                    .collect();
                let choice = Choice::new(
                    owner.clone(),
                    "Exotrireme II: which one is destroyed".to_owned(),
                    options,
                )
                .contextualized(decision(
                    context,
                    &owner,
                    "exo2",
                    "exotrireme_self",
                ));
                let answer = context
                    .ask_seeing(&choice)
                    .map_err(TimingError::IllegalChoice)?;
                let Some(picked) = choice.options.iter().position(|o| o.id == answer.id) else {
                    return Ok(());
                };
                own[picked].clone()
            };
            // Up to 2 other ships.
            let mut pool = ships;
            if let Some(index) = pool
                .iter()
                .position(|(who, ship)| who == &owner && ship == &dreadnought)
            {
                pool.remove(index);
            }
            let mut victims: Vec<(PlayerId, Unit)> = Vec::new();
            for round in 0..2 {
                if pool.is_empty() {
                    break;
                }
                let candidates = distinct(&pool);
                let mut options: Vec<ChoiceOption> = candidates
                    .iter()
                    .enumerate()
                    .map(|(index, (who, ship))| {
                        ChoiceOption::labelled(
                            format!("ship|{index}"),
                            "ship",
                            ship_label(who, ship),
                        )
                        .with("player", who.to_string())
                    })
                    .collect();
                if round > 0 {
                    options.push(ChoiceOption::decline());
                }
                let choice = Choice::new(
                    owner.clone(),
                    format!("Exotrireme II: destroy ship {} of up to 2", round + 1),
                    options,
                )
                .contextualized(decision(
                    context,
                    &owner,
                    "exo2",
                    "exotrireme_victim",
                ));
                let answer = context
                    .ask_seeing(&choice)
                    .map_err(TimingError::IllegalChoice)?;
                if answer.is_decline() {
                    break;
                }
                let Some(picked) = choice.options.iter().position(|o| o.id == answer.id) else {
                    return Ok(());
                };
                let chosen = candidates[picked].clone();
                if let Some(index) = pool.iter().position(|entry| entry == &chosen) {
                    pool.remove(index);
                }
                victims.push(chosen);
            }
            // Everything is decided: destroy itself first, then the chosen ships by owner.
            let removed = crate::combat::destroy_units(
                context.state,
                context.content,
                context.sources,
                &owner,
                &system,
                std::slice::from_ref(&dreadnought),
            );
            if removed == 0 {
                return Ok(());
            }
            let mut owners: Vec<PlayerId> = Vec::new();
            for (who, _) in &victims {
                if !owners.contains(who) {
                    owners.push(who.clone());
                }
            }
            for who in owners {
                let units: Vec<Unit> = victims
                    .iter()
                    .filter(|(victim_owner, _)| victim_owner == &who)
                    .map(|(_, unit)| unit.clone())
                    .collect();
                crate::combat::destroy_units(
                    context.state,
                    context.content,
                    context.sources,
                    &who,
                    &system,
                    &units,
                );
            }
            Ok(())
        }),
    )
    .with_optional(true)
    .with_stateful_condition(Arc::new(move |event, _, context| {
        event.text("system").is_some_and(|system| {
            exotrireme_ready(
                context,
                &condition_owner,
                &SystemId::new(system),
                fought_in(event, &condition_owner),
            )
        })
    }))
}

/// Valkyrie Exoskeleton: "After this unit uses it's SUSTAIN DAMAGE ability during ground combat,
/// it produces 1 hit against your opponent's ground forces on this planet."
///
/// Mandatory; the hit is an ordinary ground hit (the opponent sustains first, else loses the
/// cheapest ground force) assigned after the round's simultaneous removals.
fn mech_hit(owner_name: &str, seat: &PlayerId) -> Ability {
    let owner = seat.clone();
    let condition_owner = seat.clone();
    Ability::stateful(
        format!("unit:{owner_name}:sardakk_mech:GROUND_FORCE_SUSTAINED:after"),
        seat.clone(),
        "GROUND_FORCE_SUSTAINED",
        Relation::After,
        Arc::new(move |event, resolver, context| {
            let (Some(system), Some(planet)) = (
                event.text("system").map(SystemId::new),
                event.text("planet").map(PlanetId::new),
            ) else {
                return Ok(());
            };
            let Some(opponent) = ground_force_owners(
                context.state,
                context.content,
                context.sources,
                &system,
                &planet,
            )
            .into_iter()
            .find(|other| other != &owner) else {
                return Ok(()); // nothing left to hit
            };
            crate::invasion::assign_ground_hits_in_timing(
                resolver,
                context,
                &system,
                &planet,
                &opponent,
                1,
                "ground_combat",
            )?;
            Ok(())
        }),
    )
    .with_stateful_condition(Arc::new(move |event, _, _context| {
        event.text("player") == Some(condition_owner.as_str())
            && event.text("unit") == Some("sardakk_mech")
            && event.text("cause") == Some("ground_combat")
    }))
}

/// Tekklar Legion, played by whoever holds it into a ground combat they fight in.
fn tekklar_legion(owner_name: &str, seat: &PlayerId) -> Ability {
    let owner = seat.clone();
    let condition_owner = seat.clone();
    Ability::stateful(
        format!("promissory:{owner_name}:tekklar:GROUND_COMBAT_STARTED:after"),
        seat.clone(),
        "GROUND_COMBAT_STARTED",
        Relation::After,
        Arc::new(move |event, _resolver, context| {
            let (Some(system), Some(planet)) = (event.text("system"), event.text("planet")) else {
                return Ok(());
            };
            let opponent = if event.text("attacker") == Some(owner.as_str()) {
                event.text("defender")
            } else {
                event.text("attacker")
            }
            .unwrap_or_default()
            .to_owned();
            if crate::promissory::held_foreign(context.state, &owner, "tekklar") == 0 {
                return Ok(());
            }
            let norr = context
                .state
                .player(&PlayerId::new(opponent.as_str()))
                .filter(|seat| seat.faction.as_str() == FACTION)
                .map(|_| opponent)
                .unwrap_or_default();
            context.state.faction_marks.insert(
                TEKKLAR_MARK.to_owned(),
                format!("{system}|{planet}|{owner}|{norr}"),
            );
            crate::promissory::give_back(context.state, TEKKLAR_NOTE);
            Ok(())
        }),
    )
    .with_optional(true)
    .with_stateful_condition(Arc::new(move |event, _, context| {
        fought_in(event, &condition_owner)
            && crate::promissory::held_foreign(context.state, &condition_owner, "tekklar") > 0
    }))
}

/// Whether this seat is the one that owns the Tekklar Legion note (the N'orr player's seat). The
/// bookkeeping abilities belong to that one seat, so a game with no N'orr player never offers them
/// and a game with one offers them once, not once per seat.
fn is_norr_seat(state: &GameState, seat: &PlayerId) -> bool {
    state
        .player(seat)
        .is_some_and(|player| player.faction.as_str() == FACTION)
}

/// The Tekklar Legion bonus ends with the combat it was played into.
fn tekklar_cleanup_ended(owner_name: &str, seat: &PlayerId) -> Ability {
    let condition_seat = seat.clone();
    Ability::stateful(
        format!("promissory:{owner_name}:tekklar_end:GROUND_COMBAT_ENDED:after"),
        seat.clone(),
        "GROUND_COMBAT_ENDED",
        Relation::After,
        Arc::new(|event, _resolver, context| {
            let (Some(system), Some(planet)) = (event.text("system"), event.text("planet")) else {
                return Ok(());
            };
            let here = format!("{system}|{planet}|");
            if context
                .state
                .faction_marks
                .get(TEKKLAR_MARK)
                .is_some_and(|mark| mark.starts_with(&here))
            {
                context.state.faction_marks.remove(TEKKLAR_MARK);
            }
            Ok(())
        }),
    )
    .with_stateful_condition(Arc::new(move |_event, _, context| {
        is_norr_seat(context.state, &condition_seat)
            && context.state.faction_marks.contains_key(TEKKLAR_MARK)
    }))
}

/// A combat that never reached its end (a side wiped out by a reaction) must not leak the bonus
/// into a later turn.
fn tekklar_cleanup_turn(owner_name: &str, seat: &PlayerId) -> Ability {
    let condition_seat = seat.clone();
    Ability::stateful(
        format!("promissory:{owner_name}:tekklar_turn:TURN_BEGAN:after"),
        seat.clone(),
        "TURN_BEGAN",
        Relation::After,
        Arc::new(|_event, _resolver, context| {
            context.state.faction_marks.remove(TEKKLAR_MARK);
            Ok(())
        }),
    )
    .with_stateful_condition(Arc::new(move |_event, _, context| {
        is_norr_seat(context.state, &condition_seat)
            && context.state.faction_marks.contains_key(TEKKLAR_MARK)
    }))
}

// -- T'ro ----------------------------------------------------------------------------------------

fn tactical_key(player: &PlayerId) -> String {
    format!("sardakk:agent:tactical:{player}")
}

/// Whether this seat holds T'ro ready, the only state in which remembering a tactical action
/// matters.
fn agent_ready(state: &GameState, seat: &PlayerId) -> bool {
    leader_status(state, seat, "sardakkagent") == Some(LeaderStatus::Readied)
}

/// `ACTION_COMPLETED` names only the player and fires after the active system is cleared, so the
/// agent's "end of a tactical action" is remembered from the activation: the system the acting
/// player activated this turn. Offered only to a seat holding T'ro ready.
fn tactical_mark(owner_name: &str, seat: &PlayerId) -> Ability {
    let condition_seat = seat.clone();
    Ability::stateful(
        format!("leader:{owner_name}:sardakkagent_mark:SYSTEM_ACTIVATED:after"),
        seat.clone(),
        "SYSTEM_ACTIVATED",
        Relation::After,
        Arc::new(move |event, _resolver, context| {
            if let (Some(actor), Some(system)) = (event.text("player"), event.text("system")) {
                context
                    .state
                    .faction_marks
                    .insert(tactical_key(&PlayerId::new(actor)), system.to_owned());
            }
            Ok(())
        }),
    )
    .with_stateful_condition(Arc::new(move |_event, _, context| {
        agent_ready(context.state, &condition_seat)
    }))
}

/// Forget an activation that was never followed by `ACTION_COMPLETED` (the turn ended another way)
/// or that belongs to a strategic action. Offered only while a mark for the event's player exists.
fn tactical_unmark(
    owner_name: &str,
    seat: &PlayerId,
    event_type: &'static str,
    label: &'static str,
) -> Ability {
    let condition_seat = seat.clone();
    Ability::stateful(
        format!("leader:{owner_name}:sardakkagent_{label}:{event_type}:after"),
        seat.clone(),
        event_type,
        Relation::After,
        Arc::new(move |event, _resolver, context| {
            if let Some(actor) = event.text("player") {
                context
                    .state
                    .faction_marks
                    .remove(&tactical_key(&PlayerId::new(actor)));
            }
            Ok(())
        }),
    )
    .with_stateful_condition(Arc::new(move |event, _, context| {
        leader_status(context.state, &condition_seat, "sardakkagent").is_some()
            && event.text("player").is_some_and(|actor| {
                context
                    .state
                    .faction_marks
                    .contains_key(&tactical_key(&PlayerId::new(actor)))
            })
    }))
}

/// The system and acting player of a tactical action that just ended, if T'ro can act on it.
fn agent_window(
    context: &crate::timing::TimingContext<'_>,
    owner: &PlayerId,
    actor: &PlayerId,
) -> Option<SystemId> {
    if !agent_ready(context.state, owner) {
        return None;
    }
    let system = SystemId::new(context.state.faction_marks.get(&tactical_key(actor))?);
    let spots = crate::action_cards::placement_spots(
        context.state,
        context.content,
        context.sources,
        actor,
        crate::action_cards::PlacementTarget::ControlledPlanet,
        Some(&system),
    );
    (!spots.is_empty()).then_some(system)
}

/// T'ro: "At the end of a player's tactical action: You may exhaust this card: if you do, that
/// player may place 2 infantry from their reinforcements on a planet they control in the active
/// system."
fn agent(owner_name: &str, seat: &PlayerId) -> Ability {
    let owner = seat.clone();
    let condition_owner = seat.clone();
    Ability::stateful(
        format!("leader:{owner_name}:sardakkagent:ACTION_COMPLETED:after"),
        seat.clone(),
        "ACTION_COMPLETED",
        Relation::After,
        Arc::new(move |event, _resolver, context| {
            let Some(actor) = event.text("player").map(PlayerId::new) else {
                return Ok(());
            };
            let Some(system) = agent_window(context, &owner, &actor) else {
                return Ok(());
            };
            if !crate::leaders::exhaust(context.state, &owner, &LeaderId::new("sardakkagent")) {
                return Ok(());
            }
            context.state.faction_marks.remove(&tactical_key(&actor));
            crate::action_cards::place_units_choosing(
                context,
                &actor,
                "infantry",
                2,
                crate::action_cards::PlacementTarget::ControlledPlanet,
                Some(&system),
                true,
                "sardakkagent",
                crate::action_cards::PlacementLimits::Respect,
            )
            .map_err(TimingError::IllegalChoice)?;
            Ok(())
        }),
    )
    .with_optional(true)
    .with_stateful_condition(Arc::new(move |event, _, context| {
        event
            .text("player")
            .map(PlayerId::new)
            .is_some_and(|actor| agent_window(context, &condition_owner, &actor).is_some())
    }))
}

// -- N'orr Supremacy -----------------------------------------------------------------------------

fn supremacy_options(
    context: &crate::timing::TimingContext<'_>,
    player: &PlayerId,
) -> Vec<ChoiceOption> {
    let mut options = vec![ChoiceOption::labelled(
        "token".to_owned(),
        "breakthrough",
        "gain 1 command token".to_owned(),
    )];
    for alias in
        crate::technology::researchable(context.state, context.content, context.sources, player)
    {
        if crate::technology::is_unit_upgrade(context.content, &alias) {
            options.push(ChoiceOption::labelled(
                format!("research|{alias}"),
                "breakthrough",
                format!(
                    "research {}",
                    crate::technology::name(context.content, &alias)
                ),
            ));
        }
    }
    options
}

/// N'orr Supremacy: "After you win a combat, either gain 1 command token or research a unit
/// upgrade technology."
fn supremacy(owner_name: &str, seat: &PlayerId, event_type: &'static str) -> Ability {
    let owner = seat.clone();
    let condition_owner = seat.clone();
    Ability::stateful(
        format!("breakthrough:{owner_name}:sardakkbt:{event_type}:after"),
        seat.clone(),
        event_type,
        Relation::After,
        Arc::new(move |event, _resolver, context| {
            if event.text("winner") != Some(owner.as_str())
                || !crate::breakthroughs::holds(context.state, &owner, "sardakkbt")
            {
                return Ok(());
            }
            let options = supremacy_options(context, &owner);
            let choice = Choice::new(
                owner.clone(),
                "N'orr Supremacy: gain a command token or research a unit upgrade".to_owned(),
                options,
            )
            .contextualized(decision(context, &owner, "sardakkbt", "norr_supremacy"));
            let answer = context
                .ask_seeing(&choice)
                .map_err(TimingError::IllegalChoice)?;
            if answer.id == "token" {
                crate::strategy_cards::gain_tokens(
                    context.state,
                    context.content,
                    context.sources,
                    context.galaxy,
                    context.table,
                    &owner,
                    1,
                )
                .map_err(TimingError::IllegalChoice)?;
            } else if let Some(alias) = answer.id.strip_prefix("research|") {
                crate::technology::research(
                    context.state,
                    context.content,
                    context.sources,
                    &owner,
                    &TechnologyId::new(alias),
                );
            }
            Ok(())
        }),
    )
    .with_optional(true)
    .with_stateful_condition(Arc::new(move |event, _, context| {
        event.text("winner") == Some(condition_owner.as_str())
            && crate::breakthroughs::holds(context.state, &condition_owner, "sardakkbt")
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use ti4_model::content_types::DEFAULT;

    fn a() -> PlayerId {
        PlayerId::new("a")
    }
    fn b() -> PlayerId {
        PlayerId::new("b")
    }

    const EXO: &str = "technology:sardakk:exo2:SPACE_COMBAT_ROUND_ENDED:after";

    fn game() -> GameState {
        crate::fixtures::seated_game(&[("a", FACTION), ("b", "sol")], DEFAULT)
    }

    fn payload(pairs: &[(&str, &str)]) -> BTreeMap<String, serde_json::Value> {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_owned(), serde_json::Value::from(*value)))
            .collect()
    }

    /// Emit one typed event through a resolver armed exactly as the game arms it.
    fn emit(
        state: &mut GameState,
        table: &mut crate::choice::Table,
        event_type: &str,
        pairs: &[(&str, &str)],
    ) {
        let mut resolver = crate::fixtures::armed_resolver(state);
        crate::fixtures::with_context(state, DEFAULT, None, table, |ctx| {
            let event = ctx
                .event_sequence
                .next(event_type, payload(pairs))
                .expect("an event id");
            resolver
                .emit_with_context(ctx, event, |_, _| {})
                .expect("the window resolves");
        });
    }

    fn scripted(answers: &[&str]) -> crate::choice::Table {
        crate::choice::Table::with_default(Box::new(crate::choice::Scripted::new(
            answers.iter().map(|answer| (*answer).to_owned()),
        )))
    }

    fn count(state: &GameState, system: &SystemId, owner: &PlayerId, kind: &str) -> usize {
        state
            .system_state(system)
            .units
            .iter()
            .filter(|unit| &unit.owner == owner && unit.type_id.as_str() == kind)
            .count()
    }

    fn on_planet(
        state: &GameState,
        system: &SystemId,
        planet: &PlanetId,
        owner: &PlayerId,
        kind: &str,
    ) -> usize {
        state
            .system_state(system)
            .on_planet_of(planet, owner)
            .into_iter()
            .filter(|unit| unit.type_id.as_str() == kind)
            .count()
    }

    fn home_of(state: &GameState, who: &PlayerId) -> SystemId {
        state.player(who).unwrap().home_system.clone().unwrap()
    }

    fn unit(kind: &str, who: &PlayerId) -> Unit {
        Unit::new(ti4_model::id::UnitTypeId::new(kind), who.clone())
    }

    // -- unrelenting (shared code) ---------------------------------------------------------------

    #[test]
    fn unrelenting_adds_one_to_every_roll_through_the_shared_path() {
        let state = game();
        let content = ContentStore::embedded();
        for context in ["space", "ground"] {
            assert_eq!(
                crate::faction_abilities::combat_modifier(&state, content, &a(), context),
                1,
                "{context}"
            );
            assert_eq!(
                crate::faction_abilities::combat_modifier(&state, content, &b(), context),
                0,
                "only the N'orr player: {context}"
            );
        }
        // And it is what a unit actually rolls against: +1 to the roll is a threshold one lower.
        let theirs = crate::combat::effective_hits_on(
            &state,
            content,
            DEFAULT,
            &a(),
            &unit("cruiser", &a()),
        );
        let sols = crate::combat::effective_hits_on(
            &state,
            content,
            DEFAULT,
            &b(),
            &unit("cruiser", &b()),
        );
        assert_eq!(sols.unwrap() - theirs.unwrap(), 1);
    }

    // -- Exotrireme I and II: statistics ---------------------------------------------------------

    #[test]
    fn the_exotrireme_upgrade_applies_its_data_driven_statistics() {
        let mut state = game();
        let content = ContentStore::embedded();
        let home = home_of(&state, &a());
        crate::fixtures::put(&mut state, &home, "sardakk_dreadnought", &a(), 1);
        let types = catalogue(content, DEFAULT);
        let one = types.get("sardakk_dreadnought").expect("Exotrireme I");
        assert_eq!(
            (
                one.move_value(),
                one.combat_hits_on(),
                one.combat_dice(),
                one.can_be_direct_hit()
            ),
            (1, Some(5), 1, true)
        );
        assert_eq!(one.bombard_hits_on(), Some(4));
        assert!(one.sustain_damage());

        crate::technology::grant(&mut state, &a(), &TechnologyId::new("exo2"));
        crate::technology::apply_unit_upgrades(&mut state, content, DEFAULT, &a());
        assert_eq!(
            count(&state, &home, &a(), "sardakk_dreadnought2"),
            1,
            "the board is upgraded"
        );
        assert_eq!(count(&state, &home, &a(), "sardakk_dreadnought"), 0);
        let two = types.get("sardakk_dreadnought2").expect("Exotrireme II");
        assert_eq!(
            (
                two.move_value(),
                two.combat_hits_on(),
                two.combat_dice(),
                two.can_be_direct_hit()
            ),
            (2, Some(5), 1, false),
            "moves 2 and Direct Hit cannot destroy it"
        );
        assert_eq!(two.bombard_hits_on(), Some(4));
        assert!(two.sustain_damage());
        // The printed flag is what the Direct Hit window reads.
        assert!(!crate::combat::direct_hittable_at(
            &state,
            content,
            DEFAULT,
            &a(),
            &home,
            "sardakk_dreadnought2"
        ));
        assert!(crate::combat::direct_hittable_at(
            &state,
            content,
            DEFAULT,
            &a(),
            &home,
            "sardakk_dreadnought"
        ));
    }

    // -- C'morran N'orr --------------------------------------------------------------------------

    #[test]
    fn the_flagship_helps_only_its_owners_other_ships_in_its_system() {
        let mut state = game();
        let content = ContentStore::embedded();
        let home = home_of(&state, &a());
        let elsewhere = SystemId::new("18");
        state.active_system = Some(home.clone());
        let hits = |state: &GameState, who: &PlayerId, kind: &str| {
            crate::combat::effective_hits_on(state, content, DEFAULT, who, &unit(kind, who))
                .expect("it fights")
        };
        state.system_mut(&home).units.clear();
        let without = hits(&state, &a(), "cruiser");
        let flagship_without = hits(&state, &a(), "sardakk_flagship");

        crate::fixtures::put(&mut state, &home, "sardakk_flagship", &a(), 1);
        assert_eq!(
            without - hits(&state, &a(), "cruiser"),
            1,
            "+1 to the roll is one less to hit"
        );
        assert_eq!(
            hits(&state, &a(), "sardakk_flagship"),
            flagship_without,
            "not its own roll"
        );
        let theirs = hits(&state, &b(), "cruiser");
        crate::fixtures::put(&mut state, &home, "cruiser", &b(), 1);
        assert_eq!(
            hits(&state, &b(), "cruiser"),
            theirs,
            "an opponent gets nothing"
        );

        // A flagship elsewhere does not help a fleet in the active system.
        state.system_mut(&home).units.clear();
        state.system_mut(&elsewhere).units.clear();
        crate::fixtures::put(&mut state, &elsewhere, "sardakk_flagship", &a(), 1);
        assert_eq!(
            hits(&state, &a(), "cruiser"),
            without,
            "only in its own system"
        );
    }

    // -- Valkyrie Particle Weave -----------------------------------------------------------------

    #[test]
    fn particle_weave_adds_a_hit_only_when_the_opponent_produced_one() {
        let mut state = game();
        let (system, planet) = crate::fixtures::a_placed_planet();
        let extra = |state: &GameState, who: &PlayerId, own: usize, theirs: usize| {
            crate::factions::hooks_ground::ground_rolls_extra_hits(
                state,
                ContentStore::embedded(),
                DEFAULT,
                who,
                &system,
                &planet,
                own,
                theirs,
            )
        };
        assert_eq!(extra(&state, &a(), 0, 3), 0, "not researched yet");
        crate::technology::grant(&mut state, &a(), &TechnologyId::new("vpw"));
        assert_eq!(extra(&state, &a(), 0, 1), 1, "even with no hits of its own");
        assert_eq!(extra(&state, &a(), 4, 2), 1, "one more, not one per hit");
        assert_eq!(extra(&state, &a(), 3, 0), 0, "the opponent produced none");
        assert_eq!(extra(&state, &b(), 0, 2), 0, "only the holder");
    }

    // -- Valkyrie Exoskeleton --------------------------------------------------------------------

    fn mech_arena() -> (GameState, SystemId, PlanetId) {
        let mut state = game();
        let (system, planet) = crate::fixtures::a_placed_planet();
        crate::fixtures::put_on_planet(&mut state, &system, &planet, "sardakk_mech", &a(), 1);
        crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &b(), 2);
        (state, system, planet)
    }

    fn run_sustained(
        state: &mut GameState,
        system: &SystemId,
        planet: &PlanetId,
        who: &str,
        kind: &str,
        cause: &str,
    ) {
        emit(
            state,
            &mut crate::choice::Table::default(),
            "GROUND_FORCE_SUSTAINED",
            &[
                ("system", system.as_str()),
                ("planet", planet.as_str()),
                ("player", who),
                ("unit", kind),
                ("cause", cause),
            ],
        );
    }

    #[test]
    fn the_mech_produces_a_hit_after_it_sustains_in_ground_combat() {
        let (mut state, system, planet) = mech_arena();
        run_sustained(
            &mut state,
            &system,
            &planet,
            "a",
            "sardakk_mech",
            "ground_combat",
        );
        assert_eq!(
            on_planet(&state, &system, &planet, &b(), "infantry"),
            1,
            "one hit, one infantry"
        );
        assert_eq!(on_planet(&state, &system, &planet, &a(), "sardakk_mech"), 1);
    }

    #[test]
    fn the_mech_hit_needs_its_own_sustain_during_ground_combat() {
        for (who, kind, cause) in [
            ("a", "sardakk_mech", "harrow"),
            ("a", "sardakk_mech", "bombardment"),
            ("a", "sardakk_mech", "space_cannon_defense"),
            ("a", "infantry", "ground_combat"),
            ("b", "sol_mech", "ground_combat"),
        ] {
            let (mut state, system, planet) = mech_arena();
            run_sustained(&mut state, &system, &planet, who, kind, cause);
            assert_eq!(
                on_planet(&state, &system, &planet, &b(), "infantry"),
                2,
                "{who} {kind} {cause}"
            );
        }
    }

    #[test]
    fn the_mech_hit_is_wasted_when_nothing_of_the_opponent_is_left() {
        let (mut state, system, planet) = mech_arena();
        state
            .system_mut(&system)
            .planet_units
            .get_mut(&planet)
            .unwrap()
            .retain(|u| u.owner == a());
        run_sustained(
            &mut state,
            &system,
            &planet,
            "a",
            "sardakk_mech",
            "ground_combat",
        );
        assert_eq!(on_planet(&state, &system, &planet, &a(), "sardakk_mech"), 1);
    }

    // -- Exotrireme II: the destroy ability ------------------------------------------------------

    fn exo_arena() -> (GameState, SystemId) {
        let mut state = game();
        let home = home_of(&state, &a());
        state.system_mut(&home).units.clear();
        crate::fixtures::put(&mut state, &home, "sardakk_dreadnought2", &a(), 1);
        crate::fixtures::put(&mut state, &home, "cruiser", &b(), 3);
        crate::technology::grant(&mut state, &a(), &TechnologyId::new("exo2"));
        (state, home)
    }

    fn round_ended(state: &mut GameState, table: &mut crate::choice::Table, system: &SystemId) {
        emit(
            state,
            table,
            "SPACE_COMBAT_ROUND_ENDED",
            &[
                ("system", system.as_str()),
                ("round", "1"),
                ("attacker", "a"),
                ("defender", "b"),
            ],
        );
    }

    #[test]
    fn exotrireme_destroys_itself_and_up_to_two_chosen_ships() {
        let (mut state, home) = exo_arena();
        let mut table = scripted(&[EXO, "ship|0", "ship|0"]);
        round_ended(&mut state, &mut table, &home);
        assert_eq!(
            count(&state, &home, &a(), "sardakk_dreadnought2"),
            0,
            "it destroyed itself"
        );
        assert_eq!(
            count(&state, &home, &b(), "cruiser"),
            1,
            "and two of the three cruisers"
        );
        assert_eq!(
            state.pending_destructions.len(),
            3,
            "each destruction is staged for announcement"
        );
    }

    #[test]
    fn exotrireme_may_stop_after_one_ship_or_decline_entirely() {
        let (mut state, home) = exo_arena();
        round_ended(
            &mut state,
            &mut scripted(&[EXO, "ship|0", "decline"]),
            &home,
        );
        assert_eq!(count(&state, &home, &b(), "cruiser"), 2);
        assert_eq!(count(&state, &home, &a(), "sardakk_dreadnought2"), 0);

        let (mut state, home) = exo_arena();
        round_ended(&mut state, &mut scripted(&["decline"]), &home);
        assert_eq!(
            count(&state, &home, &b(), "cruiser"),
            3,
            "declined: nothing changes"
        );
        assert_eq!(count(&state, &home, &a(), "sardakk_dreadnought2"), 1);
        assert!(state.pending_destructions.is_empty());
    }

    #[test]
    fn exotrireme_is_not_offered_without_the_card_the_ship_or_a_target() {
        let (mut state, home) = exo_arena();
        state
            .player_mut(&a())
            .unwrap()
            .technologies
            .remove(&TechnologyId::new("exo2"));
        round_ended(&mut state, &mut scripted(&[EXO, "ship|0", "ship|0"]), &home);
        assert_eq!(count(&state, &home, &b(), "cruiser"), 3, "no exo2");

        let (mut state, home) = exo_arena();
        state.system_mut(&home).units.retain(|u| u.owner != a());
        crate::fixtures::put(&mut state, &home, "sardakk_dreadnought", &a(), 1);
        round_ended(&mut state, &mut scripted(&[EXO, "ship|0", "ship|0"]), &home);
        assert_eq!(count(&state, &home, &b(), "cruiser"), 3, "Exotrireme I");

        let (mut state, home) = exo_arena();
        state.system_mut(&home).units.retain(|u| u.owner == a());
        round_ended(&mut state, &mut scripted(&[EXO]), &home);
        assert_eq!(
            count(&state, &home, &a(), "sardakk_dreadnought2"),
            1,
            "no target, not offered"
        );
    }

    // -- Tekklar Legion --------------------------------------------------------------------------

    const TEKKLAR: &str = "promissory:sol:tekklar:GROUND_COMBAT_STARTED:after";

    fn lend_tekklar(state: &mut GameState) {
        state.promissory_notes.insert(TEKKLAR_NOTE.to_owned(), b());
    }

    fn started(
        state: &mut GameState,
        table: &mut crate::choice::Table,
        system: &SystemId,
        planet: &PlanetId,
    ) {
        emit(
            state,
            table,
            "GROUND_COMBAT_STARTED",
            &[
                ("system", system.as_str()),
                ("planet", planet.as_str()),
                ("attacker", "b"),
                ("defender", "a"),
            ],
        );
    }

    fn roll_shift(state: &GameState, who: &PlayerId, system: &SystemId, planet: &PlanetId) -> i64 {
        // The module hook alone: `unrelenting` is a shared-code shift, not part of this sum.
        crate::factions::unit_roll_modifier(
            state,
            ContentStore::embedded(),
            DEFAULT,
            &CombatUnit {
                player: who,
                system: Some(system),
                planet: Some(planet),
                unit_type: "infantry",
                context: "ground",
            },
        )
    }

    #[test]
    fn tekklar_legion_helps_the_holder_hinders_the_norr_player_and_goes_home() {
        let mut state = game();
        let (system, planet) = crate::fixtures::a_placed_planet();
        lend_tekklar(&mut state);
        assert_eq!(
            roll_shift(&state, &b(), &system, &planet),
            0,
            "nothing before it is played"
        );

        started(&mut state, &mut scripted(&[TEKKLAR]), &system, &planet);
        assert_eq!(
            roll_shift(&state, &b(), &system, &planet),
            1,
            "+1 to the holder's units"
        );
        assert_eq!(
            roll_shift(&state, &a(), &system, &planet),
            -1,
            "-1 to the N'orr player's"
        );
        let other = PlanetId::new("elsewhere");
        assert_eq!(
            roll_shift(&state, &b(), &system, &other),
            0,
            "only this combat"
        );
        assert_eq!(
            state.promissory_notes.get(TEKKLAR_NOTE),
            Some(&a()),
            "returned to the N'orr player"
        );

        emit(
            &mut state,
            &mut crate::choice::Table::default(),
            "GROUND_COMBAT_ENDED",
            &[
                ("system", system.as_str()),
                ("planet", planet.as_str()),
                ("attacker", "b"),
                ("defender", "a"),
            ],
        );
        assert_eq!(
            roll_shift(&state, &b(), &system, &planet),
            0,
            "it ends with the combat"
        );
        assert_eq!(roll_shift(&state, &a(), &system, &planet), 0);
    }

    #[test]
    fn tekklar_legion_may_be_declined_and_is_not_the_owners_to_play() {
        let (system, planet) = crate::fixtures::a_placed_planet();
        let mut state = game();
        lend_tekklar(&mut state);
        started(&mut state, &mut scripted(&["decline"]), &system, &planet);
        assert_eq!(roll_shift(&state, &b(), &system, &planet), 0, "declined");
        assert_eq!(
            state.promissory_notes.get(TEKKLAR_NOTE),
            Some(&b()),
            "and kept"
        );

        // The N'orr player holds their own copy: it is not a note in play.
        let mut state = game();
        started(&mut state, &mut scripted(&[TEKKLAR]), &system, &planet);
        assert_eq!(roll_shift(&state, &b(), &system, &planet), 0);
        assert_eq!(roll_shift(&state, &a(), &system, &planet), 0);

        // A holder who is not in the combat cannot play it.
        let mut state =
            crate::fixtures::seated_game(&[("a", FACTION), ("b", "sol"), ("c", "hacan")], DEFAULT);
        state
            .promissory_notes
            .insert(TEKKLAR_NOTE.to_owned(), PlayerId::new("c"));
        started(&mut state, &mut scripted(&[TEKKLAR]), &system, &planet);
        assert_eq!(roll_shift(&state, &PlayerId::new("c"), &system, &planet), 0);
        assert_eq!(
            state.promissory_notes.get(TEKKLAR_NOTE),
            Some(&PlayerId::new("c"))
        );
    }

    #[test]
    fn tekklar_legion_does_not_hinder_a_player_who_is_not_the_norr_player() {
        let (system, planet) = crate::fixtures::a_placed_planet();
        let mut state =
            crate::fixtures::seated_game(&[("a", FACTION), ("b", "sol"), ("c", "hacan")], DEFAULT);
        state.promissory_notes.insert(TEKKLAR_NOTE.to_owned(), b());
        // b attacks c: the card is b's, but c is not the N'orr player.
        emit(
            &mut state,
            &mut scripted(&[TEKKLAR]),
            "GROUND_COMBAT_STARTED",
            &[
                ("system", system.as_str()),
                ("planet", planet.as_str()),
                ("attacker", "b"),
                ("defender", "c"),
            ],
        );
        assert_eq!(roll_shift(&state, &b(), &system, &planet), 1);
        assert_eq!(roll_shift(&state, &PlayerId::new("c"), &system, &planet), 0);
    }

    // -- T'ro ------------------------------------------------------------------------------------

    const AGENT: &str = "leader:sardakk:sardakkagent:ACTION_COMPLETED:after";

    fn activate(state: &mut GameState, who: &str, system: &SystemId) {
        emit(
            state,
            &mut crate::choice::Table::default(),
            "SYSTEM_ACTIVATED",
            &[("player", who), ("system", system.as_str())],
        );
    }

    fn completed(state: &mut GameState, table: &mut crate::choice::Table, who: &str) {
        emit(state, table, "ACTION_COMPLETED", &[("player", who)]);
    }

    fn sol_planet(state: &GameState) -> (SystemId, PlanetId) {
        let home = home_of(state, &b());
        let planet = state
            .controlled_planets(&b())
            .into_iter()
            .find(|(system, _)| **system == home)
            .map(|(_, planet)| planet.clone())
            .expect("Sol controls a planet at home");
        (home, planet)
    }

    fn infantry_on(state: &GameState, system: &SystemId, planet: &PlanetId) -> usize {
        // Sol's infantry is its own unit; count by base type.
        let types = catalogue(ContentStore::embedded(), DEFAULT);
        state
            .system_state(system)
            .on_planet_of(planet, &b())
            .into_iter()
            .filter(|unit| {
                types
                    .get(unit.type_id.as_str())
                    .is_some_and(|kind| kind.base_type() == "infantry")
            })
            .count()
    }

    #[test]
    fn the_agent_lets_the_active_player_place_two_infantry_after_a_tactical_action() {
        let mut state = game();
        let (home, planet) = sol_planet(&state);
        let before = infantry_on(&state, &home, &planet);
        assert_eq!(
            leader_status(&state, &a(), "sardakkagent"),
            Some(LeaderStatus::Readied)
        );
        activate(&mut state, "b", &home);
        let spot = format!("{home}|{planet}");
        completed(&mut state, &mut scripted(&[AGENT, &spot]), "b");
        assert_eq!(infantry_on(&state, &home, &planet), before + 2);
        assert_eq!(
            leader_status(&state, &a(), "sardakkagent"),
            Some(LeaderStatus::Exhausted)
        );
    }

    #[test]
    fn the_agent_is_the_actives_choice_to_use_and_the_owner_to_exhaust() {
        // The owner declines: nothing is placed and the agent stays ready.
        let mut state = game();
        let (home, planet) = sol_planet(&state);
        let before = infantry_on(&state, &home, &planet);
        activate(&mut state, "b", &home);
        completed(&mut state, &mut scripted(&["decline"]), "b");
        assert_eq!(infantry_on(&state, &home, &planet), before);
        assert_eq!(
            leader_status(&state, &a(), "sardakkagent"),
            Some(LeaderStatus::Readied)
        );

        // The active player declines the placement: the agent was still used (they "may" place).
        let mut state = game();
        activate(&mut state, "b", &home);
        completed(&mut state, &mut scripted(&[AGENT, "decline"]), "b");
        assert_eq!(infantry_on(&state, &home, &planet), before);
        assert_eq!(
            leader_status(&state, &a(), "sardakkagent"),
            Some(LeaderStatus::Exhausted)
        );
    }

    #[test]
    fn the_agent_needs_a_tactical_action_a_readied_card_and_a_planet_to_place_on() {
        let spot = |state: &GameState| {
            let (home, planet) = sol_planet(state);
            format!("{home}|{planet}")
        };
        // A strategic action (no activation this turn), or one that began after the activation.
        let mut state = game();
        let (home, planet) = sol_planet(&state);
        let before = infantry_on(&state, &home, &planet);
        let go = spot(&state);
        completed(&mut state, &mut scripted(&[AGENT, &go]), "b");
        assert_eq!(
            infantry_on(&state, &home, &planet),
            before,
            "no tactical action"
        );
        activate(&mut state, "b", &home);
        emit(
            &mut state,
            &mut crate::choice::Table::default(),
            "STRATEGIC_ACTION_BEGAN",
            &[("player", "b")],
        );
        let go = spot(&state);
        completed(&mut state, &mut scripted(&[AGENT, &go]), "b");
        assert_eq!(
            infantry_on(&state, &home, &planet),
            before,
            "not a tactical action"
        );

        // Exhausted.
        let mut state = game();
        crate::leaders::exhaust(&mut state, &a(), &LeaderId::new("sardakkagent"));
        activate(&mut state, "b", &home);
        let go = spot(&state);
        completed(&mut state, &mut scripted(&[AGENT, &go]), "b");
        assert_eq!(
            infantry_on(&state, &home, &planet),
            before,
            "agent exhausted"
        );

        // The player controls no planet in the active system.
        let mut state = game();
        let empty = SystemId::new("18");
        activate(&mut state, "b", &empty);
        let go = spot(&state);
        completed(&mut state, &mut scripted(&[AGENT, &go]), "b");
        assert_eq!(
            infantry_on(&state, &home, &planet),
            before,
            "nowhere to place"
        );
        assert_eq!(
            leader_status(&state, &a(), "sardakkagent"),
            Some(LeaderStatus::Readied),
            "and the agent is not spent for nothing"
        );
    }

    // -- G'hom Sek'kus ---------------------------------------------------------------------------

    fn non_home_planets_of(count: usize) -> Vec<(SystemId, PlanetId)> {
        let content = ContentStore::embedded();
        ti4_content::galaxy::all_planets(content, DEFAULT)
            .iter()
            .filter(|(_, planet)| {
                planet.homeworld_of().is_none() && !planet.is_placed_during_play()
            })
            .filter_map(|(id, planet)| {
                let system = planet.system_id()?;
                (!ti4_content::galaxy::is_home_system(content, system, DEFAULT))
                    .then(|| (SystemId::new(system), PlanetId::new(*id)))
            })
            .take(count)
            .collect()
    }

    #[test]
    fn the_commander_unlocks_with_five_planets_outside_home_systems() {
        let mut state = game();
        let content = ContentStore::embedded();
        let commander = LeaderId::new("sardakkcommander");
        let unlocked = |state: &GameState| {
            crate::leaders::commander_unlocked(state, content, DEFAULT, None, &a(), &commander)
        };
        assert_eq!(
            unlocked(&state),
            Some(false),
            "a fresh seat controls only home planets"
        );
        let planets = non_home_planets_of(5);
        assert_eq!(planets.len(), 5);
        for (system, planet) in &planets[..4] {
            state.system_mut(system).set_control(planet.clone(), a());
        }
        assert_eq!(unlocked(&state), Some(false), "four is not five");
        let (system, planet) = &planets[4];
        state.system_mut(system).set_control(planet.clone(), a());
        assert_eq!(unlocked(&state), Some(true));
        // Someone else's planets do not count for them.
        let mut state = game();
        for (system, planet) in &planets {
            state.system_mut(system).set_control(planet.clone(), b());
        }
        assert_eq!(unlocked(&state), Some(false));
        assert_eq!(
            crate::leaders::commander_unlocked(
                &state,
                content,
                DEFAULT,
                None,
                &a(),
                &LeaderId::new("sardakkagent")
            ),
            None,
            "not this module's commander"
        );
    }

    fn two_planet_system() -> (SystemId, PlanetId, PlanetId) {
        let content = ContentStore::embedded();
        let mut by_system: std::collections::BTreeMap<String, Vec<String>> =
            std::collections::BTreeMap::default();
        for (id, planet) in ti4_content::galaxy::all_planets(content, DEFAULT) {
            if planet.homeworld_of().is_none()
                && !planet.is_placed_during_play()
                && let Some(system) = planet.system_id()
            {
                by_system
                    .entry(system.to_owned())
                    .or_default()
                    .push((*id).to_owned());
            }
        }
        let (system, planets) = by_system
            .into_iter()
            .find(|(_, planets)| planets.len() >= 2)
            .expect("a system with two planets");
        (
            SystemId::new(system),
            PlanetId::new(&planets[0]),
            PlanetId::new(&planets[1]),
        )
    }

    #[test]
    fn the_commander_offers_one_ground_force_from_each_planet_of_the_active_system() {
        let mut state = game();
        let content = ContentStore::embedded();
        let (system, first, second) = two_planet_system();
        crate::fixtures::put_on_planet(&mut state, &system, &first, "infantry", &a(), 2);
        crate::fixtures::put_on_planet(&mut state, &system, &first, "sardakk_mech", &a(), 1);
        crate::fixtures::put_on_planet(&mut state, &system, &second, "infantry", &a(), 1);
        crate::fixtures::put_on_planet(&mut state, &system, &second, "infantry", &b(), 1);
        let candidates = |state: &GameState, already: &[CommitOrigin]| {
            crate::factions::hooks_ground::commit_candidates(
                state,
                content,
                DEFAULT,
                &a(),
                &system,
                already,
            )
        };
        assert!(candidates(&state, &[]).is_empty(), "locked commander");

        state
            .player_mut(&a())
            .unwrap()
            .leaders
            .insert(LeaderId::new("sardakkcommander"), LeaderStatus::Unlocked);
        let all = candidates(&state, &[]);
        let from = |planet: &PlanetId| all.iter().filter(|c| &c.planet == planet).count();
        assert_eq!(
            from(&first),
            2,
            "the infantry and the mech are distinct choices, two infantry are one"
        );
        assert_eq!(from(&second), 1, "only the invader's own forces");
        assert!(all.iter().all(|c| c.unit.owner == a()));

        let after = candidates(&state, &[(system.clone(), first.clone())]);
        assert!(
            after.iter().all(|c| c.planet == second),
            "up to 1 from each planet"
        );
        assert_eq!(after.len(), 1);
    }

    // -- N'orr Supremacy -------------------------------------------------------------------------

    const SUPREMACY_SPACE: &str = "breakthrough:sardakk:sardakkbt:SPACE_COMBAT_ENDED:after";
    const SUPREMACY_GROUND: &str = "breakthrough:sardakk:sardakkbt:GROUND_COMBAT_ENDED:after";

    fn with_breakthrough(state: &mut GameState) {
        state.player_mut(&a()).unwrap().breakthrough =
            Some(ti4_model::id::BreakthroughId::new("sardakkbt"));
    }

    fn won(
        state: &mut GameState,
        table: &mut crate::choice::Table,
        event_type: &str,
        winner: &str,
    ) {
        let (system, planet) = crate::fixtures::a_placed_planet();
        emit(
            state,
            table,
            event_type,
            &[
                ("system", system.as_str()),
                ("planet", planet.as_str()),
                ("attacker", "a"),
                ("defender", "b"),
                ("winner", winner),
            ],
        );
    }

    #[test]
    fn supremacy_after_a_won_combat_gains_a_command_token() {
        for (event_type, id) in [
            ("SPACE_COMBAT_ENDED", SUPREMACY_SPACE),
            ("GROUND_COMBAT_ENDED", SUPREMACY_GROUND),
        ] {
            let mut state = game();
            with_breakthrough(&mut state);
            let before = state.player(&a()).unwrap().fleet_tokens;
            won(
                &mut state,
                &mut scripted(&[id, "token", "fleet_tokens"]),
                event_type,
                "a",
            );
            assert_eq!(
                state.player(&a()).unwrap().fleet_tokens,
                before + 1,
                "{event_type}"
            );
        }
    }

    #[test]
    fn supremacy_after_a_won_combat_researches_a_unit_upgrade() {
        let mut state = game();
        with_breakthrough(&mut state);
        let content = ContentStore::embedded();
        // Enough coloured technologies that every unit upgrade's prerequisites are met.
        let mut per_colour: std::collections::BTreeMap<&str, usize> =
            std::collections::BTreeMap::default();
        for alias in crate::technology::active_aliases(content) {
            if crate::technology::faction_of(content, &alias).is_some() {
                continue;
            }
            if let Some(colour) = crate::technology::colour_type(content, &alias) {
                let held = per_colour.entry(colour).or_default();
                if *held < 3 {
                    *held += 1;
                    crate::technology::grant(&mut state, &a(), &alias);
                }
            }
        }
        let upgrades: Vec<TechnologyId> =
            crate::technology::researchable(&state, content, DEFAULT, &a())
                .into_iter()
                .filter(|alias| crate::technology::is_unit_upgrade(content, alias))
                .collect();
        let target = upgrades
            .first()
            .expect("a unit upgrade is open to research")
            .clone();
        // Only unit upgrades are offered for research, never a coloured technology.
        let option = format!("research|{target}");
        won(
            &mut state,
            &mut scripted(&[SUPREMACY_SPACE, &option]),
            "SPACE_COMBAT_ENDED",
            "a",
        );
        assert!(has_technology(&state, &a(), target.as_str()), "{target}");
    }

    #[test]
    fn supremacy_needs_the_breakthrough_and_a_win() {
        let mut state = game();
        let before = state.player(&a()).unwrap().fleet_tokens;
        won(
            &mut state,
            &mut scripted(&[SUPREMACY_SPACE, "token", "fleet_tokens"]),
            "SPACE_COMBAT_ENDED",
            "a",
        );
        assert_eq!(
            state.player(&a()).unwrap().fleet_tokens,
            before,
            "no breakthrough"
        );

        let mut state = game();
        with_breakthrough(&mut state);
        won(
            &mut state,
            &mut scripted(&[SUPREMACY_SPACE, "token", "fleet_tokens"]),
            "SPACE_COMBAT_ENDED",
            "b",
        );
        assert_eq!(
            state.player(&a()).unwrap().fleet_tokens,
            before,
            "lost the combat"
        );

        let mut state = game();
        with_breakthrough(&mut state);
        won(
            &mut state,
            &mut scripted(&["decline"]),
            "SPACE_COMBAT_ENDED",
            "a",
        );
        assert_eq!(state.player(&a()).unwrap().fleet_tokens, before, "declined");
    }

    // -- no Sardakk, no questions ----------------------------------------------------------------

    #[test]
    fn a_game_without_the_card_is_never_asked_anything_by_this_module() {
        let (decider, seen) = crate::choice::Capturing::new(Box::new(crate::choice::FirstOption));
        let mut table = crate::choice::Table::with_default(Box::new(decider));
        let mut state = crate::fixtures::seated_game(&[("a", "hacan"), ("b", "sol")], DEFAULT);
        let (system, planet) = crate::fixtures::a_placed_planet();
        let (system, planet) = (system.to_string(), planet.to_string());
        let events: [(&str, Vec<(&str, &str)>); 11] = [
            (
                "SPACE_COMBAT_ROUND_ENDED",
                vec![("system", &system), ("attacker", "a"), ("defender", "b")],
            ),
            (
                "SPACE_COMBAT_ENDED",
                vec![("system", &system), ("winner", "a")],
            ),
            (
                "GROUND_COMBAT_ENDED",
                vec![("system", &system), ("planet", &planet), ("winner", "a")],
            ),
            (
                "GROUND_COMBAT_STARTED",
                vec![
                    ("system", &system),
                    ("planet", &planet),
                    ("attacker", "a"),
                    ("defender", "b"),
                ],
            ),
            (
                "GROUND_FORCE_SUSTAINED",
                vec![
                    ("system", &system),
                    ("planet", &planet),
                    ("player", "a"),
                    ("unit", "mech"),
                    ("cause", "ground_combat"),
                ],
            ),
            (
                "SYSTEM_ACTIVATED",
                vec![("player", "a"), ("system", &system)],
            ),
            ("ACTION_COMPLETED", vec![("player", "a")]),
            ("TURN_BEGAN", vec![("player", "a")]),
            ("STRATEGIC_ACTION_BEGAN", vec![("player", "a")]),
            ("TURN_BEGAN", vec![("player", "b")]),
            ("ACTION_COMPLETED", vec![("player", "b")]),
        ];
        for (event_type, pairs) in &events {
            emit(&mut state, &mut table, event_type, pairs);
        }
        let asked: Vec<String> = seen
            .borrow()
            .iter()
            .filter(|choice| {
                choice
                    .ids()
                    .iter()
                    .any(|id| id.contains(":sardakk:") || id.contains("sardakk"))
            })
            .map(|choice| choice.prompt.clone())
            .collect();
        assert!(asked.is_empty(), "{asked:?}");
    }
}
