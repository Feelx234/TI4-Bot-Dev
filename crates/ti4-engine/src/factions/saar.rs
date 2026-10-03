//! The Clan of Saar (`saar`). See `factions/mod.rs` for the contract and
//! `plans/BASE_FACTIONS_PLAN_2026-10-02.md` for scope; the per-item record is
//! `plans/evidence/BF-saar.md`.
//!
//! Card texts (latest printing, `crates/ti4-content/content/*.json`):
//!
//! * Nomadic: "You can score objectives even if you do not control the planets in your home system."
//! * Scavenge: "After you gain control of a planet: Gain 1 trade good."
//! * Scavenger Zeta (mech): "DEPLOY: After you gain control of a planet, you may spend 1 trade
//!   good to place 1 mech on that planet"
//! * Ragh's Call (`ragh`): "After you commit 1 or more units to land on a planet: Remove all of
//!   the Saar player's ground forces from that planet and place them on a planet controlled by the
//!   Saar player. Then, return this card to the Saar player."
//! * Chaos Mapping (`cm`): "Other players cannot activate asteroid fields that contain 1 or more
//!   of your ships. At the start of your turn during the action phase, you may produce 1 unit in a
//!   system that contains at least 1 of your units that has PRODUCTION."
//! * Gurno Aggero (`saarhero`): "ACTION: Choose 1 system that is adjacent to 1 of your space
//!   docks. Destroy all other players' infantry and fighters in that system. Then, purge this
//!   card." Unlock: "Have 3 scored objectives."
//! * Rowl Sarring (`saarcommander`) unlock: "Have 3 space docks on the game board."
//! * Son of Ragh (flagship): ANTI-FIGHTER BARRAGE 6 (x4) -- statistics only.

use std::sync::Arc;

use ti4_content::ContentStore;
use ti4_content::units::{UnitType, catalogue};
use ti4_model::content_types::SourceSet;
use ti4_model::id::{LeaderId, PlanetId, PlayerId, SystemId, UnitTypeId};
use ti4_model::state::{GameState, LeaderStatus};
use ti4_model::units::Unit;

use super::hooks_movement::{MoveSite, MovementHooks};
use super::hooks_strategy::StrategyHooks;
use super::{FactionModule, Hooks};
use crate::choice::{Choice, ChoiceOption, Resolving, TimingHandle, Window};
use crate::decision_context::{DecisionContext, DecisionSource};
use crate::production::ProductionWindow;
use crate::timing::{Ability, Relation, Resolver, TimingContext, TimingError};

/// The faction alias; also the faction name in promissory note ids (`ragh:saar`).
const FACTION: &str = "saar";
/// Ragh's Call's note id.
const RAGH_NOTE: &str = "ragh:saar";

/// What this faction implements.
pub const MODULE: FactionModule = FactionModule {
    alias: FACTION,
    abilities: &["nomadic", "scavenge"],
    technologies: &["cm", "ffac2"],
    units: &[
        "saar_flagship",
        "saar_mech",
        "saar_spacedock",
        "saar_spacedock2",
    ],
    promissory: &["ragh"],
    leaders: &["saarhero", "saaragent"],
    breakthroughs: &[],
    hooks: Hooks {
        timing_abilities: Some(timing_abilities),
        commander_unlocked: Some(commander_unlocked),
        leader_action: Some(leader_action),
        use_leader: Some(use_leader),
        movement: MovementHooks {
            cannot_activate: Some(chaos_mapping_blocks_activation),
            move_bonus: Some(agent_move_bonus),
            ..MovementHooks::NONE
        },
        strategy: StrategyHooks {
            scores_without_home: Some(nomadic),
            ..StrategyHooks::NONE
        },
        ..Hooks::NONE
    },
};

// -- small readers -------------------------------------------------------------------------------

fn is_saar(state: &GameState, player: &PlayerId) -> bool {
    state
        .player(player)
        .is_some_and(|seat| seat.faction.as_str() == FACTION)
}

fn has_technology(state: &GameState, player: &PlayerId, alias: &str) -> bool {
    state.player(player).is_some_and(|seat| {
        seat.technologies
            .contains(&ti4_model::id::TechnologyId::new(alias))
    })
}

/// The Saar seat, if one is playing.
fn saar_seat(state: &GameState) -> Option<PlayerId> {
    state
        .players
        .iter()
        .find(|seat| seat.faction.as_str() == FACTION)
        .map(|seat| seat.id.clone())
}

fn illegal(error: crate::choice::IllegalChoice) -> TimingError {
    TimingError::IllegalChoice(error)
}

fn ask(
    context: &mut TimingContext<'_>,
    player: &PlayerId,
    prompt: String,
    ability: &str,
    subtype: &str,
    mut options: Vec<ChoiceOption>,
    optional: bool,
) -> Result<ChoiceOption, crate::choice::IllegalChoice> {
    if optional {
        options.push(ChoiceOption::decline());
    }
    let choice = Choice::new(player.clone(), prompt, options).contextualized(DecisionContext::new(
        player.clone(),
        DecisionSource::FactionAbility(ability.to_owned()),
        subtype,
        context.state.phase,
        context.state.round,
    ));
    context.ask_seeing(&choice)
}

/// Every unit of `player` (space and planets) whose base type is `base`, with where it stands.
fn units_of_base(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    base: &str,
) -> Vec<(SystemId, Option<PlanetId>, Unit)> {
    let types = catalogue(content, sources);
    let is = |unit: &Unit| {
        &unit.owner == player
            && types
                .get(unit.type_id.as_str())
                .is_some_and(|kind| kind.base_type() == base)
    };
    let mut found = Vec::new();
    for (system, board) in &state.board {
        for unit in board.units.iter().filter(|unit| is(unit)) {
            found.push((system.clone(), None, unit.clone()));
        }
        for (planet, units) in &board.planet_units {
            for unit in units.iter().filter(|unit| is(unit)) {
                found.push((system.clone(), Some(planet.clone()), unit.clone()));
            }
        }
    }
    found
}

fn leader_status(state: &GameState, player: &PlayerId, leader: &str) -> Option<LeaderStatus> {
    crate::leaders::status(state, player, &LeaderId::new(leader))
}

// -- Rowl Sarring unlock -------------------------------------------------------------------------

/// "Have 3 space docks on the game board." A Floating Factory is a space dock (same base type).
fn commander_unlocked(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    _galaxy: Option<&ti4_content::galaxy::Galaxy>,
    player: &PlayerId,
    leader: &LeaderId,
) -> Option<bool> {
    if leader.as_str() != "saarcommander" {
        return None;
    }
    Some(units_of_base(state, content, sources, player, "spacedock").len() >= 3)
}

/// Rowl Sarring: "When you produce fighters or infantry, you may place each of those units at any
/// of your space docks that are not in or adjacent to a system that contains another player's
/// units." The docks that qualify, as `(system, planet)`; `planet` is `None` for a Floating Factory
/// in the space area (fighters only). Needs the unlocked commander; the shared production placement
/// hook (see `plans/evidence/BF-saar.md`, Hook requests) would call this, so it is not yet claimed.
#[allow(
    dead_code,
    reason = "waits for the shared production placement hook; covered by tests"
)]
fn commander_docks(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    galaxy: &ti4_content::galaxy::Galaxy,
    player: &PlayerId,
) -> Vec<(SystemId, Option<PlanetId>)> {
    if !is_saar(state, player)
        || leader_status(state, player, "saarcommander") != Some(LeaderStatus::Unlocked)
    {
        return Vec::new();
    }
    let adjacency = crate::movement::PlayerAdjacency::new(state, content, sources, galaxy, player);
    let occupied = |system: &SystemId| {
        let board = state.system_state(system);
        board
            .units
            .iter()
            .any(|unit| &unit.owner != player && !crate::neutral_units::is_neutral(&unit.owner))
            || board.planet_units.values().any(|units| {
                units.iter().any(|unit| {
                    &unit.owner != player && !crate::neutral_units::is_neutral(&unit.owner)
                })
            })
    };
    units_of_base(state, content, sources, player, "spacedock")
        .into_iter()
        .filter(|(system, _, _)| {
            !occupied(system)
                && !adjacency
                    .neighbours(system.as_str())
                    .iter()
                    .any(|n| occupied(&SystemId::new(n.clone())))
        })
        .map(|(system, planet, _)| (system, planet))
        .collect()
}

// -- timing abilities ----------------------------------------------------------------------------

fn timing_abilities(_state: &GameState, owner_name: &str, seat: &PlayerId) -> Vec<Ability> {
    vec![
        scavenge(owner_name, seat),
        scavenger_zeta(owner_name, seat),
        ragh_call(owner_name, seat),
        chaos_mapping_production(owner_name, seat),
        agent(owner_name, seat),
    ]
}

fn gained_by(event: &crate::event::Event, player: &PlayerId) -> Option<(SystemId, PlanetId)> {
    if event.text("player") != Some(player.as_str()) {
        return None;
    }
    Some((
        SystemId::new(event.text("system")?),
        PlanetId::new(event.text("planet")?),
    ))
}

/// Scavenge: "After you gain control of a planet: Gain 1 trade good." Mandatory.
fn scavenge(owner_name: &str, seat: &PlayerId) -> Ability {
    let owner = seat.clone();
    let condition_owner = seat.clone();
    Ability::stateful(
        format!("ability:{owner_name}:scavenge:PLANET_CONTROL_GAINED:after"),
        seat.clone(),
        "PLANET_CONTROL_GAINED",
        Relation::After,
        Arc::new(move |_event, resolver, context| {
            crate::supply::gain_trade_goods_via(context, resolver, &owner, 1, "scavenge")?;
            Ok(())
        }),
    )
    .with_stateful_condition(Arc::new(move |event, _, context| {
        crate::faction_abilities::has(context.state, context.content, &condition_owner, "scavenge")
            && gained_by(event, &condition_owner).is_some()
    }))
}

fn mech_id(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
) -> Option<UnitTypeId> {
    let faction = state.player(player)?.faction.to_string();
    ti4_content::units::faction_unit(content, &faction, "mech", sources)
        .map(|unit| UnitTypeId::new(unit.id().to_owned()))
}

/// Scavenger Zeta DEPLOY: "After you gain control of a planet, you may spend 1 trade good to place
/// 1 mech on that planet." The window's own use/decline is the "may".
fn scavenger_zeta(owner_name: &str, seat: &PlayerId) -> Ability {
    let owner = seat.clone();
    let condition_owner = seat.clone();
    Ability::stateful(
        format!("unit:{owner_name}:saar_mech:PLANET_CONTROL_GAINED:after"),
        seat.clone(),
        "PLANET_CONTROL_GAINED",
        Relation::After,
        Arc::new(move |event, _resolver, context| {
            let Some((system, planet)) = gained_by(event, &owner) else {
                return Ok(());
            };
            if !deploy_ready(context.state, context.content, context.sources, &owner) {
                return Ok(());
            }
            if let Some(seat) = context.state.player_mut(&owner) {
                seat.trade_goods -= 1;
            }
            crate::action_cards::place_units_counted(
                context,
                &owner,
                &system,
                Some(&planet),
                "mech",
                1,
            );
            Ok(())
        }),
    )
    .with_optional(true)
    .with_stateful_condition(Arc::new(move |event, _, context| {
        gained_by(event, &condition_owner).is_some()
            && deploy_ready(
                context.state,
                context.content,
                context.sources,
                &condition_owner,
            )
    }))
}

/// A trade good to spend and a mech left in the box (31.4).
fn deploy_ready(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    owner: &PlayerId,
) -> bool {
    is_saar(state, owner)
        && state
            .player(owner)
            .is_some_and(|seat| seat.trade_goods >= 1)
        && mech_id(state, content, sources, owner)
            .is_some_and(|id| crate::supply::allowed(state, content, sources, owner, &id, 1) > 0)
}

// -- Ragh's Call ---------------------------------------------------------------------------------

/// The Saar ground forces on a planet, and the Saar player's other planets they can go to.
#[allow(
    clippy::type_complexity,
    reason = "one private tuple: saar seat, its forces, its destinations"
)]
fn ragh_targets(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    holder: &PlayerId,
    system: &SystemId,
    planet: &PlanetId,
) -> Option<(PlayerId, Vec<Unit>, Vec<(SystemId, PlanetId)>)> {
    if crate::promissory::held_foreign(state, holder, "ragh") == 0 {
        return None;
    }
    let saar = saar_seat(state)?;
    if &saar == holder {
        return None;
    }
    let types = catalogue(content, sources);
    let forces: Vec<Unit> = state
        .system_state(system)
        .on_planet_of(planet, &saar)
        .into_iter()
        .filter(|unit| {
            types
                .get(unit.type_id.as_str())
                .is_some_and(UnitType::is_ground_force)
        })
        .cloned()
        .collect();
    if forces.is_empty() {
        return None;
    }
    let destinations: Vec<(SystemId, PlanetId)> = state
        .controlled_planets(&saar)
        .into_iter()
        .filter(|(to_system, to_planet)| !(*to_system == system && *to_planet == planet))
        .map(|(to_system, to_planet)| (to_system.clone(), to_planet.clone()))
        .collect();
    if destinations.is_empty() {
        return None;
    }
    Some((saar, forces, destinations))
}

/// Ragh's Call, played by its holder after committing units to a planet. The Saar player picks the
/// destination (it is "a planet controlled by the Saar player"); the choice is asked before any
/// unit moves.
fn ragh_call(owner_name: &str, seat: &PlayerId) -> Ability {
    let holder = seat.clone();
    let condition_holder = seat.clone();
    Ability::stateful(
        format!("promissory:{owner_name}:ragh:UNITS_COMMITTED:after"),
        seat.clone(),
        "UNITS_COMMITTED",
        Relation::After,
        Arc::new(move |event, _resolver, context| {
            let Some((system, planet)) = gained_by(event, &holder) else {
                return Ok(());
            };
            let Some((saar, forces, destinations)) = ragh_targets(
                context.state,
                context.content,
                context.sources,
                &holder,
                &system,
                &planet,
            ) else {
                return Ok(());
            };
            let options = destinations
                .iter()
                .map(|(to_system, to_planet)| {
                    ChoiceOption::labelled(
                        format!("{to_system}|{to_planet}"),
                        "planet",
                        format!("move the Saar ground forces to {to_planet}"),
                    )
                })
                .collect();
            let answer = ask(
                context,
                &saar,
                format!("Ragh's Call: where do the Saar ground forces on {planet} go"),
                "ragh",
                "ragh_destination",
                options,
                false,
            )
            .map_err(illegal)?;
            let Some((to_system, to_planet)) = destinations
                .iter()
                .find(|(s, p)| answer.id == format!("{s}|{p}"))
            else {
                return Ok(());
            };
            context
                .state
                .system_mut(&system)
                .remove_from_planet(&planet, &forces);
            context
                .state
                .system_mut(to_system)
                .planet_units
                .entry(to_planet.clone())
                .or_default()
                .extend(forces);
            crate::promissory::give_back(context.state, RAGH_NOTE);
            Ok(())
        }),
    )
    .with_optional(true)
    .with_stateful_condition(Arc::new(move |event, _, context| {
        gained_by(event, &condition_holder).is_some_and(|(system, planet)| {
            ragh_targets(
                context.state,
                context.content,
                context.sources,
                &condition_holder,
                &system,
                &planet,
            )
            .is_some()
        })
    }))
}

// -- Nomadic and Chaos Mapping activation --------------------------------------------------------

/// "You can score objectives even if you do not control the planets in your home system."
/// Lifts only the home-control prerequisite (LRR 61.16); objective requirements still apply.
fn nomadic(state: &GameState, player: &PlayerId) -> bool {
    is_saar(state, player)
}

/// "Other players cannot activate asteroid fields that contain 1 or more of your ships."
/// Checked while generating legal activations (LRR 89.1). Floating Factories move as ships but
/// remain structures, so they do not satisfy this ship requirement.
fn chaos_mapping_blocks_activation(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    activator: &PlayerId,
    system: &SystemId,
) -> bool {
    if !ti4_content::galaxy::system(content, system.as_str(), sources)
        .is_some_and(|tile| tile.is_asteroid_field())
    {
        return false;
    }
    let types = catalogue(content, sources);
    state.system_state(system).units.iter().any(|unit| {
        &unit.owner != activator
            && has_technology(state, &unit.owner, "cm")
            && types
                .get(unit.type_id.as_str())
                .is_some_and(UnitType::is_ship)
    })
}

// -- Chaos Mapping: start-of-turn production -----------------------------------------------------

/// Systems where `player` could produce at least 1 unit by ability now.
fn production_systems(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
) -> Vec<SystemId> {
    state
        .board
        .keys()
        .filter(|system| {
            !crate::production::producers(state, content, sources, player, system).is_empty()
                && ProductionWindow::for_ability(state, content, sources, player, system, Some(1))
                    .pending_choice(state, content, sources)
                    .is_some()
        })
        .cloned()
        .collect()
}

fn chaos_ready(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    owner: &PlayerId,
) -> bool {
    has_technology(state, owner, "cm")
        && state.phase == ti4_model::state::Phase::Action
        && !production_systems(state, content, sources, owner).is_empty()
}

/// "At the start of your turn during the action phase, you may produce 1 unit in a system that
/// contains at least 1 of your units that has PRODUCTION."
fn chaos_mapping_production(owner_name: &str, seat: &PlayerId) -> Ability {
    let owner = seat.clone();
    let condition_owner = seat.clone();
    Ability::stateful(
        format!("technology:{owner_name}:cm:TURN_BEGAN:after"),
        seat.clone(),
        "TURN_BEGAN",
        Relation::After,
        Arc::new(move |event, resolver, context| {
            if event.text("player") != Some(owner.as_str())
                || !chaos_ready(context.state, context.content, context.sources, &owner)
            {
                return Ok(());
            }
            let systems =
                production_systems(context.state, context.content, context.sources, &owner);
            let system = if let [only] = systems.as_slice() {
                only.clone()
            } else {
                let options = systems
                    .iter()
                    .map(|s| ChoiceOption::labelled(s.to_string(), "system", format!("system {s}")))
                    .collect();
                let answer = ask(
                    context,
                    &owner,
                    "Chaos Mapping: produce 1 unit in which system".to_owned(),
                    "cm",
                    "chaos_system",
                    options,
                    true,
                )
                .map_err(illegal)?;
                if answer.is_decline() {
                    return Ok(());
                }
                let Some(picked) = systems.iter().find(|s| s.as_str() == answer.id) else {
                    return Ok(());
                };
                picked.clone()
            };
            // Production can ask several payment/placement questions. A failed later choice
            // must not leave this faction effect partially paid or placed.
            let before = context.state.clone();
            if let Err(error) = produce(context, resolver, &owner, &system) {
                *context.state = before;
                return Err(error);
            }
            Ok(())
        }),
    )
    .with_optional(true)
    .with_stateful_condition(Arc::new(move |event, _, context| {
        event.text("player") == Some(condition_owner.as_str())
            && chaos_ready(
                context.state,
                context.content,
                context.sources,
                &condition_owner,
            )
    }))
}

fn produce(
    context: &mut TimingContext<'_>,
    resolver: &mut Resolver,
    player: &PlayerId,
    system: &SystemId,
) -> Result<(), TimingError> {
    let TimingContext {
        state,
        content,
        sources,
        table,
        dice,
        rng,
        event_sequence,
        galaxy,
    } = context;
    let galaxy = *galaxy;
    let mut ctx = Resolving {
        content,
        sources: *sources,
        dice,
        rng,
        table,
        timing: Some(TimingHandle {
            resolver,
            sequence: event_sequence,
            galaxy,
        }),
    };
    crate::production::produce_by_ability(state, &mut ctx, galaxy, player, system, Some(1))
        .map_err(TimingError::IllegalChoice)?;
    Ok(())
}

// -- Captain Mendosa -----------------------------------------------------------------------------

/// `GameState::faction_marks` key for the ship the agent boosted, valued
/// `"<activation_seq>|<origin>|<index>|<bonus>"`, per activating player.
fn agent_key(player: &PlayerId) -> String {
    format!("saar:agent:boost:{player}")
}

/// The highest printed move value of any ship on the board.
fn highest_move(state: &GameState, content: &ContentStore, sources: SourceSet) -> i64 {
    let types = catalogue(content, sources);
    state
        .board
        .values()
        .flat_map(|board| board.units.iter())
        .filter_map(|unit| types.get(unit.type_id.as_str()))
        .filter(|kind| kind.is_ship())
        .map(UnitType::move_value)
        .max()
        .unwrap_or(0)
}

/// `activator`'s ships that are slower than the fastest ship on the board:
/// `(origin, index in ships_of, bonus, label)`.
fn agent_ships(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    activator: &PlayerId,
) -> Vec<(SystemId, usize, i64, String)> {
    let top = highest_move(state, content, sources);
    let types = catalogue(content, sources);
    let mut found = Vec::new();
    for system in state.board.keys() {
        for (index, unit) in state.ships_of(activator, system).into_iter().enumerate() {
            let Some(kind) = types.get(unit.type_id.as_str()) else {
                continue;
            };
            let own = kind.move_value();
            if kind.is_ship() && own < top {
                found.push((
                    system.clone(),
                    index,
                    top - own,
                    format!("{} in {system} (move {own} to {top})", unit.type_id),
                ));
            }
        }
    }
    found
}

/// Captain Mendosa: "When a player activates a system: You may exhaust this card to increase the
/// move value of 1 of that player's ships to match the move value of the ship on the game board that
/// has the highest move value." The boost is remembered against this activation and read by
/// [`agent_move_bonus`].
fn agent(owner_name: &str, seat: &PlayerId) -> Ability {
    let owner = seat.clone();
    let condition_owner = seat.clone();
    Ability::stateful(
        format!("leader:{owner_name}:saaragent:SYSTEM_ACTIVATED:after"),
        seat.clone(),
        "SYSTEM_ACTIVATED",
        Relation::After,
        Arc::new(move |event, _resolver, context| {
            let Some(activator) = event.text("player").map(PlayerId::new) else {
                return Ok(());
            };
            if leader_status(context.state, &owner, "saaragent") != Some(LeaderStatus::Readied) {
                return Ok(());
            }
            let ships = agent_ships(context.state, context.content, context.sources, &activator);
            if ships.is_empty() {
                return Ok(());
            }
            let options = ships
                .iter()
                .enumerate()
                .map(|(n, (_, _, _, label))| {
                    ChoiceOption::labelled(format!("ship|{n}"), "ship", label.clone())
                })
                .collect();
            let answer = ask(
                context,
                &owner,
                "Captain Mendosa: raise which of the activating player's ships".to_owned(),
                "saaragent",
                "agent_ship",
                options,
                true,
            )
            .map_err(illegal)?;
            let Some((origin, index, bonus, _)) = ships
                .iter()
                .enumerate()
                .find(|(n, _)| answer.id == format!("ship|{n}"))
                .map(|(_, ship)| ship)
            else {
                return Ok(());
            };
            if !crate::leaders::exhaust(context.state, &owner, &LeaderId::new("saaragent")) {
                return Ok(());
            }
            let seq = context.state.activation_seq;
            context.state.faction_marks.insert(
                agent_key(&activator),
                format!("{seq}|{origin}|{index}|{bonus}"),
            );
            Ok(())
        }),
    )
    .with_optional(true)
    .with_stateful_condition(Arc::new(move |event, _, context| {
        leader_status(context.state, &condition_owner, "saaragent") == Some(LeaderStatus::Readied)
            && event.text("player").is_some_and(|activator| {
                !agent_ships(
                    context.state,
                    context.content,
                    context.sources,
                    &PlayerId::new(activator),
                )
                .is_empty()
            })
    }))
}

/// The boost the agent granted to one ship for this activation.
fn agent_move_bonus(state: &GameState, site: &MoveSite<'_>) -> i32 {
    let Some(mark) = state.faction_marks.get(&agent_key(site.player)) else {
        return 0;
    };
    let mut parts = mark.split('|');
    let (Some(seq), Some(origin), Some(index), Some(bonus)) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return 0;
    };
    if seq == state.activation_seq.to_string()
        && origin == site.origin.as_str()
        && Some(index) == site.index.map(|i| i.to_string()).as_deref()
    {
        bonus.parse().unwrap_or(0)
    } else {
        0
    }
}

// -- Gurno Aggero --------------------------------------------------------------------------------

fn dock_systems(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
) -> Vec<SystemId> {
    let mut found: Vec<SystemId> = units_of_base(state, content, sources, player, "spacedock")
        .into_iter()
        .map(|(system, _, _)| system)
        .collect();
    found.dedup();
    found
}

fn leader_action(
    state: &GameState,
    content: &ContentStore,
    player: &PlayerId,
    leader: &LeaderId,
) -> Option<bool> {
    (leader.as_str() == "saarhero").then(|| {
        // Any adjacent system is a legal choice, so the action can resolve whenever a dock stands.
        !dock_systems(state, content, ti4_model::content_types::DEFAULT, player).is_empty()
            && leader_status(state, player, "saarhero").is_some()
    })
}

fn use_leader(
    context: &mut TimingContext<'_>,
    player: &PlayerId,
    leader: &LeaderId,
) -> Option<bool> {
    (leader.as_str() == "saarhero").then(|| hero(context, player))
}

/// "Choose 1 system that is adjacent to 1 of your space docks. Destroy all other players' infantry
/// and fighters in that system." The system is chosen before anything is destroyed.
fn hero(context: &mut TimingContext<'_>, player: &PlayerId) -> bool {
    let Some(galaxy) = context.galaxy else {
        return false;
    };
    let docks = dock_systems(context.state, context.content, context.sources, player);
    let mut candidates: Vec<SystemId> = Vec::new();
    let adjacency = crate::movement::PlayerAdjacency::new(
        context.state,
        context.content,
        context.sources,
        galaxy,
        player,
    );
    for dock in &docks {
        for neighbour in adjacency.neighbours(dock.as_str()) {
            let neighbour = SystemId::new(neighbour);
            if !candidates.contains(&neighbour) {
                candidates.push(neighbour);
            }
        }
    }
    candidates.sort();
    if candidates.is_empty() {
        return false;
    }
    let options = candidates
        .iter()
        .map(|s| ChoiceOption::labelled(s.to_string(), "system", format!("system {s}")))
        .collect();
    let Ok(answer) = ask(
        context,
        player,
        "Gurno Aggero: destroy all other players' infantry and fighters in which system".to_owned(),
        "saarhero",
        "armageddon_relay",
        options,
        false,
    ) else {
        return false;
    };
    let Some(system) = candidates.iter().find(|s| s.as_str() == answer.id).cloned() else {
        return false;
    };
    // Decide everything first, then remove. Fighters and infantry alike are removed directly (no
    // `pending_destructions` entry is staged: a leader effect has no way to announce them).
    let mut doomed: Vec<(Option<PlanetId>, Unit)> = Vec::new();
    for seat in &context.state.players {
        if &seat.id == player {
            continue;
        }
        for base in ["fighter", "infantry"] {
            for (at, planet, unit) in units_of_base(
                context.state,
                context.content,
                context.sources,
                &seat.id,
                base,
            ) {
                if at == system {
                    doomed.push((planet, unit));
                }
            }
        }
    }
    for (planet, unit) in doomed {
        let board = context.state.system_mut(&system);
        match planet {
            Some(planet) => board.remove_from_planet(&planet, std::slice::from_ref(&unit)),
            None => board.remove(std::slice::from_ref(&unit)),
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use ti4_model::content_types::DEFAULT;

    fn a() -> PlayerId {
        PlayerId::new("a")
    }

    fn b() -> PlayerId {
        PlayerId::new("b")
    }

    fn game() -> GameState {
        crate::fixtures::seated_game(&[("a", FACTION), ("b", "sol")], DEFAULT)
    }

    fn emit(
        state: &mut GameState,
        table: &mut crate::choice::Table,
        event_type: &str,
        pairs: &[(&str, &str)],
    ) {
        let mut resolver = crate::fixtures::armed_resolver(state);
        crate::fixtures::with_context(state, DEFAULT, None, table, |ctx| {
            let payload = pairs
                .iter()
                .map(|(k, v)| ((*k).to_owned(), serde_json::Value::from(*v)))
                .collect();
            let event = ctx
                .event_sequence
                .next(event_type, payload)
                .expect("an event id");
            resolver
                .emit_with_context(ctx, event, |_, _| {})
                .expect("the window resolves");
        });
    }

    fn scripted(answers: &[&str]) -> crate::choice::Table {
        crate::choice::Table::with_default(Box::new(crate::choice::Scripted::new(
            answers.iter().map(|s| (*s).to_owned()),
        )))
    }

    fn trade_goods(state: &GameState, who: &PlayerId) -> i32 {
        state.player(who).unwrap().trade_goods
    }

    /// A planet in the Saar home system, as `(system, planet)`.
    fn home_planet(state: &GameState, who: &PlayerId) -> (SystemId, PlanetId) {
        let (s, p) = state.controlled_planets(who)[0];
        (s.clone(), p.clone())
    }

    fn mechs(state: &GameState, system: &SystemId, planet: &PlanetId, who: &PlayerId) -> usize {
        state
            .system_state(system)
            .on_planet_of(planet, who)
            .iter()
            .filter(|unit| unit.type_id.as_str() == "saar_mech")
            .count()
    }

    #[test]
    fn scavenge_pays_a_trade_good_for_each_planet_gained_and_only_to_the_gainer() {
        let mut state = game();
        let (system, planet) = home_planet(&state, &a());
        let before = (trade_goods(&state, &a()), trade_goods(&state, &b()));
        emit(
            &mut state,
            &mut scripted(&["decline"]),
            "PLANET_CONTROL_GAINED",
            &[
                ("player", "a"),
                ("system", system.as_str()),
                ("planet", planet.as_str()),
            ],
        );
        assert_eq!(trade_goods(&state, &a()), before.0 + 1);
        assert_eq!(trade_goods(&state, &b()), before.1);
        // The other player gaining a planet pays Saar nothing.
        emit(
            &mut state,
            &mut scripted(&[]),
            "PLANET_CONTROL_GAINED",
            &[
                ("player", "b"),
                ("system", system.as_str()),
                ("planet", planet.as_str()),
            ],
        );
        assert_eq!(trade_goods(&state, &a()), before.0 + 1);
    }

    #[test]
    fn scavenger_zeta_costs_a_trade_good_and_places_a_mech_or_is_declined() {
        let mut state = game();
        let (system, planet) = home_planet(&state, &a());
        let id = "unit:saar:saar_mech:PLANET_CONTROL_GAINED:after";
        let base = mechs(&state, &system, &planet, &a());
        let tg = trade_goods(&state, &a());
        emit(
            &mut state,
            &mut scripted(&[id]),
            "PLANET_CONTROL_GAINED",
            &[
                ("player", "a"),
                ("system", system.as_str()),
                ("planet", planet.as_str()),
            ],
        );
        assert_eq!(mechs(&state, &system, &planet, &a()), base + 1);
        assert_eq!(trade_goods(&state, &a()), tg, "+1 Scavenge, -1 DEPLOY");
        // Declined: the mech is not placed and the trade good is kept.
        let mut state = game();
        let tg = trade_goods(&state, &a());
        emit(
            &mut state,
            &mut scripted(&["decline"]),
            "PLANET_CONTROL_GAINED",
            &[
                ("player", "a"),
                ("system", system.as_str()),
                ("planet", planet.as_str()),
            ],
        );
        assert_eq!(mechs(&state, &system, &planet, &a()), base);
        assert_eq!(trade_goods(&state, &a()), tg + 1);
    }

    #[test]
    fn the_stats_of_the_flagship_are_data_driven() {
        let content = ContentStore::embedded();
        let types = catalogue(content, DEFAULT);
        let flagship = types.get("saar_flagship").expect("flagship");
        assert!(flagship.is_ship());
        assert_eq!(flagship.base_type(), "flagship");
    }

    #[test]
    fn the_commander_unlocks_at_three_space_docks() {
        let content = ContentStore::embedded();
        let mut state = game();
        let leader = LeaderId::new("saarcommander");
        let (system, planet) = home_planet(&state, &a());
        let unlocked =
            |state: &GameState| commander_unlocked(state, content, DEFAULT, None, &a(), &leader);
        let docks = units_of_base(&state, content, DEFAULT, &a(), "spacedock").len();
        assert!(docks < 3);
        for _ in docks..2 {
            crate::fixtures::put_on_planet(&mut state, &system, &planet, "saar_spacedock", &a(), 1);
        }
        assert_eq!(unlocked(&state), Some(false), "two docks");
        // A dock in the space area counts (a Floating Factory).
        state
            .system_mut(&system)
            .units
            .push(Unit::new(UnitTypeId::new("saar_spacedock"), a()));
        assert_eq!(unlocked(&state), Some(true), "three docks");
        assert_eq!(
            commander_unlocked(&state, content, DEFAULT, None, &a(), &LeaderId::new("x")),
            None
        );
    }

    #[test]
    fn the_commander_docks_exclude_systems_in_or_next_to_other_players_units() {
        let content = ContentStore::embedded();
        let mut state = game();
        let hub = crate::fixtures::plain_hub();
        let dock = SystemId::new(hub.outer[0].clone());
        let next_to = SystemId::new(hub.centre.clone());
        // The fixture seeds ships around the hub: start from an empty neighbourhood.
        for id in std::iter::once(&hub.centre).chain(hub.outer.iter()) {
            let board = state.system_mut(&SystemId::new(id.clone()));
            board.units.clear();
            board.planet_units.clear();
        }
        state
            .system_mut(&dock)
            .units
            .push(Unit::new(UnitTypeId::new("saar_spacedock"), a()));
        let docks = |state: &GameState| {
            commander_docks(state, content, DEFAULT, &hub.galaxy, &a())
                .into_iter()
                .filter(|(system, _)| system == &dock)
                .count()
        };
        assert_eq!(docks(&state), 0, "commander still locked");
        state
            .player_mut(&a())
            .unwrap()
            .leaders
            .insert(LeaderId::new("saarcommander"), LeaderStatus::Unlocked);
        assert_eq!(docks(&state), 1, "an empty neighbourhood qualifies");
        state
            .system_mut(&next_to)
            .units
            .push(Unit::new(UnitTypeId::new("cruiser"), b()));
        assert_eq!(
            docks(&state),
            0,
            "an adjacent system holds another player's ship"
        );
        state.system_mut(&next_to).units.clear();
        state
            .system_mut(&dock)
            .units
            .push(Unit::new(UnitTypeId::new("cruiser"), b()));
        assert_eq!(docks(&state), 0, "the dock's own system holds one");
        let sol = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan")], DEFAULT);
        assert!(commander_docks(&sol, content, DEFAULT, &hub.galaxy, &a()).is_empty());
    }

    #[test]
    fn a_game_without_saar_is_offered_no_saar_ability() {
        let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan")], DEFAULT);
        let (system, planet) = {
            let (s, p) = state.controlled_planets(&a())[0];
            (s.clone(), p.clone())
        };
        let before = state.clone();
        for event in ["PLANET_CONTROL_GAINED", "UNITS_COMMITTED", "TURN_BEGAN"] {
            // An empty script fails the test if any choice is asked.
            emit(
                &mut state,
                &mut scripted(&[]),
                event,
                &[
                    ("player", "a"),
                    ("system", system.as_str()),
                    ("planet", planet.as_str()),
                ],
            );
        }
        assert_eq!(
            state.player(&a()).unwrap().trade_goods,
            before.player(&a()).unwrap().trade_goods
        );
        assert_eq!(state.board, before.board);
        let content = ContentStore::embedded();
        assert_eq!(
            leader_action(&state, content, &a(), &LeaderId::new("saarhero")),
            Some(false)
        );
    }

    #[test]
    fn ragh_s_call_moves_the_saar_forces_to_a_planet_they_control_and_returns_the_note() {
        let mut state = game();
        let (home_system, home_planet_id) = home_planet(&state, &a());
        let (their_system, their_planet) = home_planet(&state, &b());
        crate::fixtures::put_on_planet(
            &mut state,
            &their_system,
            &their_planet,
            "infantry",
            &a(),
            2,
        );
        state.promissory_notes.insert(RAGH_NOTE.to_owned(), b());
        let destination = format!("{home_system}|{home_planet_id}");
        let at_home = |state: &GameState| {
            state
                .system_state(&home_system)
                .on_planet_of(&home_planet_id, &a())
                .len()
        };
        let home_before = at_home(&state);
        emit(
            &mut state,
            &mut scripted(&["promissory:sol:ragh:UNITS_COMMITTED:after", &destination]),
            "UNITS_COMMITTED",
            &[
                ("player", "b"),
                ("system", their_system.as_str()),
                ("planet", their_planet.as_str()),
            ],
        );
        assert!(
            state
                .system_state(&their_system)
                .on_planet_of(&their_planet, &a())
                .is_empty()
        );
        assert_eq!(at_home(&state), home_before + 2);
        assert_ne!(
            state.promissory_notes.get(RAGH_NOTE),
            Some(&b()),
            "returned"
        );
    }

    #[test]
    fn ragh_s_call_is_not_offered_without_the_note_or_without_saar_forces_there() {
        let mut state = game();
        let (their_system, their_planet) = home_planet(&state, &b());
        crate::fixtures::put_on_planet(
            &mut state,
            &their_system,
            &their_planet,
            "infantry",
            &a(),
            1,
        );
        let pairs = [
            ("player", "b"),
            ("system", their_system.as_str()),
            ("planet", their_planet.as_str()),
        ];
        // No note.
        let before = state.board.clone();
        emit(&mut state, &mut scripted(&[]), "UNITS_COMMITTED", &pairs);
        assert_eq!(state.board, before);
        // Note held, but no Saar ground force on the planet.
        let mut state = game();
        state.promissory_notes.insert(RAGH_NOTE.to_owned(), b());
        let before = state.board.clone();
        emit(&mut state, &mut scripted(&[]), "UNITS_COMMITTED", &pairs);
        assert_eq!(state.board, before);
    }

    #[test]
    fn the_hero_destroys_other_players_infantry_and_fighters_next_to_a_dock_only() {
        let mut state = game();
        let hub = crate::fixtures::plain_hub();
        let dock = SystemId::new(hub.outer[0].clone());
        let target = SystemId::new(hub.centre.clone());
        let far = SystemId::new(hub.across(&hub.outer[0]));
        state
            .system_mut(&dock)
            .units
            .push(Unit::new(UnitTypeId::new("saar_spacedock"), a()));
        let planet = PlanetId::new("hero_test_planet");
        for system in [&target, &far] {
            state
                .system_mut(system)
                .units
                .push(Unit::new(UnitTypeId::new("fighter"), b()));
            state
                .system_mut(system)
                .units
                .push(Unit::new(UnitTypeId::new("infantry"), b()));
            crate::fixtures::put_on_planet(&mut state, system, &planet, "infantry", &b(), 1);
            crate::fixtures::put_on_planet(&mut state, system, &planet, "infantry", &a(), 1);
        }
        let mut table = scripted(&[target.as_str()]);
        let done = crate::fixtures::with_context(
            &mut state,
            DEFAULT,
            Some(&hub.galaxy),
            &mut table,
            |context| use_leader(context, &a(), &LeaderId::new("saarhero")),
        );
        assert_eq!(done, Some(true));
        let board = state.system_state(&target);
        assert!(
            board.units.iter().all(|u| u.type_id.as_str() != "fighter"),
            "the fighter is destroyed"
        );
        assert_eq!(
            board.units.len(),
            3,
            "the carriers and destroyer are not fighters"
        );
        assert!(board.on_planet_of(&planet, &b()).is_empty());
        assert_eq!(
            board.on_planet_of(&planet, &a()).len(),
            1,
            "Saar's own stay"
        );
        let board = state.system_state(&far);
        assert_eq!(
            board.units.len(),
            2,
            "a system not adjacent to a dock is untouched"
        );
        assert_eq!(board.on_planet_of(&planet, &b()).len(), 1);
    }

    #[test]
    fn chaos_mapping_production_needs_the_technology_the_action_phase_and_a_producer() {
        let content = ContentStore::embedded();
        let mut state = game();
        state.phase = ti4_model::state::Phase::Action;
        assert!(!has_technology(&state, &a(), "cm") || chaos_ready(&state, content, DEFAULT, &a()));
        state
            .player_mut(&a())
            .unwrap()
            .technologies
            .retain(|t| t.as_str() != "cm");
        assert!(
            !chaos_ready(&state, content, DEFAULT, &a()),
            "no technology"
        );
        state
            .player_mut(&a())
            .unwrap()
            .technologies
            .insert(ti4_model::id::TechnologyId::new("cm"));
        assert!(
            !production_systems(&state, content, DEFAULT, &a()).is_empty(),
            "the home dock can produce"
        );
        assert!(chaos_ready(&state, content, DEFAULT, &a()));
        assert!(!chaos_ready(&state, content, DEFAULT, &b()), "b has no cm");
        state.phase = ti4_model::state::Phase::Status;
        assert!(
            !chaos_ready(&state, content, DEFAULT, &a()),
            "not the action phase"
        );
        // Declined through the window: nothing is produced.
        state.phase = ti4_model::state::Phase::Action;
        let before = state.board.clone();
        emit(
            &mut state,
            &mut scripted(&["decline"]),
            "TURN_BEGAN",
            &[("player", "a")],
        );
        assert_eq!(state.board, before);
    }

    fn asteroid_field() -> SystemId {
        let content = ContentStore::embedded();
        (1..=100)
            .map(|n| n.to_string())
            .find(|id| {
                ti4_content::galaxy::system(content, id, DEFAULT)
                    .is_some_and(|tile| tile.is_asteroid_field())
            })
            .map(SystemId::new)
            .expect("an asteroid field tile")
    }

    #[test]
    fn chaos_mapping_bars_other_players_from_an_asteroid_field_holding_a_cm_ship() {
        let content = ContentStore::embedded();
        let mut state = game();
        let field = asteroid_field();
        state
            .system_mut(&field)
            .units
            .push(Unit::new(UnitTypeId::new("cruiser"), a()));
        let barred = |state: &GameState, who: &PlayerId| {
            chaos_mapping_blocks_activation(state, content, DEFAULT, who, &field)
        };
        state
            .player_mut(&a())
            .unwrap()
            .technologies
            .remove(&ti4_model::id::TechnologyId::new("cm"));
        assert!(!barred(&state, &b()), "no technology");
        state
            .player_mut(&a())
            .unwrap()
            .technologies
            .insert(ti4_model::id::TechnologyId::new("cm"));
        assert!(barred(&state, &b()));
        assert!(!barred(&state, &a()), "not the owner");
        state.system_mut(&field).units.clear();
        assert!(!barred(&state, &b()), "no ship left");
    }

    #[test]
    fn nomadic_scores_without_the_home_planets_and_others_still_need_them() {
        let content = ContentStore::embedded();
        let mut state = game();
        for who in [a(), b()] {
            let held: Vec<(SystemId, PlanetId)> = state
                .controlled_planets(&who)
                .into_iter()
                .map(|(s, p)| (s.clone(), p.clone()))
                .collect();
            for (system, planet) in held {
                state.system_mut(&system).planet_control.remove(&planet);
            }
        }
        let home = |state: &GameState, who: &PlayerId| {
            crate::objectives::controls_home_system(&crate::objectives::Position::new(
                state, content, DEFAULT, who,
            ))
        };
        assert!(home(&state, &a()), "Nomadic");
        assert!(!home(&state, &b()), "61.16 still binds Sol");
    }

    #[test]
    fn floating_factories_move_as_ships_and_a_blockade_destroys_them() {
        let content = ContentStore::embedded();
        let types = catalogue(content, DEFAULT);
        let ffac2 = types.get("saar_spacedock2").expect("ffac2 unit");
        assert!(ffac2.moves_as_ship());
        assert_eq!(ffac2.move_value(), 2);
        let mut state = game();
        let hub = crate::fixtures::plain_hub();
        let origin = SystemId::new(hub.outer[0].clone());
        let active = SystemId::new(hub.centre.clone());
        state
            .system_mut(&origin)
            .units
            .push(Unit::new(UnitTypeId::new("saar_spacedock"), a()));
        let moves =
            crate::tactical::movable_into(&state, content, DEFAULT, &hub.galaxy, &a(), &active);
        assert!(
            moves
                .iter()
                .any(|m| m.origin == origin && m.unit.type_id.as_str() == "saar_spacedock"),
            "the dock is offered as a mover"
        );
        state
            .system_mut(&origin)
            .units
            .push(Unit::new(UnitTypeId::new("cruiser"), b()));
        let gone = crate::production::destroy_blockaded_mobile_docks(
            &mut state, content, DEFAULT, &origin,
        );
        assert_eq!(gone.len(), 1);
    }

    #[test]
    fn the_agent_raises_one_ship_to_the_fastest_move_on_the_board() {
        let mut state = game();
        let content = ContentStore::embedded();
        let hub = crate::fixtures::plain_hub();
        let origin = SystemId::new(hub.outer[0].clone());
        state
            .system_mut(&origin)
            .units
            .push(Unit::new(UnitTypeId::new("carrier"), b()));
        state
            .system_mut(&SystemId::new(hub.outer[1].clone()))
            .units
            .push(Unit::new(UnitTypeId::new("cruiser"), a()));
        state.activation_seq = 7;
        let ships = agent_ships(&state, content, DEFAULT, &b());
        let n = ships
            .iter()
            .position(|(system, ..)| system == &origin)
            .expect("the carrier is slower");
        let (_, index, bonus, _) = ships[n].clone();
        let types = catalogue(content, DEFAULT);
        let carrier = types.get("carrier").unwrap();
        let (pb, o) = (b(), origin.clone());
        let site = |idx: Option<usize>| MoveSite {
            player: &pb,
            origin: &o,
            index: idx,
            ship: carrier,
        };
        assert_eq!(
            agent_move_bonus(&state, &site(Some(index))),
            0,
            "nothing chosen yet"
        );
        emit(
            &mut state,
            &mut scripted(&[
                "leader:saar:saaragent:SYSTEM_ACTIVATED:after",
                &format!("ship|{n}"),
            ]),
            "SYSTEM_ACTIVATED",
            &[("player", "b"), ("system", hub.centre.as_str())],
        );
        assert_eq!(
            leader_status(&state, &a(), "saaragent"),
            Some(LeaderStatus::Exhausted)
        );
        assert_eq!(
            i64::from(agent_move_bonus(&state, &site(Some(index)))),
            bonus
        );
        assert_eq!(
            agent_move_bonus(&state, &site(None)),
            0,
            "needs the ship index"
        );
        state.activation_seq = 8;
        assert_eq!(
            agent_move_bonus(&state, &site(Some(index))),
            0,
            "next activation"
        );
    }

    #[test]
    fn the_agent_is_not_offered_exhausted_or_in_a_game_without_saar() {
        let mut state = game();
        state
            .player_mut(&a())
            .unwrap()
            .leaders
            .insert(LeaderId::new("saaragent"), LeaderStatus::Exhausted);
        emit(
            &mut state,
            &mut scripted(&[]),
            "SYSTEM_ACTIVATED",
            &[("player", "b"), ("system", "18")],
        );
        assert!(state.faction_marks.is_empty());
        let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan")], DEFAULT);
        emit(
            &mut state,
            &mut scripted(&[]),
            "SYSTEM_ACTIVATED",
            &[("player", "b"), ("system", "18")],
        );
        assert!(state.faction_marks.is_empty());
    }
}
