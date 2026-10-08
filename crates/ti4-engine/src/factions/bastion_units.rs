//! Last Bastion units, technologies and leaders: the flagship (The Egeiro), the A3 Valiance mech,
//! the 4x41C "Helios" docks, Proxima Targeting VI, Dame Briar and Lyra Keen. The rest of the faction
//! (Galvanize itself, Liberate, Phoenix Standard, Raise the Standard, The Icon, Nip and Tuck) is in
//! `bastion.rs`. Record: `plans/evidence/BF-bastion.md`.
//!
//! Card texts (content corpus, latest printing):
//!
//! * The Egeiro, flagship: "Apply +1 to the result of each of this unit's combat rolls for each
//!   non-home system that contains a planet you control."
//! * A3 Valiance, mech: "When this unit is destroyed, if it was galvanized, galvanize up to 3 of your
//!   infantry in its system."
//! * 4x41C "Helios" V1/V2 (space dock): "The resource value of this planet is increased by 1 (V2:
//!   2). This unit's PRODUCTION value is equal to 2 (V2: 4) more than the resource value of this
//!   planet. Up to 3 fighters in this system do not count against your ships' capacity."
//! * Proxima Targeting VI (`proxima`): "Cancel 1 hit produced by BOMBARDMENT rolls made against your
//!   ground forces for each of your galvanized units present. At the start of a round of ground
//!   combat, you may resolve BOMBARDMENT 8 (x3) against your opponent's ground forces; if you do,
//!   make an identical roll against your ground forces."
//! * Dame Briar (`bastionagent`): "When a player's unit is destroyed: You may exhaust this card to
//!   galvanize another of that player's units in the destroyed unit's system." (The agent's owner
//!   chooses which unit.)
//! * Lyra Keen (`bastionhero`): "When one of your galvanized units is destroyed: You may purge this
//!   card to roll 1 die for each unit in its system that belongs to another player; if the result is
//!   equal to or greater than the galvanized unit's combat value, destroy that unit."
//!
//! Routes:
//!
//! * The Egeiro is a roll bonus ([`unit_roll_modifier`], `combat::effective_from`); the Nekro Z token
//!   lends it through `flagship_has_text`.
//! * The Helios docks are data: PRODUCTION and the 3 free fighters read the unit text
//!   (`UnitType::production`, `UnitType::fighter_support`), the upgrade is the unit upgrade of
//!   `helios2`. The planet's resource bonus is [`resource_bonus`], read by `production::planet_value_now`.
//! * Destroyed units are announced as `SHIP_DESTROYED` / `GROUND_FORCE_DESTROYED`; both carry
//!   `galvanized` (the token returns to the supply with the unit, so the event is the only record).
//! * Proxima's start-of-round roll hangs on `GROUND_COMBAT_ROUND_BEGAN`, which the live invasion
//!   window emits only while a seated player holds the technology ([`watches_ground_round`]).

use std::sync::Arc;

use ti4_content::ContentStore;
use ti4_model::content_types::SourceSet;
use ti4_model::id::{LeaderId, PlanetId, PlayerId, SystemId};
use ti4_model::state::{GameState, LeaderStatus};
use ti4_model::units::Unit;

use super::CombatUnit;
use super::bastion::{Located, galvanizable_in_system, galvanize_chosen};
use crate::event::Event;
use crate::timing::{Ability, Relation, TimingContext};

const FLAGSHIP: &str = "bastion_flagship";
const MECH: &str = "bastion_mech";
const DOCK: &str = "bastion_spacedock";
const DOCK2: &str = "bastion_spacedock2";
const PROXIMA: &str = "proxima";
const AGENT: &str = "bastionagent";
const HERO: &str = "bastionhero";
/// The event the live invasion window emits at the start of each ground combat round.
pub const ROUND_BEGAN: &str = "GROUND_COMBAT_ROUND_BEGAN";
/// BOMBARDMENT 8 (x3).
const PROXIMA_HITS_ON: u32 = 8;
const PROXIMA_DICE: usize = 3;
/// Infantry the mech galvanizes.
const MECH_INFANTRY: usize = 3;

/// Technologies claimed: Proxima Targeting VI here, the Helios V2 upgrade as data (tests).
pub const TECHNOLOGIES: &[&str] = &[PROXIMA, "helios2"];
/// Units claimed: The Egeiro, A3 Valiance and both Helios docks.
pub const UNITS: &[&str] = &[FLAGSHIP, MECH, DOCK, DOCK2];
/// Leaders claimed.
pub const LEADERS: &[&str] = &[AGENT, "bastioncommander", HERO];

fn faction_in_game(state: &GameState) -> bool {
    state
        .players
        .iter()
        .any(|seat| seat.faction.as_str() == super::bastion::FACTION)
}

// -- The Egeiro ------------------------------------------------------------------------------------

/// Non-home systems that contain a planet `player` controls. A space station is not a planet.
#[must_use]
pub fn non_home_systems_with_planets(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
) -> usize {
    let homes: std::collections::BTreeSet<&SystemId> = state
        .players
        .iter()
        .filter_map(|seat| seat.home_system.as_ref())
        .collect();
    state
        .board
        .iter()
        .filter(|(system, board)| {
            !homes.contains(system)
                && board.planet_control.iter().any(|(planet, owner)| {
                    owner == player
                        && !ti4_content::galaxy::is_space_station(content, planet.as_str(), sources)
                })
        })
        .count()
}

/// The Egeiro: +1 to each combat roll for each non-home system with a planet its owner controls.
pub(crate) fn unit_roll_modifier(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    unit: &CombatUnit<'_>,
) -> i64 {
    if unit.context != "space"
        || !faction_in_game(state)
        || !super::flagship_has_text(state, unit.player, unit.unit_type, FLAGSHIP)
    {
        return 0;
    }
    i64::try_from(non_home_systems_with_planets(
        state,
        content,
        sources,
        unit.player,
    ))
    .unwrap_or(i64::MAX)
}

// -- the Helios docks' resource bonus --------------------------------------------------------------

/// What the Helios docks add to `planet`'s resource value: +1 for V1, +2 for V2. Zero unless a Last
/// Bastion is seated and one of its docks stands on the planet. Read by `production::planet_value_now`.
#[must_use]
pub(crate) fn resource_bonus(state: &GameState, planet: &PlanetId) -> i64 {
    if !faction_in_game(state) {
        return 0;
    }
    state
        .board
        .values()
        .filter_map(|board| board.planet_units.get(planet))
        .flatten()
        .map(|unit| match unit.type_id.as_str() {
            DOCK => 1,
            DOCK2 => 2,
            _ => 0,
        })
        .max()
        .unwrap_or(0)
}

// -- Proxima: cancelling bombardment hits ------------------------------------------------------------

/// Galvanized units of `owner` standing on `planet`.
fn galvanized_present(
    state: &GameState,
    owner: &PlayerId,
    system: &SystemId,
    planet: &PlanetId,
) -> usize {
    state.board.get(system).map_or(0, |board| {
        board
            .on_planet(planet)
            .iter()
            .filter(|unit| &unit.owner == owner && unit.galvanized)
            .count()
    })
}

/// Hits left after Proxima Targeting VI's cancellation: "Cancel 1 hit produced by BOMBARDMENT rolls
/// made against your ground forces for each of your galvanized units present." `hits` are all the
/// hits one bombardment (one planet) produced against `victim`'s ground forces.
#[must_use]
pub(crate) fn cancel_bombardment_hits(
    state: &GameState,
    victim: &PlayerId,
    system: &SystemId,
    planet: &PlanetId,
    hits: usize,
) -> usize {
    if hits == 0 || !crate::technology::has_technology_text(state, victim, PROXIMA) {
        return hits;
    }
    hits.saturating_sub(galvanized_present(state, victim, system, planet))
}

/// [`cancel_bombardment_hits`] over the per-unit hit groups of one planet's bombardment: the budget
/// of cancelled hits is spent over the groups in roll order; emptied groups are dropped.
pub(crate) fn cancel_in_groups(
    state: &GameState,
    victim: &PlayerId,
    system: &SystemId,
    planet: &PlanetId,
    groups: &mut Vec<usize>,
) {
    if groups.is_empty() || !crate::technology::has_technology_text(state, victim, PROXIMA) {
        return;
    }
    let mut budget = galvanized_present(state, victim, system, planet);
    for group in groups.iter_mut() {
        let cancelled = (*group).min(budget);
        *group -= cancelled;
        budget -= cancelled;
    }
    groups.retain(|group| *group > 0);
}

/// Whether a seated player of this combat holds Proxima Targeting VI (so the round-start window is
/// worth emitting).
#[must_use]
pub(crate) fn watches_ground_round(state: &GameState, sides: &[&PlayerId]) -> bool {
    sides
        .iter()
        .any(|side| crate::technology::has_technology_text(state, side, PROXIMA))
}

/// Proxima Targeting VI at the start of a round of ground combat: "you may resolve BOMBARDMENT 8
/// (x3) against your opponent's ground forces; if you do, make an identical roll against your ground
/// forces."
///
/// Offered only when the bombardment can be used (planetary shield, Conventions of War, entropic
/// scars) and the opponent has ground forces to hit. "An identical roll" is read as a second roll of
/// the same dice (BOMBARDMENT 8, 3 dice, fresh results) whose hits land on the owner's own ground
/// forces, after the cancellation of this same technology; recorded as an open question.
fn proxima(owner_name: &str, seat: &PlayerId) -> Ability {
    let (owner, condition_owner) = (seat.clone(), seat.clone());
    Ability::stateful(
        format!("technology:{owner_name}:{PROXIMA}:{ROUND_BEGAN}:after"),
        seat.clone(),
        ROUND_BEGAN,
        Relation::After,
        Arc::new(move |event, resolver, context| {
            let Some((system, planet, opponent)) = proxima_target(
                context.state,
                context.content,
                context.sources,
                event,
                &owner,
            ) else {
                return Ok(());
            };
            let against = context.dice.roll_by(
                context.rng,
                PROXIMA_DICE,
                "proxima targeting",
                Some(PROXIMA_HITS_ON),
                &owner,
            );
            crate::invasion::assign_ground_hits_in_timing(
                resolver,
                context,
                &system,
                &planet,
                &opponent,
                against.hits(),
                "bombardment",
            )?;
            let back = context.dice.roll_by(
                context.rng,
                PROXIMA_DICE,
                "proxima targeting",
                Some(PROXIMA_HITS_ON),
                &owner,
            );
            let remaining =
                cancel_bombardment_hits(context.state, &owner, &system, &planet, back.hits());
            crate::invasion::assign_ground_hits_in_timing(
                resolver,
                context,
                &system,
                &planet,
                &owner,
                remaining,
                "bombardment",
            )?;
            Ok(())
        }),
    )
    .with_optional(true)
    .with_stateful_condition(Arc::new(move |event, _, context| {
        proxima_target(
            context.state,
            context.content,
            context.sources,
            event,
            &condition_owner,
        )
        .is_some()
    }))
}

/// The system, planet and opponent a Proxima round-start bombardment would be made against.
fn proxima_target(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    event: &Event,
    owner: &PlayerId,
) -> Option<(SystemId, PlanetId, PlayerId)> {
    if !crate::technology::has_technology_text(state, owner, PROXIMA) {
        return None;
    }
    let opponent = match (event.text("attacker"), event.text("defender")) {
        (Some(a), Some(d)) if a == owner.as_str() => PlayerId::new(d),
        (Some(a), Some(d)) if d == owner.as_str() => PlayerId::new(a),
        _ => return None,
    };
    let system = SystemId::new(event.text("system")?);
    let planet = PlanetId::new(event.text("planet")?);
    if !crate::entropic_scars::abilities_usable(content, sources, &system, Some(&system))
        || !crate::invasion::bombardable(state, content, sources, &system, &planet, owner)
    {
        return None;
    }
    let types = ti4_content::units::catalogue(content, sources);
    state
        .board
        .get(&system)?
        .on_planet(&planet)
        .iter()
        .any(|unit| {
            unit.owner == opponent
                && types
                    .get(unit.type_id.as_str())
                    .is_some_and(|kind| kind.is_ground_force())
        })
        .then_some((system, planet, opponent))
}

// -- A3 Valiance -----------------------------------------------------------------------------------

/// "When this unit is destroyed, if it was galvanized, galvanize up to 3 of your infantry in its
/// system." Not optional ("up to 3" is as many as tokens and infantry allow, never fewer).
fn valiance(owner_name: &str, seat: &PlayerId) -> Ability {
    let (owner, condition_owner) = (seat.clone(), seat.clone());
    Ability::stateful(
        format!("unit:{owner_name}:{MECH}:GROUND_FORCE_DESTROYED:after"),
        seat.clone(),
        "GROUND_FORCE_DESTROYED",
        Relation::After,
        Arc::new(move |event, _resolver, context| {
            let Some(system) = event.text("system").map(SystemId::new) else {
                return Ok(());
            };
            for _ in 0..MECH_INFANTRY {
                let candidates = infantry_to_galvanize(
                    context.state,
                    context.content,
                    context.sources,
                    &owner,
                    &system,
                );
                if !galvanize_chosen(context, &owner, "A3 Valiance", candidates)? {
                    break;
                }
            }
            Ok(())
        }),
    )
    .with_stateful_condition(Arc::new(move |event, _, context| {
        event.text("player") == Some(condition_owner.as_str())
            && event.text("unit") == Some(MECH)
            && event.boolean("galvanized") == Some(true)
            && event.text("system").is_some_and(|system| {
                !infantry_to_galvanize(
                    context.state,
                    context.content,
                    context.sources,
                    &condition_owner,
                    &SystemId::new(system),
                )
                .is_empty()
            })
    }))
}

/// `owner`'s ungalvanized infantry in `system`, one entry per distinct value.
fn infantry_to_galvanize(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    owner: &PlayerId,
    system: &SystemId,
) -> Vec<Located> {
    let types = ti4_content::units::catalogue(content, sources);
    galvanizable_in_system(state, owner, system)
        .into_iter()
        .filter(|found| {
            types
                .get(found.unit.type_id.as_str())
                .is_some_and(|kind| kind.base_type() == "infantry")
        })
        .collect()
}

// -- Dame Briar --------------------------------------------------------------------------------------

fn leader_readied(state: &GameState, owner: &PlayerId, leader: &str) -> bool {
    state
        .player(owner)
        .and_then(|seat| seat.leaders.get(&LeaderId::new(leader)))
        == Some(&LeaderStatus::Readied)
}

/// The owner and system of the unit a destroyed-unit event reports.
fn destroyed_unit(event: &Event) -> Option<(PlayerId, SystemId)> {
    Some((
        PlayerId::new(event.text("player")?),
        SystemId::new(event.text("system")?),
    ))
}

/// Dame Briar: "When a player's unit is destroyed: You may exhaust this card to galvanize another
/// of that player's units in the destroyed unit's system." The agent's owner chooses which unit.
fn dame_briar(owner_name: &str, seat: &PlayerId, event_type: &'static str) -> Ability {
    let (owner, condition_owner) = (seat.clone(), seat.clone());
    Ability::stateful(
        format!("leader:{owner_name}:{AGENT}:{event_type}:when"),
        seat.clone(),
        event_type,
        Relation::When,
        Arc::new(move |event, _resolver, context| {
            let Some((victim, system)) = destroyed_unit(event) else {
                return Ok(());
            };
            let candidates = galvanizable_in_system(context.state, &victim, &system);
            if candidates.is_empty() || !leader_readied(context.state, &owner, AGENT) {
                return Ok(());
            }
            crate::leaders::exhaust(context.state, &owner, &LeaderId::new(AGENT));
            galvanize_chosen(context, &owner, "Dame Briar", candidates)?;
            Ok(())
        }),
    )
    .with_optional(true)
    .with_stateful_condition(Arc::new(move |event, _, context| {
        leader_readied(context.state, &condition_owner, AGENT)
            && destroyed_unit(event).is_some_and(|(victim, system)| {
                !galvanizable_in_system(context.state, &victim, &system).is_empty()
            })
    }))
}

// -- Lyra Keen ---------------------------------------------------------------------------------------

/// The printed combat value of the destroyed unit type an event names.
fn destroyed_value(content: &ContentStore, sources: SourceSet, event: &Event) -> Option<u32> {
    let value = ti4_content::units::catalogue(content, sources)
        .get(event.text("unit")?)
        .and_then(ti4_content::units::UnitType::combat_hits_on)?;
    u32::try_from(value).ok()
}

/// Every unit in `system` that belongs to a seated player other than `owner`: the space area first
/// (seat order), then each planet.
fn rivals_in(
    state: &GameState,
    owner: &PlayerId,
    system: &SystemId,
) -> Vec<(Option<PlanetId>, Unit)> {
    let Some(board) = state.board.get(system) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for seat in state.players.iter().filter(|seat| seat.id != *owner) {
        for unit in board.units.iter().filter(|unit| unit.owner == seat.id) {
            found.push((None, unit.clone()));
        }
    }
    for (planet, units) in &board.planet_units {
        for seat in state.players.iter().filter(|seat| seat.id != *owner) {
            for unit in units.iter().filter(|unit| unit.owner == seat.id) {
                found.push((Some(planet.clone()), unit.clone()));
            }
        }
    }
    found
}

/// Lyra Keen: "When one of your galvanized units is destroyed: You may purge this card to roll 1 die
/// for each unit in its system that belongs to another player; if the result is equal to or greater
/// than the galvanized unit's combat value, destroy that unit."
///
/// "Destroy" ignores SUSTAIN DAMAGE. Ships are destroyed through the shared destruction route (and
/// announced), ground forces are removed and announced as `GROUND_FORCE_DESTROYED`, structures are
/// removed (no event exists for them).
fn lyra_keen(owner_name: &str, seat: &PlayerId, event_type: &'static str) -> Ability {
    let (owner, condition_owner) = (seat.clone(), seat.clone());
    let eligible = move |state: &GameState,
                         content: &ContentStore,
                         sources: SourceSet,
                         event: &Event,
                         who: &PlayerId| {
        state
            .player(who)
            .and_then(|seat| seat.leaders.get(&LeaderId::new(HERO)))
            == Some(&LeaderStatus::Unlocked)
            && event.text("player") == Some(who.as_str())
            && event.boolean("galvanized") == Some(true)
            && destroyed_value(content, sources, event).is_some()
            && event
                .text("system")
                .is_some_and(|system| !rivals_in(state, who, &SystemId::new(system)).is_empty())
    };
    Ability::stateful(
        format!("leader:{owner_name}:{HERO}:{event_type}:when"),
        seat.clone(),
        event_type,
        Relation::When,
        Arc::new(move |event, _resolver, context| {
            if !eligible(
                context.state,
                context.content,
                context.sources,
                event,
                &owner,
            ) {
                return Ok(());
            }
            let (Some(value), Some(system)) = (
                destroyed_value(context.content, context.sources, event),
                event.text("system").map(SystemId::new),
            ) else {
                return Ok(());
            };
            crate::leaders::purge(context.state, &owner, &LeaderId::new(HERO));
            let rivals = rivals_in(context.state, &owner, &system);
            let roll =
                context
                    .dice
                    .roll_by(context.rng, rivals.len(), "lyra keen", Some(value), &owner);
            let doomed: Vec<(Option<PlanetId>, Unit)> = rivals
                .into_iter()
                .zip(roll.faces.iter())
                .filter(|(_, face)| **face >= value)
                .map(|(target, _)| target)
                .collect();
            destroy_all(context, &system, doomed);
            Ok(())
        }),
    )
    .with_optional(true)
    .with_stateful_condition(Arc::new(move |event, _, context| {
        eligible(
            context.state,
            context.content,
            context.sources,
            event,
            &condition_owner,
        )
    }))
}

/// Destroy the named units of other players in `system`.
fn destroy_all(
    context: &mut TimingContext<'_>,
    system: &SystemId,
    doomed: Vec<(Option<PlanetId>, Unit)>,
) {
    let types = ti4_content::units::catalogue(context.content, context.sources);
    let mut ships: std::collections::BTreeMap<PlayerId, Vec<Unit>> =
        std::collections::BTreeMap::new();
    for (spot, unit) in doomed {
        let kind = types.get(unit.type_id.as_str());
        match spot {
            None if kind.is_some_and(|kind| kind.is_ship()) => {
                ships.entry(unit.owner.clone()).or_default().push(unit);
            }
            None => context.state.system_mut(system).remove(&[unit]),
            Some(planet) => {
                let ground = kind.is_some_and(|kind| kind.is_ground_force());
                context
                    .state
                    .system_mut(system)
                    .remove_from_planet(&planet, std::slice::from_ref(&unit));
                if ground {
                    crate::factions::hooks_ground::stage_ground_force_destroyed(
                        context.state,
                        system,
                        &planet,
                        &unit,
                        "leader_bastionhero",
                    );
                }
            }
        }
    }
    for (owner, victims) in ships {
        crate::combat::destroy_units_with_context(
            context.state,
            context.content,
            context.sources,
            &owner,
            system,
            &victims,
            "leader:bastionhero",
            false,
        );
    }
}

// -- registration ------------------------------------------------------------------------------------

/// Timing abilities of the units, technology and leaders, for one seat.
pub(crate) fn timing_abilities(
    _state: &GameState,
    owner_name: &str,
    seat: &PlayerId,
) -> Vec<Ability> {
    vec![
        valiance(owner_name, seat),
        dame_briar(owner_name, seat, "SHIP_DESTROYED"),
        dame_briar(owner_name, seat, "GROUND_FORCE_DESTROYED"),
        lyra_keen(owner_name, seat, "SHIP_DESTROYED"),
        lyra_keen(owner_name, seat, "GROUND_FORCE_DESTROYED"),
        proxima(owner_name, seat),
    ]
}

#[cfg(test)]
mod tests {
    use super::super::bastion::galvanized_on_board;
    use super::super::bastion::testkit::*;
    use super::*;
    use ti4_model::content_types::DEFAULT;
    use ti4_model::id::TechnologyId;

    const AGENT_SHIP: &str = "leader:bastion:bastionagent:SHIP_DESTROYED:when";
    const HERO_SHIP: &str = "leader:bastion:bastionhero:SHIP_DESTROYED:when";
    const HERO_GROUND: &str = "leader:bastion:bastionhero:GROUND_FORCE_DESTROYED:when";
    const PROXIMA_ID: &str = "technology:bastion:proxima:GROUND_COMBAT_ROUND_BEGAN:after";

    fn two_planets_in_distinct_systems() -> Vec<(SystemId, PlanetId)> {
        let mut found: Vec<(SystemId, PlanetId)> = Vec::new();
        for id in crate::fixtures::non_home_planets(60) {
            let planet = ti4_content::galaxy::planet(content(), &id, DEFAULT).unwrap();
            let system = SystemId::new(planet.system_id().unwrap());
            if found.iter().all(|(s, _)| *s != system)
                && !ti4_content::galaxy::is_space_station(content(), &id, DEFAULT)
            {
                found.push((system, PlanetId::new(id)));
            }
            if found.len() == 2 {
                break;
            }
        }
        found
    }

    // -- The Egeiro ------------------------------------------------------------------------------

    #[test]
    fn the_egeiro_adds_one_per_non_home_system_with_a_controlled_planet() {
        let mut state = game();
        let homes = state.player(&a()).unwrap().home_system.clone().unwrap();
        let spots = two_planets_in_distinct_systems();
        let (system, _) = &spots[0];
        crate::fixtures::put(&mut state, system, "bastion_flagship", &a(), 1);
        state.active_system = Some(system.clone());
        let flagship = unit("bastion_flagship", &a());
        let threshold = |s: &GameState| {
            crate::combat::effective_hits_on(s, content(), DEFAULT, &a(), &flagship).unwrap()
        };
        assert_eq!(
            threshold(&state),
            9,
            "no planet controlled outside a home system"
        );
        for (index, (system, planet)) in spots.iter().enumerate() {
            state.system_mut(system).set_control(planet.clone(), a());
            assert_eq!(threshold(&state), 9 - i64::try_from(index + 1).unwrap());
        }
        // A planet in a home system, and a rival's planet, add nothing; a second planet in an
        // already counted system adds nothing.
        let (home_planet, _) = state
            .board
            .get(&homes)
            .unwrap()
            .planet_control
            .iter()
            .next()
            .map(|(p, o)| (p.clone(), o.clone()))
            .unwrap();
        assert_eq!(state.board[&homes].planet_control[&home_planet], a());
        assert_eq!(threshold(&state), 7, "the home planets are not counted");
        let other = crate::combat::effective_hits_on(
            &state,
            content(),
            DEFAULT,
            &b(),
            &unit("bastion_flagship", &b()),
        )
        .unwrap();
        assert_eq!(
            other, 9,
            "another player's copy counts its own planets only"
        );
    }

    #[test]
    fn a_nekro_flagship_with_the_z_token_gains_the_egeiros_bonus() {
        let mut state =
            crate::fixtures::nekro_with_z(&[("a", "nekro"), ("b", "bastion")], &["bastion"]);
        let spots = two_planets_in_distinct_systems();
        for (system, planet) in &spots {
            state.system_mut(system).set_control(planet.clone(), a());
        }
        let modifier = unit_roll_modifier(
            &state,
            content(),
            DEFAULT,
            &CombatUnit {
                player: &a(),
                system: Some(&spots[0].0),
                planet: None,
                unit_type: "nekro_flagship",
                context: "space",
            },
        );
        assert_eq!(modifier, 2);
        let plain = crate::fixtures::nekro_with_z(&[("a", "nekro"), ("b", "bastion")], &[]);
        assert_eq!(
            unit_roll_modifier(
                &plain,
                content(),
                DEFAULT,
                &CombatUnit {
                    player: &a(),
                    system: None,
                    planet: None,
                    unit_type: "nekro_flagship",
                    context: "space",
                },
            ),
            0,
            "no token, no text"
        );
    }

    // -- the Helios docks ------------------------------------------------------------------------

    #[test]
    fn the_helios_docks_add_to_resources_production_and_free_fighters() {
        let mut state = game();
        let (system, planet) = arena();
        state.system_mut(&system).units.clear();
        state.system_mut(&system).planet_units.clear();
        state.system_mut(&system).set_control(planet.clone(), a());
        let printed = crate::production::planet_value(
            content(),
            DEFAULT,
            &planet,
            crate::production::Spend::Resources,
        );
        crate::fixtures::put_on_planet(&mut state, &system, &planet, "bastion_spacedock", &a(), 1);
        let now = |s: &GameState| {
            crate::production::planet_value_now(
                s,
                content(),
                DEFAULT,
                &planet,
                crate::production::Spend::Resources,
            )
        };
        assert_eq!(now(&state), printed + 1);
        assert_eq!(
            crate::production::capacity(&state, content(), DEFAULT, &a(), &system),
            printed + 1 + 2,
            "PRODUCTION is 2 more than the (increased) resource value"
        );
        // Three fighters sit in space with no carrier: free; a fourth is not.
        crate::fixtures::put(&mut state, &system, "fighter", &a(), 3);
        let excess = |s: &GameState| {
            crate::fleet::standing(s, content(), DEFAULT, &a(), &system, None).capacity_excess
        };
        assert_eq!(excess(&state), 0, "3 fighters are supported");
        crate::fixtures::put(&mut state, &system, "fighter", &a(), 1);
        assert_eq!(excess(&state), 1, "the fourth is not");
        // Helios V2 through the real upgrade route.
        crate::technology::grant(&mut state, &a(), &TechnologyId::new("helios2"));
        crate::technology::apply_unit_upgrades(&mut state, content(), DEFAULT, &a());
        assert!(
            state
                .system_state(&system)
                .on_planet(&planet)
                .iter()
                .any(|u| u.type_id.as_str() == "bastion_spacedock2"),
            "the dock is upgraded in place"
        );
        assert_eq!(now(&state), printed + 2);
        assert_eq!(
            crate::production::capacity(&state, content(), DEFAULT, &a(), &system),
            printed + 2 + 4
        );
        // Another planet is unaffected, and so is another faction's dock.
        assert_eq!(resource_bonus(&state, &PlanetId::new("nowhere")), 0);
    }

    // -- the mech --------------------------------------------------------------------------------

    fn mech_lost(
        system: &SystemId,
        planet: &PlanetId,
        galvanized: bool,
    ) -> Vec<(&'static str, serde_json::Value)> {
        let mut payload = vec![
            ("system", system.to_string().into()),
            ("planet", planet.to_string().into()),
            ("player", "a".into()),
            ("unit", "bastion_mech".into()),
            ("damaged", false.into()),
            ("cause", "ground_combat".into()),
        ];
        if galvanized {
            payload.push(("galvanized", true.into()));
        }
        payload
    }

    #[test]
    fn a_destroyed_galvanized_mech_galvanizes_up_to_three_infantry_in_its_system() {
        let (system, planet) = arena();
        let mut state = game();
        crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &a(), 5);
        emit(
            &mut state,
            &["decline"],
            "GROUND_FORCE_DESTROYED",
            &mech_lost(&system, &planet, true),
        );
        assert_eq!(galvanized_count(&state, &a(), "infantry"), 3);
        // An ungalvanized mech does nothing.
        let mut plain = game();
        crate::fixtures::put_on_planet(&mut plain, &system, &planet, "infantry", &a(), 5);
        emit(
            &mut plain,
            &["decline"],
            "GROUND_FORCE_DESTROYED",
            &mech_lost(&system, &planet, false),
        );
        assert_eq!(galvanized_count(&plain, &a(), "infantry"), 0);
        // Fewer infantry, fewer tokens: as many as can be.
        let mut few = game();
        crate::fixtures::put_on_planet(&mut few, &system, &planet, "infantry", &a(), 2);
        emit(
            &mut few,
            &["decline"],
            "GROUND_FORCE_DESTROYED",
            &mech_lost(&system, &planet, true),
        );
        assert_eq!(galvanized_count(&few, &a(), "infantry"), 2);
        let mut tight = game();
        crate::fixtures::put_on_planet(&mut tight, &system, &planet, "infantry", &a(), 5);
        let (other, _) = (SystemId::new("19"), 0);
        put_galvanized(&mut tight, &other, "cruiser", &b(), 6);
        emit(
            &mut tight,
            &["decline"],
            "GROUND_FORCE_DESTROYED",
            &mech_lost(&system, &planet, true),
        );
        assert_eq!(
            galvanized_count(&tight, &a(), "infantry"),
            1,
            "one token left"
        );
    }

    // -- Dame Briar ------------------------------------------------------------------------------

    fn ship_lost(
        system: &SystemId,
        player: &str,
        unit: &str,
        galvanized: bool,
    ) -> Vec<(&'static str, serde_json::Value)> {
        let mut payload = vec![
            ("system", system.to_string().into()),
            ("player", player.into()),
            ("unit", unit.into()),
            ("last", false.into()),
            ("cause", "space_combat".into()),
            ("during_space_combat", true.into()),
        ];
        if galvanized {
            payload.push(("galvanized", true.into()));
        }
        payload
    }

    #[test]
    fn dame_briar_galvanizes_another_unit_of_the_destroyed_units_owner_by_the_agents_choice() {
        let (system, _) = arena();
        let mut state = game();
        crate::fixtures::put(&mut state, &system, "cruiser", &b(), 1);
        crate::fixtures::put(&mut state, &system, "destroyer", &b(), 1);
        let pick = format!("{system}|space|destroyer|undamaged");
        emit(
            &mut state,
            &[AGENT_SHIP, &pick],
            "SHIP_DESTROYED",
            &ship_lost(&system, "b", "cruiser", false),
        );
        assert_eq!(galvanized_count(&state, &b(), "destroyer"), 1);
        assert_eq!(galvanized_count(&state, &b(), "cruiser"), 0);
        assert_eq!(
            state.player(&a()).unwrap().leaders[&LeaderId::new(AGENT)],
            LeaderStatus::Exhausted
        );
        // Exhausted: not offered again.
        emit(
            &mut state,
            &[AGENT_SHIP],
            "SHIP_DESTROYED",
            &ship_lost(&system, "b", "cruiser", false),
        );
        assert_eq!(galvanized_count(&state, &b(), "cruiser"), 0);
        // May: declining spends nothing.
        let mut declined = game();
        crate::fixtures::put(&mut declined, &system, "cruiser", &b(), 1);
        emit(
            &mut declined,
            &["decline"],
            "SHIP_DESTROYED",
            &ship_lost(&system, "b", "cruiser", false),
        );
        assert_eq!(galvanized_count(&declined, &b(), "cruiser"), 0);
        assert_eq!(
            declined.player(&a()).unwrap().leaders[&LeaderId::new(AGENT)],
            LeaderStatus::Readied
        );
        // Nothing else of theirs in the system: not offered at all.
        let mut alone = game();
        emit(
            &mut alone,
            &[AGENT_SHIP],
            "SHIP_DESTROYED",
            &ship_lost(&system, "b", "cruiser", false),
        );
        assert_eq!(
            alone.player(&a()).unwrap().leaders[&LeaderId::new(AGENT)],
            LeaderStatus::Readied
        );
    }

    #[test]
    fn dame_briar_answers_a_ground_force_loss_too() {
        let (system, planet) = arena();
        let mut state = game();
        crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &c(), 2);
        let mut payload = ship_lost(&system, "c", "infantry", false);
        payload.push(("planet", planet.to_string().into()));
        emit(
            &mut state,
            &["leader:bastion:bastionagent:GROUND_FORCE_DESTROYED:when"],
            "GROUND_FORCE_DESTROYED",
            &payload,
        );
        assert_eq!(galvanized_count(&state, &c(), "infantry"), 1);
    }

    // -- Lyra Keen -------------------------------------------------------------------------------

    fn unlock_hero(state: &mut GameState) {
        state
            .player_mut(&a())
            .unwrap()
            .leaders
            .insert(LeaderId::new(HERO), LeaderStatus::Unlocked);
    }

    #[test]
    fn lyra_keen_rolls_a_die_per_rival_unit_and_destroys_those_that_meet_the_value() {
        let (system, planet) = arena();
        let mut state = game();
        unlock_hero(&mut state);
        crate::fixtures::put(&mut state, &system, "cruiser", &b(), 1);
        crate::fixtures::put(&mut state, &system, "carrier", &c(), 1);
        crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &b(), 1);
        crate::fixtures::put(&mut state, &system, "cruiser", &a(), 1);
        // The destroyed galvanized cruiser (combat value 7): the dice are b's cruiser, c's carrier,
        // b's infantry.
        let dice = emit_rolling(
            &mut state,
            &mut scripted(&[HERO_SHIP]),
            &[10, 3, 7],
            "SHIP_DESTROYED",
            &ship_lost(&system, "a", "cruiser", true),
        );
        let roll = &dice.history()[0];
        assert_eq!(roll.faces, vec![10, 3, 7]);
        assert_eq!(
            state
                .system_state(&system)
                .units
                .iter()
                .filter(|u| u.owner != a())
                .count(),
            1,
            "b's cruiser (10) died, c's carrier (3) lived"
        );
        assert!(
            state.system_state(&system).on_planet(&planet).is_empty(),
            "7 meets 7"
        );
        assert_eq!(state.pending_destructions.len(), 1, "the ship is announced");
        assert!(crate::factions::hooks_ground::has_staged_events(&state));
        assert_eq!(
            state.player(&a()).unwrap().leaders[&LeaderId::new(HERO)],
            LeaderStatus::Purged
        );
    }

    #[test]
    fn lyra_keen_needs_a_galvanized_loss_of_its_owner_and_an_unlocked_card() {
        let (system, planet) = arena();
        let build = |unlocked: bool| {
            let mut state = game();
            if unlocked {
                unlock_hero(&mut state);
            }
            crate::fixtures::put(&mut state, &system, "cruiser", &b(), 1);
            state
        };
        for (unlocked, galvanized, owner) in
            [(false, true, "a"), (true, false, "a"), (true, true, "c")]
        {
            let mut state = build(unlocked);
            let dice = emit_rolling(
                &mut state,
                &mut scripted(&[HERO_SHIP]),
                &[10],
                "SHIP_DESTROYED",
                &ship_lost(&system, owner, "cruiser", galvanized),
            );
            assert!(dice.history().is_empty(), "{unlocked} {galvanized} {owner}");
            assert_eq!(state.system_state(&system).units.len(), 1);
        }
        // A ground force of the owner's, galvanized: the ground window works too.
        let mut state = build(true);
        crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &c(), 1);
        let mut payload = mech_lost(&system, &planet, true);
        payload.retain(|(key, _)| *key != "unit");
        payload.push(("unit", "infantry".into()));
        let dice = emit_rolling(
            &mut state,
            &mut scripted(&[HERO_GROUND]),
            &[10, 10],
            "GROUND_FORCE_DESTROYED",
            &payload,
        );
        assert_eq!(dice.history()[0].faces.len(), 2);
        assert!(state.system_state(&system).units.is_empty());
    }

    // -- Proxima Targeting VI --------------------------------------------------------------------

    fn proxima_holder_game() -> GameState {
        let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "bastion")], DEFAULT);
        state
            .player_mut(&b())
            .unwrap()
            .technologies
            .insert(TechnologyId::new(PROXIMA));
        state
    }

    #[test]
    fn proxima_cancels_one_bombardment_hit_per_galvanized_unit_on_the_planet() {
        let (system, planet) = arena();
        let mut state = proxima_holder_game();
        state.system_mut(&system).units.clear();
        state.system_mut(&system).planet_units.clear();
        state.system_mut(&system).set_control(planet.clone(), b());
        crate::fixtures::put(&mut state, &system, "dreadnought", &a(), 3);
        crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &b(), 4);
        put_galvanized_on(&mut state, &system, &planet, "infantry", &b(), 1);
        let mut dice = crate::dice::Dice::from_faces([10, 10, 10]);
        let mut rng = crate::rng::GameRng::new(0);
        crate::invasion::bombardment(
            &mut state,
            content(),
            DEFAULT,
            &mut dice,
            &mut rng,
            &mut scripted(&[]),
            &system,
            &a(),
        )
        .expect("bombards");
        let left = state
            .system_state(&system)
            .on_planet_of(&planet, &b())
            .len();
        assert_eq!(
            left,
            5 - 2,
            "3 hits, 1 cancelled by the galvanized infantry"
        );
        // No Proxima: all 3 land.
        let mut bare = game_without_proxima(&system, &planet);
        let mut dice = crate::dice::Dice::from_faces([10, 10, 10]);
        crate::invasion::bombardment(
            &mut bare,
            content(),
            DEFAULT,
            &mut dice,
            &mut rng,
            &mut scripted(&[]),
            &system,
            &a(),
        )
        .expect("bombards");
        assert_eq!(
            bare.system_state(&system).on_planet_of(&planet, &b()).len(),
            5 - 3
        );
    }

    fn game_without_proxima(system: &SystemId, planet: &PlanetId) -> GameState {
        let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "bastion")], DEFAULT);
        state.system_mut(system).units.clear();
        state.system_mut(system).planet_units.clear();
        state.system_mut(system).set_control(planet.clone(), b());
        crate::fixtures::put(&mut state, system, "dreadnought", &a(), 3);
        crate::fixtures::put_on_planet(&mut state, system, planet, "infantry", &b(), 4);
        put_galvanized_on(&mut state, system, planet, "infantry", &b(), 1);
        state
    }

    /// The live invasion window over `system`, armed as the game arms it, driven with first
    /// options; returns the dice for their rolls.
    fn invade(
        state: &mut GameState,
        invader: &PlayerId,
        system: &SystemId,
        prefer: &'static str,
        faces: &[u32],
    ) -> crate::dice::Dice {
        use crate::choice::{Resolving, TimingHandle, Window};
        let mut resolver = crate::fixtures::armed_resolver(state);
        let mut sequence = crate::event::EventSequence::new();
        let mut table = crate::choice::Table::with_default(Box::new(Prefer(prefer)));
        let mut dice =
            crate::dice::Dice::from_faces(faces.iter().copied().chain(std::iter::repeat_n(1, 60)));
        let mut rng = crate::rng::GameRng::new(3);
        let mut window = crate::invasion::InvasionWindow::at_commit_step(state, invader, system);
        let mut ctx = Resolving {
            content: content(),
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
        window.settle(state, &mut ctx);
        for _ in 0..30 {
            let Some(choice) = window.pending_choice(state, content(), DEFAULT) else {
                break;
            };
            let option = choice.options[0].clone();
            window.resolve(state, &mut ctx, option).expect("resolves");
            window.settle(state, &mut ctx);
        }
        drop(ctx);
        dice
    }

    #[test]
    fn proxima_bombards_at_the_start_of_a_live_ground_combat_round_and_back() {
        let (system, planet) = arena();
        let mut state = crate::fixtures::seated_game(&[("a", "bastion"), ("b", "sol")], DEFAULT);
        state
            .player_mut(&a())
            .unwrap()
            .technologies
            .insert(TechnologyId::new(PROXIMA));
        {
            let board = state.system_mut(&system);
            board.units.clear();
            board.planet_units.clear();
            board.planet_control.clear();
            board.planet_control.insert(planet.clone(), b());
        }
        crate::fixtures::put(&mut state, &system, "infantry", &a(), 3);
        crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &b(), 2);
        // Proxima: 8 8 1 (2 hits on b), then the identical roll against a: 8 1 1 (1 hit on a);
        // the ground dice that follow find the defenders already dead.
        let dice = invade(&mut state, &a(), &system, PROXIMA_ID, &[8, 8, 1, 8, 1, 1]);
        let rolls: Vec<_> = dice
            .history()
            .iter()
            .filter(|roll| roll.reason == "proxima targeting")
            .collect();
        assert_eq!(rolls.len(), 2);
        assert_eq!(rolls[0].faces, vec![8, 8, 1]);
        assert_eq!(rolls[1].faces, vec![8, 1, 1]);
        assert!(
            state
                .system_state(&system)
                .on_planet_of(&planet, &b())
                .is_empty(),
            "both defenders fell to the first roll"
        );
        // One of a's three infantry fell to the roll back, and the rest took the planet.
        let held = state
            .system_state(&system)
            .on_planet_of(&planet, &a())
            .len();
        // 3 landed, 1 fell to the identical roll; taking the planet then Liberates it.
        let resources = crate::production::planet_value_now(
            &state,
            content(),
            DEFAULT,
            &planet,
            crate::production::Spend::Resources,
        );
        assert_eq!(
            held,
            if resources <= 2 { 2 } else { 3 },
            "resources {resources}"
        );
        assert_eq!(
            state.system_state(&system).planet_control.get(&planet),
            Some(&a())
        );
    }

    #[test]
    fn proxima_is_optional_and_never_emitted_without_the_technology() {
        let (system, planet) = arena();
        let build = |tech: bool| {
            let mut state =
                crate::fixtures::seated_game(&[("a", "bastion"), ("b", "sol")], DEFAULT);
            if tech {
                state
                    .player_mut(&a())
                    .unwrap()
                    .technologies
                    .insert(TechnologyId::new(PROXIMA));
            }
            let board = state.system_mut(&system);
            board.units.clear();
            board.planet_units.clear();
            board.planet_control.clear();
            board.planet_control.insert(planet.clone(), b());
            crate::fixtures::put(&mut state, &system, "infantry", &a(), 1);
            crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &b(), 1);
            state
        };
        let mut declined = build(true);
        let dice = invade(&mut declined, &a(), &system, "decline", &[10, 1]);
        assert!(
            dice.history()
                .iter()
                .all(|roll| roll.reason != "proxima targeting")
        );
        let mut without = build(false);
        let dice = invade(&mut without, &a(), &system, PROXIMA_ID, &[10, 1]);
        assert!(
            dice.history()
                .iter()
                .all(|roll| roll.reason != "proxima targeting")
        );
    }

    /// Takes the named option when offered, else declines, else the first option.
    struct Prefer(&'static str);

    impl crate::choice::Decider for Prefer {
        fn choose(
            &mut self,
            choice: &crate::choice::Choice,
        ) -> Result<crate::choice::ChoiceOption, crate::choice::IllegalChoice> {
            if let Some(wanted) = choice.option(self.0) {
                return Ok(wanted.clone());
            }
            Ok(choice
                .options
                .iter()
                .find(|option| option.is_decline())
                .unwrap_or(&choice.options[0])
                .clone())
        }
    }

    // -- real combat: the galvanized flag survives the destruction -----------------------------------

    #[test]
    fn a_galvanized_ship_lost_in_a_real_space_combat_is_reported_galvanized_and_gives_the_hero_a_window()
     {
        use crate::choice::{Resolving, TimingHandle, Window};
        use std::sync::{Arc, Mutex};
        let (system, _) = arena();
        let mut state = game();
        state.system_mut(&system).units.clear();
        state.system_mut(&system).planet_units.clear();
        unlock_hero(&mut state);
        put_galvanized(&mut state, &system, "cruiser", &a(), 1);
        crate::fixtures::put(&mut state, &system, "dreadnought", &b(), 4);
        let seen: Arc<Mutex<Vec<(String, bool)>>> = Arc::default();
        let log = Arc::clone(&seen);
        let mut resolver = crate::fixtures::armed_resolver(&state);
        resolver.register([crate::timing::Ability::new(
            "probe:SHIP_DESTROYED",
            c(),
            "SHIP_DESTROYED",
            Relation::When,
            Arc::new(move |event, _| {
                log.lock().unwrap().push((
                    event.text("player").unwrap_or_default().to_owned(),
                    event.boolean("galvanized") == Some(true),
                ));
                Ok(())
            }),
        )]);
        let mut sequence = crate::event::EventSequence::new();
        let mut table = crate::choice::Table::with_default(Box::new(Prefer(HERO_SHIP)));
        let mut dice = crate::dice::Dice::new();
        let mut rng = crate::rng::GameRng::new(5);
        let mut window = crate::combat::CombatWindow::new(&state, content(), DEFAULT, &system);
        let mut ctx = Resolving {
            content: content(),
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
        window.settle_open(&mut state, &mut ctx).expect("opens");
        while window.outcome().is_none() {
            window.drive(&mut state, &mut ctx).expect("drives");
            if window.outcome().is_some() {
                break;
            }
            let _ = window.take_scoring_occurrence();
            window.settle_open(&mut state, &mut ctx).expect("settles");
        }
        let seen = seen.lock().unwrap().clone();
        assert!(seen.contains(&("a".to_owned(), true)), "{seen:?}");
        assert!(
            ctx.dice
                .history()
                .iter()
                .any(|roll| roll.reason == "lyra keen"),
            "the hero was offered and used"
        );
        assert_eq!(
            galvanized_on_board(&state),
            0,
            "the token went back with the ship"
        );
        assert_eq!(
            state.player(&a()).unwrap().leaders[&LeaderId::new(HERO)],
            LeaderStatus::Purged
        );
    }
}
