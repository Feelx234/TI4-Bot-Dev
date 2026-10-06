//! Nomad agents (Artuno the Betrayer, Field Marshal Mercer, The Thundarian), the Quantum
//! Manipulator mech and The Cavalry promissory note. Everything else Nomad is in `nomad.rs`.
//!
//! Card text (content corpus, latest printing):
//!
//! * Artuno the Betrayer, agent: "When you gain trade goods from the supply: You may exhaust this
//!   card to place an equal number of trade goods on this card. When this card readies, gain the
//!   trade goods on this card."
//! * Field Marshal Mercer, agent: "At the end of a player's turn: You may exhaust this card to
//!   allow that player to remove up to 2 of their ground forces from the game board and place them
//!   on planets they control in the active system." The corpus notes that the window is the end of
//!   the tactical action, as the bot treats it; it is hung on `TACTICAL_ACTION_ENDED`.
//! * The Thundarian, agent: "After the \"Roll Dice\" step of combat: You may exhaust this card. If
//!   you do, hits are not assigned to either player's units. Return to the start of this combat
//!   round's \"Roll Dice\" step."
//! * Quantum Manipulator, mech: "While this unit is in a space area during combat, you may use its
//!   SUSTAIN DAMAGE ability to cancel a hit that is produced against your ships in this system."
//! * The Cavalry, promissory note: "At the start of a space combat against a player other than the
//!   Nomad: During this combat, treat 1 of your non-fighter ships as if it has the SUSTAIN DAMAGE
//!   ability, combat value, and ANTI-FIGHTER BARRAGE value of the Nomad's flagship. Return this
//!   card to the Nomad player at the end of the combat."
//!
//! Routes: Artuno hangs on `TRADE_GOODS_GAINED` (staged or emitted by the gain sites) and its card
//! pile is the public mark `nomad:artuno:<player>`, paid out by `leaders::ready_all` / `ready`.
//! Mercer hangs on `TACTICAL_ACTION_ENDED`. The Thundarian hangs on the new
//! `SPACE_COMBAT_ROLL_STEP_ENDED` (`combat::CombatWindow::roll_round`) and
//! `GROUND_COMBAT_ROLL_STEP_ENDED` (`invasion::resolve_ground_round`), which replay the step when
//! it asks (`combat::request_roll_replay`). The mech and The Cavalry use the combat hooks
//! `grants_sustain` and `borrowed_stats`; The Cavalry is played on `SPACE_COMBAT_STARTED` and
//! returned on `SPACE_COMBAT_ENDED`.

use std::collections::BTreeSet;
use std::sync::Arc;

use ti4_content::ContentStore;
use ti4_model::content_types::SourceSet;
use ti4_model::id::{LeaderId, PlanetId, PlayerId, SystemId};
use ti4_model::state::{GameState, LeaderStatus};
use ti4_model::units::Unit;

use super::CombatUnit;
use super::hooks_combat::{BorrowedStats, CombatHooks};
use super::hooks_ground::GroundHooks;
use crate::choice::{Choice, ChoiceOption};
use crate::decision_context::{DecisionContext, DecisionSource};
use crate::event::Event;
use crate::timing::{Ability, Relation, TimingContext, TimingError};

const ARTUNO: &str = "nomadagentartuno";
const MERCER: &str = "nomadagentmercer";
const THUNDARIAN: &str = "nomadagentthundarian";
const MECH: &str = "nomad_mech";
const CAVALRY: &str = "cavalry";
/// The faction that owns the note, and its name in note ids.
const FACTION: &str = super::nomad::FACTION;
/// Ground forces Mercer lets a player move.
const MERCER_GROUND_FORCES: usize = 2;
/// `TRADE_GOODS_GAINED` sources that are not "from the supply": goods another player gave, took or
/// paid, and goods Artuno's own pile pays out.
const NOT_FROM_THE_SUPPLY: &[&str] = &[
    "transaction",
    "pillage",
    "extreme_duress",
    "trade_agreement",
    "artuno",
];

/// Leaders claimed by the Nomad module. `nomad.rs` reports its own leader claims (hero,
/// commander) to the coordinator, who adds them here.
pub const LEADERS: &[&str] = &[ARTUNO, MERCER, THUNDARIAN, "nomadcommander", "nomadhero"];
/// Units claimed here (the mech); `nomad.rs` reports its flagship claims to the coordinator.
pub const UNITS: &[&str] = &[MECH, "nomad_flagship", "nomad_flagship2"];
/// Promissory notes claimed by the Nomad module.
pub const PROMISSORY: &[&str] = &[CAVALRY];
/// Combat hooks for the agents, mech and note.
pub const COMBAT_HOOKS: CombatHooks = CombatHooks {
    grants_sustain: Some(grants_sustain),
    borrowed_stats: Some(borrowed_stats),
    ..CombatHooks::NONE
};
/// Ground hooks for the agents. None: the agents' ground windows are timing abilities.
pub const GROUND_HOOKS: GroundHooks = GroundHooks::NONE;

/// Timing abilities of the agents, mech and note, for one seat.
pub(crate) fn timing_abilities(
    _state: &GameState,
    owner_name: &str,
    seat: &PlayerId,
) -> Vec<Ability> {
    vec![
        artuno(owner_name, seat),
        mercer(owner_name, seat),
        thundarian(owner_name, seat, "SPACE_COMBAT_ROLL_STEP_ENDED"),
        thundarian(owner_name, seat, "GROUND_COMBAT_ROLL_STEP_ENDED"),
        cavalry_play(owner_name, seat),
        cavalry_return(owner_name, seat),
    ]
}

/// Whether `player` holds `agent` readied.
fn agent_ready(state: &GameState, player: &PlayerId, agent: &str) -> bool {
    state
        .player(player)
        .is_some_and(|seat| seat.leaders.get(&LeaderId::new(agent)) == Some(&LeaderStatus::Readied))
}

/// The one question every Nomad ability here asks. `ability` names the card for the decision
/// context; `subtype` the question.
fn ask(
    context: &mut TimingContext<'_>,
    who: &PlayerId,
    ability: &str,
    subtype: &str,
    prompt: String,
    options: Vec<ChoiceOption>,
) -> Result<ChoiceOption, TimingError> {
    let choice = Choice::new(who.clone(), prompt, options).contextualized(DecisionContext::new(
        who.clone(),
        DecisionSource::FactionAbility(ability.to_owned()),
        subtype,
        context.state.phase,
        context.state.round,
    ));
    context
        .ask_seeing(&choice)
        .map_err(TimingError::IllegalChoice)
}

// -- Artuno the Betrayer -------------------------------------------------------------------------

/// The public mark holding the trade goods on Artuno's card.
fn artuno_pile_key(owner: &PlayerId) -> String {
    format!("nomad:artuno:{owner}")
}

/// Trade goods on Artuno's card.
#[must_use]
pub fn artuno_pile(state: &GameState, owner: &PlayerId) -> i32 {
    state
        .faction_marks
        .get(&artuno_pile_key(owner))
        .and_then(|goods| goods.parse().ok())
        .unwrap_or(0)
}

/// Whether `event` is a gain of trade goods from the supply by `owner`; the amount gained.
fn supply_gain(event: &Event, owner: &PlayerId) -> Option<i32> {
    let amount = i32::try_from(event.integer("amount")?).ok()?;
    (event.text("player") == Some(owner.as_str())
        && amount > 0
        && !NOT_FROM_THE_SUPPLY.contains(&event.text("source").unwrap_or_default()))
    .then_some(amount)
}

/// Artuno: "When you gain trade goods from the supply: You may exhaust this card to place an equal
/// number of trade goods on this card." The gain itself still happens (the card only copies it);
/// the window is read after the gain, which is when the engine announces it, and the number on the
/// card is the number gained.
fn artuno(owner_name: &str, seat: &PlayerId) -> Ability {
    let (owner, condition_owner) = (seat.clone(), seat.clone());
    Ability::stateful(
        format!("leader:{owner_name}:{ARTUNO}:TRADE_GOODS_GAINED:after"),
        seat.clone(),
        "TRADE_GOODS_GAINED",
        Relation::After,
        Arc::new(move |event, _, context| {
            let Some(amount) = supply_gain(event, &owner) else {
                return Ok(());
            };
            if !crate::leaders::exhaust(context.state, &owner, &LeaderId::new(ARTUNO)) {
                return Ok(());
            }
            let pile = artuno_pile(context.state, &owner) + amount;
            context
                .state
                .faction_marks
                .insert(artuno_pile_key(&owner), pile.to_string());
            Ok(())
        }),
    )
    .with_optional(true)
    .with_stateful_condition(Arc::new(move |event, _, context| {
        supply_gain(event, &condition_owner).is_some()
            && agent_ready(context.state, &condition_owner, ARTUNO)
    }))
}

/// "When this card readies, gain the trade goods on this card." Called by `leaders::ready_all` and
/// `leaders::ready` for every leader they ready.
pub(crate) fn agent_readied(state: &mut GameState, player: &PlayerId, leader: &LeaderId) {
    if leader.as_str() != ARTUNO {
        return;
    }
    let Some(pile) = state.faction_marks.remove(&artuno_pile_key(player)) else {
        return;
    };
    let goods = pile.parse::<i32>().unwrap_or(0);
    crate::supply::gain_trade_goods_staged(state, player, goods, "artuno");
}

// -- Field Marshal Mercer ------------------------------------------------------------------------

/// Where one ground force stands: a system, and the planet (`None` in the space area).
type Place = (SystemId, Option<PlanetId>);

/// `player`'s ground forces on the game board, in board order. The card says "ground forces", so a
/// unit that is a ground force is movable even when it is also a structure (the Titans' PDS, Hel-Titan);
/// a structure that is not a ground force (a plain PDS, a space dock) is not.
fn ground_forces(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
) -> Vec<(Place, Unit)> {
    let types = ti4_content::units::catalogue(content, sources);
    let movable = |unit: &&Unit| {
        &unit.owner == player
            && types
                .get(unit.type_id.as_str())
                .is_some_and(|kind| kind.is_ground_force())
    };
    let mut found = Vec::new();
    for (system, board) in &state.board {
        for unit in board.units.iter().filter(movable) {
            found.push(((system.clone(), None), unit.clone()));
        }
        for (planet, units) in &board.planet_units {
            for unit in units.iter().filter(movable) {
                found.push(((system.clone(), Some(planet.clone())), unit.clone()));
            }
        }
    }
    found
}

/// The planets `player` controls in `system` that units may be placed on.
fn landing_planets(state: &GameState, player: &PlayerId, system: &SystemId) -> Vec<PlanetId> {
    state.board.get(system).map_or_else(Vec::new, |board| {
        board
            .planet_control
            .iter()
            .filter(|(planet, owner)| {
                *owner == player && !crate::laws::planet_is_demilitarized(state, planet)
            })
            .map(|(planet, _)| planet.clone())
            .collect()
    })
}

/// The player and active system a tactical action's end names, when Mercer could act on it:
/// readied for `owner`, a ground force to move and a planet to put it on.
fn mercer_window(
    context: &TimingContext<'_>,
    owner: &PlayerId,
    event: &Event,
) -> Option<(PlayerId, SystemId)> {
    if !agent_ready(context.state, owner, MERCER) {
        return None;
    }
    let actor = PlayerId::new(event.text("player")?);
    let system = SystemId::new(event.text("system")?);
    (!landing_planets(context.state, &actor, &system).is_empty()
        && !ground_forces(context.state, context.content, context.sources, &actor).is_empty())
    .then_some((actor, system))
}

/// The id of the option that takes one ground force off the board.
fn take_id(place: &Place, unit: &Unit) -> String {
    format!(
        "take|{}|{}|{}|{}",
        place.0,
        place
            .1
            .as_ref()
            .map_or_else(|| "space".to_owned(), ToString::to_string),
        unit.type_id,
        u8::from(unit.sustained_damage)
    )
}

/// Ask `actor` which ground forces to remove (up to two, one question each) and where each goes.
/// Nothing is changed; the answers come back as `(from, unit, to)`.
fn mercer_plan(
    context: &mut TimingContext<'_>,
    actor: &PlayerId,
    system: &SystemId,
) -> Result<Vec<(Place, Unit, PlanetId)>, TimingError> {
    let mut pool = ground_forces(context.state, context.content, context.sources, actor);
    let landing = landing_planets(context.state, actor, system);
    let mut taken: Vec<(Place, Unit)> = Vec::new();
    for _ in 0..MERCER_GROUND_FORCES {
        let mut options: Vec<ChoiceOption> = Vec::new();
        let mut seen = BTreeSet::new();
        for (place, unit) in &pool {
            let id = take_id(place, unit);
            if seen.insert(id.clone()) {
                let at = place.1.as_ref().map_or_else(
                    || format!("the space area of {}", place.0),
                    ToString::to_string,
                );
                options.push(
                    ChoiceOption::labelled(
                        id,
                        "mercer_take",
                        format!("remove {} from {at}", unit.type_id),
                    )
                    .with("unit", unit.type_id.to_string())
                    .with("system", place.0.to_string()),
                );
            }
        }
        if options.is_empty() {
            break;
        }
        options.push(ChoiceOption::decline());
        let answer = ask(
            context,
            actor,
            MERCER,
            "mercer_take",
            format!(
                "Field Marshal Mercer: remove a ground force ({} of {MERCER_GROUND_FORCES})",
                taken.len() + 1
            ),
            options,
        )?;
        if answer.is_decline() {
            break;
        }
        let Some(position) = pool
            .iter()
            .position(|(place, unit)| take_id(place, unit) == answer.id)
        else {
            break;
        };
        taken.push(pool.remove(position));
    }
    let mut plan = Vec::new();
    for (place, unit) in taken {
        let target = if let [only] = landing.as_slice() {
            only.clone()
        } else {
            let options = landing
                .iter()
                .map(|planet| {
                    ChoiceOption::labelled(
                        planet.to_string(),
                        "mercer_place",
                        format!("place {} on {planet}", unit.type_id),
                    )
                    .with("unit", unit.type_id.to_string())
                    .with("system", system.to_string())
                })
                .collect();
            let answer = ask(
                context,
                actor,
                MERCER,
                "mercer_place",
                format!("Field Marshal Mercer: place {} on a planet", unit.type_id),
                options,
            )?;
            landing
                .iter()
                .find(|planet| planet.as_str() == answer.id)
                .cloned()
                .unwrap_or_else(|| landing[0].clone())
        };
        plan.push((place, unit, target));
    }
    Ok(plan)
}

/// Remove one ground force from where it stands. `false` (nothing changed) if it is not there.
fn remove_ground_force(state: &mut GameState, place: &Place, unit: &Unit) -> bool {
    let Some(board) = state.board.get_mut(&place.0) else {
        return false;
    };
    let units = match &place.1 {
        Some(planet) => board.planet_units.get_mut(planet),
        None => Some(&mut board.units),
    };
    let Some(units) = units else {
        return false;
    };
    let Some(index) = units.iter().position(|found| found == unit) else {
        return false;
    };
    units.remove(index);
    true
}

/// Field Marshal Mercer: the agent is exhausted, the player takes up to 2 of their ground forces
/// off the board and puts them on planets they control in the active system. A unit removed from
/// the board and placed is placed fresh (undamaged). If the player removes nothing, nothing
/// happens and the card stays readied.
fn mercer(owner_name: &str, seat: &PlayerId) -> Ability {
    let (owner, condition_owner) = (seat.clone(), seat.clone());
    Ability::stateful(
        format!("leader:{owner_name}:{MERCER}:TACTICAL_ACTION_ENDED:after"),
        seat.clone(),
        "TACTICAL_ACTION_ENDED",
        Relation::After,
        Arc::new(move |event, _, context| {
            let Some((actor, system)) = mercer_window(context, &owner, event) else {
                return Ok(());
            };
            let plan = mercer_plan(context, &actor, &system)?;
            if plan.is_empty() {
                return Ok(());
            }
            // The removals are checked against the board before the first one is made.
            let board = ground_forces(context.state, context.content, context.sources, &actor);
            for (place, unit, _) in &plan {
                let wanted = plan
                    .iter()
                    .filter(|(there, found, _)| there == place && found == unit)
                    .count();
                let held = board
                    .iter()
                    .filter(|(there, found)| there == place && found == unit)
                    .count();
                if held < wanted {
                    return Ok(());
                }
            }
            if !crate::leaders::exhaust(context.state, &owner, &LeaderId::new(MERCER)) {
                return Ok(());
            }
            for (place, unit, target) in plan {
                remove_ground_force(context.state, &place, &unit);
                context
                    .state
                    .system_mut(&system)
                    .planet_units
                    .entry(target)
                    .or_default()
                    .push(Unit::new(unit.type_id, actor.clone()));
            }
            Ok(())
        }),
    )
    .with_optional(true)
    .with_stateful_condition(Arc::new(move |event, _, context| {
        mercer_window(context, &condition_owner, event).is_some()
    }))
}

// -- The Thundarian ------------------------------------------------------------------------------

/// Whether a combat's "Roll Dice" step end has anyone to tell: some seat holds a readied The
/// Thundarian, or a seat holding a readied Ssruu could borrow a seated Nomad's agent. Mirrors
/// `nomad::watches_agent_exhaustion`: a game with no such seat emits nothing, so its event ids and
/// logs are exactly what they were before the agent existed.
#[must_use]
pub fn watches_roll_step(state: &GameState) -> bool {
    let holds = |seat: &ti4_model::state::Player, status: &[LeaderStatus]| {
        seat.leaders
            .get(&LeaderId::new(THUNDARIAN))
            .is_some_and(|held| status.contains(held))
    };
    state
        .players
        .iter()
        .any(|seat| holds(seat, &[LeaderStatus::Readied]))
        || (state.players.iter().any(|seat| {
            seat.leaders.get(&LeaderId::new("yssarilagent")) == Some(&LeaderStatus::Readied)
        }) && state
            .players
            .iter()
            .any(|seat| holds(seat, &[LeaderStatus::Readied, LeaderStatus::Exhausted])))
}

/// The Thundarian: exhausting it sends the round back to the start of its "Roll Dice" step
/// (`combat::request_roll_replay`): the combat reads the request when the step's event closes,
/// throws the dice away without producing or assigning a hit, and rolls the step again.
///
/// Offered to the Nomad in any combat, its own or not: the text says "the \"Roll Dice\" step of
/// combat" and names no player.
fn thundarian(owner_name: &str, seat: &PlayerId, event: &'static str) -> Ability {
    let (owner, condition_owner) = (seat.clone(), seat.clone());
    Ability::stateful(
        format!("leader:{owner_name}:{THUNDARIAN}:{event}:after"),
        seat.clone(),
        event,
        Relation::After,
        Arc::new(move |_, _, context| {
            if crate::leaders::exhaust(context.state, &owner, &LeaderId::new(THUNDARIAN)) {
                crate::combat::request_roll_replay(context.state);
            }
            Ok(())
        }),
    )
    .with_optional(true)
    .with_stateful_condition(Arc::new(move |_, _, context| {
        agent_ready(context.state, &condition_owner, THUNDARIAN)
            && !context
                .state
                .faction_marks
                .contains_key(crate::combat::ROLL_REPLAY_MARK)
    }))
}

// -- Quantum Manipulator and The Cavalry: SUSTAIN DAMAGE ------------------------------------------

/// The Cavalry's mark: the system of the combat and the type of the ship that borrows the
/// flagship's abilities, kept by the player who played the note.
fn cavalry_key(holder: &PlayerId) -> String {
    format!("nomad:cavalry:{holder}")
}

/// The note id of The Cavalry.
fn cavalry_note() -> String {
    crate::promissory::note_id(CAVALRY, FACTION)
}

/// The ship type `player` has chosen for The Cavalry in the combat in `system`, while the note is
/// still in play with them.
fn cavalry_ship_type(state: &GameState, player: &PlayerId, system: &SystemId) -> Option<String> {
    let mark = state.faction_marks.get(&cavalry_key(player))?;
    let (marked, kind) = mark.split_once('|')?;
    let note = cavalry_note();
    (marked == system.as_str()
        && state.promissory_notes.get(&note) == Some(player)
        && state.promissory_faceup.contains(&note))
    .then(|| kind.to_owned())
}

/// Quantum Manipulator and The Cavalry give SUSTAIN DAMAGE against a hit in a space combat.
///
/// The mech may use its own SUSTAIN DAMAGE "while in a space area during combat": `context` is
/// `"space"` for the hits of a combat and `"space_cannon"` for SPACE CANNON OFFENSE, which is not
/// part of one.
///
/// Owner gating is not done here: `combat::sustains_in_space` only asks about a unit the hit's
/// player owns (`unit.owner == player`) and has already refused a damaged one, so `player` is the
/// owner and this hook only decides whether the unit type or the Cavalry's chosen ship qualifies.
///
/// The Cavalry's ship has the ability while it is undamaged. Ships of one type are
/// interchangeable, so the borrower is whichever of them has not used it: once one of the type is
/// damaged (a type that cannot sustain otherwise can only have been damaged by this ability), no
/// other ship of that type can sustain.
fn grants_sustain(
    state: &GameState,
    _content: &ContentStore,
    _sources: SourceSet,
    unit: &CombatUnit<'_>,
) -> bool {
    if unit.context != "space" {
        return false;
    }
    if unit.unit_type == MECH {
        return true;
    }
    let Some(system) = unit.system else {
        return false;
    };
    cavalry_ship_type(state, unit.player, system).is_some_and(|kind| kind == unit.unit_type)
        && !state.board.get(system).is_some_and(|board| {
            board.units.iter().any(|ship| {
                &ship.owner == unit.player
                    && ship.type_id.as_str() == unit.unit_type
                    && ship.sustained_damage
            })
        })
}

/// The Cavalry: the Nomad flagship's combat value and ANTI-FIGHTER BARRAGE for the chosen ship.
///
/// The flagship is the Nomad's own, upgrade included (Memoria II once the Nomad owns it). Only the
/// combat *value* is lent (the ship keeps its own number of dice). The card lends the ANTI-FIGHTER
/// BARRAGE *value* only: a borrower that already has a barrage keeps its own number of dice (a
/// destroyer still fires 2); a borrower with none has no dice of its own to fire the value with,
/// so it fires the flagship's number of dice (documented choice). The ship
/// that borrows is the damaged one of its type when the type cannot sustain by itself (it is the
/// one that used the borrowed ability), else the first of its type in board order.
fn borrowed_stats(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    system: &SystemId,
) -> Option<BorrowedStats> {
    let kind = cavalry_ship_type(state, player, system)?;
    let nomad = crate::promissory::seat_of(state, FACTION)?;
    let flagship_id =
        crate::action_cards::placed_unit_id(state, content, sources, &nomad, "flagship")?;
    let types = ti4_content::units::catalogue(content, sources);
    let flagship = types.get(flagship_id.as_str())?;
    let own_type = types.get(kind.as_str())?;
    let own_sustain = own_type.sustain_damage();
    let barrage_dice = if own_type.afb_hits_on().is_some() {
        own_type.afb_dice()
    } else {
        flagship.afb_dice()
    };
    let ships: Vec<Unit> = crate::combat::ships_of(state, content, sources, player, system)
        .into_iter()
        .filter(|ship| ship.type_id.as_str() == kind)
        .collect();
    let unit = ships
        .iter()
        .find(|ship| !own_sustain && ship.sustained_damage)
        .or_else(|| ships.first())?
        .clone();
    Some(BorrowedStats {
        unit,
        hits_on: flagship.combat_hits_on(),
        barrage: flagship.afb_hits_on().map(|value| (value, barrage_dice)),
    })
}

// -- The Cavalry: play and return ----------------------------------------------------------------

/// Non-fighter ship types `player` has in the space area of `system`, in a stable order.
fn non_fighter_types(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    system: &SystemId,
) -> Vec<String> {
    let types = ti4_content::units::catalogue(content, sources);
    crate::combat::ships_of(state, content, sources, player, system)
        .into_iter()
        .filter(|ship| {
            types
                .get(ship.type_id.as_str())
                .is_some_and(|kind| kind.is_ship() && !kind.is_fighter())
        })
        .map(|ship| ship.type_id.to_string())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// The system of a space combat `holder` may play The Cavalry at the start of: they hold the note
/// (another player's, in hand), fight a seated player who is not the Nomad, and have a non-fighter
/// ship there.
fn cavalry_combat(
    context: &TimingContext<'_>,
    holder: &PlayerId,
    event: &Event,
) -> Option<SystemId> {
    let state = &*context.state;
    let note = cavalry_note();
    if state.promissory_notes.get(&note) != Some(holder)
        || state.promissory_faceup.contains(&note)
        || crate::promissory::faction_name(state, holder) == FACTION
    {
        return None;
    }
    let (attacker, defender) = (event.text("attacker")?, event.text("defender")?);
    let opponent = if holder.as_str() == attacker {
        defender
    } else if holder.as_str() == defender {
        attacker
    } else {
        return None;
    };
    if crate::promissory::faction_name(state, &PlayerId::new(opponent)) == FACTION
        || state.player(&PlayerId::new(opponent)).is_none()
    {
        return None;
    }
    let system = SystemId::new(event.text("system")?);
    (!non_fighter_types(state, context.content, context.sources, holder, &system).is_empty())
        .then_some(system)
}

/// The Cavalry: played at the start of a space combat. The holder picks the ship type (one type is
/// not a decision); the note stays faceup with them until the combat ends.
fn cavalry_play(owner_name: &str, seat: &PlayerId) -> Ability {
    let (holder, condition_holder) = (seat.clone(), seat.clone());
    Ability::stateful(
        format!("promissory:{owner_name}:{CAVALRY}:SPACE_COMBAT_STARTED:after"),
        seat.clone(),
        "SPACE_COMBAT_STARTED",
        Relation::After,
        Arc::new(move |event, _, context| {
            let Some(system) = cavalry_combat(context, &holder, event) else {
                return Ok(());
            };
            let kinds = non_fighter_types(
                context.state,
                context.content,
                context.sources,
                &holder,
                &system,
            );
            let kind = if let [only] = kinds.as_slice() {
                only.clone()
            } else {
                let options = kinds
                    .iter()
                    .map(|kind| {
                        ChoiceOption::labelled(
                            kind.clone(),
                            "cavalry_ship",
                            format!("treat a {kind} as the Nomad flagship"),
                        )
                        .with("unit", kind.clone())
                    })
                    .collect();
                let answer = ask(
                    context,
                    &holder,
                    CAVALRY,
                    "cavalry_ship",
                    "The Cavalry: which non-fighter ship takes the Nomad flagship's abilities"
                        .to_owned(),
                    options,
                )?;
                kinds
                    .iter()
                    .find(|kind| **kind == answer.id)
                    .unwrap_or(&kinds[0])
                    .clone()
            };
            context.state.promissory_faceup.insert(cavalry_note());
            context
                .state
                .faction_marks
                .insert(cavalry_key(&holder), format!("{system}|{kind}"));
            Ok(())
        }),
    )
    .with_optional(true)
    .with_stateful_condition(Arc::new(move |event, _, context| {
        cavalry_combat(context, &condition_holder, event).is_some()
    }))
}

/// "Return this card to the Nomad player at the end of the combat."
fn cavalry_return(owner_name: &str, seat: &PlayerId) -> Ability {
    let (holder, condition_holder) = (seat.clone(), seat.clone());
    Ability::stateful(
        format!("promissory:{owner_name}:{CAVALRY}:SPACE_COMBAT_ENDED:after"),
        seat.clone(),
        "SPACE_COMBAT_ENDED",
        Relation::After,
        Arc::new(move |_, _, context| {
            context.state.faction_marks.remove(&cavalry_key(&holder));
            crate::promissory::give_back(context.state, &cavalry_note());
            Ok(())
        }),
    )
    .with_stateful_condition(Arc::new(move |event, _, context| {
        event.text("system").is_some_and(|system| {
            cavalry_ship_type(context.state, &condition_holder, &SystemId::new(system)).is_some()
        })
    }))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::{Arc as Shared, Mutex};

    use super::*;
    use crate::choice::{Decider, IllegalChoice, Scripted, Table};
    use crate::event::EventSequence;
    use serde_json::Value;
    use ti4_model::content_types::DEFAULT;

    fn a() -> PlayerId {
        PlayerId::new("a")
    }
    fn b() -> PlayerId {
        PlayerId::new("b")
    }
    fn game() -> GameState {
        crate::fixtures::seated_game(&[("a", "nomad"), ("b", "sol")], DEFAULT)
    }
    fn scripted(answers: &[&str]) -> Table {
        Table::with_default(Box::new(Scripted::new(answers.iter().copied())))
    }
    fn payload(pairs: &[(&str, Value)]) -> BTreeMap<String, Value> {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_owned(), value.clone()))
            .collect()
    }
    fn status(state: &GameState, player: &PlayerId, leader: &str) -> Option<LeaderStatus> {
        state
            .player(player)
            .and_then(|seat| seat.leaders.get(&LeaderId::new(leader)).copied())
    }
    fn set_status(state: &mut GameState, player: &PlayerId, leader: &str, to: LeaderStatus) {
        state
            .player_mut(player)
            .unwrap()
            .leaders
            .insert(LeaderId::new(leader), to);
    }

    type Asked = Shared<Mutex<Vec<(String, Vec<String>)>>>;

    /// Records every question (who, option ids), then answers from a script.
    struct Recording(Scripted, Asked);

    impl Decider for Recording {
        fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
            self.1.lock().unwrap().push((
                choice.player.to_string(),
                choice.options.iter().map(|o| o.id.clone()).collect(),
            ));
            self.0.choose(choice)
        }
    }

    fn recording(answers: &[&str]) -> (Table, Asked) {
        let asked = Asked::default();
        let table = Table::with_default(Box::new(Recording(
            Scripted::new(answers.iter().copied()),
            Shared::clone(&asked),
        )));
        (table, asked)
    }

    /// Emit one typed event through a resolver armed as the game arms one.
    fn emit(state: &mut GameState, table: &mut Table, kind: &str, pairs: &[(&str, Value)]) {
        let mut resolver = crate::fixtures::armed_resolver(state);
        crate::fixtures::with_context(state, DEFAULT, None, table, |ctx| {
            let event = EventSequence::new().next(kind, payload(pairs)).unwrap();
            resolver
                .emit_with_context(ctx, event, |_, _| {})
                .expect("emits");
        });
    }

    // -- Artuno ---------------------------------------------------------------------------------

    fn gained(state: &mut GameState, who: &str, amount: i64, source: &str, answers: &[&str]) {
        emit(
            state,
            &mut scripted(answers),
            "TRADE_GOODS_GAINED",
            &[
                ("player", who.into()),
                ("amount", amount.into()),
                ("source", source.into()),
            ],
        );
    }

    const ARTUNO_WINDOW: &str = "leader:nomad:nomadagentartuno:TRADE_GOODS_GAINED:after";

    #[test]
    fn artuno_copies_a_gain_from_the_supply_onto_its_card() {
        let mut state = game();
        assert_eq!(status(&state, &a(), ARTUNO), Some(LeaderStatus::Readied));
        gained(&mut state, "a", 3, "trade_primary", &[ARTUNO_WINDOW]);
        assert_eq!(status(&state, &a(), ARTUNO), Some(LeaderStatus::Exhausted));
        assert_eq!(artuno_pile(&state, &a()), 3);
    }

    #[test]
    fn artuno_declined_gain_changes_nothing() {
        let mut state = game();
        gained(&mut state, "a", 3, "trade_primary", &["decline"]);
        assert_eq!(status(&state, &a(), ARTUNO), Some(LeaderStatus::Readied));
        assert_eq!(artuno_pile(&state, &a()), 0);
    }

    #[test]
    fn artuno_ignores_what_is_not_a_gain_from_the_supply() {
        for source in NOT_FROM_THE_SUPPLY {
            let mut state = game();
            let (mut table, asked) = recording(&[ARTUNO_WINDOW]);
            emit(
                &mut state,
                &mut table,
                "TRADE_GOODS_GAINED",
                &[
                    ("player", "a".into()),
                    ("amount", 3.into()),
                    ("source", (*source).into()),
                ],
            );
            assert!(
                asked.lock().unwrap().is_empty(),
                "{source}: nothing offered"
            );
            assert_eq!(status(&state, &a(), ARTUNO), Some(LeaderStatus::Readied));
            assert_eq!(artuno_pile(&state, &a()), 0, "{source}");
        }
        // Another player's gain, and no gain at all.
        let mut state = game();
        let (mut table, asked) = recording(&[ARTUNO_WINDOW]);
        for (who, amount) in [("b", 3), ("a", 0)] {
            emit(
                &mut state,
                &mut table,
                "TRADE_GOODS_GAINED",
                &[
                    ("player", who.into()),
                    ("amount", amount.into()),
                    ("source", "trade_primary".into()),
                ],
            );
        }
        assert!(asked.lock().unwrap().is_empty());
        assert_eq!(artuno_pile(&state, &a()), 0);
        // An exhausted card places nothing more.
        let mut state = game();
        set_status(&mut state, &a(), ARTUNO, LeaderStatus::Exhausted);
        let (mut table, asked) = recording(&[ARTUNO_WINDOW]);
        emit(
            &mut state,
            &mut table,
            "TRADE_GOODS_GAINED",
            &[
                ("player", "a".into()),
                ("amount", 2.into()),
                ("source", "trade_primary".into()),
            ],
        );
        assert!(asked.lock().unwrap().is_empty());
        assert_eq!(artuno_pile(&state, &a()), 0);
    }

    #[test]
    fn artuno_pays_its_pile_out_when_it_readies() {
        let mut state = game();
        gained(&mut state, "a", 3, "trade_primary", &[ARTUNO_WINDOW]);
        let before = state.player(&a()).unwrap().trade_goods;
        // The status phase readies every exhausted card.
        let readied = crate::leaders::ready_all(&mut state, &a());
        assert!(readied.iter().any(|leader| leader.as_str() == ARTUNO));
        assert_eq!(status(&state, &a(), ARTUNO), Some(LeaderStatus::Readied));
        assert_eq!(state.player(&a()).unwrap().trade_goods, before + 3);
        assert_eq!(artuno_pile(&state, &a()), 0, "the pile is gone");
        // An empty card pays nothing, and a card readied one at a time pays as well.
        set_status(&mut state, &a(), ARTUNO, LeaderStatus::Exhausted);
        assert!(crate::leaders::ready(
            &mut state,
            &a(),
            &LeaderId::new(ARTUNO)
        ));
        assert_eq!(state.player(&a()).unwrap().trade_goods, before + 3);
        gained(&mut state, "a", 2, "trade_primary", &[ARTUNO_WINDOW]);
        assert!(crate::leaders::ready(
            &mut state,
            &a(),
            &LeaderId::new(ARTUNO)
        ));
        assert_eq!(state.player(&a()).unwrap().trade_goods, before + 5);
    }

    #[test]
    fn artuno_through_the_gain_helper_the_engine_uses() {
        let mut state = game();
        let before = state.player(&a()).unwrap().trade_goods;
        let mut resolver = crate::fixtures::armed_resolver(&state);
        let mut table = scripted(&[ARTUNO_WINDOW]);
        crate::fixtures::with_context(&mut state, DEFAULT, None, &mut table, |ctx| {
            crate::supply::gain_trade_goods_via(ctx, &mut resolver, &a(), 2, "trade_primary")
                .unwrap();
        });
        assert_eq!(
            state.player(&a()).unwrap().trade_goods,
            before + 2,
            "the gain still happens"
        );
        assert_eq!(artuno_pile(&state, &a()), 2, "and the card holds a copy");
    }

    // -- Neutrality -----------------------------------------------------------------------------

    #[test]
    fn games_without_the_nomad_see_nothing_of_the_agents() {
        let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan")], DEFAULT);
        let before = state.clone();
        let (mut table, asked) = recording(&[]);
        for (kind, pairs) in [
            (
                "TRADE_GOODS_GAINED",
                vec![
                    ("player", Value::from("a")),
                    ("amount", 3.into()),
                    ("source", "trade_primary".into()),
                ],
            ),
            (
                "TACTICAL_ACTION_ENDED",
                vec![("player", "a".into()), ("system", "18".into())],
            ),
            (
                "SPACE_COMBAT_ROLL_STEP_ENDED",
                vec![("system", "18".into())],
            ),
            (
                "GROUND_COMBAT_ROLL_STEP_ENDED",
                vec![("system", "18".into())],
            ),
            (
                "SPACE_COMBAT_STARTED",
                vec![
                    ("system", "18".into()),
                    ("attacker", "a".into()),
                    ("defender", "b".into()),
                ],
            ),
            ("SPACE_COMBAT_ENDED", vec![("system", "18".into())]),
        ] {
            emit(&mut state, &mut table, kind, &pairs);
        }
        assert!(asked.lock().unwrap().is_empty(), "nothing offered");
        assert_eq!(state, before, "nothing changed");
    }

    // -- Mercer ---------------------------------------------------------------------------------

    const MERCER_WINDOW: &str = "leader:nomad:nomadagentmercer:TACTICAL_ACTION_ENDED:after";

    /// A planet outside `avoid` put under `who`'s control; returns it and its system.
    fn take_planet(
        state: &mut GameState,
        who: &PlayerId,
        avoid: &SystemId,
    ) -> (SystemId, PlanetId) {
        let catalogue = ti4_content::galaxy::all_planets(ContentStore::embedded(), DEFAULT);
        let (system, planet) = crate::fixtures::non_home_planets(400)
            .into_iter()
            .filter(|planet| !crate::seating::is_mecatol_planet(planet))
            .filter_map(|planet| {
                let system = SystemId::new(catalogue.get(planet.as_str())?.system_id()?);
                Some((system, PlanetId::new(planet)))
            })
            .find(|(system, _)| system != avoid)
            .expect("a planet elsewhere");
        state
            .system_mut(&system)
            .planet_control
            .insert(planet.clone(), who.clone());
        (system, planet)
    }

    /// Two ordinary planets of one system.
    fn two_planets() -> (SystemId, PlanetId, PlanetId) {
        let catalogue = ti4_content::galaxy::all_planets(ContentStore::embedded(), DEFAULT);
        let mut by_system: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for planet in crate::fixtures::non_home_planets(400) {
            if let Some(system) = catalogue
                .get(planet.as_str())
                .and_then(ti4_content::Planet::system_id)
            {
                by_system.entry(system.to_owned()).or_default().push(planet);
            }
        }
        let (system, planets) = by_system
            .into_iter()
            .find(|(_, planets)| planets.len() >= 2)
            .expect("a two-planet system");
        (
            SystemId::new(system),
            PlanetId::new(&planets[0]),
            PlanetId::new(&planets[1]),
        )
    }

    fn infantry(who: &PlayerId) -> Unit {
        Unit::new(ti4_model::id::UnitTypeId::new("infantry"), who.clone())
    }

    fn count_on(state: &GameState, system: &SystemId, planet: &PlanetId, kind: &str) -> usize {
        state
            .system_state(system)
            .on_planet(planet)
            .iter()
            .filter(|unit| unit.type_id.as_str() == kind)
            .count()
    }

    fn tactical_end(state: &mut GameState, table: &mut Table, actor: &str, system: &SystemId) {
        emit(
            state,
            table,
            "TACTICAL_ACTION_ENDED",
            &[
                ("player", actor.into()),
                ("system", system.to_string().into()),
            ],
        );
    }

    #[test]
    fn mercer_moves_two_ground_forces_to_a_planet_in_the_active_system() {
        let mut state = game();
        let home = state.player(&b()).unwrap().home_system.clone().unwrap();
        let (far, far_planet) = take_planet(&mut state, &b(), &home);
        crate::fixtures::put_on_planet(&mut state, &far, &far_planet, "infantry", &b(), 3);
        let landing = landing_planets(&state, &b(), &home);
        assert_eq!(landing.len(), 1, "one planet to land on: not a decision");
        let landing = landing[0].clone();
        let before = count_on(&state, &home, &landing, "infantry");
        let take = take_id(&(far.clone(), Some(far_planet.clone())), &infantry(&b()));
        let (mut table, asked) = recording(&[MERCER_WINDOW, &take, &take]);
        tactical_end(&mut state, &mut table, "b", &home);
        assert_eq!(count_on(&state, &far, &far_planet, "infantry"), 1);
        assert_eq!(count_on(&state, &home, &landing, "infantry"), before + 2);
        assert_eq!(status(&state, &a(), MERCER), Some(LeaderStatus::Exhausted));
        // The active player was the one asked (twice), and a unit the agent cannot move is no
        // option.
        let asked = asked.lock().unwrap();
        let by_b: Vec<_> = asked.iter().filter(|(who, _)| who == "b").collect();
        assert_eq!(by_b.len(), 2, "{asked:?}");
        assert!(by_b[0].1.contains(&take) && by_b[0].1.contains(&"decline".to_owned()));
    }

    #[test]
    fn mercer_takes_from_the_space_area_and_never_moves_structures() {
        let mut state = game();
        let home = state.player(&b()).unwrap().home_system.clone().unwrap();
        let landing = landing_planets(&state, &b(), &home)[0].clone();
        let (far, far_planet) = take_planet(&mut state, &b(), &home);
        crate::fixtures::put(&mut state, &far, "infantry", &b(), 1);
        crate::fixtures::put(&mut state, &far, "mech", &b(), 1);
        crate::fixtures::put_on_planet(&mut state, &far, &far_planet, "pds", &b(), 1);
        let before = count_on(&state, &home, &landing, "infantry");
        let in_space = take_id(&(far.clone(), None), &infantry(&b()));
        let (mut table, asked) = recording(&[MERCER_WINDOW, &in_space, "decline"]);
        tactical_end(&mut state, &mut table, "b", &home);
        assert_eq!(count_on(&state, &home, &landing, "infantry"), before + 1);
        assert_eq!(
            state
                .system_state(&far)
                .units
                .iter()
                .filter(|unit| unit.type_id.as_str() == "infantry")
                .count(),
            0
        );
        // Declining the second removal ended it; the structure was never offered.
        let asked = asked.lock().unwrap();
        assert!(
            asked
                .iter()
                .all(|(_, options)| options.iter().all(|id| !id.contains("pds")))
        );
        assert!(
            asked
                .iter()
                .any(|(_, options)| options.iter().any(|id| id.contains("|mech|")))
        );
    }

    #[test]
    fn mercer_moves_a_structure_that_is_also_a_ground_force() {
        // The Titans' PDS is a ground force and a structure: "ground forces" includes it.
        let mut state = game();
        let home = state.player(&b()).unwrap().home_system.clone().unwrap();
        let landing = landing_planets(&state, &b(), &home)[0].clone();
        let (far, far_planet) = take_planet(&mut state, &b(), &home);
        crate::fixtures::put_on_planet(&mut state, &far, &far_planet, "titans_pds", &b(), 1);
        let take = take_id(
            &(far.clone(), Some(far_planet.clone())),
            &Unit::new(ti4_model::id::UnitTypeId::new("titans_pds"), b()),
        );
        let (mut table, _) = recording(&[MERCER_WINDOW, &take, "decline"]);
        tactical_end(&mut state, &mut table, "b", &home);
        assert_eq!(count_on(&state, &far, &far_planet, "titans_pds"), 0);
        assert_eq!(count_on(&state, &home, &landing, "titans_pds"), 1);
        assert_eq!(status(&state, &a(), MERCER), Some(LeaderStatus::Exhausted));
    }

    #[test]
    fn mercer_asks_where_each_unit_goes_when_there_are_several_planets() {
        let mut state = game();
        let (active, first, second) = two_planets();
        for planet in [&first, &second] {
            state
                .system_mut(&active)
                .planet_control
                .insert(planet.clone(), b());
        }
        let home = state.player(&b()).unwrap().home_system.clone().unwrap();
        let at_home = landing_planets(&state, &b(), &home)[0].clone();
        let soldier = state.system_state(&home).on_planet(&at_home)[0].clone();
        let from = take_id(&(home.clone(), Some(at_home)), &soldier);
        let (mut table, asked) = recording(&[MERCER_WINDOW, &from, "decline", second.as_str()]);
        tactical_end(&mut state, &mut table, "b", &active);
        assert_eq!(
            count_on(&state, &active, &second, soldier.type_id.as_str()),
            1
        );
        assert_eq!(
            count_on(&state, &active, &first, soldier.type_id.as_str()),
            0
        );
        let asked = asked.lock().unwrap();
        let last = asked.last().unwrap();
        assert_eq!(last.0, "b");
        assert!(last.1.contains(&first.to_string()) && last.1.contains(&second.to_string()));
    }

    #[test]
    fn mercer_declined_or_unanswered_changes_nothing() {
        let mut state = game();
        let home = state.player(&b()).unwrap().home_system.clone().unwrap();
        let before = state.clone();
        // The Nomad declines.
        tactical_end(&mut state, &mut scripted(&["decline"]), "b", &home);
        assert_eq!(state, before);
        // The active player removes nothing: the card is not spent.
        tactical_end(
            &mut state,
            &mut scripted(&[MERCER_WINDOW, "decline"]),
            "b",
            &home,
        );
        assert_eq!(state, before);
        assert_eq!(status(&state, &a(), MERCER), Some(LeaderStatus::Readied));
    }

    #[test]
    fn mercer_is_not_offered_without_a_planet_a_ground_force_or_a_ready_card() {
        let home_of = |state: &GameState| state.player(&b()).unwrap().home_system.clone().unwrap();
        // A system where the active player controls no planet.
        let state = game();
        let home = home_of(&state);
        let elsewhere = crate::fixtures::plain_systems(3)
            .into_iter()
            .map(SystemId::new)
            .find(|system| *system != home && landing_planets(&state, &b(), system).is_empty())
            .unwrap();
        let offers = |state: &GameState, system: &SystemId| {
            let mut state = state.clone();
            let (mut table, asked) = recording(&[MERCER_WINDOW]);
            tactical_end(&mut state, &mut table, "b", system);
            let asked = asked.lock().unwrap();
            asked
                .iter()
                .any(|(_, options)| options.iter().any(|id| id == MERCER_WINDOW))
        };
        assert!(offers(&state, &home), "the baseline is offered");
        assert!(!offers(&state, &elsewhere), "no planet to land on");
        let mut exhausted = state.clone();
        set_status(&mut exhausted, &a(), MERCER, LeaderStatus::Exhausted);
        assert!(!offers(&exhausted, &home), "an exhausted card");
        let mut empty = state.clone();
        for board in empty.board.values_mut() {
            board.units.retain(|unit| unit.owner != b());
            for units in board.planet_units.values_mut() {
                units.retain(|unit| unit.owner != b());
            }
        }
        assert!(!offers(&empty, &home), "no ground force to move");
    }

    #[test]
    fn mercer_refuses_an_unoffered_answer_before_changing_anything() {
        let mut state = game();
        let home = state.player(&b()).unwrap().home_system.clone().unwrap();
        let before = state.clone();
        let mut resolver = crate::fixtures::armed_resolver(&state);
        let mut table = scripted(&[MERCER_WINDOW, "take|nowhere|space|infantry|0"]);
        let result = crate::fixtures::with_context(&mut state, DEFAULT, None, &mut table, |ctx| {
            let event = EventSequence::new()
                .next(
                    "TACTICAL_ACTION_ENDED",
                    payload(&[("player", "b".into()), ("system", home.to_string().into())]),
                )
                .unwrap();
            resolver.emit_with_context(ctx, event, |_, _| {})
        });
        assert!(result.is_err());
        assert_eq!(state, before, "atomic");
    }

    // -- space combat ---------------------------------------------------------------------------

    /// What a fought combat left behind.
    struct Fought {
        winner: Option<PlayerId>,
        rounds: u32,
        /// Every roll the dice recorded: reason, threshold, faces.
        rolls: Vec<(String, Option<u32>, Vec<u32>)>,
        asked: Asked,
        /// The id the next typed event would take: how many the combat allocated, plus one.
        next_event: u64,
    }

    impl Fought {
        fn rolls_for(&self, reason: &str) -> Vec<&(String, Option<u32>, Vec<u32>)> {
            self.rolls.iter().filter(|roll| roll.0 == reason).collect()
        }
        fn asked_any(&self, wanted: &str) -> bool {
            self.asked
                .lock()
                .unwrap()
                .iter()
                .any(|(_, options)| options.iter().any(|id| id.contains(wanted)))
        }
    }

    /// Fight the space combat in `system` through the real combat window, with the resolver armed
    /// as the game arms it. `faces` are the dice, in the order they are rolled.
    fn fight(state: &mut GameState, system: &SystemId, answers: &[&str], faces: &[u32]) -> Fought {
        use crate::choice::{Resolving, TimingHandle, Window};
        let content = ContentStore::embedded();
        let mut resolver = crate::fixtures::armed_resolver(state);
        let mut sequence = EventSequence::new();
        let (mut table, asked) = recording(answers);
        let mut dice = crate::dice::Dice::from_faces(faces.iter().copied());
        let mut rng = crate::rng::GameRng::new(1);
        let mut window = crate::combat::CombatWindow::new(state, content, DEFAULT, system);
        let mut ctx = Resolving {
            content,
            sources: DEFAULT,
            dice: &mut dice,
            rng: &mut rng,
            table: &mut table,
            timing: Some(TimingHandle {
                resolver: &mut resolver,
                sequence: &mut sequence,
                galaxy: None,
            }),
        };
        window.settle_open(state, &mut ctx).unwrap();
        while window.outcome().is_none() {
            window.drive(state, &mut ctx).expect("only offered answers");
            if window.outcome().is_some() {
                break;
            }
            let _ = window.take_scoring_occurrence();
            window.settle_open(state, &mut ctx).unwrap();
        }
        let outcome = window.outcome().unwrap();
        let rolls = dice
            .history()
            .iter()
            .map(|roll| (roll.reason.clone(), roll.hits_on, roll.faces.clone()))
            .collect();
        Fought {
            winner: outcome.winner,
            rounds: outcome.rounds,
            rolls,
            asked,
            next_event: sequence.next("PROBE", BTreeMap::new()).unwrap().id,
        }
    }

    fn arena() -> SystemId {
        SystemId::new("18")
    }

    fn ships(state: &GameState, system: &SystemId, who: &PlayerId, kind: &str) -> usize {
        state
            .system_state(system)
            .units
            .iter()
            .filter(|unit| &unit.owner == who && unit.type_id.as_str() == kind)
            .count()
    }

    const THUNDARIAN_SPACE: &str =
        "leader:nomad:nomadagentthundarian:SPACE_COMBAT_ROLL_STEP_ENDED:after";

    /// a (Nomad, the attacker) and b (Sol) fight with one cruiser each; both hit on 7.
    fn cruisers() -> (GameState, SystemId) {
        let mut state = game();
        state.active = Some(a());
        let system = arena();
        crate::fixtures::put(&mut state, &system, "cruiser", &a(), 1);
        crate::fixtures::put(&mut state, &system, "cruiser", &b(), 1);
        (state, system)
    }

    /// [`cruisers`] with the Thundarian spent, so the Nomad is not asked about it every round.
    fn quiet_cruisers() -> (GameState, SystemId) {
        let (mut state, system) = cruisers();
        set_status(&mut state, &a(), THUNDARIAN, LeaderStatus::Exhausted);
        (state, system)
    }

    #[test]
    fn the_thundarian_discards_the_roll_and_plays_the_step_again() {
        // First roll: both cruisers hit (a, then b). Replayed roll: only a hits.
        let faces = [10, 10, 10, 1];
        let (mut state, system) = cruisers();
        let used = fight(&mut state, &system, &[THUNDARIAN_SPACE], &faces);
        let rolled = used.rolls_for("space combat");
        assert_eq!(
            rolled.len(),
            4,
            "two rolls, discarded, then two more: {:?}",
            used.rolls
        );
        assert_eq!(rolled[0].2, [10]);
        assert_eq!(rolled[2].2, [10]);
        // The discarded roll hit both fleets and assigned nothing: b alone lost its cruiser.
        assert_eq!(used.winner, Some(a()));
        assert_eq!(ships(&state, &system, &a(), "cruiser"), 1);
        assert_eq!(ships(&state, &system, &b(), "cruiser"), 0);
        assert_eq!(used.rounds, 1, "the same round");
        assert_eq!(
            status(&state, &a(), THUNDARIAN),
            Some(LeaderStatus::Exhausted)
        );
        assert!(
            !state
                .faction_marks
                .contains_key(crate::combat::ROLL_REPLAY_MARK)
        );

        // Declined, the same first roll destroys both fleets.
        let (mut state, system) = cruisers();
        let declined = fight(&mut state, &system, &["decline"], &faces);
        assert_eq!(declined.rolls_for("space combat").len(), 2);
        assert_eq!(declined.winner, None, "both cruisers fell");
        assert_eq!(
            status(&state, &a(), THUNDARIAN),
            Some(LeaderStatus::Readied)
        );
    }

    #[test]
    fn the_thundarian_is_used_once_and_its_replay_cannot_be_replayed() {
        // The replayed roll is not offered the (exhausted) agent again.
        let (mut state, system) = cruisers();
        let fought = fight(&mut state, &system, &[THUNDARIAN_SPACE], &[1, 1, 10, 1]);
        let asked = fought.asked.lock().unwrap();
        let offers = asked
            .iter()
            .filter(|(_, options)| options.iter().any(|id| id == THUNDARIAN_SPACE))
            .count();
        assert_eq!(offers, 1, "{asked:?}");
    }

    #[test]
    fn the_thundarian_is_offered_in_a_combat_the_nomad_is_not_part_of() {
        let mut state =
            crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan"), ("c", "nomad")], DEFAULT);
        state.active = Some(PlayerId::new("a"));
        let system = arena();
        crate::fixtures::put(&mut state, &system, "cruiser", &PlayerId::new("a"), 1);
        crate::fixtures::put(&mut state, &system, "cruiser", &b(), 1);
        let fought = fight(&mut state, &system, &[THUNDARIAN_SPACE], &[10, 10, 1, 10]);
        let nomad = PlayerId::new("c");
        assert_eq!(
            status(&state, &nomad, THUNDARIAN),
            Some(LeaderStatus::Exhausted)
        );
        assert_eq!(fought.rolls_for("space combat").len(), 4);
        assert_eq!(fought.winner, Some(PlayerId::new("b")));
        // Only the Nomad holds the agent: nobody else is asked.
        let asked = fought.asked.lock().unwrap();
        assert!(asked.iter().all(|(who, options)| {
            who == "c"
                || options
                    .iter()
                    .all(|id| !id.contains("nomadagentthundarian"))
        }));
    }

    #[test]
    fn the_roll_step_event_is_emitted_only_when_a_thundarian_could_hear_it() {
        // One round either way. With the card exhausted nothing listens and nothing is emitted;
        // with it readied (and declined) the step's event is allocated once for the round.
        let faces = [10, 1, 10, 1];
        let (mut state, system) = quiet_cruisers();
        let quiet = fight(&mut state, &system, &[], &faces);
        let (mut state, system) = cruisers();
        let listening = fight(&mut state, &system, &["decline"], &faces);
        assert_eq!(quiet.rounds, 1);
        assert_eq!(listening.next_event, quiet.next_event + 1);
        // A game with no Nomad at all: no seat holds the agent.
        let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan")], DEFAULT);
        state.active = Some(a());
        let system = arena();
        crate::fixtures::put(&mut state, &system, "cruiser", &a(), 1);
        crate::fixtures::put(&mut state, &system, "cruiser", &b(), 1);
        assert!(!watches_roll_step(&state));
        let none = fight(&mut state, &system, &[], &faces);
        assert_eq!(none.next_event, quiet.next_event);
    }

    #[test]
    fn an_illegal_thundarian_answer_is_refused_without_changing_the_board() {
        use crate::choice::{Resolving, TimingHandle, Window};
        let (mut state, system) = cruisers();
        let content = ContentStore::embedded();
        let mut resolver = crate::fixtures::armed_resolver(&mut state);
        let mut sequence = EventSequence::new();
        let (mut table, _) = recording(&["not-an-offered-answer"]);
        let mut dice = crate::dice::Dice::from_faces([10, 10, 10, 10]);
        let mut rng = crate::rng::GameRng::new(1);
        let mut window = crate::combat::CombatWindow::new(&state, content, DEFAULT, &system);
        let before = (
            state.board.clone(),
            state.player(&a()).unwrap().leaders.clone(),
        );
        let mut ctx = Resolving {
            content,
            sources: DEFAULT,
            dice: &mut dice,
            rng: &mut rng,
            table: &mut table,
            timing: Some(TimingHandle {
                resolver: &mut resolver,
                sequence: &mut sequence,
                galaxy: None,
            }),
        };
        let refused = window.settle_open(&mut state, &mut ctx).is_err()
            || window.drive(&mut state, &mut ctx).is_err();
        assert!(
            refused,
            "an unoffered answer is an error, not a skipped step"
        );
        assert_eq!(
            (
                state.board.clone(),
                state.player(&a()).unwrap().leaders.clone()
            ),
            before
        );
        assert!(
            !state
                .faction_marks
                .contains_key(crate::combat::ROLL_REPLAY_MARK)
        );
    }

    #[test]
    fn the_thundarian_needs_a_ready_card() {
        let (mut state, system) = cruisers();
        set_status(&mut state, &a(), THUNDARIAN, LeaderStatus::Exhausted);
        let fought = fight(&mut state, &system, &[], &[10, 10]);
        assert!(!fought.asked_any("nomadagentthundarian"));
        assert_eq!(fought.rolls_for("space combat").len(), 2);
    }

    // -- Quantum Manipulator --------------------------------------------------------------------

    /// The nomad cruiser and mech (in the space area) face a Sol cruiser; b's die hits.
    fn mech_in_space() -> (GameState, SystemId) {
        let (mut state, system) = quiet_cruisers();
        crate::fixtures::put(&mut state, &system, "nomad_mech", &a(), 1);
        (state, system)
    }

    fn mech_index(state: &GameState, system: &SystemId) -> usize {
        state
            .system_state(system)
            .units
            .iter()
            .position(|unit| unit.type_id.as_str() == MECH)
            .unwrap()
    }

    #[test]
    fn the_mech_cancels_a_hit_against_the_nomads_ships_with_its_sustain_damage() {
        let (mut state, system) = mech_in_space();
        let sustain = format!("sustain|{}", mech_index(&state, &system));
        // Round 1: a misses, b hits. Round 2: a hits, b misses.
        let fought = fight(&mut state, &system, &[&sustain], &[1, 10, 10, 1]);
        assert_eq!(fought.winner, Some(a()), "the cruiser lived to win");
        assert_eq!(ships(&state, &system, &a(), "cruiser"), 1);
        let mech = state
            .system_state(&system)
            .units
            .iter()
            .find(|unit| unit.type_id.as_str() == MECH)
            .cloned()
            .expect("the mech is still there");
        assert!(mech.sustained_damage, "it paid with its sustain damage");
        assert_eq!(fought.rounds, 2);
        assert!(
            fought.asked_any("sustain|"),
            "the offer was made to the Nomad"
        );
    }

    #[test]
    fn without_the_mech_the_same_hit_destroys_the_cruiser() {
        let (mut state, system) = quiet_cruisers();
        let fought = fight(&mut state, &system, &[], &[1, 10, 10, 1]);
        assert_eq!(fought.winner, Some(b()));
        assert_eq!(ships(&state, &system, &a(), "cruiser"), 0);
    }

    #[test]
    fn the_mech_can_decline_and_a_damaged_mech_cancels_nothing() {
        let (mut state, system) = mech_in_space();
        let fought = fight(&mut state, &system, &["decline"], &[1, 10, 1, 1]);
        assert_eq!(fought.winner, Some(b()), "the hit was taken");
        assert!(
            !state
                .system_state(&system)
                .units
                .iter()
                .any(|unit| unit.type_id.as_str() == MECH && unit.sustained_damage)
        );
        // Already damaged: not offered again.
        let (mut state, system) = mech_in_space();
        let index = mech_index(&state, &system);
        state.system_mut(&system).units[index].sustained_damage = true;
        let fought = fight(&mut state, &system, &[], &[1, 10, 1, 1]);
        assert!(!fought.asked_any("sustain|"));
        assert_eq!(fought.winner, Some(b()));
    }

    #[test]
    fn a_mech_on_a_planet_or_a_mech_that_is_not_the_nomads_kind_cancels_no_hit() {
        // On a planet, not in a space area.
        let (mut state, system) = quiet_cruisers();
        let (planet_system, planet) = crate::fixtures::a_placed_planet();
        let _ = planet_system;
        crate::fixtures::put_on_planet(&mut state, &system, &planet, "nomad_mech", &a(), 1);
        let fought = fight(&mut state, &system, &[], &[1, 10, 1, 1]);
        assert!(!fought.asked_any("sustain|"));
        assert_eq!(fought.winner, Some(b()));
        // Another faction's mech in the space area is not given the ability.
        let (mut state, system) = quiet_cruisers();
        crate::fixtures::put(&mut state, &system, "mech", &a(), 1);
        let fought = fight(&mut state, &system, &[], &[1, 10, 1, 1]);
        assert!(!fought.asked_any("sustain|"));
    }

    #[test]
    fn the_mech_is_not_a_combat_ship_and_space_cannon_hits_are_not_part_of_a_combat() {
        let (mut state, system) = mech_in_space();
        let content = ContentStore::embedded();
        // A hit that is not part of a combat (the windowless route SPACE CANNON hits take): the
        // mech may not cancel it, so the cruiser, the only ship, is lost.
        let (mut table, asked) = recording(&[]);
        crate::combat::absorb_hits(
            &mut state,
            content,
            DEFAULT,
            &mut table,
            &a(),
            &system,
            &b(),
            1,
        )
        .unwrap();
        assert!(
            asked
                .lock()
                .unwrap()
                .iter()
                .all(|(_, options)| options.iter().all(|id| !id.starts_with("sustain|")))
        );
        assert_eq!(ships(&state, &system, &a(), "cruiser"), 0);
        assert_eq!(ships(&state, &system, &a(), MECH), 1, "never a casualty");
    }

    // -- The Cavalry ----------------------------------------------------------------------------

    const CAVALRY_PLAY: &str = "promissory:sol:cavalry:SPACE_COMBAT_STARTED:after";

    /// a (Nomad), b (Sol, who holds The Cavalry) and c (Hacan). b and c fight in the arena, b the
    /// attacker, each with one destroyer. The Nomad's own agent is spent so it is not asked about.
    fn cavalry_game() -> (GameState, SystemId) {
        let mut state =
            crate::fixtures::seated_game(&[("a", "nomad"), ("b", "sol"), ("c", "hacan")], DEFAULT);
        state.active = Some(b());
        set_status(&mut state, &a(), THUNDARIAN, LeaderStatus::Exhausted);
        crate::promissory::take(&mut state, ContentStore::embedded(), &b(), &cavalry_note());
        let system = arena();
        crate::fixtures::put(&mut state, &system, "destroyer", &b(), 1);
        crate::fixtures::put(&mut state, &system, "destroyer", &PlayerId::new("c"), 1);
        (state, system)
    }

    #[test]
    fn the_cavalry_lends_a_ship_the_flagships_combat_value_and_barrage_then_returns() {
        // Round 1: b's barrage (2 dice: the lent value, the destroyer's own dice), c's barrage (2),
        // then b's die (7), c's die.
        let faces = [1, 1, 1, 1, 7, 1];
        let (mut state, system) = cavalry_game();
        let played = fight(&mut state, &system, &[CAVALRY_PLAY], &faces);
        let barrage = played.rolls_for("anti-fighter barrage");
        assert_eq!(
            barrage[0].2.len(),
            2,
            "only the value is lent: the destroyer keeps its own 2 dice"
        );
        assert_eq!(barrage[0].1, Some(8), "Memoria's ANTI-FIGHTER BARRAGE 8");
        let combat = played.rolls_for("space combat");
        assert_eq!(
            combat[0].1,
            Some(7),
            "Memoria's combat value 7, not the destroyer's 9"
        );
        assert_eq!(
            played.winner,
            Some(b()),
            "a 7 hits only with the borrowed value"
        );
        assert_eq!(ships(&state, &system, &PlayerId::new("c"), "destroyer"), 0);
        // The card is back with the Nomad, faceup nowhere, and no mark is left.
        let note = cavalry_note();
        assert_eq!(state.promissory_notes.get(&note), Some(&a()));
        assert!(!state.promissory_faceup.contains(&note));
        assert!(!state.faction_marks.contains_key(&cavalry_key(&b())));

        // Declined: the destroyer's own 2 dice (9 and 9 (x2)), and a 7 misses.
        let (mut state, system) = cavalry_game();
        let declined = fight(&mut state, &system, &["decline"], &[1, 1, 1, 1, 1, 10]);
        assert_eq!(declined.rolls_for("anti-fighter barrage")[0].2.len(), 2);
        assert_eq!(declined.rolls_for("anti-fighter barrage")[0].1, Some(9));
        assert_eq!(declined.rolls_for("space combat")[0].1, Some(9));
        assert_eq!(declined.winner, Some(PlayerId::new("c")));
        assert_eq!(state.promissory_notes.get(&note), Some(&b()), "still held");
    }

    #[test]
    fn a_borrower_with_no_barrage_fires_the_lent_value_with_the_flagships_dice() {
        // A cruiser has no ANTI-FIGHTER BARRAGE of its own: it fires Memoria's value (8) with
        // Memoria's 3 dice. (The documented choice: no own dice to keep.)
        let mut state =
            crate::fixtures::seated_game(&[("a", "nomad"), ("b", "sol"), ("c", "hacan")], DEFAULT);
        state.active = Some(b());
        set_status(&mut state, &a(), THUNDARIAN, LeaderStatus::Exhausted);
        crate::promissory::take(&mut state, ContentStore::embedded(), &b(), &cavalry_note());
        let system = arena();
        crate::fixtures::put(&mut state, &system, "cruiser", &b(), 1);
        crate::fixtures::put(&mut state, &system, "destroyer", &PlayerId::new("c"), 1);
        let fought = fight(
            &mut state,
            &system,
            &[CAVALRY_PLAY],
            &[1, 1, 1, 1, 1, 10, 1],
        );
        let barrage = fought.rolls_for("anti-fighter barrage");
        assert_eq!((barrage[0].1, barrage[0].2.len()), (Some(8), 3));
    }

    #[test]
    fn the_cavalry_follows_the_nomads_flagship_upgrade() {
        let (mut state, system) = cavalry_game();
        state
            .player_mut(&a())
            .unwrap()
            .technologies
            .insert(ti4_model::id::TechnologyId::new("m2"));
        let fought = fight(&mut state, &system, &[CAVALRY_PLAY], &[1, 1, 1, 1, 5, 1]);
        let barrage = fought.rolls_for("anti-fighter barrage");
        assert_eq!(
            (barrage[0].1, barrage[0].2.len()),
            (Some(5), 2),
            "Memoria II"
        );
        assert_eq!(fought.rolls_for("space combat")[0].1, Some(5));
        assert_eq!(fought.winner, Some(b()));
    }

    #[test]
    fn the_cavalry_ship_can_sustain_damage_once() {
        let (mut state, system) = cavalry_game();
        let destroyer = state
            .system_state(&system)
            .units
            .iter()
            .position(|unit| unit.owner == b() && unit.type_id.as_str() == "destroyer")
            .unwrap();
        let sustain = format!("sustain|{destroyer}");
        // Round 1: b misses, c hits (the hit is cancelled). Round 2: b hits, c misses.
        let fought = fight(
            &mut state,
            &system,
            &[CAVALRY_PLAY, &sustain],
            &[1, 1, 1, 1, 1, 1, 10, 10, 1],
        );
        assert_eq!(fought.winner, Some(b()));
        assert_eq!(fought.rounds, 2);
        let survivor = state
            .system_state(&system)
            .units
            .iter()
            .find(|unit| unit.owner == b())
            .cloned()
            .expect("the destroyer lived");
        assert!(survivor.sustained_damage);
        assert!(fought.asked_any("sustain|"));
        // Without the card the same dice lose the destroyer.
        let (mut state, system) = cavalry_game();
        let fought = fight(&mut state, &system, &["decline"], &[1, 1, 1, 1, 1, 10]);
        assert_eq!(fought.winner, Some(PlayerId::new("c")));
        assert!(!fought.asked_any("sustain|"));
    }

    fn started(state: &mut GameState, table: &mut Table, attacker: &str, defender: &str) {
        emit(
            state,
            table,
            "SPACE_COMBAT_STARTED",
            &[
                ("system", "18".into()),
                ("attacker", attacker.into()),
                ("defender", defender.into()),
                ("round", 1.into()),
                ("player", attacker.into()),
            ],
        );
    }

    #[test]
    fn the_cavalry_asks_which_ship_when_there_is_a_choice() {
        let (mut state, system) = cavalry_game();
        crate::fixtures::put(&mut state, &system, "cruiser", &b(), 1);
        crate::fixtures::put(&mut state, &system, "fighter", &b(), 2);
        let (mut table, asked) = recording(&[CAVALRY_PLAY, "cruiser"]);
        started(&mut state, &mut table, "b", "c");
        assert_eq!(
            state
                .faction_marks
                .get(&cavalry_key(&b()))
                .map(String::as_str),
            Some("18|cruiser")
        );
        assert!(state.promissory_faceup.contains(&cavalry_note()));
        let asked = asked.lock().unwrap();
        let which = asked.last().unwrap();
        assert_eq!(which.0, "b");
        assert_eq!(which.1, ["cruiser", "destroyer"], "never a fighter");
        assert_eq!(asked.len(), 2, "the play, then the ship");
    }

    #[test]
    fn the_cavalry_is_played_only_against_a_player_other_than_the_nomad() {
        let offered = |state: &mut GameState, attacker: &str, defender: &str| {
            let (mut table, asked) = recording(&[CAVALRY_PLAY]);
            started(state, &mut table, attacker, defender);
            let asked = asked.lock().unwrap();
            asked
                .iter()
                .any(|(_, options)| options.iter().any(|id| id == CAVALRY_PLAY))
        };
        let (mut state, system) = cavalry_game();
        crate::fixtures::put(&mut state, &system, "carrier", &a(), 1);
        assert!(offered(&mut state.clone(), "b", "c"), "the baseline");
        assert!(!offered(&mut state.clone(), "b", "a"), "against the Nomad");
        assert!(
            !offered(&mut state.clone(), "a", "b"),
            "against the Nomad, as the defender"
        );
        assert!(
            !offered(&mut state.clone(), "a", "c"),
            "b is not in that combat"
        );

        let mut empty = state.clone();
        empty
            .system_mut(&system)
            .units
            .retain(|unit| unit.owner != b());
        crate::fixtures::put(&mut empty, &system, "fighter", &b(), 2);
        assert!(!offered(&mut empty, "b", "c"), "only fighters");

        let mut own = state.clone();
        crate::promissory::give_back(&mut own, &cavalry_note());
        assert!(!offered(&mut own, "b", "c"), "still with the Nomad");
        let mut played = state.clone();
        played.promissory_faceup.insert(cavalry_note());
        assert!(!offered(&mut played, "b", "c"), "already in play");
    }

    #[test]
    fn the_cavalry_returns_at_the_end_of_the_combat_and_only_then() {
        let (mut state, _) = cavalry_game();
        let (mut table, _) = recording(&[CAVALRY_PLAY]);
        started(&mut state, &mut table, "b", "c");
        // Another system's combat ending does not return it.
        emit(
            &mut state,
            &mut table,
            "SPACE_COMBAT_ENDED",
            &[("system", "19".into())],
        );
        assert_eq!(state.promissory_notes.get(&cavalry_note()), Some(&b()));
        emit(
            &mut state,
            &mut table,
            "SPACE_COMBAT_ENDED",
            &[("system", "18".into())],
        );
        assert_eq!(state.promissory_notes.get(&cavalry_note()), Some(&a()));
        assert!(!state.promissory_faceup.contains(&cavalry_note()));
        assert!(!state.faction_marks.contains_key(&cavalry_key(&b())));
    }

    // END-OF-TESTS
}
